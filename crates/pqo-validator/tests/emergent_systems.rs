use pqo_core::{
    CapabilityDraft, DataType, DiagnosticCode, KernelAbiDraft, KernelDraft, Literal, ModuleBuilder,
    PassDraft, ScenarioDraft, ScheduleDraft, SlotAccess, SlotDraft, StreamDraft, Unit, ValueDraft,
    packaged_metal_implementation,
};
use pqo_validator::Validator;

const UPDATE_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;
kernel void update_state(device uint* state [[buffer(0)]],
                         uint gid [[thread_position_in_grid]]) {
    state[gid] += 1;
}
"#;

fn dynamic_population_builder(grant_authority: bool) -> ModuleBuilder {
    let pass = PassDraft::new("update", "update_state")
        .bind("state", "cells.state")
        .dispatch_over("cells.state");
    let pass = if grant_authority {
        pass.grant("mutate_cells")
    } else {
        pass
    };

    ModuleBuilder::new("dynamic_population")
        .stream(
            StreamDraft::new("cells.active_count", DataType::u32(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .write_authority("mutate_cells")
                .initial(Literal::Array(vec![Literal::U32(1)])),
        )
        .stream(
            StreamDraft::new("cells.state", DataType::u32(), Unit::DIMENSIONLESS)
                .capacity(1024)
                .dynamic_length("cells.active_count")
                .write_authority("mutate_cells")
                .initial_repeat(Literal::U32(0), 1),
        )
        .kernel(
            KernelDraft::new("update_state")
                .slot(SlotDraft::stream(
                    "state",
                    DataType::u32(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::ReadWrite,
                ))
                .abi(KernelAbiDraft::new(["state"]))
                .implementation(packaged_metal_implementation(
                    "tests/update_state.metal",
                    "update_state",
                    UPDATE_SOURCE,
                )),
        )
        .pass(pass)
        .schedule(ScheduleDraft::fixed("simulation", 120).run("update"))
        .capability(CapabilityDraft::membership_mutate(
            "mutate_cells",
            "cells.active_count",
            ["cells.state"],
        ))
}

#[test]
fn dynamic_population_uses_a_mutable_count_stream_and_explicit_authority() {
    let graph = dynamic_population_builder(true).build().unwrap();
    let report = Validator::validate(&graph);

    assert!(
        report.is_valid(),
        "unexpected diagnostics: {:#?}",
        report.diagnostics
    );
    assert_eq!(graph.schema_version, 4);
    assert!(matches!(
        graph.resources.streams[1].length,
        pqo_core::StreamLength::Dynamic(_)
    ));
    assert_eq!(graph.passes[0].capabilities.len(), 1);
}

#[test]
fn protected_stream_rejects_an_unprivileged_writer() {
    let graph = dynamic_population_builder(false).build().unwrap();
    let report = Validator::validate(&graph);

    assert!(!report.is_valid());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == DiagnosticCode::MissingWriteAuthority })
    );
}

#[test]
fn membership_capability_rejects_a_non_count_stream() {
    let mut graph = dynamic_population_builder(true).build().unwrap();
    let capability = &mut graph.capabilities[0];
    let pqo_core::CapabilityKind::MembershipMutate { count, .. } = &mut capability.kind else {
        panic!("expected membership capability");
    };
    *count = graph.resources.streams[1].id;

    let report = Validator::validate(&graph);
    assert!(!report.is_valid());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == DiagnosticCode::InvalidMembershipAuthority })
    );
}

