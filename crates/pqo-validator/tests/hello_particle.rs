use pqo_core::{
    CapabilityKind, ContractClause, DataType, DependencySemantics, DiagnosticCode, GraphEdit,
    KernelId, Literal, ModuleBuilder, PresentationLifetimePolicy, QueueModel, ResourceId,
    SnapshotSemantics, StreamDraft, StreamInitializer, TickOverlapPolicy, Unit, ValueDraft,
    ValueKind, ViewState, canonicalize,
    conformance::{HelloParticleConfig, hello_batch_builder, hello_particle_builder},
};
use pqo_validator::{
    ConcurrencyBasis, PresentationConcurrencyBasis, RepairError, RepairPlan, ValidationPass,
    Validator,
};

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
        .filter(|diagnostic| {
            matches!(
                diagnostic.code,
                DiagnosticCode::InsufficientBufferVersions
                    | DiagnosticCode::UnsafePresentationLifetime
            )
        })
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
            pqo_core::ScheduleItemId::Pass(id) => graph.pass(*id).unwrap().name.as_str(),
            pqo_core::ScheduleItemId::View(id) => graph.view(*id).unwrap().name.as_str(),
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
fn hello_batch_uses_the_same_language_path_at_one_thousand_particles() {
    let graph = hello_batch_builder(1_000).build().unwrap();
    let report = Validator::validate(&graph);

    assert_diagnostics_empty(&report);
    assert_eq!(graph.name, "hello_batch");
    assert!(graph.resources.streams.iter().all(|stream| {
        stream.capacity == 1_000 && stream.length == pqo_core::StreamLength::Fixed(1_000)
    }));
    let schedule = &report
        .validated
        .as_ref()
        .unwrap()
        .execution_plan()
        .schedules[0];
    assert!(schedule.passes.iter().all(|pass| {
        matches!(
            pass.dispatch,
            pqo_core::DispatchDomain::OverStream(stream)
                if graph.stream(stream).unwrap().capacity == 1_000
        )
    }));
    assert_eq!(schedule.requested_ticks, 4);
    assert_eq!(schedule.effective_ticks, 4);
    assert!(
        schedule
            .completion_requirements
            .iter()
            .filter_map(|requirement| match requirement {
                pqo_validator::CompletionRequirement::BeforeNextTick { enforcement, .. } =>
                    Some(enforcement),
                pqo_validator::CompletionRequirement::WithinTick { .. } => None,
            })
            .all(|enforcement| {
                *enforcement == pqo_validator::CompletionEnforcement::SerialQueueOrder
            })
    );
}

#[test]
fn million_particle_initial_state_is_compact_and_validated() {
    let graph = hello_batch_builder(1_000_000).build().unwrap();
    let report = Validator::validate(&graph);

    assert_diagnostics_empty(&report);
    assert!(
        serde_json::to_vec(&graph).unwrap().len() < 100_000,
        "compact initializers must keep the semantic graph independent of particle count"
    );
    assert!(graph.resources.streams.iter().all(|stream| {
        !matches!(
            stream.initial,
            Some(StreamInitializer::Explicit(Literal::Array(_)))
        )
    }));
}

#[test]
fn invalid_compact_initializer_parameters_are_diagnosed() {
    let mut graph = hello_batch_builder(1_000).build().unwrap();
    let position = graph
        .resources
        .streams
        .iter_mut()
        .find(|stream| stream.name == "particles.position")
        .unwrap();
    let Some(StreamInitializer::Grid2D { columns, .. }) = &mut position.initial else {
        panic!("Hello Batch position should use a grid initializer");
    };
    *columns = 0;

    let report = Validator::validate(&graph);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidInitialData
            && diagnostic.primary
                == pqo_core::SemanticPath::new("streams.particles.position.initial")
    }));
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
    assert_eq!(fixed_dt.unit, pqo_core::Unit::SECOND);

    let fall = graph
        .passes
        .iter()
        .find(|pass| pass.name == "fall")
        .unwrap();
    let kernel = graph.kernel(fall.kernel).unwrap();
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
    let pqo_core::CapabilityKind::Inspect { snapshot, .. } = &capability.kind else {
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
    assert_eq!(
        first_report.source_graph.bytes,
        second_report.source_graph.bytes
    );
    assert_eq!(
        first_report.artifact_fingerprint(),
        second_report.artifact_fingerprint()
    );
    assert_eq!(first_report.artifact_fingerprint().unwrap().len(), 64);
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
fn reordered_semantic_sets_have_the_same_canonical_fingerprint() {
    let graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    let mut reordered = graph.clone();
    reordered.schedules.nodes[0]
        .execution_dependencies
        .reverse();
    reordered.passes.nodes[0].bindings.reverse();
    reordered.views[0].reads.reverse();

    assert_ne!(
        serde_json::to_vec(&graph).unwrap(),
        serde_json::to_vec(&reordered).unwrap()
    );
    assert_eq!(
        canonicalize(&graph).fingerprint,
        canonicalize(&reordered).fingerprint
    );
    let first = Validator::validate(&graph);
    let second = Validator::validate(&reordered);
    assert_diagnostics_empty(&first);
    assert_diagnostics_empty(&second);
    assert_eq!(first.artifact_fingerprint(), second.artifact_fingerprint());
}

#[test]
fn reordered_declarations_are_rejected_at_the_structural_boundary() {
    let mut graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    graph.resources.values.swap(0, 1);
    graph.resources.values[0].id = pqo_core::ValueId(0);
    graph.resources.values[1].id = pqo_core::ValueId(1);

    let report = Validator::validate(&graph);

    assert!(!report.is_valid());
    assert_eq!(
        report.completed_passes,
        [ValidationPass::StructuralReferences]
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::NonCanonicalOrder)
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
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UnsafePresentationLifetime)
    );
}

