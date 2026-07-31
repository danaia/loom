//! Hello Organism: the first coupled dynamic-population/field specimen.

use crate::*;

const ORGANISM_SOURCE: &str = include_str!("../../../kernels/organism.metal");
const FIELD_WIDTH: u32 = 256;
const FIELD_CELLS: u32 = FIELD_WIDTH * FIELD_WIDTH;
const SPATIAL_BIN_AXIS: u32 = 64;
const SPATIAL_BIN_COUNT: u32 = SPATIAL_BIN_AXIS * SPATIAL_BIN_AXIS;
const SPATIAL_BIN_CAPACITY: u32 = 128;
const SPATIAL_INDEX_COUNT: u32 = SPATIAL_BIN_COUNT * SPATIAL_BIN_CAPACITY;
const SCAN_BLOCK_SIZE: u32 = 256;
const COMPONENT_RELAXATION_ROUNDS: u32 = 64;
const HOMEOSTASIS_METRIC_COUNT: u32 = 16;

/// Causal controls for the Hello Organism reference and field ablations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelloOrganismConfig {
    pub capacity: u32,
    pub activator_transport: bool,
    pub inhibitor_transport: bool,
}

impl HelloOrganismConfig {
    pub const fn reference(capacity: u32) -> Self {
        Self {
            capacity,
            activator_transport: true,
            inhibitor_transport: true,
        }
    }
}

fn packaged_kernel(name: &str, slots: Vec<SlotDraft>) -> KernelDraft {
    let binding_order = slots
        .iter()
        .map(|slot| slot.name.clone())
        .collect::<Vec<_>>();
    slots.into_iter().fold(
        KernelDraft::new(name)
            .abi(KernelAbiDraft::new(binding_order))
            .implementation(packaged_metal_implementation(
                "kernels/organism.metal",
                name,
                ORGANISM_SOURCE,
            )),
        |kernel, slot| kernel.slot(slot),
    )
}

fn stream_slot(name: &str, data_type: DataType, access: SlotAccess, whole: bool) -> SlotDraft {
    stream_slot_unit(name, data_type, Unit::DIMENSIONLESS, access, whole)
}

fn stream_slot_unit(
    name: &str,
    data_type: DataType,
    unit: Unit,
    access: SlotAccess,
    whole: bool,
) -> SlotDraft {
    let slot = SlotDraft::stream(name, data_type, unit, access);
    if whole { slot.whole_resource() } else { slot }
}

fn value_slot(name: &str, data_type: DataType) -> SlotDraft {
    SlotDraft::value(name, data_type, Unit::DIMENSIONLESS)
}

fn dynamic_stream(
    name: &str,
    data_type: DataType,
    unit: Unit,
    initial: Literal,
    capacity: u32,
    authority: Option<&str>,
) -> StreamDraft {
    let mut stream = StreamDraft::new(name, data_type, unit)
        .capacity(capacity)
        .dynamic_length("cells.active_count")
        .initial_repeat(initial, 1);
    if let Some(authority) = authority {
        stream = stream.write_authority(authority);
    }
    stream
}

/// Builds the first 2D/2.5D morphogenesis specimen.
///
/// Population membership uses global stable-ID radix ordering, canonical spatial
/// bins, bounded qualification, hierarchical prefix scans, parallel compaction,
/// stable-ID birth allocation, and an authoritative final count commit.
/// Developmental perception derives quantized density, contact, and exact
/// eight-sector exposure from those bins. Convergence-audited component labels
/// feed GPU morphology reductions. One organizer constructs a connected,
/// differentiated reference body through local field and placement laws.
/// Per-tick energy accounting and disjoint envelope audits prove sustained
/// homeostasis and return after a recorded nutrient perturbation.
pub fn hello_organism_builder(capacity: u32) -> ModuleBuilder {
    hello_organism_builder_with_config(HelloOrganismConfig::reference(capacity))
}

