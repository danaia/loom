//! Deterministic reference semantics for Pqo's emergent-systems specimens.
//!
//! These routines are deliberately backend-neutral. Metal kernels must match these
//! rules, while contracts and scenario tests can use them as a small correctness
//! oracle.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const Q16_SCALE: u32 = 1 << 16;
pub const MAX_DEPOSIT_PER_CELL_Q16: u32 = Q16_SCALE;
pub const MAX_DEPOSIT_CONTRIBUTORS: u32 = 1024;
pub const DECISION_BIN_MAX: u32 = 4095;
pub const PHYSICAL_NEIGHBOR_LIMIT: usize = 128;
pub const PERCEPTION_NEIGHBOR_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u32)]
pub enum Fate {
    Organizer,
    Undifferentiated,
    Boundary,
    Interior,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u32)]
pub enum DevelopmentalPhase {
    Immature,
    Competent,
    Committed,
    Mature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u32)]
pub enum Health {
    Healthy,
    Damaged,
    Dying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DerivedBehavior {
    Quiescent,
    Dividing,
    Migrating,
    Repairing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedIntents {
    pub division: bool,
    pub migration: bool,
    pub repair: bool,
}

pub fn derive_behavior(health: Health, intents: AcceptedIntents) -> DerivedBehavior {
    if intents.division {
        DerivedBehavior::Dividing
    } else if intents.migration {
        DerivedBehavior::Migrating
    } else if health == Health::Damaged && intents.repair {
        DerivedBehavior::Repairing
    } else {
        DerivedBehavior::Quiescent
    }
}

pub fn fate_transition_allowed(from: Fate, to: Fate) -> bool {
    matches!(
        (from, to),
        (Fate::Organizer, Fate::Organizer)
            | (Fate::Undifferentiated, Fate::Undifferentiated)
            | (Fate::Undifferentiated, Fate::Boundary)
            | (Fate::Undifferentiated, Fate::Interior)
            | (Fate::Boundary, Fate::Boundary)
            | (Fate::Boundary, Fate::Interior)
            | (Fate::Interior, Fate::Interior)
            | (Fate::Interior, Fate::Boundary)
    )
}

pub fn phase_transition_allowed(
    from: DevelopmentalPhase,
    to: DevelopmentalPhase,
    health: Health,
    accepted_repair: bool,
) -> bool {
    from == to
        || matches!(
            (from, to),
            (DevelopmentalPhase::Immature, DevelopmentalPhase::Competent)
                | (DevelopmentalPhase::Competent, DevelopmentalPhase::Committed)
                | (DevelopmentalPhase::Committed, DevelopmentalPhase::Mature)
        )
        || (from == DevelopmentalPhase::Mature
            && to == DevelopmentalPhase::Competent
            && health == Health::Damaged
            && accepted_repair)
}

pub fn health_transition_allowed(from: Health, to: Health) -> bool {
    from == to
        || matches!(
            (from, to),
            (Health::Healthy, Health::Damaged)
                | (Health::Damaged, Health::Healthy)
                | (Health::Damaged, Health::Dying)
        )
}

pub fn quantize_decision(value: f32, maximum: f32) -> u32 {
    if !value.is_finite() || maximum <= 0.0 {
        return 0;
    }
    ((value.clamp(0.0, maximum) / maximum) * DECISION_BIN_MAX as f32).round_ties_even() as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DivisionDecision {
    pub phase: DevelopmentalPhase,
    pub health: Health,
    pub age_ticks: u32,
    pub energy_bin: u32,
    pub nutrient_bin: u32,
    pub inhibitor_bin: u32,
}

pub fn division_intent(input: DivisionDecision) -> bool {
    input.phase >= DevelopmentalPhase::Competent
        && input.health == Health::Healthy
        && input.age_ticks >= 240
        && input.energy_bin >= 1536
        && input.nutrient_bin >= 2048
        && input.inhibitor_bin < 3072
}

pub fn update_energy(
    previous: f32,
    nutrient_bin: u32,
    requested_signal_q16: u32,
    emitted_intents: u32,
) -> f32 {
    let absorbed = nutrient_bin.min(DECISION_BIN_MAX) as f32 / DECISION_BIN_MAX as f32 * 0.009_78;
    let maintenance = 0.001 + previous * 0.002_2;
    let signaling = requested_signal_q16 as f32 / Q16_SCALE as f32 * 0.0001;
    let decisions = emitted_intents as f32 * 0.00001;
    (previous + absorbed - maintenance - signaling - decisions).max(0.0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepositGrid {
    width: usize,
    height: usize,
    values: Vec<u32>,
    contributors: Vec<u32>,
    pub saturation_count: u64,
    pub contributor_overflow_count: u64,
}

impl DepositGrid {
    pub fn new(width: usize, height: usize) -> Self {
        assert!(width > 0 && height > 0);
        Self {
            width,
            height,
            values: vec![0; width * height],
            contributors: vec![0; width * height],
            saturation_count: 0,
            contributor_overflow_count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.values.fill(0);
        self.contributors.fill(0);
        self.saturation_count = 0;
        self.contributor_overflow_count = 0;
    }

    pub fn deposit_radial(&mut self, x: usize, y: usize, requested_q16: u32) {
        let requested = requested_q16.min(MAX_DEPOSIT_PER_CELL_Q16);
        const WEIGHTS: [[u32; 3]; 3] = [[1, 2, 1], [2, 4, 2], [1, 2, 1]];
        for (row, weights) in WEIGHTS.iter().enumerate() {
            for (column, weight) in weights.iter().enumerate() {
                let sample_x = reflect_index(x as isize + column as isize - 1, self.width);
                let sample_y = reflect_index(y as isize + row as isize - 1, self.height);
                let index = sample_y * self.width + sample_x;
                self.contributors[index] = self.contributors[index].saturating_add(1);
                if self.contributors[index] > MAX_DEPOSIT_CONTRIBUTORS {
                    self.contributor_overflow_count += 1;
                }
                let weighted = (u64::from(requested) * u64::from(*weight) / 16) as u32;
                let (next, overflowed) = self.values[index].overflowing_add(weighted);
                if overflowed {
                    self.values[index] = u32::MAX;
                    self.saturation_count += 1;
                } else {
                    self.values[index] = next;
                }
            }
        }
    }

    pub fn value_q16(&self, x: usize, y: usize) -> u32 {
        self.values[y * self.width + x]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarField {
    width: usize,
    height: usize,
    values: Vec<f32>,
}

impl ScalarField {
    pub fn new(width: usize, height: usize, initial: f32) -> Self {
        Self {
            width,
            height,
            values: vec![initial.max(0.0); width * height],
        }
    }

    pub fn value(&self, x: usize, y: usize) -> f32 {
        self.values[y * self.width + x]
    }

    pub fn step(&mut self, deposits: &DepositGrid, diffusion_alpha: f32, decay: f32, maximum: f32) {
        assert_eq!((self.width, self.height), (deposits.width, deposits.height));
        assert!((0.0..=0.25).contains(&diffusion_alpha));
        assert!((0.0..=1.0).contains(&decay));
        let mut next = vec![0.0; self.values.len()];
        for y in 0..self.height {
            for x in 0..self.width {
                let center = self.value(x, y);
                let left = self.value(reflect_index(x as isize - 1, self.width), y);
                let right = self.value(reflect_index(x as isize + 1, self.width), y);
                let down = self.value(x, reflect_index(y as isize - 1, self.height));
                let up = self.value(x, reflect_index(y as isize + 1, self.height));
                let laplacian = left + right + down + up - 4.0 * center;
                let deposit = deposits.value_q16(x, y) as f32 / Q16_SCALE as f32;
                next[y * self.width + x] = (center + diffusion_alpha * laplacian - decay * center
                    + deposit)
                    .clamp(0.0, maximum);
            }
        }
        self.values = next;
    }
}

fn reflect_index(index: isize, length: usize) -> usize {
    match index {
        value if value < 0 => (-value - 1) as usize,
        value if value >= length as isize => (2 * length as isize - value - 1) as usize,
        value => value as usize,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedCell {
    pub stable_id: u32,
    pub x_q16: i32,
    pub y_q16: i32,
    pub radius_q16: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Neighborhood {
    pub physical: Vec<u32>,
    pub perception: Vec<u32>,
    pub physical_overflow: u32,
    pub perception_truncation: u32,
    pub angular_occupancy: u8,
}

pub fn build_neighborhoods(
    cells: &[QuantizedCell],
    bin_size_q16: i32,
    perception_radius_q16: u32,
) -> BTreeMap<u32, Neighborhood> {
    assert!(bin_size_q16 > 0);
    let mut bins = BTreeMap::<(i32, i32), Vec<QuantizedCell>>::new();
    for cell in cells {
        bins.entry((
            cell.x_q16.div_euclid(bin_size_q16),
            cell.y_q16.div_euclid(bin_size_q16),
        ))
        .or_default()
        .push(*cell);
    }
    for bin in bins.values_mut() {
        bin.sort_by_key(|cell| cell.stable_id);
    }

    let by_id = cells
        .iter()
        .map(|cell| (cell.stable_id, *cell))
        .collect::<BTreeMap<_, _>>();
    let mut result = BTreeMap::new();
    for cell in by_id.values() {
        let base = (
            cell.x_q16.div_euclid(bin_size_q16),
            cell.y_q16.div_euclid(bin_size_q16),
        );
        let mut physical = Vec::new();
        let mut perception = Vec::new();
        let mut sectors = 0_u8;
        for offset_y in -1..=1 {
            for offset_x in -1..=1 {
                for other in bins
                    .get(&(base.0 + offset_x, base.1 + offset_y))
                    .into_iter()
                    .flatten()
                {
                    if other.stable_id == cell.stable_id {
                        continue;
                    }
                    let dx = i64::from(other.x_q16) - i64::from(cell.x_q16);
                    let dy = i64::from(other.y_q16) - i64::from(cell.y_q16);
                    let distance_squared = dx * dx + dy * dy;
                    let contact = u64::from(cell.radius_q16 + other.radius_q16)
                        + u64::from(cell.radius_q16.min(other.radius_q16)) / 4;
                    if distance_squared as u64 <= contact * contact {
                        physical.push(other.stable_id);
                    }
                    if distance_squared as u64
                        <= u64::from(perception_radius_q16) * u64::from(perception_radius_q16)
                    {
                        perception.push(other.stable_id);
                        sectors |= 1 << angular_sector(dx, dy);
                    }
                }
            }
        }
        let physical_overflow = physical.len().saturating_sub(PHYSICAL_NEIGHBOR_LIMIT) as u32;
        let perception_truncation =
            perception.len().saturating_sub(PERCEPTION_NEIGHBOR_LIMIT) as u32;
        physical.truncate(PHYSICAL_NEIGHBOR_LIMIT);
        perception.truncate(PERCEPTION_NEIGHBOR_LIMIT);
        result.insert(
            cell.stable_id,
            Neighborhood {
                physical,
                perception,
                physical_overflow,
                perception_truncation,
                angular_occupancy: sectors,
            },
        );
    }
    result
}

fn angular_sector(dx: i64, dy: i64) -> u8 {
    let ax = dx.unsigned_abs();
    let ay = dy.unsigned_abs();
    if ax >= ay.saturating_mul(2) {
        if dx >= 0 { 0 } else { 4 }
    } else if ay >= ax.saturating_mul(2) {
        if dy >= 0 { 2 } else { 6 }
    } else {
        match (dx >= 0, dy >= 0) {
            (true, true) => 1,
            (false, true) => 3,
            (false, false) => 5,
            (true, false) => 7,
        }
    }
}

pub fn connected_components(cells: &[QuantizedCell]) -> usize {
    if cells.is_empty() {
        return 0;
    }
    let neighborhoods = build_neighborhoods(
        cells,
        cells
            .iter()
            .map(|cell| cell.radius_q16)
            .max()
            .unwrap_or(Q16_SCALE)
            .saturating_mul(3) as i32,
        0,
    );
    let mut parent = cells
        .iter()
        .map(|cell| (cell.stable_id, cell.stable_id))
        .collect::<BTreeMap<_, _>>();
    for (id, neighbors) in neighborhoods {
        for neighbor in neighbors.physical {
            union(&mut parent, id, neighbor);
        }
    }
    cells
        .iter()
        .map(|cell| find(&mut parent, cell.stable_id))
        .collect::<BTreeSet<_>>()
        .len()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BirthRequest {
    pub parent_id: u32,
    pub daughter: QuantizedCell,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BirthAllocation {
    pub accepted: Vec<QuantizedCell>,
    pub placement_rejected: Vec<u32>,
    pub capacity_rejected: Vec<u32>,
    pub next_stable_id: u32,
}

pub fn allocate_births(
    living: &[QuantizedCell],
    requests: &[BirthRequest],
    capacity: usize,
    next_stable_id: u32,
    region_min_q16: (i32, i32),
    region_max_q16: (i32, i32),
) -> BirthAllocation {
    let living_by_id = living
        .iter()
        .map(|cell| (cell.stable_id, *cell))
        .collect::<BTreeMap<_, _>>();
    let mut ordered = requests.to_vec();
    ordered.sort_by_key(|request| request.parent_id);
    let mut result = BirthAllocation {
        next_stable_id,
        ..BirthAllocation::default()
    };
    let mut occupied = living.to_vec();
    for request in ordered {
        let Some(parent) = living_by_id.get(&request.parent_id) else {
            result.placement_rejected.push(request.parent_id);
            continue;
        };
        if !valid_daughter_placement(
            *parent,
            request.daughter,
            &occupied,
            region_min_q16,
            region_max_q16,
        ) {
            result.placement_rejected.push(request.parent_id);
            continue;
        }
        if living.len() + result.accepted.len() >= capacity {
            result.capacity_rejected.push(request.parent_id);
            continue;
        }
        let mut daughter = request.daughter;
        daughter.stable_id = result.next_stable_id;
        result.next_stable_id = result.next_stable_id.saturating_add(1);
        occupied.push(daughter);
        result.accepted.push(daughter);
    }
    result
}

pub fn valid_daughter_placement(
    parent: QuantizedCell,
    daughter: QuantizedCell,
    occupied: &[QuantizedCell],
    region_min_q16: (i32, i32),
    region_max_q16: (i32, i32),
) -> bool {
    let radius = daughter.radius_q16 as i32;
    if daughter.x_q16 - radius < region_min_q16.0
        || daughter.y_q16 - radius < region_min_q16.1
        || daughter.x_q16 + radius > region_max_q16.0
        || daughter.y_q16 + radius > region_max_q16.1
        || !cells_contact(parent, daughter)
    {
        return false;
    }
    occupied.iter().all(|cell| {
        if cell.stable_id == parent.stable_id {
            return true;
        }
        let dx = i64::from(cell.x_q16) - i64::from(daughter.x_q16);
        let dy = i64::from(cell.y_q16) - i64::from(daughter.y_q16);
        let minimum = u64::from(cell.radius_q16 + daughter.radius_q16);
        dx * dx + dy * dy >= (minimum * minimum) as i64
    })
}

fn cells_contact(left: QuantizedCell, right: QuantizedCell) -> bool {
    let dx = i64::from(left.x_q16) - i64::from(right.x_q16);
    let dy = i64::from(left.y_q16) - i64::from(right.y_q16);
    let contact = u64::from(left.radius_q16 + right.radius_q16)
        + u64::from(left.radius_q16.min(right.radius_q16)) / 4;
    dx * dx + dy * dy <= (contact * contact) as i64
}

pub fn stable_compact<T>(
    items: impl IntoIterator<Item = (u32, T)>,
    removed: &BTreeSet<u32>,
) -> Vec<(u32, T)> {
    let mut retained = items
        .into_iter()
        .filter(|(stable_id, _)| !removed.contains(stable_id))
        .collect::<Vec<_>>();
    retained.sort_by_key(|(stable_id, _)| *stable_id);
    retained
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateNode {
    pub stable_id: u32,
    pub member_count: u32,
    pub minimum_q16: (i32, i32),
    pub maximum_q16: (i32, i32),
    pub centroid_q16: (i32, i32),
    pub radius_sum_q16: u64,
    pub maximum_centroid_error_q16: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenAggregate {
    pub node: AggregateNode,
    children: Vec<QuantizedCell>,
}

impl FrozenAggregate {
    pub fn expand(&self) -> Vec<QuantizedCell> {
        self.children.clone()
    }
}

pub fn aggregate_cells(stable_id: u32, cells: &[QuantizedCell]) -> Option<FrozenAggregate> {
    if cells.is_empty() {
        return None;
    }
    let mut children = cells.to_vec();
    children.sort_by_key(|cell| cell.stable_id);
    let sum_x = children
        .iter()
        .map(|cell| i64::from(cell.x_q16))
        .sum::<i64>();
    let sum_y = children
        .iter()
        .map(|cell| i64::from(cell.y_q16))
        .sum::<i64>();
    let centroid = (
        (sum_x / children.len() as i64) as i32,
        (sum_y / children.len() as i64) as i32,
    );
    let maximum_centroid_error_q16 = children
        .iter()
        .map(|cell| {
            let dx = i64::from(cell.x_q16) - i64::from(centroid.0);
            let dy = i64::from(cell.y_q16) - i64::from(centroid.1);
            ((dx * dx + dy * dy) as f64).sqrt().round() as u32
        })
        .max()
        .unwrap_or(0);
    Some(FrozenAggregate {
        node: AggregateNode {
            stable_id,
            member_count: children.len() as u32,
            minimum_q16: (
                children.iter().map(|cell| cell.x_q16).min().unwrap(),
                children.iter().map(|cell| cell.y_q16).min().unwrap(),
            ),
            maximum_q16: (
                children.iter().map(|cell| cell.x_q16).max().unwrap(),
                children.iter().map(|cell| cell.y_q16).max().unwrap(),
            ),
            centroid_q16: centroid,
            radius_sum_q16: children.iter().map(|cell| u64::from(cell.radius_q16)).sum(),
            maximum_centroid_error_q16,
        },
        children,
    })
}

fn find(parent: &mut BTreeMap<u32, u32>, id: u32) -> u32 {
    let next = parent[&id];
    if next == id {
        id
    } else {
        let root = find(parent, next);
        parent.insert(id, root);
        root
    }
}

fn union(parent: &mut BTreeMap<u32, u32>, left: u32, right: u32) {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    let root = left_root.min(right_root);
    parent.insert(left_root, root);
    parent.insert(right_root, root);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnergyLedger {
    pub previous: f64,
    pub absorbed: f64,
    pub maintenance: f64,
    pub decisions: f64,
    pub motion: f64,
    pub signaling: f64,
    pub division: f64,
    pub environmental_death_loss: f64,
    pub current: f64,
}

impl EnergyLedger {
    pub fn residual(self) -> f64 {
        self.previous + self.absorbed
            - self.maintenance
            - self.decisions
            - self.motion
            - self.signaling
            - self.division
            - self.environmental_death_loss
            - self.current
    }

    pub fn within_error(self, maximum_error: f64) -> bool {
        self.residual().abs() <= maximum_error
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceEnvelope {
    pub mean: f64,
    pub variance: f64,
    pub minimum: f64,
    pub maximum: f64,
}

impl ReferenceEnvelope {
    pub fn from_samples(samples: &[f64]) -> Option<Self> {
        if samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
            return None;
        }
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let variance = samples
            .iter()
            .map(|sample| (sample - mean).powi(2))
            .sum::<f64>()
            / samples.len() as f64;
        Some(Self {
            mean,
            variance,
            minimum: samples.iter().copied().fold(f64::INFINITY, f64::min),
            maximum: samples.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        })
    }

    pub fn contains(self, value: f64, relative_tolerance: f64) -> bool {
        let padding = self.mean.abs().max(1.0) * relative_tolerance;
        value >= self.minimum - padding && value <= self.maximum + padding
    }
}

pub fn sustained_recovery(
    samples: &[f64],
    envelope: ReferenceEnvelope,
    relative_tolerance: f64,
    consecutive_ticks: usize,
) -> bool {
    samples
        .iter()
        .rev()
        .take_while(|sample| envelope.contains(**sample, relative_tolerance))
        .count()
        >= consecutive_ticks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_tables_reject_uncontrolled_rollbacks() {
        assert!(phase_transition_allowed(
            DevelopmentalPhase::Immature,
            DevelopmentalPhase::Competent,
            Health::Healthy,
            false
        ));
        assert!(!phase_transition_allowed(
            DevelopmentalPhase::Mature,
            DevelopmentalPhase::Competent,
            Health::Healthy,
            true
        ));
        assert!(phase_transition_allowed(
            DevelopmentalPhase::Mature,
            DevelopmentalPhase::Competent,
            Health::Damaged,
            true
        ));
        assert!(!fate_transition_allowed(Fate::Interior, Fate::Organizer));
        assert!(!health_transition_allowed(Health::Healthy, Health::Dying));
    }

    #[test]
    fn derived_behavior_cannot_contradict_accepted_intents() {
        assert_eq!(
            derive_behavior(
                Health::Healthy,
                AcceptedIntents {
                    division: true,
                    ..AcceptedIntents::default()
                }
            ),
            DerivedBehavior::Dividing
        );
        assert_eq!(
            derive_behavior(Health::Damaged, AcceptedIntents::default()),
            DerivedBehavior::Quiescent
        );
    }

    #[test]
    fn field_deposits_conserve_the_normalized_kernel_and_diffuse_reflectively() {
        let mut deposits = DepositGrid::new(5, 5);
        deposits.deposit_radial(2, 2, Q16_SCALE);
        let total = (0..5)
            .flat_map(|y| (0..5).map(move |x| (x, y)))
            .map(|(x, y)| deposits.value_q16(x, y))
            .sum::<u32>();
        assert_eq!(total, Q16_SCALE);
        assert_eq!(deposits.saturation_count, 0);

        let mut field = ScalarField::new(5, 5, 0.0);
        field.step(&deposits, 0.2, 0.0, 10.0);
        assert!(field.value(2, 2) > field.value(0, 0));
        assert!(field.values.iter().all(|value| *value >= 0.0));
    }

    #[test]
    fn neighborhoods_and_connectivity_ignore_input_storage_order() {
        let cells = vec![
            QuantizedCell {
                stable_id: 3,
                x_q16: 2 * Q16_SCALE as i32,
                y_q16: 0,
                radius_q16: Q16_SCALE,
            },
            QuantizedCell {
                stable_id: 1,
                x_q16: 0,
                y_q16: 0,
                radius_q16: Q16_SCALE,
            },
            QuantizedCell {
                stable_id: 2,
                x_q16: Q16_SCALE as i32,
                y_q16: 0,
                radius_q16: Q16_SCALE,
            },
        ];
        let mut permuted = cells.clone();
        permuted.rotate_left(1);
        assert_eq!(
            build_neighborhoods(&cells, 3 * Q16_SCALE as i32, 4 * Q16_SCALE),
            build_neighborhoods(&permuted, 3 * Q16_SCALE as i32, 4 * Q16_SCALE)
        );
        assert_eq!(connected_components(&cells), 1);
        assert_eq!(connected_components(&permuted), 1);
    }

    #[test]
    fn dense_neighborhoods_report_physical_and_perception_overflow() {
        let cells = (1..=130)
            .map(|stable_id| QuantizedCell {
                stable_id,
                x_q16: 0,
                y_q16: 0,
                radius_q16: Q16_SCALE,
            })
            .collect::<Vec<_>>();
        let neighborhoods = build_neighborhoods(&cells, 3 * Q16_SCALE as i32, Q16_SCALE);
        let first = &neighborhoods[&1];
        assert_eq!(first.physical.len(), PHYSICAL_NEIGHBOR_LIMIT);
        assert_eq!(first.physical_overflow, 1);
        assert_eq!(first.perception.len(), PERCEPTION_NEIGHBOR_LIMIT);
        assert_eq!(first.perception_truncation, 65);
    }

    #[test]
    fn recovery_requires_a_sustained_reference_envelope() {
        let envelope = ReferenceEnvelope::from_samples(&[9.8, 10.0, 10.2]).unwrap();
        let transient = [15.0, 10.0, 15.0];
        assert!(!sustained_recovery(&transient, envelope, 0.1, 2));
        let sustained = [15.0, 10.1, 9.9, 10.0];
        assert!(sustained_recovery(&sustained, envelope, 0.1, 3));
    }

    #[test]
    fn energy_ledger_exposes_environmental_loss_and_residual() {
        let ledger = EnergyLedger {
            previous: 10.0,
            absorbed: 5.0,
            maintenance: 1.0,
            decisions: 1.0,
            motion: 1.0,
            signaling: 1.0,
            division: 2.0,
            environmental_death_loss: 1.0,
            current: 8.0,
        };
        assert!(ledger.within_error(f64::EPSILON));
    }

    #[test]
    fn birth_allocation_is_parent_id_ordered_and_rejects_isolated_daughters() {
        let living = vec![
            QuantizedCell {
                stable_id: 2,
                x_q16: 4 * Q16_SCALE as i32,
                y_q16: 0,
                radius_q16: Q16_SCALE,
            },
            QuantizedCell {
                stable_id: 1,
                x_q16: 0,
                y_q16: 0,
                radius_q16: Q16_SCALE,
            },
        ];
        let requests = vec![
            BirthRequest {
                parent_id: 2,
                daughter: QuantizedCell {
                    stable_id: 0,
                    x_q16: 6 * Q16_SCALE as i32,
                    y_q16: 0,
                    radius_q16: Q16_SCALE,
                },
            },
            BirthRequest {
                parent_id: 1,
                daughter: QuantizedCell {
                    stable_id: 0,
                    x_q16: 20 * Q16_SCALE as i32,
                    y_q16: 0,
                    radius_q16: Q16_SCALE,
                },
            },
        ];
        let result = allocate_births(
            &living,
            &requests,
            3,
            3,
            (-(Q16_SCALE as i32), -(Q16_SCALE as i32)),
            (32 * Q16_SCALE as i32, 32 * Q16_SCALE as i32),
        );
        assert_eq!(result.placement_rejected, vec![1]);
        assert_eq!(result.accepted[0].stable_id, 3);
        assert_eq!(result.accepted.len(), 1);
    }

    #[test]
    fn repeated_compaction_cannot_reintroduce_storage_order() {
        let removed = BTreeSet::from([2, 4]);
        let first = stable_compact(
            vec![(5, "e"), (1, "a"), (4, "d"), (2, "b"), (3, "c")],
            &removed,
        );
        let second = stable_compact(first.into_iter().rev(), &BTreeSet::new());
        assert_eq!(second, vec![(1, "a"), (3, "c"), (5, "e")]);
    }

    #[test]
    fn zero_nutrient_extinguishes_energy_and_zero_inhibitor_releases_growth() {
        let mut energy = 0.1;
        for _ in 0..200 {
            energy = update_energy(energy, 0, 0, 0);
        }
        assert_eq!(energy, 0.0);

        assert!(division_intent(DivisionDecision {
            phase: DevelopmentalPhase::Mature,
            health: Health::Healthy,
            age_ticks: 240,
            energy_bin: 2048,
            nutrient_bin: 4095,
            inhibitor_bin: 0,
        }));
        assert!(!division_intent(DivisionDecision {
            phase: DevelopmentalPhase::Mature,
            health: Health::Healthy,
            age_ticks: 240,
            energy_bin: 2048,
            nutrient_bin: 4095,
            inhibitor_bin: 4095,
        }));
    }

    #[test]
    fn frozen_aggregation_preserves_totals_and_expands_in_stable_order() {
        let cells = vec![
            QuantizedCell {
                stable_id: 9,
                x_q16: 2 * Q16_SCALE as i32,
                y_q16: 0,
                radius_q16: Q16_SCALE,
            },
            QuantizedCell {
                stable_id: 3,
                x_q16: 0,
                y_q16: 0,
                radius_q16: 2 * Q16_SCALE,
            },
        ];
        let aggregate = aggregate_cells(100, &cells).unwrap();
        assert_eq!(aggregate.node.member_count, 2);
        assert_eq!(aggregate.node.radius_sum_q16, 3 * u64::from(Q16_SCALE));
        assert_eq!(
            aggregate
                .expand()
                .iter()
                .map(|cell| cell.stable_id)
                .collect::<Vec<_>>(),
            vec![3, 9]
        );
    }
}