#[test]
fn historical_lag_and_render_concurrency_share_one_live_range() {
    let mut graph = hello_particle_builder(HelloParticleConfig {
        particle_buffering: 2,
        presentation_lifetime: PresentationLifetimePolicy::RequireResourceVersions,
        ..HelloParticleConfig::default()
    })
    .build()
    .unwrap();
    graph.views[0].state = ViewState::PreviousStableTick { lag: 1 };

    let report = Validator::validate(&graph);
    let position = graph
        .resources
        .streams
        .iter()
        .find(|stream| stream.name == "particles.position")
        .unwrap();
    let allocation = report.effective_concurrency[0]
        .resource_versions
        .iter()
        .find(|allocation| allocation.stream == position.id)
        .unwrap();

    assert_eq!(allocation.simulation_live_versions, 1);
    assert_eq!(allocation.presentation_live_versions, 3);
    assert_eq!(allocation.required_versions, 3);
    assert_eq!(allocation.allocated_versions, 2);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UnsafePresentationLifetime)
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
    gravity.unit = pqo_core::Unit::METER;

    let report = Validator::validate(&graph);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UnitMismatch)
    );
}

#[test]
fn malformed_references_stop_before_semantic_validation_without_panicking() {
    let mut graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    graph.passes.nodes[0].kernel = KernelId(99_999);

    assert!(graph.kernel(KernelId(99_999)).is_none());
    let result = std::panic::catch_unwind(|| Validator::validate(&graph));
    let report = result.expect("untrusted graph validation must not panic");

    assert!(!report.is_valid());
    assert!(report.validated.is_none());
    assert_eq!(
        report.completed_passes,
        [ValidationPass::StructuralReferences]
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidReference)
    );
}

#[test]
fn unsafe_presentation_reuse_is_rejected_independently_of_tick_serialization() {
    let graph = hello_particle_builder(HelloParticleConfig {
        presentation_lifetime: PresentationLifetimePolicy::RequireResourceVersions,
        ..HelloParticleConfig::default()
    })
    .build()
    .unwrap();
    let report = Validator::validate(&graph);

    assert!(!report.is_valid());
    assert!(report.artifact_fingerprint().is_none());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UnsafePresentationLifetime)
    );
    assert_eq!(
        report.effective_concurrency[0].presentation_basis,
        PresentationConcurrencyBasis::Invalid
    );
}

#[test]
fn default_presentation_policy_blocks_reuse_until_the_view_finishes() {
    let graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    let report = Validator::validate(&graph);

    assert_diagnostics_empty(&report);
    assert_eq!(
        report.effective_concurrency[0].presentation_basis,
        PresentationConcurrencyBasis::BlockUntilPresentationCompletes
    );
    assert_eq!(report.effective_concurrency[0].effective_render_frames, 1);
}

#[test]
fn invalid_literals_and_stream_initial_data_are_diagnosed() {
    let mut graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    let gravity = graph
        .resources
        .values
        .iter_mut()
        .find(|value| value.name == "world.gravity")
        .unwrap();
    gravity.kind = ValueKind::Constant(Literal::Vector(vec![
        Literal::f32(0.0),
        Literal::f32(-9.81),
    ]));
    let position = graph
        .resources
        .streams
        .iter_mut()
        .find(|stream| stream.name == "particles.position")
        .unwrap();
    position.initial = Some(StreamInitializer::Explicit(Literal::Array(vec![
        Literal::f32(1.0),
    ])));

    let report = Validator::validate(&graph);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidLiteral)
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidInitialData)
    );
}

#[test]
fn zero_rate_incomplete_view_and_bad_metric_units_are_rejected() {
    let mut graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    let pqo_core::ScheduleTiming::Fixed { rate_hz, .. } = &mut graph.schedules.nodes[0].timing;
    *rate_hz = 0;
    graph.views[0].implementations[0].entry.clear();
    graph.views[0].implementations[0].entry_points.clear();
    let realtime = graph
        .contracts
        .iter_mut()
        .find(|contract| contract.name == "realtime")
        .unwrap();
    let maximum = realtime
        .clauses
        .iter_mut()
        .find_map(|clause| match clause {
            ContractClause::MetricLimit { maximum, .. } => Some(maximum),
            _ => None,
        })
        .unwrap();
    maximum.unit = Unit::METER;

    let report = Validator::validate(&graph);
    for code in [
        DiagnosticCode::InvalidOverloadPolicy,
        DiagnosticCode::MissingBackendImplementation,
        DiagnosticCode::InvalidMetricUnit,
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code),
            "missing diagnostic {code:?}"
        );
    }
}