#[test]
fn field_specimen_declares_whole_resource_reach_and_explicit_commit_order() {
    let graph = pqo_core::conformance::hello_field_builder()
        .build()
        .unwrap();
    let report = Validator::validate(&graph);
    assert!(
        report.is_valid(),
        "unexpected diagnostics: {:#?}",
        report.diagnostics
    );

    let validated = report.validated.unwrap();
    let schedule = &validated.execution_plan().schedules[0];
    let ordered_names = schedule
        .order
        .iter()
        .filter_map(|item| match item {
            pqo_core::ScheduleItemId::Pass(id) => Some(&graph.pass(*id).unwrap().name),
            pqo_core::ScheduleItemId::View(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ordered_names,
        vec!["clear_deposits", "seed", "diffuse", "commit"]
    );
}

#[test]
fn organism_specimen_separates_decision_state_field_and_membership_authority() {
    let graph = pqo_core::hello_organism_builder(16_384).build().unwrap();
    let report = Validator::validate(&graph);
    assert!(
        report.is_valid(),
        "unexpected diagnostics: {:#?}",
        report.diagnostics
    );

    let decide = graph
        .passes
        .iter()
        .find(|pass| pass.name == "decide")
        .unwrap();
    assert!(decide.capabilities.is_empty());
    let resolve_state = graph
        .passes
        .iter()
        .find(|pass| pass.name == "resolve_state")
        .unwrap();
    assert_eq!(resolve_state.capabilities.len(), 1);
    let commit_population = graph
        .passes
        .iter()
        .find(|pass| pass.name == "commit_population_core")
        .unwrap();
    assert_eq!(commit_population.capabilities.len(), 1);
    let finalize_population = graph
        .passes
        .iter()
        .find(|pass| pass.name == "finalize_population")
        .unwrap();
    assert_eq!(finalize_population.capabilities.len(), 2);
    let scan = report
        .validated
        .as_ref()
        .unwrap()
        .execution_plan()
        .schedules
        .iter()
        .flat_map(|schedule| &schedule.passes)
        .find(|pass| graph.pass(pass.pass).unwrap().name == "scan_population_blocks")
        .unwrap();
    assert_eq!(scan.threads_per_threadgroup, Some(256));
    let schedule = report
        .validated
        .as_ref()
        .unwrap()
        .execution_plan()
        .schedules
        .first()
        .unwrap();
    let planned_names = schedule
        .order
        .iter()
        .filter_map(|item| match item {
            pqo_core::ScheduleItemId::Pass(pass) => Some(graph.pass(*pass).unwrap().name.as_str()),
            pqo_core::ScheduleItemId::View(_) => None,
        })
        .collect::<Vec<_>>();
    let position = |name: &str| {
        planned_names
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap()
    };
    assert!(position("observe_neighbors") < position("decide"));
    assert!(position("relax_components_63") < position("reduce_morphology"));
    for metric in [
        "morphology.component_count",
        "morphology.boundary_count",
        "morphology.interior_count",
        "morphology.compactness_q16",
        "morphology.radial_density",
    ] {
        assert!(
            graph
                .resources
                .streams
                .iter()
                .any(|stream| stream.name == metric)
        );
    }
}

#[test]
fn crystal_specimen_keeps_damage_interactive_and_orders_components_metrics_and_surface() {
    let graph = pqo_core::hello_crystal_builder(32 * 32 * 32)
        .build()
        .unwrap();
    let report = Validator::validate(&graph);
    assert!(
        report.is_valid(),
        "unexpected diagnostics: {:#?}",
        report.diagnostics
    );

    for resource in [
        "field.phase",
        "field.solute",
        "field.temperature",
        "material.damage",
        "material.component",
        "material.position",
        "interaction.slice_count",
        "interaction.camera_yaw",
        "interaction.camera_pitch",
        "interaction.camera_zoom",
        "interaction.pick_hit",
        "render.normal",
        "metrics.snapshot",
    ] {
        assert!(
            graph
                .resources
                .streams
                .iter()
                .any(|stream| stream.name == resource),
            "missing crystal resource {resource}"
        );
    }

    let validated = report.validated.unwrap();
    let schedule = &validated.execution_plan().schedules[0];
    let names = schedule
        .order
        .iter()
        .filter_map(|item| match item {
            pqo_core::ScheduleItemId::Pass(id) => Some(graph.pass(*id).unwrap().name.as_str()),
            pqo_core::ScheduleItemId::View(_) => None,
        })
        .collect::<Vec<_>>();
    let at = |name: &str| {
        names
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap()
    };
    assert!(at("evolve_growth_fields") < at("initialize_solid_components"));
    assert!(!names.contains(&"slice_material"));
    assert!(!names.contains(&"orbit_camera"));
    assert!(!names.contains(&"zoom_camera"));
    assert!(names.contains(&"self_heal_material"));
    assert!(!names.contains(&"clear_pointer_pick"));
    assert!(!names.contains(&"pick_crystal"));
    let interventions = validated
        .execution_plan()
        .intervention_passes
        .iter()
        .map(|pass| graph.pass(pass.pass).unwrap().name.as_str())
        .collect::<Vec<_>>();
    for name in [
        "slice_material",
        "orbit_camera",
        "zoom_camera",
        "clear_pointer_pick",
        "pick_crystal",
    ] {
        assert!(
            interventions.contains(&name),
            "missing interactive crystal intervention {name}"
        );
    }
    assert!(at("relax_solid_components_7") < at("integrate_fragments"));
    assert!(at("integrate_fragments") < at("self_heal_material"));
    assert!(at("self_heal_material") < at("extract_crystal_surface"));
    assert!(at("extract_crystal_surface") < at("reduce_crystal_metrics"));
}

#[test]
#[should_panic(expected = "perfect cube")]
fn crystal_rejects_a_non_cubic_dense_field() {
    let _ = pqo_core::hello_crystal_builder(1_000_001);
}

#[test]
fn invalid_threadgroup_width_is_rejected_before_runtime_lowering() {
    let mut graph = dynamic_population_builder(true).build().unwrap();
    graph.passes.nodes[0].threads_per_threadgroup = Some(0);
    let report = Validator::validate(&graph);
    assert!(report.validated.is_none());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == pqo_core::DiagnosticCode::InvalidDispatch })
    );
}

#[test]
fn scenario_interventions_are_tick_addressed_and_canonical() {
    let graph = dynamic_population_builder(true)
        .value(ValueDraft::constant(
            "lesion.radius",
            DataType::u32(),
            Unit::DIMENSIONLESS,
            Literal::U32(1),
        ))
        .pass(
            PassDraft::new("lesion", "update_state")
                .bind("state", "cells.state")
                .dispatch_over("cells.state")
                .grant("mutate_cells"),
        )
        .scenario(ScenarioDraft::new("injury", "simulation", 100).intervene(
            10,
            "lesion",
            [("lesion.radius", Literal::U32(4))],
        ))
        .build()
        .unwrap();
    let report = Validator::validate(&graph);
    assert!(
        report.is_valid(),
        "unexpected diagnostics: {:#?}",
        report.diagnostics
    );
    assert_eq!(graph.scenarios[0].interventions[0].tick, 10);

    let mut invalid = graph;
    invalid.scenarios[0].interventions[0].tick = 100;
    let report = Validator::validate(&invalid);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == DiagnosticCode::InvalidIntervention })
    );
}