/// Builds Hello Organism with explicit causal field controls.
pub fn hello_organism_builder_with_config(config: HelloOrganismConfig) -> ModuleBuilder {
    let capacity = config.capacity;
    assert!(capacity >= 2);
    assert!(
        capacity <= SCAN_BLOCK_SIZE * SCAN_BLOCK_SIZE,
        "the v0 two-level population scan supports at most 65,536 cells"
    );
    let scan_capacity = capacity.next_multiple_of(SCAN_BLOCK_SIZE);
    let scan_block_count = scan_capacity / SCAN_BLOCK_SIZE;
    let u32_type = DataType::u32();
    let f32_type = DataType::f32();
    let vec3 = DataType::vec3_f32();
    let vec4 = DataType::vec4_f32();
    let vector =
        |values: &[f32]| Literal::Vector(values.iter().copied().map(Literal::f32).collect());

    let committed = [
        "cells.stable_id",
        "cells.parent_id",
        "cells.position",
        "cells.radius",
        "cells.energy",
        "cells.age",
        "cells.fate",
        "cells.phase",
        "cells.health",
        "cells.previous_fate",
        "cells.fate_confidence",
        "cells.time_in_fate",
        "cells.event_hash",
        "cells.recent_activator",
        "cells.recent_inhibitor",
        "cells.recent_surface_exposure",
        "cells.color",
    ];
    let transient = [
        "perception.activator_bin",
        "perception.inhibitor_bin",
        "perception.nutrient_bin",
        "perception.density_bin",
        "perception.injury_bin",
        "perception.energy_bin",
        "perception.local_density_bin",
        "perception.neighbor_count",
        "perception.contact_count",
        "perception.surface_mask",
        "perception.surface_exposure_bin",
        "intent.requested_fate",
        "intent.requested_phase",
        "intent.requested_health",
        "intent.divide",
        "intent.death",
        "intent.repair",
        "intent.activator_deposit",
        "intent.inhibitor_deposit",
        "intent.injury_deposit",
    ];
    let ledger_cells = [
        "ledger.cell_absorbed",
        "ledger.cell_maintenance",
        "ledger.cell_decisions",
        "ledger.cell_signaling",
    ];
    let field_state = [
        "field.activator",
        "field.inhibitor",
        "field.nutrient",
        "field.density",
        "field.injury",
        "field.activator_next",
        "field.inhibitor_next",
        "field.nutrient_next",
        "field.density_next",
        "field.injury_next",
    ];

    let mut builder = ModuleBuilder::new("hello_organism")
        .target(Target::Metal)
        .value(ValueDraft::constant(
            "organism.capacity",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(capacity),
        ))
        .value(ValueDraft::constant(
            "field.width",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(FIELD_WIDTH),
        ))
        .value(ValueDraft::constant(
            "field.activator_transport",
            f32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(if config.activator_transport { 1.0 } else { 0.0 }),
        ))
        .value(ValueDraft::constant(
            "field.inhibitor_transport",
            f32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(if config.inhibitor_transport { 1.0 } else { 0.0 }),
        ))
        .value(ValueDraft::constant(
            "environment.reference_nutrient_supply",
            f32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(1.0),
        ))
        .value(ValueDraft::constant(
            "environment.requested_injury_transport",
            f32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(1.0),
        ))
        .value(ValueDraft::constant(
            "environment.requested_repair_enabled",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(1),
        ))
        .value(ValueDraft::constant(
            "lesion.reference_center_x_q16",
            DataType::Scalar(ScalarType::I32),
            Unit::DIMENSIONLESS,
            Literal::I32(5_571),
        ))
        .value(ValueDraft::constant(
            "lesion.reference_center_y_q16",
            DataType::Scalar(ScalarType::I32),
            Unit::DIMENSIONLESS,
            Literal::I32(0),
        ))
        .value(ValueDraft::constant(
            "lesion.reference_radius_q16",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(3_932),
        ))
        .value(ValueDraft::constant(
            "lesion.reference_injury",
            f32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(4.0),
        ))
        .value(ValueDraft::constant(
            "population.scan_block_count",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(scan_block_count),
        ))
        .stream(
            StreamDraft::new("cells.active_count", u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .write_authority("mutate_cell_membership")
                .initial(Literal::Array(vec![Literal::U32(1)])),
        )
        .stream(
            StreamDraft::new(
                "cells.next_stable_id",
                u32_type.clone(),
                Unit::DIMENSIONLESS,
            )
            .capacity(1)
            .length(1)
            .write_authority("mutate_cell_state")
            .initial(Literal::Array(vec![Literal::U32(2)])),
        )
        .stream(
            StreamDraft::new("simulation.tick", u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .write_authority("advance_simulation_time")
                .initial(Literal::Array(vec![Literal::U32(0)])),
        )
        .stream(
            StreamDraft::new(
                "environment.nutrient_supply",
                f32_type.clone(),
                Unit::DIMENSIONLESS,
            )
            .capacity(1)
            .length(1)
            .write_authority("mutate_environment")
            .initial(Literal::Array(vec![Literal::f32(1.0)])),
        )
        .stream(
            StreamDraft::new(
                "environment.injury_transport",
                f32_type.clone(),
                Unit::DIMENSIONLESS,
            )
            .capacity(1)
            .length(1)
            .write_authority("mutate_environment")
            .initial(Literal::Array(vec![Literal::f32(1.0)])),
        )
        .stream(
            StreamDraft::new(
                "environment.repair_enabled",
                u32_type.clone(),
                Unit::DIMENSIONLESS,
            )
            .capacity(1)
            .length(1)
            .write_authority("mutate_environment")
            .initial(Literal::Array(vec![Literal::U32(1)])),
        )
        .stream(
            StreamDraft::new("homeostasis.window", u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(4)
                .length(4)
                .initial(Literal::Array(vec![
                    Literal::U32(10_000),
                    Literal::U32(11_000),
                    Literal::U32(29_000),
                    Literal::U32(30_000),
                ])),
        )
        .stream(
            StreamDraft::new(
                "homeostasis.perturbation_window",
                u32_type.clone(),
                Unit::DIMENSIONLESS,
            )
            .capacity(2)
            .length(2)
            .initial(Literal::Array(vec![
                Literal::U32(12_000),
                Literal::U32(14_000),
            ])),
        )
        .stream(
            StreamDraft::new("regeneration.window", u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(3)
                .length(3)
                .initial(Literal::Array(vec![
                    Literal::U32(30_000),
                    Literal::U32(50_000),
                    Literal::U32(500),
                ])),
        );

    for (name, data_type, unit, initial) in [
        (
            "cells.stable_id",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(1),
        ),
        (
            "cells.parent_id",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.position",
            vec3.clone(),
            Unit::METER,
            vector(&[0.0, 0.0, 0.0]),
        ),
        (
            "cells.radius",
            f32_type.clone(),
            Unit::METER,
            Literal::f32(0.012),
        ),
        (
            "cells.energy",
            f32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(4.0),
        ),
        (
            "cells.age",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.fate",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.phase",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.health",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.previous_fate",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.fate_confidence",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.time_in_fate",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.event_hash",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(2_166_136_261),
        ),
        (
            "cells.recent_activator",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.recent_inhibitor",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "cells.recent_surface_exposure",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(4095),
        ),
        (
            "cells.color",
            vec4.clone(),
            Unit::DIMENSIONLESS,
            vector(&[1.0, 0.25, 0.25, 1.0]),
        ),
    ] {
        builder = builder.stream(dynamic_stream(
            name,
            data_type,
            unit,
            initial,
            capacity,
            Some("mutate_cell_state"),
        ));
    }
    for name in transient {
        builder = builder.stream(dynamic_stream(
            name,
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
            capacity,
            None,
        ));
    }
    for name in ledger_cells {
        builder = builder.stream(dynamic_stream(
            name,
            f32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
            capacity,
            None,
        ));
    }
    for name in [
        "ledger.previous_total",
        "ledger.absorbed",
        "ledger.maintenance",
        "ledger.decisions",
        "ledger.motion",
        "ledger.signaling",
        "ledger.division",
        "ledger.environmental_death_loss",
        "ledger.current_total",
        "ledger.residual",
        "ledger.cumulative_residual",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, f32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .initial(Literal::Array(vec![Literal::f32(0.0)])),
        );
    }
    for name in ["homeostasis.metric_min", "homeostasis.metric_max"] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(HOMEOSTASIS_METRIC_COUNT)
                .length(HOMEOSTASIS_METRIC_COUNT)
                .initial_repeat(Literal::U32(0), HOMEOSTASIS_METRIC_COUNT),
        );
    }
    for name in ["homeostasis.metric_sum", "homeostasis.metric_sum_sq"] {
        builder = builder.stream(
            StreamDraft::new(name, f32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(HOMEOSTASIS_METRIC_COUNT)
                .length(HOMEOSTASIS_METRIC_COUNT)
                .initial_repeat(Literal::f32(0.0), HOMEOSTASIS_METRIC_COUNT),
        );
    }
    for name in [
        "homeostasis.reference_samples",
        "homeostasis.validation_samples",
        "homeostasis.validation_violations",
        "homeostasis.invariant_violations",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .initial(Literal::Array(vec![Literal::U32(0)])),
        );
    }
    for name in [
        "homeostasis.energy_min",
        "homeostasis.energy_max",
        "homeostasis.energy_sum",
        "homeostasis.energy_sum_sq",
        "homeostasis.perturbation_energy_min",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, f32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .initial(Literal::Array(vec![Literal::f32(0.0)])),
        );
    }
    for name in field_state {
        let initial = if name == "field.nutrient" || name == "field.nutrient_next" {
            1.0
        } else {
            0.0
        };
        builder = builder.stream(
            StreamDraft::new(name, f32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(FIELD_CELLS)
                .length(FIELD_CELLS)
                .write_authority("mutate_field_state")
                .initial_repeat(Literal::f32(initial), FIELD_CELLS),
        );
    }
    for name in [
        "deposit.activator_q16",
        "deposit.inhibitor_q16",
        "deposit.density_q16",
        "deposit.injury_q16",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(FIELD_CELLS)
                .length(FIELD_CELLS)
                .initial_repeat(Literal::U32(0), FIELD_CELLS),
        );
    }

    for name in [
        "population.survival_flag",
        "population.birth_prequalified",
        "population.birth_flag",
        "population.candidate_sector",
        "population.survival_prefix",
        "population.birth_prefix",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(scan_capacity)
                .length(scan_capacity)
                .initial_repeat(Literal::U32(0), scan_capacity),
        );
    }
    for name in ["population.order_a", "population.order_b"] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(scan_capacity)
                .length(scan_capacity)
                .initial_repeat(Literal::U32(u32::MAX), scan_capacity),
        );
    }
    let radix_scratch_count = scan_block_count * 16;
    for name in ["population.radix_block_count", "population.radix_offset"] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(radix_scratch_count)
                .length(radix_scratch_count)
                .initial_repeat(Literal::U32(0), radix_scratch_count),
        );
    }
    for shift in (0..32).step_by(4) {
        builder = builder.value(ValueDraft::constant(
            format!("population.radix_shift_{shift}"),
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(shift),
        ));
    }
    for name in [
        "population.survival_block_sum",
        "population.birth_block_sum",
        "population.survival_block_prefix",
        "population.birth_block_prefix",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(scan_block_count)
                .length(scan_block_count)
                .initial_repeat(Literal::U32(0), scan_block_count),
        );
    }
    for name in [
        "population.survivor_count",
        "population.accepted_birth_count",
        "population.next_count",
        "population.rejected_births",
        "population.neighbor_overflow",
        "population.physical_neighbor_overflow",
        "population.perception_truncation",
        "deposit.saturation_count",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .initial(Literal::Array(vec![Literal::U32(0)])),
        );
    }
    for name in [
        "morphology.component_label_a",
        "morphology.component_label_b",
    ] {
        builder = builder.stream(dynamic_stream(
            name,
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(u32::MAX),
            capacity,
            None,
        ));
    }
    for name in [
        "morphology.population",
        "morphology.component_count",
        "morphology.component_unresolved",
        "morphology.organizer_count",
        "morphology.undifferentiated_count",
        "morphology.boundary_count",
        "morphology.interior_count",
        "morphology.area_q16",
        "morphology.perimeter_q16",
        "morphology.compactness_q16",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .initial(Literal::Array(vec![Literal::U32(0)])),
        );
    }
    for name in [
        "morphology.centroid_sum_x_q16",
        "morphology.centroid_sum_y_q16",
        "morphology.centroid_x_q16",
        "morphology.centroid_y_q16",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, DataType::Scalar(ScalarType::I32), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .initial(Literal::Array(vec![Literal::I32(0)])),
        );
    }
    builder = builder.stream(
        StreamDraft::new(
            "morphology.radial_density",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
        )
        .capacity(8)
        .length(8)
        .initial_repeat(Literal::U32(0), 8),
    );
    builder = builder
        .stream(
            StreamDraft::new("lesion.removed_ids", u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(64)
                .length(64)
                .initial_repeat(Literal::U32(0), 64),
        )
        .stream(
            StreamDraft::new(
                "lesion.center_q16",
                DataType::Scalar(ScalarType::I32),
                Unit::DIMENSIONLESS,
            )
            .capacity(2)
            .length(2)
            .initial_repeat(Literal::I32(0), 2),
        );
    for name in [
        "lesion.radius_q16",
        "lesion.removed_count",
        "lesion.damaged_count",
        "lesion.region_occupancy",
        "regeneration.injury_total_q16",
        "regeneration.injury_peak_q16",
        "regeneration.post_lesion_peak_q16",
        "regeneration.consecutive_ticks",
        "regeneration.success_tick",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .initial(Literal::Array(vec![Literal::U32(0)])),
        );
    }
    builder = builder.stream(
        StreamDraft::new(
            "lesion.removed_energy",
            f32_type.clone(),
            Unit::DIMENSIONLESS,
        )
        .capacity(1)
        .length(1)
        .initial(Literal::Array(vec![Literal::f32(0.0)])),
    );
    for name in ["spatial.living_bin_count", "spatial.candidate_bin_count"] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(SPATIAL_BIN_COUNT)
                .length(SPATIAL_BIN_COUNT)
                .initial_repeat(Literal::U32(0), SPATIAL_BIN_COUNT),
        );
    }
    for name in [
        "spatial.living_bin_indices",
        "spatial.candidate_bin_indices",
    ] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(SPATIAL_INDEX_COUNT)
                .length(SPATIAL_INDEX_COUNT)
                .initial_repeat(Literal::U32(u32::MAX), SPATIAL_INDEX_COUNT),
        );
    }
    for (name, data_type, unit, initial) in [
        (
            "population.stage_stable_id",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_parent_id",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_position",
            vec3.clone(),
            Unit::METER,
            vector(&[0.0, 0.0, 0.0]),
        ),
        (
            "population.stage_radius",
            f32_type.clone(),
            Unit::METER,
            Literal::f32(0.0),
        ),
        (
            "population.stage_energy",
            f32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ),
        (
            "population.stage_age",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_fate",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_phase",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_health",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_previous_fate",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_fate_confidence",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_time_in_fate",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_event_hash",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_recent_activator",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_recent_inhibitor",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_recent_surface_exposure",
            u32_type.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ),
        (
            "population.stage_color",
            vec4.clone(),
            Unit::DIMENSIONLESS,
            vector(&[0.0, 0.0, 0.0, 0.0]),
        ),
    ] {
        builder = builder.stream(
            StreamDraft::new(name, data_type, unit)
                .capacity(scan_capacity)
                .length(scan_capacity)
                .initial_repeat(initial, scan_capacity),
        );
    }

    builder = builder.pass(
        PassDraft::new(
            "initialize_population_order",
            "organism_initialize_population_order",
        )
        .bind("active_count", "cells.active_count")
        .bind("order", "population.order_a")
        .dispatch_over("population.order_a"),
    );
    let mut radix_predecessor = "initialize_population_order".to_owned();
    let mut radix_dependencies = Vec::new();
    for (digit, shift) in (0..32).step_by(4).enumerate() {
        let input = if digit % 2 == 0 {
            "population.order_a"
        } else {
            "population.order_b"
        };
        let output = if digit % 2 == 0 {
            "population.order_b"
        } else {
            "population.order_a"
        };
        let histogram = format!("radix_histogram_{shift}");
        let offsets = format!("radix_offsets_{shift}");
        let scatter = format!("radix_scatter_{shift}");
        builder = builder
            .pass(
                PassDraft::new(&histogram, "organism_radix_histogram")
                    .bind("order", input)
                    .bind("stable_id", "cells.stable_id")
                    .bind("block_count", "population.radix_block_count")
                    .bind("shift", format!("population.radix_shift_{shift}"))
                    .dispatch_over(input)
                    .threads_per_threadgroup(SCAN_BLOCK_SIZE),
            )
            .pass(
                PassDraft::new(&offsets, "organism_radix_offsets")
                    .bind("block_count", "population.radix_block_count")
                    .bind("offset", "population.radix_offset")
                    .bind("block_count_value", "population.scan_block_count"),
            )
            .pass(
                PassDraft::new(&scatter, "organism_radix_scatter")
                    .bind("input", input)
                    .bind("output", output)
                    .bind("stable_id", "cells.stable_id")
                    .bind("offset", "population.radix_offset")
                    .bind("shift", format!("population.radix_shift_{shift}"))
                    .dispatch_over(input)
                    .threads_per_threadgroup(SCAN_BLOCK_SIZE),
            );
        radix_dependencies.push((histogram.clone(), radix_predecessor));
        radix_dependencies.push((offsets.clone(), histogram));
        radix_dependencies.push((scatter.clone(), offsets));
        radix_predecessor = scatter;
    }

    builder = builder
        .pass(
            PassDraft::new("observe_neighbors", "organism_observe_neighbors")
                .bind("position", "cells.position")
                .bind("radius", "cells.radius")
                .bind("stable_id", "cells.stable_id")
                .bind("bin_count", "spatial.living_bin_count")
                .bind("bin_indices", "spatial.living_bin_indices")
                .bind("local_density_bin", "perception.local_density_bin")
                .bind("neighbor_count", "perception.neighbor_count")
                .bind("contact_count", "perception.contact_count")
                .bind("surface_mask", "perception.surface_mask")
                .bind("surface_exposure_bin", "perception.surface_exposure_bin")
                .bind("physical_overflow", "population.physical_neighbor_overflow")
                .bind("perception_truncation", "population.perception_truncation")
                .bind("active_count", "cells.active_count")
                .dispatch_over("cells.stable_id"),
        )
        .pass(
            PassDraft::new("initialize_components", "organism_initialize_components")
                .bind("stable_id", "cells.stable_id")
                .bind("label", "morphology.component_label_a")
                .bind("active_count", "cells.active_count")
                .dispatch_over("cells.stable_id"),
        );
    let mut component_predecessor = "initialize_components".to_owned();
    let mut component_dependencies = Vec::new();
    for round in 0..COMPONENT_RELAXATION_ROUNDS {
        let clear = format!("clear_component_changes_{round}");
        let relax = format!("relax_components_{round}");
        let (input, output) = if round % 2 == 0 {
            (
                "morphology.component_label_a",
                "morphology.component_label_b",
            )
        } else {
            (
                "morphology.component_label_b",
                "morphology.component_label_a",
            )
        };
        builder = builder
            .pass(
                PassDraft::new(&clear, "organism_clear_component_changes")
                    .bind("changes", "morphology.component_unresolved"),
            )
            .pass(
                PassDraft::new(&relax, "organism_relax_components")
                    .bind("position", "cells.position")
                    .bind("radius", "cells.radius")
                    .bind("bin_count", "spatial.living_bin_count")
                    .bind("bin_indices", "spatial.living_bin_indices")
                    .bind("input_label", input)
                    .bind("output_label", output)
                    .bind("changes", "morphology.component_unresolved")
                    .bind("active_count", "cells.active_count")
                    .dispatch_over("cells.stable_id"),
            );
        component_dependencies.push((clear.clone(), component_predecessor));
        component_dependencies.push((relax.clone(), clear));
        component_predecessor = relax;
    }
    builder = builder
        .pass(
            PassDraft::new("clear_morphology", "organism_clear_morphology")
                .bind("radial_density", "morphology.radial_density")
                .bind("component_count", "morphology.component_count")
                .bind("organizer_count", "morphology.organizer_count")
                .bind(
                    "undifferentiated_count",
                    "morphology.undifferentiated_count",
                )
                .bind("boundary_count", "morphology.boundary_count")
                .bind("interior_count", "morphology.interior_count")
                .bind("area_q16", "morphology.area_q16")
                .bind("perimeter_q16", "morphology.perimeter_q16")
                .bind("centroid_sum_x_q16", "morphology.centroid_sum_x_q16")
                .bind("centroid_sum_y_q16", "morphology.centroid_sum_y_q16")
                .dispatch_over("morphology.radial_density"),
        )
        .pass(
            PassDraft::new("reduce_morphology", "organism_reduce_morphology")
                .bind("stable_id", "cells.stable_id")
                .bind("fate", "cells.fate")
                .bind("position", "cells.position")
                .bind("radius", "cells.radius")
                .bind("surface_exposure_bin", "perception.surface_exposure_bin")
                .bind("component_label", "morphology.component_label_a")
                .bind("component_count", "morphology.component_count")
                .bind("organizer_count", "morphology.organizer_count")
                .bind(
                    "undifferentiated_count",
                    "morphology.undifferentiated_count",
                )
                .bind("boundary_count", "morphology.boundary_count")
                .bind("interior_count", "morphology.interior_count")
                .bind("area_q16", "morphology.area_q16")
                .bind("perimeter_q16", "morphology.perimeter_q16")
                .bind("centroid_sum_x_q16", "morphology.centroid_sum_x_q16")
                .bind("centroid_sum_y_q16", "morphology.centroid_sum_y_q16")
                .bind("active_count", "cells.active_count")
                .dispatch_over("cells.stable_id"),
        )
        .pass(
            PassDraft::new("finalize_morphology", "organism_finalize_morphology")
                .bind("active_count", "cells.active_count")
                .bind("population", "morphology.population")
                .bind("area_q16", "morphology.area_q16")
                .bind("perimeter_q16", "morphology.perimeter_q16")
                .bind("centroid_sum_x_q16", "morphology.centroid_sum_x_q16")
                .bind("centroid_sum_y_q16", "morphology.centroid_sum_y_q16")
                .bind("centroid_x_q16", "morphology.centroid_x_q16")
                .bind("centroid_y_q16", "morphology.centroid_y_q16")
                .bind("compactness_q16", "morphology.compactness_q16"),
        )
        .pass(
            PassDraft::new("reduce_radial_density", "organism_reduce_radial_density")
                .bind("position", "cells.position")
                .bind("centroid_x_q16", "morphology.centroid_x_q16")
                .bind("centroid_y_q16", "morphology.centroid_y_q16")
                .bind("radial_density", "morphology.radial_density")
                .bind("active_count", "cells.active_count")
                .dispatch_over("cells.stable_id"),
        )
        .pass(
            PassDraft::new(
                "clear_regeneration_metrics",
                "organism_clear_regeneration_metrics",
            )
            .bind("region_occupancy", "lesion.region_occupancy")
            .bind("injury_total_q16", "regeneration.injury_total_q16")
            .bind("injury_peak_q16", "regeneration.injury_peak_q16"),
        )
        .pass(
            PassDraft::new(
                "reduce_lesion_occupancy",
                "organism_reduce_lesion_occupancy",
            )
            .bind("position", "cells.position")
            .bind("cell_radius", "cells.radius")
            .bind("center_q16", "lesion.center_q16")
            .bind("radius_q16", "lesion.radius_q16")
            .bind("region_occupancy", "lesion.region_occupancy")
            .bind("active_count", "cells.active_count")
            .dispatch_over("cells.position"),
        )
        .pass(
            PassDraft::new("reduce_injury", "organism_reduce_injury")
                .bind("injury", "field.injury")
                .bind("injury_total_q16", "regeneration.injury_total_q16")
                .bind("injury_peak_q16", "regeneration.injury_peak_q16")
                .dispatch_over("field.injury"),
        )
        .pass(
            PassDraft::new("audit_regeneration", "organism_audit_regeneration")
                .bind("tick", "simulation.tick")
                .bind("population", "morphology.population")
                .bind("component_count", "morphology.component_count")
                .bind("component_unresolved", "morphology.component_unresolved")
                .bind("organizer_count", "morphology.organizer_count")
                .bind("boundary_count", "morphology.boundary_count")
                .bind("interior_count", "morphology.interior_count")
                .bind("area_q16", "morphology.area_q16")
                .bind("compactness_q16", "morphology.compactness_q16")
                .bind("centroid_x_q16", "morphology.centroid_x_q16")
                .bind("centroid_y_q16", "morphology.centroid_y_q16")
                .bind("injury_total_q16", "regeneration.injury_total_q16")
                .bind("post_lesion_peak_q16", "regeneration.post_lesion_peak_q16")
                .bind("removed_count", "lesion.removed_count")
                .bind("region_occupancy", "lesion.region_occupancy")
                .bind("metric_min", "homeostasis.metric_min")
                .bind("metric_max", "homeostasis.metric_max")
                .bind("reference_samples", "homeostasis.reference_samples")
                .bind("neighbor_overflow", "population.neighbor_overflow")
                .bind("physical_overflow", "population.physical_neighbor_overflow")
                .bind("perception_truncation", "population.perception_truncation")
                .bind("deposit_saturation", "deposit.saturation_count")
                .bind("energy_residual", "ledger.residual")
                .bind("window", "regeneration.window")
                .bind("consecutive_ticks", "regeneration.consecutive_ticks")
                .bind("success_tick", "regeneration.success_tick"),
        );

    let mut schedule = ScheduleDraft::fixed("simulation", 120)
        .run("clear_population_bins")
        .run_after("bin_living", "clear_population_bins")
        .run_after("sort_living_bins", "bin_living")
        .run_after("observe_neighbors", "sort_living_bins")
        .run_after("initialize_components", "observe_neighbors");
    for (pass, predecessor) in component_dependencies {
        schedule = schedule.run_after(pass, predecessor);
    }
    schedule = schedule
        .run_after("clear_morphology", component_predecessor)
        .run_after("reduce_morphology", "clear_morphology")
        .run_after("finalize_morphology", "reduce_morphology")
        .run_after("reduce_radial_density", "finalize_morphology")
        .run_after("clear_regeneration_metrics", "reduce_radial_density")
        .run_after("reduce_lesion_occupancy", "clear_regeneration_metrics")
        .run_after("reduce_injury", "reduce_lesion_occupancy")
        .run_after("begin_energy_ledger", "reduce_injury")
        .run_after("sample", "begin_energy_ledger")
        .run_after("decide", "sample")
        .run_after("resolve_state", "decide")
        .run_after("reduce_energy_ledger", "resolve_state")
        .run_after("clear_deposits", "reduce_energy_ledger")
        .run_after("deposit", "clear_deposits")
        .run_after("diffuse", "deposit")
        .run_after("commit_fields", "diffuse")
        .run_after("initialize_population_order", "commit_fields");
    for (pass, predecessor) in radix_dependencies {
        schedule = schedule.run_after(pass, predecessor);
    }
    schedule = schedule
        .run_after("prequalify_population", radix_predecessor)
        .run_after("bin_candidates", "prequalify_population")
        .run_after("sort_candidate_bins", "bin_candidates")
        .run_after("resolve_candidate_conflicts", "sort_candidate_bins")
        .run_after("scan_population_blocks", "resolve_candidate_conflicts")
        .run_after("scan_population_block_sums", "scan_population_blocks")
        .run_after("add_population_block_offsets", "scan_population_block_sums")
        .run_after("resolve_population_counts", "add_population_block_offsets")
        .run_after("scatter_population_core", "resolve_population_counts")
        .run_after("scatter_population_development", "scatter_population_core")
        .run_after("commit_population_core", "scatter_population_development")
        .run_after("commit_population_development", "commit_population_core")
        .run_after("finalize_population", "commit_population_development")
        .run_after("finalize_energy_ledger", "finalize_population")
        .run_after("audit_regeneration", "finalize_energy_ledger")
        .run_after("measure_homeostasis_events", "audit_regeneration")
        .run_after("audit_homeostasis", "measure_homeostasis_events")
        .run_after("advance_tick", "audit_homeostasis")
        .show_after("organism", "advance_tick")
        .tick_overlap(TickOverlapPolicy::QueueOrderedReuse)
        .presentation_lifetime(PresentationLifetimePolicy::QueueOrderedReuse)
        .queue_model(QueueModel::SingleSerialQueue);

    builder
        .kernel(packaged_kernel(
            "organism_sample",
            vec![
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("energy", f32_type.clone(), SlotAccess::Read, false),
                stream_slot("activator", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("inhibitor", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("nutrient", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("density", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("injury", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("activator_bin", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("inhibitor_bin", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("nutrient_bin", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("density_bin", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("injury_bin", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("energy_bin", u32_type.clone(), SlotAccess::Write, false),
                value_slot("width", u32_type.clone()),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_decide",
            [
                ("stable_id", SlotAccess::Read),
                ("fate", SlotAccess::Read),
                ("phase", SlotAccess::Read),
                ("health", SlotAccess::Read),
                ("age", SlotAccess::Read),
                ("fate_confidence", SlotAccess::Read),
                ("time_in_fate", SlotAccess::Read),
                ("activator_bin", SlotAccess::Read),
                ("inhibitor_bin", SlotAccess::Read),
                ("nutrient_bin", SlotAccess::Read),
                ("density_bin", SlotAccess::Read),
                ("local_density_bin", SlotAccess::Read),
                ("contact_count", SlotAccess::Read),
                ("surface_mask", SlotAccess::Read),
                ("surface_exposure_bin", SlotAccess::Read),
                ("recent_surface_exposure", SlotAccess::Read),
                ("energy_bin", SlotAccess::Read),
                ("injury_bin", SlotAccess::Read),
                ("repair_enabled", SlotAccess::Read),
                ("requested_fate", SlotAccess::Write),
                ("requested_phase", SlotAccess::Write),
                ("requested_health", SlotAccess::Write),
                ("divide_intent", SlotAccess::Write),
                ("death_intent", SlotAccess::Write),
                ("repair_intent", SlotAccess::Write),
                ("activator_deposit", SlotAccess::Write),
                ("inhibitor_deposit", SlotAccess::Write),
                ("injury_deposit", SlotAccess::Write),
            ]
            .into_iter()
            .map(|(name, access)| {
                stream_slot(name, u32_type.clone(), access, name == "repair_enabled")
            })
            .chain(std::iter::once(stream_slot(
                "active_count",
                u32_type.clone(),
                SlotAccess::Read,
                true,
            )))
            .collect(),
        ))
        .kernel(packaged_kernel(
            "organism_resolve_state",
            vec![
                stream_slot("fate", u32_type.clone(), SlotAccess::ReadWrite, false),
                stream_slot("phase", u32_type.clone(), SlotAccess::ReadWrite, false),
                stream_slot("health", u32_type.clone(), SlotAccess::ReadWrite, false),
                stream_slot("previous_fate", u32_type.clone(), SlotAccess::Write, false),
                stream_slot(
                    "fate_confidence",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    false,
                ),
                stream_slot(
                    "time_in_fate",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    false,
                ),
                stream_slot(
                    "recent_activator",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    false,
                ),
                stream_slot(
                    "recent_inhibitor",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    false,
                ),
                stream_slot(
                    "recent_surface_exposure",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    false,
                ),
                stream_slot("age", u32_type.clone(), SlotAccess::ReadWrite, false),
                stream_slot("energy", f32_type.clone(), SlotAccess::ReadWrite, false),
                stream_slot("color", vec4.clone(), SlotAccess::Write, false),
                stream_slot("requested_fate", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("requested_phase", u32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "requested_health",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("nutrient_bin", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("activator_bin", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("inhibitor_bin", u32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "surface_exposure_bin",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "activator_deposit",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "inhibitor_deposit",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("injury_deposit", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("event_hash", u32_type.clone(), SlotAccess::ReadWrite, false),
                stream_slot("divide_intent", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("death_intent", u32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "ledger_absorbed",
                    f32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "ledger_maintenance",
                    f32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "ledger_decisions",
                    f32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "ledger_signaling",
                    f32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_clear_deposits",
            vec![
                stream_slot("activator", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("inhibitor", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("density", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("injury", u32_type.clone(), SlotAccess::Write, false),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_deposit",
            vec![
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "activator_amount",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "inhibitor_amount",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("injury_amount", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("activator", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("inhibitor", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("density", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("injury", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot(
                    "saturation_count",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                value_slot("width", u32_type.clone()),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_diffuse",
            vec![
                stream_slot("activator", f32_type.clone(), SlotAccess::Read, false),
                stream_slot("inhibitor", f32_type.clone(), SlotAccess::Read, false),
                stream_slot("nutrient", f32_type.clone(), SlotAccess::Read, false),
                stream_slot("density", f32_type.clone(), SlotAccess::Read, false),
                stream_slot("injury", f32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "activator_deposit",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "inhibitor_deposit",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("density_deposit", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("injury_deposit", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("activator_next", f32_type.clone(), SlotAccess::Write, false),
                stream_slot("inhibitor_next", f32_type.clone(), SlotAccess::Write, false),
                stream_slot("nutrient_next", f32_type.clone(), SlotAccess::Write, false),
                stream_slot("density_next", f32_type.clone(), SlotAccess::Write, false),
                stream_slot("injury_next", f32_type.clone(), SlotAccess::Write, false),
                value_slot("width", u32_type.clone()),
                value_slot("activator_transport", f32_type.clone()),
                value_slot("inhibitor_transport", f32_type.clone()),
                stream_slot("nutrient_supply", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("injury_transport", f32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_commit_fields",
            [
                ("activator", SlotAccess::Write),
                ("inhibitor", SlotAccess::Write),
                ("nutrient", SlotAccess::Write),
                ("density", SlotAccess::Write),
                ("injury", SlotAccess::Write),
                ("activator_next", SlotAccess::Read),
                ("inhibitor_next", SlotAccess::Read),
                ("nutrient_next", SlotAccess::Read),
                ("density_next", SlotAccess::Read),
                ("injury_next", SlotAccess::Read),
            ]
            .into_iter()
            .map(|(name, access)| stream_slot(name, f32_type.clone(), access, false))
            .collect(),
        ))
        .kernel(packaged_kernel(
            "organism_begin_energy_ledger",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("energy", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("previous_total", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("absorbed", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("maintenance", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("decisions", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("motion", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("signaling", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("division", f32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "environmental_death_loss",
                    f32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot("current_total", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("residual", f32_type.clone(), SlotAccess::Write, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_reduce_energy_ledger",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("cell_absorbed", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("cell_maintenance", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("cell_decisions", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("cell_signaling", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("death_intent", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("energy", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("absorbed", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("maintenance", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("decisions", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("signaling", f32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "environmental_death_loss",
                    f32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_finalize_energy_ledger",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("energy", f32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "accepted_birth_count",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("previous_total", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("absorbed", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("maintenance", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("decisions", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("motion", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("signaling", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("division", f32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "environmental_death_loss",
                    f32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("current_total", f32_type.clone(), SlotAccess::Write, true),
                stream_slot("residual", f32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "cumulative_residual",
                    f32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_measure_homeostasis_events",
            vec![
                stream_slot("tick", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("component_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "component_unresolved",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("organizer_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("boundary_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("interior_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "neighbor_overflow",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "physical_overflow",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "perception_truncation",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "deposit_saturation",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("current_energy", f32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "perturbation_window",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "invariant_violations",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot(
                    "perturbation_energy_min",
                    f32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_audit_homeostasis",
            vec![
                stream_slot("tick", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("population", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("component_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "component_unresolved",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("organizer_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("boundary_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("interior_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("area_q16", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("perimeter_q16", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("compactness_q16", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "centroid_x_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "centroid_y_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("radial_density", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("current_energy", f32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "neighbor_overflow",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "physical_overflow",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "perception_truncation",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "deposit_saturation",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("metric_min", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("metric_max", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("metric_sum", f32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot(
                    "metric_sum_sq",
                    f32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot("energy_min", f32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("energy_max", f32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("energy_sum", f32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot(
                    "energy_sum_sq",
                    f32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot(
                    "reference_samples",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot(
                    "validation_samples",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot(
                    "validation_violations",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot("window", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_advance_tick",
            vec![stream_slot(
                "tick",
                u32_type.clone(),
                SlotAccess::ReadWrite,
                true,
            )],
        ))
        .kernel(packaged_kernel(
            "organism_set_nutrient_supply",
            vec![
                stream_slot("nutrient_supply", f32_type.clone(), SlotAccess::Write, true),
                value_slot("requested_supply", f32_type.clone()),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_set_injury_transport",
            vec![
                stream_slot(
                    "injury_transport",
                    f32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                value_slot("requested_transport", f32_type.clone()),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_set_repair_enabled",
            vec![
                stream_slot("repair_enabled", u32_type.clone(), SlotAccess::Write, true),
                value_slot("requested_enabled", u32_type.clone()),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_apply_lesion_cells",
            vec![
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("fate", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("health", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("event_hash", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("energy", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                value_slot("center_x_q16", DataType::Scalar(ScalarType::I32)),
                value_slot("center_y_q16", DataType::Scalar(ScalarType::I32)),
                value_slot("radius_q16", u32_type.clone()),
                stream_slot("removed_ids", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("removed_count", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("damaged_count", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("removed_energy", f32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "recorded_center_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot(
                    "recorded_radius_q16",
                    u32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_apply_lesion_field",
            vec![
                stream_slot("injury", f32_type.clone(), SlotAccess::ReadWrite, false),
                value_slot("width", u32_type.clone()),
                value_slot("center_x_q16", DataType::Scalar(ScalarType::I32)),
                value_slot("center_y_q16", DataType::Scalar(ScalarType::I32)),
                value_slot("radius_q16", u32_type.clone()),
                value_slot("injury_strength", f32_type.clone()),
                stream_slot("injury_transport", f32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_clear_regeneration_metrics",
            vec![
                stream_slot(
                    "region_occupancy",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot(
                    "injury_total_q16",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot(
                    "injury_peak_q16",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_reduce_lesion_occupancy",
            vec![
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot_unit(
                    "cell_radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "center_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("radius_q16", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "region_occupancy",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_reduce_injury",
            vec![
                stream_slot("injury", f32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "injury_total_q16",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot(
                    "injury_peak_q16",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_audit_regeneration",
            vec![
                stream_slot("tick", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("population", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("component_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "component_unresolved",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("organizer_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("boundary_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("interior_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("area_q16", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("compactness_q16", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "centroid_x_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "centroid_y_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("injury_total_q16", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "post_lesion_peak_q16",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot("removed_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("region_occupancy", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("metric_min", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("metric_max", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "reference_samples",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "neighbor_overflow",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "physical_overflow",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "perception_truncation",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "deposit_saturation",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("energy_residual", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("window", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "consecutive_ticks",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot(
                    "success_tick",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_initialize_population_order",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("order", u32_type.clone(), SlotAccess::Write, false),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_radix_histogram",
            vec![
                stream_slot("order", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("block_count", u32_type.clone(), SlotAccess::Write, true),
                value_slot("shift", u32_type.clone()),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_radix_offsets",
            vec![
                stream_slot("block_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("offset", u32_type.clone(), SlotAccess::Write, true),
                value_slot("block_count_value", u32_type.clone()),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_radix_scatter",
            vec![
                stream_slot("input", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("output", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("offset", u32_type.clone(), SlotAccess::Read, true),
                value_slot("shift", u32_type.clone()),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_clear_population_bins",
            vec![
                stream_slot("living_count", u32_type.clone(), SlotAccess::Write, false),
                stream_slot(
                    "candidate_count",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_bin_living",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("count", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("indices", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("overflow", u32_type.clone(), SlotAccess::Atomic, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_sort_bins",
            vec![
                stream_slot("count", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("indices", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_observe_neighbors",
            vec![
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot_unit(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("bin_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("bin_indices", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "local_density_bin",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot("neighbor_count", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("contact_count", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("surface_mask", u32_type.clone(), SlotAccess::Write, false),
                stream_slot(
                    "surface_exposure_bin",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "physical_overflow",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot(
                    "perception_truncation",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_initialize_components",
            vec![
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("label", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_clear_component_changes",
            vec![stream_slot(
                "changes",
                u32_type.clone(),
                SlotAccess::Write,
                true,
            )],
        ))
        .kernel(packaged_kernel(
            "organism_relax_components",
            vec![
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot_unit(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("bin_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("bin_indices", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("input_label", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("output_label", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("changes", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_clear_morphology",
            vec![
                stream_slot("radial_density", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("component_count", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("organizer_count", u32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "undifferentiated_count",
                    u32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot("boundary_count", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("interior_count", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("area_q16", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("perimeter_q16", u32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "centroid_sum_x_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot(
                    "centroid_sum_y_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Write,
                    true,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_reduce_morphology",
            vec![
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("fate", u32_type.clone(), SlotAccess::Read, false),
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot_unit(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "surface_exposure_bin",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("component_label", u32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "component_count",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot(
                    "organizer_count",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot(
                    "undifferentiated_count",
                    u32_type.clone(),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot("boundary_count", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("interior_count", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("area_q16", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("perimeter_q16", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot(
                    "centroid_sum_x_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot(
                    "centroid_sum_y_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Atomic,
                    true,
                ),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_finalize_morphology",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("population", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("area_q16", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("perimeter_q16", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "centroid_sum_x_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "centroid_sum_y_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "centroid_x_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot(
                    "centroid_y_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot("compactness_q16", u32_type.clone(), SlotAccess::Write, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_reduce_radial_density",
            vec![
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "centroid_x_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "centroid_y_q16",
                    DataType::Scalar(ScalarType::I32),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("radial_density", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_prequalify_population",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot_unit(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("age", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("surface_mask", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("divide", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("death", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("repair", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("injury", f32_type.clone(), SlotAccess::Read, true),
                stream_slot("density", f32_type.clone(), SlotAccess::Read, true),
                value_slot("field_width", u32_type.clone()),
                stream_slot("bin_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("bin_indices", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("survival", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("birth", u32_type.clone(), SlotAccess::Write, false),
                stream_slot(
                    "candidate_sector",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot("overflow", u32_type.clone(), SlotAccess::Atomic, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_bin_candidates",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot_unit(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("birth", u32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "candidate_sector",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("count", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("indices", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("overflow", u32_type.clone(), SlotAccess::Atomic, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_resolve_candidate_conflicts",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot_unit(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("prequalified", u32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "candidate_sector",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("bin_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("bin_indices", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("birth", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("overflow", u32_type.clone(), SlotAccess::Atomic, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_scan_population_blocks",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("order", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("survival", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("birth", u32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "survival_prefix",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot("birth_prefix", u32_type.clone(), SlotAccess::Write, false),
                stream_slot(
                    "survival_block_sum",
                    u32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot("birth_block_sum", u32_type.clone(), SlotAccess::Write, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_scan_population_block_sums",
            vec![
                stream_slot(
                    "survival_block_sum",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("birth_block_sum", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "survival_block_prefix",
                    u32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot(
                    "birth_block_prefix",
                    u32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                value_slot("block_count", u32_type.clone()),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_add_population_block_offsets",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "survival_prefix",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    false,
                ),
                stream_slot(
                    "birth_prefix",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    false,
                ),
                stream_slot(
                    "survival_block_prefix",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot(
                    "birth_block_prefix",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_resolve_population_counts",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("survival", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("birth", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("survival_prefix", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("birth_prefix", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("survivor_count", u32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "accepted_birth_count",
                    u32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot("next_count", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("rejected_births", u32_type.clone(), SlotAccess::Write, true),
                value_slot("capacity", u32_type.clone()),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_scatter_population_core",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("order", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("parent_id", u32_type.clone(), SlotAccess::Read, true),
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot_unit(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("energy", f32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "candidate_sector",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("survival", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("birth", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("survival_prefix", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("birth_prefix", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("survivor_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "accepted_birth_count",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("next_stable_id", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "stage_stable_id",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "stage_parent_id",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot_unit(
                    "stage_position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Write,
                    false,
                ),
                stream_slot_unit(
                    "stage_radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Write,
                    false,
                ),
                stream_slot("stage_energy", f32_type.clone(), SlotAccess::Write, false),
                stream_slot("event_hash", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "stage_event_hash",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_scatter_population_development",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("order", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("age", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("fate", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("phase", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("health", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("previous_fate", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("fate_confidence", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("time_in_fate", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("recent_activator", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("recent_inhibitor", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "recent_surface_exposure",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("color", vec4.clone(), SlotAccess::Read, true),
                stream_slot("survival", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("birth", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("survival_prefix", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("birth_prefix", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("survivor_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "accepted_birth_count",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
                stream_slot("repair", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("stage_age", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("stage_fate", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("stage_phase", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("stage_health", u32_type.clone(), SlotAccess::Write, false),
                stream_slot(
                    "stage_previous_fate",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "stage_fate_confidence",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "stage_time_in_fate",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "stage_recent_activator",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "stage_recent_inhibitor",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot(
                    "stage_recent_surface_exposure",
                    u32_type.clone(),
                    SlotAccess::Write,
                    false,
                ),
                stream_slot("stage_color", vec4.clone(), SlotAccess::Write, false),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_commit_population_core",
            vec![
                stream_slot("next_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("stage_stable_id", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("stage_parent_id", u32_type.clone(), SlotAccess::Read, false),
                stream_slot_unit(
                    "stage_position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot_unit(
                    "stage_radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("stage_energy", f32_type.clone(), SlotAccess::Read, false),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("parent_id", u32_type.clone(), SlotAccess::Write, true),
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::Write,
                    true,
                ),
                stream_slot_unit(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Write,
                    true,
                ),
                stream_slot("energy", f32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "stage_event_hash",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("event_hash", u32_type.clone(), SlotAccess::Write, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_commit_population_development",
            vec![
                stream_slot("next_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("stage_age", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("stage_fate", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("stage_phase", u32_type.clone(), SlotAccess::Read, false),
                stream_slot("stage_health", u32_type.clone(), SlotAccess::Read, false),
                stream_slot(
                    "stage_previous_fate",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "stage_fate_confidence",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "stage_time_in_fate",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "stage_recent_activator",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "stage_recent_inhibitor",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot(
                    "stage_recent_surface_exposure",
                    u32_type.clone(),
                    SlotAccess::Read,
                    false,
                ),
                stream_slot("stage_color", vec4.clone(), SlotAccess::Read, false),
                stream_slot("age", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("fate", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("phase", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("health", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("previous_fate", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("fate_confidence", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("time_in_fate", u32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "recent_activator",
                    u32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot(
                    "recent_inhibitor",
                    u32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot(
                    "recent_surface_exposure",
                    u32_type.clone(),
                    SlotAccess::Write,
                    true,
                ),
                stream_slot("color", vec4.clone(), SlotAccess::Write, true),
            ],
        ))
        .kernel(packaged_kernel(
            "organism_finalize_population",
            vec![
                stream_slot("active_count", u32_type.clone(), SlotAccess::Write, true),
                stream_slot(
                    "next_stable_id",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot("next_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot(
                    "accepted_birth_count",
                    u32_type.clone(),
                    SlotAccess::Read,
                    true,
                ),
            ],
        ))
        .pass(
            PassDraft::new("begin_energy_ledger", "organism_begin_energy_ledger")
                .bind("active_count", "cells.active_count")
                .bind("energy", "cells.energy")
                .bind("previous_total", "ledger.previous_total")
                .bind("absorbed", "ledger.absorbed")
                .bind("maintenance", "ledger.maintenance")
                .bind("decisions", "ledger.decisions")
                .bind("motion", "ledger.motion")
                .bind("signaling", "ledger.signaling")
                .bind("division", "ledger.division")
                .bind(
                    "environmental_death_loss",
                    "ledger.environmental_death_loss",
                )
                .bind("current_total", "ledger.current_total")
                .bind("residual", "ledger.residual"),
        )
        .pass(
            PassDraft::new("set_nutrient_supply", "organism_set_nutrient_supply")
                .bind("nutrient_supply", "environment.nutrient_supply")
                .bind("requested_supply", "environment.reference_nutrient_supply")
                .grant("mutate_environment"),
        )
        .pass(
            PassDraft::new("ablate_injury_transport", "organism_set_injury_transport")
                .bind("injury_transport", "environment.injury_transport")
                .bind(
                    "requested_transport",
                    "environment.requested_injury_transport",
                )
                .grant("mutate_environment"),
        )
        .pass(
            PassDraft::new("set_repair_enabled", "organism_set_repair_enabled")
                .bind("repair_enabled", "environment.repair_enabled")
                .bind("requested_enabled", "environment.requested_repair_enabled")
                .grant("mutate_environment"),
        )
        .pass(
            PassDraft::new("apply_lesion_cells", "organism_apply_lesion_cells")
                .bind("position", "cells.position")
                .bind("stable_id", "cells.stable_id")
                .bind("fate", "cells.fate")
                .bind("health", "cells.health")
                .bind("event_hash", "cells.event_hash")
                .bind("energy", "cells.energy")
                .bind("active_count", "cells.active_count")
                .bind("center_x_q16", "lesion.reference_center_x_q16")
                .bind("center_y_q16", "lesion.reference_center_y_q16")
                .bind("radius_q16", "lesion.reference_radius_q16")
                .bind("removed_ids", "lesion.removed_ids")
                .bind("removed_count", "lesion.removed_count")
                .bind("damaged_count", "lesion.damaged_count")
                .bind("removed_energy", "lesion.removed_energy")
                .bind("recorded_center_q16", "lesion.center_q16")
                .bind("recorded_radius_q16", "lesion.radius_q16")
                .grant("mutate_cell_state"),
        )
        .pass(
            PassDraft::new("apply_lesion_field", "organism_apply_lesion_field")
                .bind("injury", "field.injury")
                .bind("width", "field.width")
                .bind("center_x_q16", "lesion.reference_center_x_q16")
                .bind("center_y_q16", "lesion.reference_center_y_q16")
                .bind("radius_q16", "lesion.reference_radius_q16")
                .bind("injury_strength", "lesion.reference_injury")
                .bind("injury_transport", "environment.injury_transport")
                .dispatch_over("field.injury")
                .grant("mutate_field_state"),
        )
        .pass(
            PassDraft::new("sample", "organism_sample")
                .bind("position", "cells.position")
                .bind("energy", "cells.energy")
                .bind("activator", "field.activator")
                .bind("inhibitor", "field.inhibitor")
                .bind("nutrient", "field.nutrient")
                .bind("density", "field.density")
                .bind("injury", "field.injury")
                .bind("activator_bin", "perception.activator_bin")
                .bind("inhibitor_bin", "perception.inhibitor_bin")
                .bind("nutrient_bin", "perception.nutrient_bin")
                .bind("density_bin", "perception.density_bin")
                .bind("injury_bin", "perception.injury_bin")
                .bind("energy_bin", "perception.energy_bin")
                .bind("width", "field.width")
                .bind("active_count", "cells.active_count")
                .dispatch_over("cells.position"),
        )
        .pass(
            PassDraft::new("decide", "organism_decide")
                .bind("stable_id", "cells.stable_id")
                .bind("fate", "cells.fate")
                .bind("phase", "cells.phase")
                .bind("health", "cells.health")
                .bind("age", "cells.age")
                .bind("fate_confidence", "cells.fate_confidence")
                .bind("time_in_fate", "cells.time_in_fate")
                .bind("activator_bin", "perception.activator_bin")
                .bind("inhibitor_bin", "perception.inhibitor_bin")
                .bind("nutrient_bin", "perception.nutrient_bin")
                .bind("density_bin", "perception.density_bin")
                .bind("local_density_bin", "perception.local_density_bin")
                .bind("contact_count", "perception.contact_count")
                .bind("surface_mask", "perception.surface_mask")
                .bind("surface_exposure_bin", "perception.surface_exposure_bin")
                .bind("recent_surface_exposure", "cells.recent_surface_exposure")
                .bind("energy_bin", "perception.energy_bin")
                .bind("injury_bin", "perception.injury_bin")
                .bind("repair_enabled", "environment.repair_enabled")
                .bind("requested_fate", "intent.requested_fate")
                .bind("requested_phase", "intent.requested_phase")
                .bind("requested_health", "intent.requested_health")
                .bind("divide_intent", "intent.divide")
                .bind("death_intent", "intent.death")
                .bind("repair_intent", "intent.repair")
                .bind("activator_deposit", "intent.activator_deposit")
                .bind("inhibitor_deposit", "intent.inhibitor_deposit")
                .bind("injury_deposit", "intent.injury_deposit")
                .bind("active_count", "cells.active_count")
                .dispatch_over("cells.stable_id"),
        )
        .pass(
            PassDraft::new("resolve_state", "organism_resolve_state")
                .bind("fate", "cells.fate")
                .bind("phase", "cells.phase")
                .bind("health", "cells.health")
                .bind("previous_fate", "cells.previous_fate")
                .bind("fate_confidence", "cells.fate_confidence")
                .bind("time_in_fate", "cells.time_in_fate")
                .bind("recent_activator", "cells.recent_activator")
                .bind("recent_inhibitor", "cells.recent_inhibitor")
                .bind("recent_surface_exposure", "cells.recent_surface_exposure")
                .bind("age", "cells.age")
                .bind("energy", "cells.energy")
                .bind("color", "cells.color")
                .bind("requested_fate", "intent.requested_fate")
                .bind("requested_phase", "intent.requested_phase")
                .bind("requested_health", "intent.requested_health")
                .bind("nutrient_bin", "perception.nutrient_bin")
                .bind("activator_bin", "perception.activator_bin")
                .bind("inhibitor_bin", "perception.inhibitor_bin")
                .bind("surface_exposure_bin", "perception.surface_exposure_bin")
                .bind("activator_deposit", "intent.activator_deposit")
                .bind("inhibitor_deposit", "intent.inhibitor_deposit")
                .bind("injury_deposit", "intent.injury_deposit")
                .bind("active_count", "cells.active_count")
                .bind("stable_id", "cells.stable_id")
                .bind("event_hash", "cells.event_hash")
                .bind("divide_intent", "intent.divide")
                .bind("death_intent", "intent.death")
                .bind("ledger_absorbed", "ledger.cell_absorbed")
                .bind("ledger_maintenance", "ledger.cell_maintenance")
                .bind("ledger_decisions", "ledger.cell_decisions")
                .bind("ledger_signaling", "ledger.cell_signaling")
                .dispatch_over("cells.stable_id")
                .grant("mutate_cell_state"),
        )
        .pass(
            PassDraft::new("reduce_energy_ledger", "organism_reduce_energy_ledger")
                .bind("active_count", "cells.active_count")
                .bind("cell_absorbed", "ledger.cell_absorbed")
                .bind("cell_maintenance", "ledger.cell_maintenance")
                .bind("cell_decisions", "ledger.cell_decisions")
                .bind("cell_signaling", "ledger.cell_signaling")
                .bind("death_intent", "intent.death")
                .bind("energy", "cells.energy")
                .bind("absorbed", "ledger.absorbed")
                .bind("maintenance", "ledger.maintenance")
                .bind("decisions", "ledger.decisions")
                .bind("signaling", "ledger.signaling")
                .bind(
                    "environmental_death_loss",
                    "ledger.environmental_death_loss",
                ),
        )
        .pass(
            PassDraft::new("clear_deposits", "organism_clear_deposits")
                .bind("activator", "deposit.activator_q16")
                .bind("inhibitor", "deposit.inhibitor_q16")
                .bind("density", "deposit.density_q16")
                .bind("injury", "deposit.injury_q16")
                .dispatch_over("deposit.activator_q16"),
        )
        .pass(
            PassDraft::new("deposit", "organism_deposit")
                .bind("position", "cells.position")
                .bind("activator_amount", "intent.activator_deposit")
                .bind("inhibitor_amount", "intent.inhibitor_deposit")
                .bind("injury_amount", "intent.injury_deposit")
                .bind("activator", "deposit.activator_q16")
                .bind("inhibitor", "deposit.inhibitor_q16")
                .bind("density", "deposit.density_q16")
                .bind("injury", "deposit.injury_q16")
                .bind("saturation_count", "deposit.saturation_count")
                .bind("width", "field.width")
                .bind("active_count", "cells.active_count")
                .dispatch_over("cells.position"),
        )
        .pass(
            PassDraft::new("diffuse", "organism_diffuse")
                .bind("activator", "field.activator")
                .bind("inhibitor", "field.inhibitor")
                .bind("nutrient", "field.nutrient")
                .bind("density", "field.density")
                .bind("injury", "field.injury")
                .bind("activator_deposit", "deposit.activator_q16")
                .bind("inhibitor_deposit", "deposit.inhibitor_q16")
                .bind("density_deposit", "deposit.density_q16")
                .bind("injury_deposit", "deposit.injury_q16")
                .bind("activator_next", "field.activator_next")
                .bind("inhibitor_next", "field.inhibitor_next")
                .bind("nutrient_next", "field.nutrient_next")
                .bind("density_next", "field.density_next")
                .bind("injury_next", "field.injury_next")
                .bind("width", "field.width")
                .bind("activator_transport", "field.activator_transport")
                .bind("inhibitor_transport", "field.inhibitor_transport")
                .bind("nutrient_supply", "environment.nutrient_supply")
                .bind("injury_transport", "environment.injury_transport")
                .dispatch_over("field.activator")
                .grant("mutate_field_state"),
        )
        .pass(
            PassDraft::new("commit_fields", "organism_commit_fields")
                .bind("activator", "field.activator")
                .bind("inhibitor", "field.inhibitor")
                .bind("nutrient", "field.nutrient")
                .bind("density", "field.density")
                .bind("injury", "field.injury")
                .bind("activator_next", "field.activator_next")
                .bind("inhibitor_next", "field.inhibitor_next")
                .bind("nutrient_next", "field.nutrient_next")
                .bind("density_next", "field.density_next")
                .bind("injury_next", "field.injury_next")
                .dispatch_over("field.activator")
                .grant("mutate_field_state"),
        )
        .pass(
            PassDraft::new("clear_population_bins", "organism_clear_population_bins")
                .bind("living_count", "spatial.living_bin_count")
                .bind("candidate_count", "spatial.candidate_bin_count")
                .dispatch_over("spatial.living_bin_count"),
        )
        .pass(
            PassDraft::new("bin_living", "organism_bin_living")
                .bind("active_count", "cells.active_count")
                .bind("position", "cells.position")
                .bind("count", "spatial.living_bin_count")
                .bind("indices", "spatial.living_bin_indices")
                .bind("overflow", "population.neighbor_overflow")
                .dispatch_over("population.survival_flag"),
        )
        .pass(
            PassDraft::new("sort_living_bins", "organism_sort_bins")
                .bind("count", "spatial.living_bin_count")
                .bind("indices", "spatial.living_bin_indices")
                .bind("stable_id", "cells.stable_id")
                .dispatch_over("spatial.living_bin_count"),
        )
        .pass(
            PassDraft::new("prequalify_population", "organism_prequalify_population")
                .bind("active_count", "cells.active_count")
                .bind("position", "cells.position")
                .bind("radius", "cells.radius")
                .bind("stable_id", "cells.stable_id")
                .bind("age", "cells.age")
                .bind("surface_mask", "perception.surface_mask")
                .bind("divide", "intent.divide")
                .bind("death", "intent.death")
                .bind("repair", "intent.repair")
                .bind("injury", "field.injury")
                .bind("density", "field.density")
                .bind("field_width", "field.width")
                .bind("bin_count", "spatial.living_bin_count")
                .bind("bin_indices", "spatial.living_bin_indices")
                .bind("survival", "population.survival_flag")
                .bind("birth", "population.birth_prequalified")
                .bind("candidate_sector", "population.candidate_sector")
                .bind("overflow", "population.neighbor_overflow")
                .dispatch_over("population.survival_flag"),
        )
        .pass(
            PassDraft::new("bin_candidates", "organism_bin_candidates")
                .bind("active_count", "cells.active_count")
                .bind("position", "cells.position")
                .bind("radius", "cells.radius")
                .bind("stable_id", "cells.stable_id")
                .bind("birth", "population.birth_prequalified")
                .bind("candidate_sector", "population.candidate_sector")
                .bind("count", "spatial.candidate_bin_count")
                .bind("indices", "spatial.candidate_bin_indices")
                .bind("overflow", "population.neighbor_overflow")
                .dispatch_over("population.survival_flag"),
        )
        .pass(
            PassDraft::new("sort_candidate_bins", "organism_sort_bins")
                .bind("count", "spatial.candidate_bin_count")
                .bind("indices", "spatial.candidate_bin_indices")
                .bind("stable_id", "cells.stable_id")
                .dispatch_over("spatial.candidate_bin_count"),
        )
        .pass(
            PassDraft::new(
                "resolve_candidate_conflicts",
                "organism_resolve_candidate_conflicts",
            )
            .bind("active_count", "cells.active_count")
            .bind("position", "cells.position")
            .bind("radius", "cells.radius")
            .bind("stable_id", "cells.stable_id")
            .bind("prequalified", "population.birth_prequalified")
            .bind("candidate_sector", "population.candidate_sector")
            .bind("bin_count", "spatial.candidate_bin_count")
            .bind("bin_indices", "spatial.candidate_bin_indices")
            .bind("birth", "population.birth_flag")
            .bind("overflow", "population.neighbor_overflow")
            .dispatch_over("population.survival_flag"),
        )
        .pass(
            PassDraft::new("scan_population_blocks", "organism_scan_population_blocks")
                .bind("active_count", "cells.active_count")
                .bind("order", "population.order_a")
                .bind("survival", "population.survival_flag")
                .bind("birth", "population.birth_flag")
                .bind("survival_prefix", "population.survival_prefix")
                .bind("birth_prefix", "population.birth_prefix")
                .bind("survival_block_sum", "population.survival_block_sum")
                .bind("birth_block_sum", "population.birth_block_sum")
                .dispatch_over("population.survival_flag")
                .threads_per_threadgroup(SCAN_BLOCK_SIZE),
        )
        .pass(
            PassDraft::new(
                "scan_population_block_sums",
                "organism_scan_population_block_sums",
            )
            .bind("survival_block_sum", "population.survival_block_sum")
            .bind("birth_block_sum", "population.birth_block_sum")
            .bind("survival_block_prefix", "population.survival_block_prefix")
            .bind("birth_block_prefix", "population.birth_block_prefix")
            .bind("block_count", "population.scan_block_count")
            .dispatch_fixed(SCAN_BLOCK_SIZE)
            .threads_per_threadgroup(SCAN_BLOCK_SIZE),
        )
        .pass(
            PassDraft::new(
                "add_population_block_offsets",
                "organism_add_population_block_offsets",
            )
            .bind("active_count", "cells.active_count")
            .bind("survival_prefix", "population.survival_prefix")
            .bind("birth_prefix", "population.birth_prefix")
            .bind("survival_block_prefix", "population.survival_block_prefix")
            .bind("birth_block_prefix", "population.birth_block_prefix")
            .dispatch_over("population.survival_flag")
            .threads_per_threadgroup(SCAN_BLOCK_SIZE),
        )
        .pass(
            PassDraft::new(
                "resolve_population_counts",
                "organism_resolve_population_counts",
            )
            .bind("active_count", "cells.active_count")
            .bind("survival", "population.survival_flag")
            .bind("birth", "population.birth_flag")
            .bind("survival_prefix", "population.survival_prefix")
            .bind("birth_prefix", "population.birth_prefix")
            .bind("survivor_count", "population.survivor_count")
            .bind("accepted_birth_count", "population.accepted_birth_count")
            .bind("next_count", "population.next_count")
            .bind("rejected_births", "population.rejected_births")
            .bind("capacity", "organism.capacity"),
        )
        .pass(
            PassDraft::new(
                "scatter_population_core",
                "organism_scatter_population_core",
            )
            .bind("active_count", "cells.active_count")
            .bind("order", "population.order_a")
            .bind("stable_id", "cells.stable_id")
            .bind("parent_id", "cells.parent_id")
            .bind("position", "cells.position")
            .bind("radius", "cells.radius")
            .bind("energy", "cells.energy")
            .bind("candidate_sector", "population.candidate_sector")
            .bind("survival", "population.survival_flag")
            .bind("birth", "population.birth_flag")
            .bind("survival_prefix", "population.survival_prefix")
            .bind("birth_prefix", "population.birth_prefix")
            .bind("survivor_count", "population.survivor_count")
            .bind("accepted_birth_count", "population.accepted_birth_count")
            .bind("next_stable_id", "cells.next_stable_id")
            .bind("stage_stable_id", "population.stage_stable_id")
            .bind("stage_parent_id", "population.stage_parent_id")
            .bind("stage_position", "population.stage_position")
            .bind("stage_radius", "population.stage_radius")
            .bind("stage_energy", "population.stage_energy")
            .bind("event_hash", "cells.event_hash")
            .bind("stage_event_hash", "population.stage_event_hash")
            .dispatch_over("population.survival_flag"),
        )
        .pass(
            PassDraft::new(
                "scatter_population_development",
                "organism_scatter_population_development",
            )
            .bind("active_count", "cells.active_count")
            .bind("order", "population.order_a")
            .bind("age", "cells.age")
            .bind("fate", "cells.fate")
            .bind("phase", "cells.phase")
            .bind("health", "cells.health")
            .bind("previous_fate", "cells.previous_fate")
            .bind("fate_confidence", "cells.fate_confidence")
            .bind("time_in_fate", "cells.time_in_fate")
            .bind("recent_activator", "cells.recent_activator")
            .bind("recent_inhibitor", "cells.recent_inhibitor")
            .bind("recent_surface_exposure", "cells.recent_surface_exposure")
            .bind("color", "cells.color")
            .bind("survival", "population.survival_flag")
            .bind("birth", "population.birth_flag")
            .bind("survival_prefix", "population.survival_prefix")
            .bind("birth_prefix", "population.birth_prefix")
            .bind("survivor_count", "population.survivor_count")
            .bind("accepted_birth_count", "population.accepted_birth_count")
            .bind("repair", "intent.repair")
            .bind("stage_age", "population.stage_age")
            .bind("stage_fate", "population.stage_fate")
            .bind("stage_phase", "population.stage_phase")
            .bind("stage_health", "population.stage_health")
            .bind("stage_previous_fate", "population.stage_previous_fate")
            .bind("stage_fate_confidence", "population.stage_fate_confidence")
            .bind("stage_time_in_fate", "population.stage_time_in_fate")
            .bind(
                "stage_recent_activator",
                "population.stage_recent_activator",
            )
            .bind(
                "stage_recent_inhibitor",
                "population.stage_recent_inhibitor",
            )
            .bind(
                "stage_recent_surface_exposure",
                "population.stage_recent_surface_exposure",
            )
            .bind("stage_color", "population.stage_color")
            .dispatch_over("population.survival_flag"),
        )
        .pass(
            PassDraft::new("commit_population_core", "organism_commit_population_core")
                .bind("next_count", "population.next_count")
                .bind("stage_stable_id", "population.stage_stable_id")
                .bind("stage_parent_id", "population.stage_parent_id")
                .bind("stage_position", "population.stage_position")
                .bind("stage_radius", "population.stage_radius")
                .bind("stage_energy", "population.stage_energy")
                .bind("stable_id", "cells.stable_id")
                .bind("parent_id", "cells.parent_id")
                .bind("position", "cells.position")
                .bind("radius", "cells.radius")
                .bind("energy", "cells.energy")
                .bind("stage_event_hash", "population.stage_event_hash")
                .bind("event_hash", "cells.event_hash")
                .dispatch_over("population.survival_flag")
                .grant("mutate_cell_state"),
        )
        .pass(
            PassDraft::new(
                "commit_population_development",
                "organism_commit_population_development",
            )
            .bind("next_count", "population.next_count")
            .bind("stage_age", "population.stage_age")
            .bind("stage_fate", "population.stage_fate")
            .bind("stage_phase", "population.stage_phase")
            .bind("stage_health", "population.stage_health")
            .bind("stage_previous_fate", "population.stage_previous_fate")
            .bind("stage_fate_confidence", "population.stage_fate_confidence")
            .bind("stage_time_in_fate", "population.stage_time_in_fate")
            .bind(
                "stage_recent_activator",
                "population.stage_recent_activator",
            )
            .bind(
                "stage_recent_inhibitor",
                "population.stage_recent_inhibitor",
            )
            .bind(
                "stage_recent_surface_exposure",
                "population.stage_recent_surface_exposure",
            )
            .bind("stage_color", "population.stage_color")
            .bind("age", "cells.age")
            .bind("fate", "cells.fate")
            .bind("phase", "cells.phase")
            .bind("health", "cells.health")
            .bind("previous_fate", "cells.previous_fate")
            .bind("fate_confidence", "cells.fate_confidence")
            .bind("time_in_fate", "cells.time_in_fate")
            .bind("recent_activator", "cells.recent_activator")
            .bind("recent_inhibitor", "cells.recent_inhibitor")
            .bind("recent_surface_exposure", "cells.recent_surface_exposure")
            .bind("color", "cells.color")
            .dispatch_over("population.survival_flag")
            .grant("mutate_cell_state"),
        )
        .pass(
            PassDraft::new("finalize_population", "organism_finalize_population")
                .bind("active_count", "cells.active_count")
                .bind("next_stable_id", "cells.next_stable_id")
                .bind("next_count", "population.next_count")
                .bind("accepted_birth_count", "population.accepted_birth_count")
                .grant("mutate_cell_state")
                .grant("mutate_cell_membership"),
        )
        .pass(
            PassDraft::new("finalize_energy_ledger", "organism_finalize_energy_ledger")
                .bind("active_count", "cells.active_count")
                .bind("energy", "cells.energy")
                .bind("accepted_birth_count", "population.accepted_birth_count")
                .bind("previous_total", "ledger.previous_total")
                .bind("absorbed", "ledger.absorbed")
                .bind("maintenance", "ledger.maintenance")
                .bind("decisions", "ledger.decisions")
                .bind("motion", "ledger.motion")
                .bind("signaling", "ledger.signaling")
                .bind("division", "ledger.division")
                .bind(
                    "environmental_death_loss",
                    "ledger.environmental_death_loss",
                )
                .bind("current_total", "ledger.current_total")
                .bind("residual", "ledger.residual")
                .bind("cumulative_residual", "ledger.cumulative_residual"),
        )
        .pass(
            PassDraft::new(
                "measure_homeostasis_events",
                "organism_measure_homeostasis_events",
            )
            .bind("tick", "simulation.tick")
            .bind("component_count", "morphology.component_count")
            .bind("component_unresolved", "morphology.component_unresolved")
            .bind("organizer_count", "morphology.organizer_count")
            .bind("boundary_count", "morphology.boundary_count")
            .bind("interior_count", "morphology.interior_count")
            .bind("neighbor_overflow", "population.neighbor_overflow")
            .bind("physical_overflow", "population.physical_neighbor_overflow")
            .bind("perception_truncation", "population.perception_truncation")
            .bind("deposit_saturation", "deposit.saturation_count")
            .bind("current_energy", "ledger.current_total")
            .bind("perturbation_window", "homeostasis.perturbation_window")
            .bind("invariant_violations", "homeostasis.invariant_violations")
            .bind(
                "perturbation_energy_min",
                "homeostasis.perturbation_energy_min",
            ),
        )
        .pass(
            PassDraft::new("audit_homeostasis", "organism_audit_homeostasis")
                .bind("tick", "simulation.tick")
                .bind("population", "morphology.population")
                .bind("component_count", "morphology.component_count")
                .bind("component_unresolved", "morphology.component_unresolved")
                .bind("organizer_count", "morphology.organizer_count")
                .bind("boundary_count", "morphology.boundary_count")
                .bind("interior_count", "morphology.interior_count")
                .bind("area_q16", "morphology.area_q16")
                .bind("perimeter_q16", "morphology.perimeter_q16")
                .bind("compactness_q16", "morphology.compactness_q16")
                .bind("centroid_x_q16", "morphology.centroid_x_q16")
                .bind("centroid_y_q16", "morphology.centroid_y_q16")
                .bind("radial_density", "morphology.radial_density")
                .bind("current_energy", "ledger.current_total")
                .bind("neighbor_overflow", "population.neighbor_overflow")
                .bind("physical_overflow", "population.physical_neighbor_overflow")
                .bind("perception_truncation", "population.perception_truncation")
                .bind("deposit_saturation", "deposit.saturation_count")
                .bind("metric_min", "homeostasis.metric_min")
                .bind("metric_max", "homeostasis.metric_max")
                .bind("metric_sum", "homeostasis.metric_sum")
                .bind("metric_sum_sq", "homeostasis.metric_sum_sq")
                .bind("energy_min", "homeostasis.energy_min")
                .bind("energy_max", "homeostasis.energy_max")
                .bind("energy_sum", "homeostasis.energy_sum")
                .bind("energy_sum_sq", "homeostasis.energy_sum_sq")
                .bind("reference_samples", "homeostasis.reference_samples")
                .bind("validation_samples", "homeostasis.validation_samples")
                .bind("validation_violations", "homeostasis.validation_violations")
                .bind("window", "homeostasis.window"),
        )
        .pass(
            PassDraft::new("advance_tick", "organism_advance_tick")
                .bind("tick", "simulation.tick")
                .grant("advance_simulation_time"),
        )
        .view(
            ViewDraft::render(
                "organism",
                metal_implementation("shaders/particle.metal", "particle_pipeline"),
            )
            .read("color", "cells.color")
            .read("position", "cells.position")
            .read("radius", "cells.radius"),
        )
        .schedule(schedule)
        .contract(ContractDraft::new("organism_finite", "simulation").clause(
            ContractClauseDraft::Invariant {
                observation: ObservationDraft::AfterTickExecution("simulation".to_owned()),
                predicate: PredicateDraft::FiniteStreams(vec![
                    "cells.position".to_owned(),
                    "cells.energy".to_owned(),
                    "field.activator".to_owned(),
                    "field.inhibitor".to_owned(),
                ]),
            },
        ))
        .contract(ContractDraft::new("logical_replay", "simulation").clause(
            ContractClauseDraft::Determinism(DeterminismContract {
                tier: DeterminismTier::Tier1,
                scope: DeterminismScope::ExactExecutionFingerprint,
            }),
        ))
        .scenario(
            ScenarioDraft::new("reference_development", "simulation", 30_000).expect(
                ObservationDraft::AfterTickExecution("simulation".to_owned()),
                PredicateDraft::FiniteStreams(vec![
                    "cells.position".to_owned(),
                    "cells.energy".to_owned(),
                ]),
            ),
        )
        .scenario(
            ScenarioDraft::new("homeostasis_perturbation", "simulation", 30_000)
                .intervene(
                    12_000,
                    "set_nutrient_supply",
                    [("environment.reference_nutrient_supply", Literal::f32(0.25))],
                )
                .intervene(
                    14_000,
                    "set_nutrient_supply",
                    [("environment.reference_nutrient_supply", Literal::f32(1.0))],
                )
                .expect(
                    ObservationDraft::AfterTickExecution("simulation".to_owned()),
                    PredicateDraft::FiniteStreams(vec![
                        "cells.energy".to_owned(),
                        "ledger.current_total".to_owned(),
                        "ledger.residual".to_owned(),
                        "ledger.cumulative_residual".to_owned(),
                    ]),
                ),
        )
        .scenario(ScenarioDraft::new(
            "regeneration_control",
            "simulation",
            50_000,
        ))
        .scenario(
            ScenarioDraft::new("structural_regeneration", "simulation", 50_000)
                .intervene(
                    30_000,
                    "apply_lesion_cells",
                    [
                        ("lesion.reference_center_x_q16", Literal::I32(5_571)),
                        ("lesion.reference_center_y_q16", Literal::I32(0)),
                        ("lesion.reference_radius_q16", Literal::U32(3_932)),
                    ],
                )
                .intervene(
                    30_000,
                    "apply_lesion_field",
                    [
                        ("lesion.reference_center_x_q16", Literal::I32(5_571)),
                        ("lesion.reference_center_y_q16", Literal::I32(0)),
                        ("lesion.reference_radius_q16", Literal::U32(3_932)),
                        ("lesion.reference_injury", Literal::f32(4.0)),
                    ],
                ),
        )
        .scenario(
            ScenarioDraft::new("regeneration_without_injury", "simulation", 50_000)
                .intervene(
                    30_000,
                    "ablate_injury_transport",
                    [("environment.requested_injury_transport", Literal::f32(0.0))],
                )
                .intervene(
                    30_000,
                    "apply_lesion_cells",
                    [
                        ("lesion.reference_center_x_q16", Literal::I32(5_571)),
                        ("lesion.reference_center_y_q16", Literal::I32(0)),
                        ("lesion.reference_radius_q16", Literal::U32(3_932)),
                    ],
                )
                .intervene(
                    30_000,
                    "apply_lesion_field",
                    [
                        ("lesion.reference_center_x_q16", Literal::I32(5_571)),
                        ("lesion.reference_center_y_q16", Literal::I32(0)),
                        ("lesion.reference_radius_q16", Literal::U32(3_932)),
                        ("lesion.reference_injury", Literal::f32(4.0)),
                    ],
                ),
        )
        .scenario(
            ScenarioDraft::new("regeneration_without_repair", "simulation", 50_000)
                .intervene(
                    30_000,
                    "set_repair_enabled",
                    [("environment.requested_repair_enabled", Literal::U32(0))],
                )
                .intervene(
                    30_000,
                    "apply_lesion_cells",
                    [
                        ("lesion.reference_center_x_q16", Literal::I32(5_571)),
                        ("lesion.reference_center_y_q16", Literal::I32(0)),
                        ("lesion.reference_radius_q16", Literal::U32(3_932)),
                    ],
                )
                .intervene(
                    30_000,
                    "apply_lesion_field",
                    [
                        ("lesion.reference_center_x_q16", Literal::I32(5_571)),
                        ("lesion.reference_center_y_q16", Literal::I32(0)),
                        ("lesion.reference_radius_q16", Literal::U32(3_932)),
                        ("lesion.reference_injury", Literal::f32(4.0)),
                    ],
                ),
        )
        .capability(CapabilityDraft::state_mutate(
            "mutate_cell_state",
            committed
                .into_iter()
                .chain(std::iter::once("cells.next_stable_id")),
        ))
        .capability(CapabilityDraft::membership_mutate(
            "mutate_cell_membership",
            "cells.active_count",
            committed.into_iter().chain(transient).chain(ledger_cells),
        ))
        .capability(CapabilityDraft::state_mutate(
            "mutate_field_state",
            field_state,
        ))
        .capability(CapabilityDraft::state_mutate(
            "mutate_environment",
            [
                "environment.nutrient_supply",
                "environment.injury_transport",
                "environment.repair_enabled",
            ],
        ))
        .capability(CapabilityDraft::state_mutate(
            "advance_simulation_time",
            ["simulation.tick"],
        ))
}
