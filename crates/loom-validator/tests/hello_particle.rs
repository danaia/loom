use loom_core::{
    DataType, DependencySemantics, DiagnosticCode, GraphEdit, Literal, ModuleBuilder, QueueModel,
    ResourceId, SnapshotSemantics, StreamDraft, TickOverlapPolicy, Unit, ValueDraft, ValueKind,
    ViewState, canonicalize,
    conformance::{HelloParticleConfig, hello_particle_builder},
};
use loom_validator::{ConcurrencyBasis, Validator};

#[test]
fn rejects_single_buffer_with_four_unproven_overlapping_ticks() {
    let graph = hello_particle_builder(HelloParticleConfig::unsafe_unproven_overlap())
        .build()
        .expect("the graph should resolve before execution validation");
    let report = Validator::validate(&graph);

    assert!(!report.is_valid());
    let conflicts = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::InsufficientBufferVersions)
        .collect::<Vec<_>>();
    assert_eq!(conflicts.len(), 2, "position and velocity both mutate");
    assert!(conflicts.iter().all(|diagnostic| matches!(
        diagnostic.suggested_fix,
        Some(GraphEdit::SetStreamBuffering { versions: 4, .. })
    )));

    let concurrency = &report.effective_concurrency[0];
    assert_eq!(concurrency.requested_ticks, 4);
    assert_eq!(concurrency.effective_ticks, 0);
    assert_eq!(concurrency.basis, ConcurrencyBasis::Invalid);
}

#[test]
fn accepts_four_versions_for_four_overlapping_ticks() {
    let graph = hello_particle_builder(HelloParticleConfig {
        particle_buffering: 4,
        tick_overlap: TickOverlapPolicy::RequireResourceVersions,
        ..HelloParticleConfig::default()
    })
    .build()
    .unwrap();
    let report = Validator::validate(&graph);

    assert_diagnostics_empty(&report);
    assert_eq!(report.effective_concurrency[0].effective_ticks, 4);
    assert_eq!(
        report.effective_concurrency[0].basis,
        ConcurrencyBasis::ResourceVersions
    );
}

#[test]
fn explicit_serialization_reduces_effective_concurrency_to_one() {
    let graph = hello_particle_builder(HelloParticleConfig {
        ..HelloParticleConfig::default()
    })
    .build()
    .unwrap();
    let report = Validator::validate(&graph);

    assert_diagnostics_empty(&report);
    assert_eq!(report.effective_concurrency[0].requested_ticks, 4);
    assert_eq!(report.effective_concurrency[0].effective_ticks, 1);
    assert_eq!(
        report.effective_concurrency[0].basis,
        ConcurrencyBasis::SerializedConflicts
    );
}

#[test]
fn serial_queue_completion_proof_allows_single_buffer_reuse() {
    let graph = hello_particle_builder(HelloParticleConfig {
        tick_overlap: TickOverlapPolicy::QueueOrderedReuse,
        queue_model: QueueModel::SingleSerialQueue,
        ..HelloParticleConfig::default()
    })
    .build()
    .unwrap();
    let report = Validator::validate(&graph);

    assert_diagnostics_empty(&report);
    assert_eq!(report.effective_concurrency[0].effective_ticks, 4);
    assert_eq!(
        report.effective_concurrency[0].basis,
        ConcurrencyBasis::ProvenSerialQueue
    );
}

#[test]
fn queue_reuse_without_a_proof_is_rejected_with_a_mechanical_fix() {
    let graph = hello_particle_builder(HelloParticleConfig {
        tick_overlap: TickOverlapPolicy::QueueOrderedReuse,
        queue_model: QueueModel::Unproven,
        ..HelloParticleConfig::default()
    })
    .build()
    .unwrap();
    let report = Validator::validate(&graph);

    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::UnprovenQueueReuse)
        .expect("queue proof diagnostic");
    assert!(matches!(
        diagnostic.suggested_fix,
        Some(GraphEdit::SetTickOverlapPolicy {
            policy: TickOverlapPolicy::SerializeConflictingTicks,
            ..
        })
    ));
}

#[test]
fn topological_order_is_fall_then_bounce_then_viewport() {
    let graph = hello_particle_builder(HelloParticleConfig {
        particle_buffering: 4,
        ..HelloParticleConfig::default()
    })
    .build()
    .unwrap();
    let report = Validator::validate(&graph);
    assert_diagnostics_empty(&report);

    let labels = report.topological_orders[0]
        .items
        .iter()
        .map(|item| match item {
            loom_core::ScheduleItemId::Pass(id) => graph.pass(*id).name.as_str(),
            loom_core::ScheduleItemId::View(id) => graph.view(*id).name.as_str(),
        })
        .collect::<Vec<_>>();
    assert_eq!(labels, ["fall", "bounce", "viewport"]);

    let schedule = &graph.schedules[0];
    assert!(
        schedule
            .execution_dependencies
            .iter()
            .all(|edge| edge.semantics == DependencySemantics::Completion)
    );
    assert!(
        schedule
            .presentation_dependencies
            .iter()
            .all(|edge| edge.semantics == DependencySemantics::Completion)
    );
}