#[test]
fn capability_authority_is_validated() {
    let mut graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    let position = graph
        .resources
        .streams
        .iter()
        .find(|stream| stream.name == "particles.position")
        .unwrap()
        .id;
    graph.capabilities[0].kind = CapabilityKind::HostMutate {
        streams: vec![position],
    };

    let report = Validator::validate(&graph);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidCapability)
    );
}

#[test]
fn repair_plan_is_hash_bound_atomic_and_revalidated() {
    let graph = hello_particle_builder(HelloParticleConfig::unsafe_unproven_overlap())
        .build()
        .unwrap();
    let report = Validator::validate(&graph);
    assert!(report.artifact_fingerprint().is_none());

    let repair = RepairPlan::from_report(&report).expect("repair plan");
    assert_eq!(repair.edits.len(), 2);
    let validated = repair
        .apply_and_validate(&graph)
        .expect("atomic repair should validate");
    assert_eq!(validated.artifact_fingerprint().len(), 64);
    for stream in &validated.graph().resources.streams {
        if stream.name == "particles.position" || stream.name == "particles.velocity" {
            assert_eq!(stream.buffering, 4);
        }
    }

    let mut stale = graph.clone();
    stale.name.push_str("_changed");
    assert!(matches!(
        repair.apply_and_validate(&stale),
        Err(RepairError::SourceHashMismatch { .. })
    ));

    let mut wrong_old_value = graph.clone();
    wrong_old_value
        .resources
        .streams
        .iter_mut()
        .find(|stream| stream.name == "particles.position")
        .unwrap()
        .buffering = 2;
    let mut expected_value_plan = repair.clone();
    expected_value_plan.source_graph_hash = canonicalize(&wrong_old_value).fingerprint;
    assert!(matches!(
        expected_value_plan.apply_and_validate(&wrong_old_value),
        Err(RepairError::ExpectedValueMismatch { .. })
    ));
}

#[test]
fn only_validated_graphs_receive_execution_plans_and_artifact_fingerprints() {
    let valid_graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    let valid = Validator::validate(&valid_graph);
    let validated = valid.validated.as_ref().expect("validated graph");
    assert_eq!(validated.execution_plan().schedules.len(), 1);
    let schedule = &validated.execution_plan().schedules[0];
    assert_eq!(schedule.passes.len(), 2);
    assert_eq!(schedule.views.len(), 1);
    assert!(!schedule.accesses.is_empty());
    assert_eq!(schedule.completion_requirements.len(), 4);
    assert!(!schedule.resource_versions.is_empty());
    assert!(schedule.completion_requirements.iter().any(|requirement| {
        matches!(
            requirement,
            pqo_validator::CompletionRequirement::BeforeNextTick {
                after: pqo_core::ScheduleItemId::View(_),
                streams,
                enforcement: pqo_validator::CompletionEnforcement::HostWait,
            } if !streams.is_empty()
        )
    }));
    assert_eq!(validated.artifact_fingerprint().len(), 64);

    let invalid_graph = hello_particle_builder(HelloParticleConfig::unsafe_unproven_overlap())
        .build()
        .unwrap();
    let invalid = Validator::validate(&invalid_graph);
    assert!(invalid.validated.is_none());
    assert!(invalid.artifact_fingerprint().is_none());
    assert_eq!(invalid.source_graph.fingerprint.len(), 64);
}

#[test]
fn dropped_rendering_releases_only_unsubmitted_view_leases() {
    let graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .unwrap();
    let report = Validator::validate(&graph);
    let schedule = &report
        .validated
        .as_ref()
        .unwrap()
        .execution_plan()
        .schedules[0];

    assert_eq!(
        schedule.dropped_presentation,
        pqo_validator::DroppedPresentationPolicy::ReleaseUnsubmittedLeases
    );
    assert!(schedule.completion_requirements.iter().any(|requirement| {
        matches!(
            requirement,
            pqo_validator::CompletionRequirement::BeforeNextTick {
                after: pqo_core::ScheduleItemId::View(_),
                ..
            }
        )
    }));
    assert!(schedule.resource_versions.iter().any(|allocation| {
        allocation.simulation_live_versions == 1
            && allocation.presentation_live_versions == 1
            && allocation.required_versions == 1
    }));
}

fn assert_diagnostics_empty(report: &pqo_validator::ValidationReport) {
    assert!(
        report.is_valid(),
        "unexpected diagnostics:\n{}",
        serde_json::to_string_pretty(&report.diagnostics).unwrap()
    );
}
