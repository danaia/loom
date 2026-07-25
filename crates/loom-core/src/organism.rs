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
/// Morphology reductions and sustained homeostasis remain later proof gates.
pub fn hello_organism_builder(capacity: u32) -> ModuleBuilder {
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
        "cells.color",
    ];
    let transient = [
        "perception.activator_bin",
        "perception.inhibitor_bin",
        "perception.nutrient_bin",
        "perception.density_bin",
        "perception.energy_bin",
        "intent.requested_fate",
        "intent.requested_phase",
        "intent.requested_health",
        "intent.divide",
        "intent.death",
        "intent.activator_deposit",
        "intent.inhibitor_deposit",
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
    ] {
        builder = builder.stream(
            StreamDraft::new(name, u32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .initial(Literal::Array(vec![Literal::U32(0)])),
        );
    }
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

    let mut schedule = ScheduleDraft::fixed("simulation", 120)
        .run("sample")
        .run_after("decide", "sample")
        .run_after("resolve_state", "decide")
        .run_after("clear_deposits", "resolve_state")
        .run_after("deposit", "clear_deposits")
        .run_after("diffuse", "deposit")
        .run_after("commit_fields", "diffuse")
        .run_after("initialize_population_order", "commit_fields");
    for (pass, predecessor) in radix_dependencies {
        schedule = schedule.run_after(pass, predecessor);
    }
    schedule = schedule
        .run_after("clear_population_bins", radix_predecessor)
        .run_after("bin_living", "clear_population_bins")
        .run_after("sort_living_bins", "bin_living")
        .run_after("prequalify_population", "sort_living_bins")
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
        .show_after("organism", "finalize_population")
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
                stream_slot("activator_bin", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("inhibitor_bin", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("nutrient_bin", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("density_bin", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("energy_bin", u32_type.clone(), SlotAccess::Write, false),
                value_slot("width", u32_type.clone()),
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
                ("activator_bin", SlotAccess::Read),
                ("inhibitor_bin", SlotAccess::Read),
                ("nutrient_bin", SlotAccess::Read),
                ("density_bin", SlotAccess::Read),
                ("energy_bin", SlotAccess::Read),
                ("requested_fate", SlotAccess::Write),
                ("requested_phase", SlotAccess::Write),
                ("requested_health", SlotAccess::Write),
                ("divide_intent", SlotAccess::Write),
                ("death_intent", SlotAccess::Write),
                ("activator_deposit", SlotAccess::Write),
                ("inhibitor_deposit", SlotAccess::Write),
            ]
            .into_iter()
            .map(|(name, access)| stream_slot(name, u32_type.clone(), access, false))
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
            ],
        ))
        .kernel(packaged_kernel(
            "organism_clear_deposits",
            ["activator", "inhibitor", "density"]
                .into_iter()
                .map(|name| stream_slot(name, u32_type.clone(), SlotAccess::Write, false))
                .collect(),
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
                stream_slot("activator", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("inhibitor", u32_type.clone(), SlotAccess::Atomic, true),
                stream_slot("density", u32_type.clone(), SlotAccess::Atomic, true),
                value_slot("width", u32_type.clone()),
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
                stream_slot("activator_next", f32_type.clone(), SlotAccess::Write, false),
                stream_slot("inhibitor_next", f32_type.clone(), SlotAccess::Write, false),
                stream_slot("nutrient_next", f32_type.clone(), SlotAccess::Write, false),
                stream_slot("density_next", f32_type.clone(), SlotAccess::Write, false),
                stream_slot("injury_next", f32_type.clone(), SlotAccess::Write, false),
                value_slot("width", u32_type.clone()),
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
                stream_slot("overflow", u32_type.clone(), SlotAccess::Write, true),
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
                stream_slot("divide", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("death", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("bin_count", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("bin_indices", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("survival", u32_type.clone(), SlotAccess::Write, false),
                stream_slot("birth", u32_type.clone(), SlotAccess::Write, false),
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
                stream_slot("stage_color", vec4.clone(), SlotAccess::Read, false),
                stream_slot("age", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("fate", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("phase", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("health", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("previous_fate", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("fate_confidence", u32_type.clone(), SlotAccess::Write, true),
                stream_slot("time_in_fate", u32_type.clone(), SlotAccess::Write, true),
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
            PassDraft::new("sample", "organism_sample")
                .bind("position", "cells.position")
                .bind("energy", "cells.energy")
                .bind("activator", "field.activator")
                .bind("inhibitor", "field.inhibitor")
                .bind("nutrient", "field.nutrient")
                .bind("density", "field.density")
                .bind("activator_bin", "perception.activator_bin")
                .bind("inhibitor_bin", "perception.inhibitor_bin")
                .bind("nutrient_bin", "perception.nutrient_bin")
                .bind("density_bin", "perception.density_bin")
                .bind("energy_bin", "perception.energy_bin")
                .bind("width", "field.width")
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
                .bind("activator_bin", "perception.activator_bin")
                .bind("inhibitor_bin", "perception.inhibitor_bin")
                .bind("nutrient_bin", "perception.nutrient_bin")
                .bind("density_bin", "perception.density_bin")
                .bind("energy_bin", "perception.energy_bin")
                .bind("requested_fate", "intent.requested_fate")
                .bind("requested_phase", "intent.requested_phase")
                .bind("requested_health", "intent.requested_health")
                .bind("divide_intent", "intent.divide")
                .bind("death_intent", "intent.death")
                .bind("activator_deposit", "intent.activator_deposit")
                .bind("inhibitor_deposit", "intent.inhibitor_deposit")
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
                .bind("age", "cells.age")
                .bind("energy", "cells.energy")
                .bind("color", "cells.color")
                .bind("requested_fate", "intent.requested_fate")
                .bind("requested_phase", "intent.requested_phase")
                .bind("requested_health", "intent.requested_health")
                .bind("nutrient_bin", "perception.nutrient_bin")
                .bind("activator_deposit", "intent.activator_deposit")
                .bind("inhibitor_deposit", "intent.inhibitor_deposit")
                .dispatch_over("cells.stable_id")
                .grant("mutate_cell_state"),
        )
        .pass(
            PassDraft::new("clear_deposits", "organism_clear_deposits")
                .bind("activator", "deposit.activator_q16")
                .bind("inhibitor", "deposit.inhibitor_q16")
                .bind("density", "deposit.density_q16")
                .dispatch_over("deposit.activator_q16"),
        )
        .pass(
            PassDraft::new("deposit", "organism_deposit")
                .bind("position", "cells.position")
                .bind("activator_amount", "intent.activator_deposit")
                .bind("inhibitor_amount", "intent.inhibitor_deposit")
                .bind("activator", "deposit.activator_q16")
                .bind("inhibitor", "deposit.inhibitor_q16")
                .bind("density", "deposit.density_q16")
                .bind("width", "field.width")
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
                .bind("activator_next", "field.activator_next")
                .bind("inhibitor_next", "field.inhibitor_next")
                .bind("nutrient_next", "field.nutrient_next")
                .bind("density_next", "field.density_next")
                .bind("injury_next", "field.injury_next")
                .bind("width", "field.width")
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
                .bind("overflow", "population.neighbor_overflow")
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
                .bind("divide", "intent.divide")
                .bind("death", "intent.death")
                .bind("bin_count", "spatial.living_bin_count")
                .bind("bin_indices", "spatial.living_bin_indices")
                .bind("survival", "population.survival_flag")
                .bind("birth", "population.birth_prequalified")
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
            .bind("color", "cells.color")
            .bind("survival", "population.survival_flag")
            .bind("birth", "population.birth_flag")
            .bind("survival_prefix", "population.survival_prefix")
            .bind("birth_prefix", "population.birth_prefix")
            .bind("survivor_count", "population.survivor_count")
            .bind("accepted_birth_count", "population.accepted_birth_count")
            .bind("stage_age", "population.stage_age")
            .bind("stage_fate", "population.stage_fate")
            .bind("stage_phase", "population.stage_phase")
            .bind("stage_health", "population.stage_health")
            .bind("stage_previous_fate", "population.stage_previous_fate")
            .bind("stage_fate_confidence", "population.stage_fate_confidence")
            .bind("stage_time_in_fate", "population.stage_time_in_fate")
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
            .bind("stage_color", "population.stage_color")
            .bind("age", "cells.age")
            .bind("fate", "cells.fate")
            .bind("phase", "cells.phase")
            .bind("health", "cells.health")
            .bind("previous_fate", "cells.previous_fate")
            .bind("fate_confidence", "cells.fate_confidence")
            .bind("time_in_fate", "cells.time_in_fate")
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
        .capability(CapabilityDraft::state_mutate(
            "mutate_cell_state",
            committed
                .into_iter()
                .chain(std::iter::once("cells.next_stable_id")),
        ))
        .capability(CapabilityDraft::membership_mutate(
            "mutate_cell_membership",
            "cells.active_count",
            committed.into_iter().chain(transient),
        ))
        .capability(CapabilityDraft::state_mutate(
            "mutate_field_state",
            field_state,
        ))
}