#[test]
fn fixed_dt_is_a_typed_builtin_resource_and_is_explicitly_bound() {
    let graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    let fixed_dt = graph
        .resources
        .values
        .iter()
        .find(|value| value.name == "simulation.fixed_dt")
        .expect("fixed dt value");
    assert!(matches!(fixed_dt.kind, ValueKind::ScheduleFixedDt { .. }));
    assert_eq!(fixed_dt.unit, loom_core::Unit::SECOND);

    let fall = graph
        .passes
        .iter()
        .find(|pass| pass.name == "fall")
        .unwrap();
    let kernel = graph.kernel(fall.kernel);
    let dt_slot = kernel.slots.iter().find(|slot| slot.name == "dt").unwrap();
    let binding = fall
        .bindings
        .iter()
        .find(|binding| binding.slot == dt_slot.id)
        .unwrap();
    assert_eq!(binding.resource, ResourceId::Value(fixed_dt.id));
}

#[test]
fn inspection_returns_the_next_gpu_completed_tick_after_request() {
    let graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    let capability = graph
        .capabilities
        .iter()
        .find(|capability| capability.name == "inspect_particle_state")
        .unwrap();
    let loom_core::CapabilityKind::Inspect { snapshot, .. } = &capability.kind else {
        panic!("expected inspection capability");
    };
    assert_eq!(
        *snapshot,
        SnapshotSemantics::NextGpuCompletedTickAfterRequest
    );
}

#[test]
fn normalized_graph_fingerprint_is_stable() {
    let config = HelloParticleConfig {
        particle_buffering: 4,
        tick_overlap: TickOverlapPolicy::RequireResourceVersions,
        ..HelloParticleConfig::default()
    };
    let first = hello_particle_builder(config.clone()).build().unwrap();
    let second = hello_particle_builder(config).build().unwrap();
    let first_report = Validator::validate(&first);
    let second_report = Validator::validate(&second);

    assert_diagnostics_empty(&first_report);
    assert_eq!(first_report.canonical.bytes, second_report.canonical.bytes);
    assert_eq!(
        first_report.canonical.fingerprint,
        second_report.canonical.fingerprint
    );
    assert_eq!(first_report.canonical.fingerprint.len(), 64);
}

#[test]
fn declaration_insertion_order_does_not_change_typed_ids_or_fingerprint() {
    let value_a =
        ValueDraft::constant("a", DataType::f32(), Unit::DIMENSIONLESS, Literal::f32(1.0));
    let value_b =
        ValueDraft::constant("b", DataType::f32(), Unit::DIMENSIONLESS, Literal::f32(2.0));
    let stream_c = StreamDraft::new("c", DataType::f32(), Unit::DIMENSIONLESS);
    let stream_d = StreamDraft::new("d", DataType::f32(), Unit::DIMENSIONLESS);

    let first = ModuleBuilder::new("ordered")
        .value(value_a.clone())
        .value(value_b.clone())
        .stream(stream_c.clone())
        .stream(stream_d.clone())
        .build()
        .unwrap();
    let second = ModuleBuilder::new("ordered")
        .stream(stream_d)
        .value(value_b)
        .stream(stream_c)
        .value(value_a)
        .build()
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(
        canonicalize(&first).fingerprint,
        canonicalize(&second).fingerprint
    );
}

#[test]
fn historical_view_requires_enough_buffer_versions_for_its_lag() {
    let mut graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    graph.views[0].state = ViewState::PreviousStableTick { lag: 1 };

    let report = Validator::validate(&graph);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InsufficientBufferVersions)
    );
}

#[test]
fn unit_mismatch_is_rejected_without_rebuilding_text() {
    let mut graph = hello_particle_builder(HelloParticleConfig {
        particle_buffering: 4,
        tick_overlap: TickOverlapPolicy::RequireResourceVersions,
        ..HelloParticleConfig::default()
    })
    .build()
    .unwrap();
    let gravity = graph
        .resources
        .values
        .iter_mut()
        .find(|value| value.name == "world.gravity")
        .unwrap();
    gravity.unit = loom_core::Unit::METER;

    let report = Validator::validate(&graph);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UnitMismatch)
    );
}

fn assert_diagnostics_empty(report: &loom_validator::ValidationReport) {
    assert!(
        report.is_valid(),
        "unexpected diagnostics:\n{}",
        serde_json::to_string_pretty(&report.diagnostics).unwrap()
    );
}
