//! Hello Organism: the first coupled dynamic-population/field specimen.

use crate::*;

const ORGANISM_SOURCE: &str = include_str!("../../../kernels/organism.metal");
const FIELD_WIDTH: u32 = 256;
const FIELD_CELLS: u32 = FIELD_WIDTH * FIELD_WIDTH;

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
/// The optimized spatial-neighbor and prefix-allocation passes remain separate
/// milestones; this correctness specimen uses a serial authoritative membership
/// resolver while all per-cell and field work executes in parallel.
pub fn hello_organism_builder(capacity: u32) -> ModuleBuilder {
    assert!(capacity >= 2);
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
            "organism_resolve_population",
            vec![
                stream_slot(
                    "active_count",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot(
                    "next_stable_id",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot("stable_id", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("parent_id", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot_unit(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot_unit(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot("energy", f32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("age", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("fate", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("phase", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot("health", u32_type.clone(), SlotAccess::ReadWrite, true),
                stream_slot(
                    "previous_fate",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot(
                    "fate_confidence",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot(
                    "time_in_fate",
                    u32_type.clone(),
                    SlotAccess::ReadWrite,
                    true,
                ),
                stream_slot("color", vec4.clone(), SlotAccess::ReadWrite, true),
                stream_slot("divide_intent", u32_type.clone(), SlotAccess::Read, true),
                stream_slot("death_intent", u32_type.clone(), SlotAccess::Read, true),
                value_slot("capacity", u32_type.clone()),
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
            PassDraft::new("resolve_population", "organism_resolve_population")
                .bind("active_count", "cells.active_count")
                .bind("next_stable_id", "cells.next_stable_id")
                .bind("stable_id", "cells.stable_id")
                .bind("parent_id", "cells.parent_id")
                .bind("position", "cells.position")
                .bind("radius", "cells.radius")
                .bind("energy", "cells.energy")
                .bind("age", "cells.age")
                .bind("fate", "cells.fate")
                .bind("phase", "cells.phase")
                .bind("health", "cells.health")
                .bind("previous_fate", "cells.previous_fate")
                .bind("fate_confidence", "cells.fate_confidence")
                .bind("time_in_fate", "cells.time_in_fate")
                .bind("color", "cells.color")
                .bind("divide_intent", "intent.divide")
                .bind("death_intent", "intent.death")
                .bind("capacity", "organism.capacity")
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
        .schedule(
            ScheduleDraft::fixed("simulation", 120)
                .run("sample")
                .run_after("decide", "sample")
                .run_after("resolve_state", "decide")
                .run_after("clear_deposits", "resolve_state")
                .run_after("deposit", "clear_deposits")
                .run_after("diffuse", "deposit")
                .run_after("commit_fields", "diffuse")
                .run_after("resolve_population", "commit_fields")
                .show_after("organism", "resolve_population")
                .tick_overlap(TickOverlapPolicy::QueueOrderedReuse)
                .presentation_lifetime(PresentationLifetimePolicy::QueueOrderedReuse)
                .queue_model(QueueModel::SingleSerialQueue),
        )
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
