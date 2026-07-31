//! Independent deterministic validation passes for normalized Pqo graphs.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ops::Deref,
};

use pqo_core::{
    AliasingRule, Backend, CanonicalGraph, ContractClause, DataType, DeterminismScope,
    DeterminismTier, Diagnostic, DiagnosticCode, DispatchDomain, GraphEdit, KernelNode, Literal,
    ModuleGraph, ObservationPoint, Predicate, PresentationLifetimePolicy, QueueModel,
    RenderOverloadPolicy, ReplayOverloadPolicy, ResourceId, ScenarioDuration, ScenarioTimePolicy,
    ScheduleId, ScheduleItemId, SemanticPath, SlotAccess, SlotResourceType, StreamId, StreamLength,
    TickOverlapPolicy, Unit, ValueKind, ViewState, canonicalize,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationPass {
    StructuralReferences,
    TypesShapesAndUnits,
    ResourceAccess,
    CapacityLengthAndDispatch,
    BindingsAndAliasing,
    HazardConstruction,
    ScheduleDag,
    BufferVersionsAndInFlight,
    BackendAbi,
    ObservationPoints,
    DeterminismAndOverload,
    ValidatedArtifactFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleOrder {
    pub schedule: ScheduleId,
    pub items: Vec<ScheduleItemId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveConcurrency {
    pub schedule: ScheduleId,
    pub requested_ticks: u32,
    pub effective_ticks: u32,
    pub requested_render_frames: u32,
    pub effective_render_frames: u32,
    pub basis: ConcurrencyBasis,
    pub presentation_basis: PresentationConcurrencyBasis,
    pub resource_versions: Vec<ResourceVersionAllocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConcurrencyBasis {
    ResourceVersions,
    SerializedConflicts,
    ProvenSerialQueue,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationConcurrencyBasis {
    ResourceVersions,
    BlockUntilPresentationCompletes,
    ProvenSerialQueue,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub schema_version: u32,
    pub schedules: Vec<ExecutionSchedule>,
    pub intervention_passes: Vec<PlannedPass>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSchedule {
    pub schedule: ScheduleId,
    pub order: Vec<ScheduleItemId>,
    pub requested_ticks: u32,
    pub effective_ticks: u32,
    pub requested_render_frames: u32,
    pub effective_render_frames: u32,
    pub resource_versions: Vec<ResourceVersionAllocation>,
    pub passes: Vec<PlannedPass>,
    pub views: Vec<PlannedView>,
    pub accesses: Vec<PlannedAccess>,
    pub completion_requirements: Vec<CompletionRequirement>,
    pub dropped_presentation: DroppedPresentationPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceVersionAllocation {
    pub stream: StreamId,
    pub simulation_live_versions: u32,
    pub presentation_live_versions: u32,
    pub required_versions: u32,
    pub allocated_versions: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedPass {
    pub pass: pqo_core::PassId,
    pub kernel: pqo_core::KernelId,
    pub bindings: Vec<pqo_core::Binding>,
    pub dispatch: DispatchDomain,
    pub threads_per_threadgroup: Option<u32>,
    pub abi: pqo_core::KernelAbi,
    pub implementation: pqo_core::BackendImplementation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedView {
    pub view: pqo_core::ViewId,
    pub reads: Vec<pqo_core::ViewRead>,
    pub state: ViewState,
    pub implementation: pqo_core::BackendImplementation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedAccess {
    pub item: ScheduleItemId,
    pub stream: StreamId,
    pub reads: bool,
    pub writes: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompletionRequirement {
    WithinTick {
        before: ScheduleItemId,
        after: ScheduleItemId,
    },
    BeforeNextTick {
        after: ScheduleItemId,
        streams: Vec<StreamId>,
        enforcement: CompletionEnforcement,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompletionEnforcement {
    HostWait,
    SerialQueueOrder,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DroppedPresentationPolicy {
    ReleaseUnsubmittedLeases,
}

#[derive(Clone, Debug)]
pub struct ValidatedModuleGraph {
    graph: ModuleGraph,
    execution_plan: ExecutionPlan,
    artifact_fingerprint: String,
}

impl ValidatedModuleGraph {
    pub fn graph(&self) -> &ModuleGraph {
        &self.graph
    }

    pub fn execution_plan(&self) -> &ExecutionPlan {
        &self.execution_plan
    }

    pub fn artifact_fingerprint(&self) -> &str {
        &self.artifact_fingerprint
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairPlan {
    pub source_graph_hash: String,
    pub edits: Vec<GraphEdit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepairError {
    SourceHashMismatch { expected: String, actual: String },
    ExpectedValueMismatch { target: SemanticPath },
    InvalidTarget { target: SemanticPath },
    UnsupportedDependencyShape,
    RevalidationFailed(Vec<Diagnostic>),
}

impl RepairPlan {
    pub fn from_report(report: &ValidationReport) -> Option<Self> {
        let mut stream_edits = BTreeMap::<StreamId, (u32, u32)>::new();
        let mut other_edits = Vec::new();
        for edit in report
            .diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.suggested_fix.clone())
        {
            match edit {
                GraphEdit::SetStreamBuffering {
                    stream,
                    expected,
                    versions,
                } => {
                    let entry = stream_edits.entry(stream).or_insert((expected, versions));
                    entry.1 = entry.1.max(versions);
                }
                edit => other_edits.push(edit),
            }
        }
        let mut edits = stream_edits
            .into_iter()
            .map(
                |(stream, (expected, versions))| GraphEdit::SetStreamBuffering {
                    stream,
                    expected,
                    versions,
                },
            )
            .chain(other_edits)
            .collect::<Vec<_>>();
        edits.sort_by_cached_key(|edit| {
            serde_json::to_vec(edit).expect("repair edit serialization")
        });
        (!edits.is_empty()).then(|| Self {
            source_graph_hash: report.source_graph.fingerprint.clone(),
            edits,
        })
    }

    pub fn apply_and_validate(
        &self,
        graph: &ModuleGraph,
    ) -> Result<ValidatedModuleGraph, RepairError> {
        let actual_hash = canonicalize(graph).fingerprint;
        if actual_hash != self.source_graph_hash {
            return Err(RepairError::SourceHashMismatch {
                expected: self.source_graph_hash.clone(),
                actual: actual_hash,
            });
        }

        let mut candidate = graph.clone();
        for edit in &self.edits {
            apply_edit(&mut candidate, edit)?;
        }
        let report = Validator::validate(&candidate);
        report
            .validated
            .ok_or(RepairError::RevalidationFailed(report.diagnostics))
    }
}

#[derive(Clone, Debug)]
pub struct ValidationReport {
    pub completed_passes: Vec<ValidationPass>,
    pub diagnostics: Vec<Diagnostic>,
    pub topological_orders: Vec<ScheduleOrder>,
    pub effective_concurrency: Vec<EffectiveConcurrency>,
    pub source_graph: CanonicalGraph,
    pub validated: Option<ValidatedModuleGraph>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.validated.is_some()
    }

    pub fn artifact_fingerprint(&self) -> Option<&str> {
        self.validated
            .as_ref()
            .map(ValidatedModuleGraph::artifact_fingerprint)
    }
}

pub struct Validator;

impl Validator {
    pub fn validate(graph: &ModuleGraph) -> ValidationReport {
        let source_graph = canonicalize(graph);
        let mut diagnostics = Vec::new();
        let mut completed_passes = Vec::new();

        validate_structure(graph, &mut diagnostics);
        completed_passes.push(ValidationPass::StructuralReferences);

        if has_errors(&diagnostics) {
            diagnostics
                .sort_by_key(|diagnostic| (diagnostic.code.clone(), diagnostic.primary.clone()));
            return ValidationReport {
                completed_passes,
                diagnostics,
                topological_orders: Vec::new(),
                effective_concurrency: Vec::new(),
                source_graph,
                validated: None,
            };
        }

        let graph = CheckedGraph(graph);

        validate_types_shapes_units(graph, &mut diagnostics);
        completed_passes.push(ValidationPass::TypesShapesAndUnits);

        validate_resource_access(graph, &mut diagnostics);
        completed_passes.push(ValidationPass::ResourceAccess);

        validate_capacity_length_dispatch(graph, &mut diagnostics);
        completed_passes.push(ValidationPass::CapacityLengthAndDispatch);

        validate_bindings_aliasing(graph, &mut diagnostics);
        completed_passes.push(ValidationPass::BindingsAndAliasing);

        let accesses = construct_hazards(graph);
        completed_passes.push(ValidationPass::HazardConstruction);

        let topological_orders = validate_schedule_dags(graph, &accesses, &mut diagnostics);
        completed_passes.push(ValidationPass::ScheduleDag);

        let effective_concurrency = validate_buffer_versions(graph, &accesses, &mut diagnostics);
        completed_passes.push(ValidationPass::BufferVersionsAndInFlight);

        validate_backend_abi(graph, &mut diagnostics);
        completed_passes.push(ValidationPass::BackendAbi);

        validate_observation_points(graph, &mut diagnostics);
        completed_passes.push(ValidationPass::ObservationPoints);

        validate_determinism_overload(graph, &mut diagnostics);
        completed_passes.push(ValidationPass::DeterminismAndOverload);

        let execution_plan = build_execution_plan(
            graph,
            &topological_orders,
            &effective_concurrency,
            &accesses,
        );
        let validated = if has_errors(&diagnostics) {
            None
        } else {
            let artifact_fingerprint = artifact_fingerprint(graph.0, &execution_plan);
            Some(ValidatedModuleGraph {
                graph: graph.0.clone(),
                execution_plan,
                artifact_fingerprint,
            })
        };
        completed_passes.push(ValidationPass::ValidatedArtifactFingerprint);

        diagnostics.sort_by_key(|diagnostic| (diagnostic.code.clone(), diagnostic.primary.clone()));

        ValidationReport {
            completed_passes,
            diagnostics,
            topological_orders,
            effective_concurrency,
            source_graph,
            validated,
        }
    }
}

#[derive(Clone, Copy)]
struct CheckedGraph<'a>(&'a ModuleGraph);

impl<'a> CheckedGraph<'a> {
    fn value(self, id: pqo_core::ValueId) -> &'a pqo_core::ValueNode {
        &self.0.resources.values[id.0 as usize]
    }

    fn stream(self, id: StreamId) -> &'a pqo_core::StreamNode {
        &self.0.resources.streams[id.0 as usize]
    }

    fn kernel(self, id: pqo_core::KernelId) -> &'a KernelNode {
        &self.0.kernels[id.0 as usize]
    }

    fn pass(self, id: pqo_core::PassId) -> &'a pqo_core::PassNode {
        &self.0.passes[id.0 as usize]
    }

    fn view(self, id: pqo_core::ViewId) -> &'a pqo_core::ViewNode {
        &self.0.views[id.0 as usize]
    }

    fn schedule(self, id: ScheduleId) -> &'a pqo_core::ScheduleNode {
        &self.0.schedules[id.0 as usize]
    }
}

impl Deref for CheckedGraph<'_> {
    type Target = ModuleGraph;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

fn validate_structure(graph: &ModuleGraph, diagnostics: &mut Vec<Diagnostic>) {
    check_ids_and_names(
        graph
            .resources
            .values
            .iter()
            .map(|node| (node.id.0, &node.name)),
        "values",
        diagnostics,
    );
    check_ids_and_names(
        graph
            .resources
            .streams
            .iter()
            .map(|node| (node.id.0, &node.name)),
        "streams",
        diagnostics,
    );
    check_ids_and_names(
        graph.kernels.iter().map(|node| (node.id.0, &node.name)),
        "kernels",
        diagnostics,
    );
    check_ids_and_names(
        graph.passes.iter().map(|node| (node.id.0, &node.name)),
        "passes",
        diagnostics,
    );
    check_ids_and_names(
        graph.views.iter().map(|node| (node.id.0, &node.name)),
        "views",
        diagnostics,
    );
    check_ids_and_names(
        graph.schedules.iter().map(|node| (node.id.0, &node.name)),
        "schedules",
        diagnostics,
    );
    check_ids_and_names(
        graph.contracts.iter().map(|node| (node.id.0, &node.name)),
        "contracts",
        diagnostics,
    );
    check_ids_and_names(
        graph.scenarios.iter().map(|node| (node.id.0, &node.name)),
        "scenarios",
        diagnostics,
    );
    check_ids_and_names(
        graph.benchmarks.iter().map(|node| (node.id.0, &node.name)),
        "benchmarks",
        diagnostics,
    );
    check_ids_and_names(
        graph
            .capabilities
            .iter()
            .map(|node| (node.id.0, &node.name)),
        "capabilities",
        diagnostics,
    );

    for value in &graph.resources.values {
        if let ValueKind::ScheduleFixedDt { schedule } = value.kind {
            check_reference(
                schedule.0,
                graph.schedules.len(),
                path("values", &value.name).child("schedule"),
                "schedule",
                diagnostics,
            );
        }
    }
    for stream in &graph.resources.streams {
        if let StreamLength::Dynamic(count) = stream.length {
            check_reference(
                count.0,
                graph.resources.streams.len(),
                path("streams", &stream.name).child("length"),
                "stream",
                diagnostics,
            );
        }
        if let Some(capability) = stream.write_authority {
            check_reference(
                capability.0,
                graph.capabilities.len(),
                path("streams", &stream.name).child("write_authority"),
                "capability",
                diagnostics,
            );
        }
    }
    for kernel in &graph.kernels {
        check_ids_and_names(
            kernel.slots.iter().map(|slot| (slot.id.0, &slot.name)),
            &format!("kernels.{}.slots", kernel.name),
            diagnostics,
        );
        for slot in &kernel.abi.binding_order {
            check_reference(
                slot.0,
                kernel.slots.len(),
                path("kernels", &kernel.name).child("abi.binding_order"),
                "slot",
                diagnostics,
            );
        }
        if let AliasingRule::AllowPairs(pairs) = &kernel.abi.aliasing {
            for (left, right) in pairs {
                check_reference(
                    left.0,
                    kernel.slots.len(),
                    path("kernels", &kernel.name).child("abi.aliasing"),
                    "slot",
                    diagnostics,
                );
                check_reference(
                    right.0,
                    kernel.slots.len(),
                    path("kernels", &kernel.name).child("abi.aliasing"),
                    "slot",
                    diagnostics,
                );
            }
        }
    }
    for pass in &graph.passes {
        let pass_path = path("passes", &pass.name);
        let kernel_valid = check_reference(
            pass.kernel.0,
            graph.kernels.len(),
            pass_path.child("kernel"),
            "kernel",
            diagnostics,
        );
        for binding in &pass.bindings {
            if kernel_valid {
                check_reference(
                    binding.slot.0,
                    graph.kernels[pass.kernel.0 as usize].slots.len(),
                    pass_path.child("bindings.slot"),
                    "slot",
                    diagnostics,
                );
            }
            check_resource_reference(
                binding.resource,
                graph,
                pass_path.child("bindings.resource"),
                diagnostics,
            );
        }
        for capability in &pass.capabilities {
            check_reference(
                capability.0,
                graph.capabilities.len(),
                pass_path.child("capabilities"),
                "capability",
                diagnostics,
            );
        }
        if let DispatchDomain::OverStream(stream) = pass.dispatch {
            check_reference(
                stream.0,
                graph.resources.streams.len(),
                pass_path.child("dispatch"),
                "stream",
                diagnostics,
            );
        }
        if pass
            .threads_per_threadgroup
            .is_some_and(|count| count == 0 || count > 1024)
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidDispatch,
                "threads per threadgroup must be between 1 and 1024",
                pass_path.child("threads_per_threadgroup"),
            ));
        }
    }
    for view in &graph.views {
        for read in &view.reads {
            check_reference(
                read.stream.0,
                graph.resources.streams.len(),
                path("views", &view.name).child(format!("reads.{}", read.name)),
                "stream",
                diagnostics,
            );
        }
    }
    for schedule in &graph.schedules {
        let schedule_path = path("schedules", &schedule.name);
        let pqo_core::ScheduleTiming::Fixed { fixed_dt, .. } = schedule.timing;
        check_reference(
            fixed_dt.0,
            graph.resources.values.len(),
            schedule_path.child("fixed_dt"),
            "value",
            diagnostics,
        );
        for pass in &schedule.execution_passes {
            check_reference(
                pass.0,
                graph.passes.len(),
                schedule_path.child("execution_passes"),
                "pass",
                diagnostics,
            );
        }
        for view in &schedule.presentation_views {
            check_reference(
                view.0,
                graph.views.len(),
                schedule_path.child("presentation_views"),
                "view",
                diagnostics,
            );
        }
        for edge in &schedule.execution_dependencies {
            check_reference(
                edge.before.0,
                graph.passes.len(),
                schedule_path.child("execution_dependencies.before"),
                "pass",
                diagnostics,
            );
            check_reference(
                edge.after.0,
                graph.passes.len(),
                schedule_path.child("execution_dependencies.after"),
                "pass",
                diagnostics,
            );
        }
        for edge in &schedule.presentation_dependencies {
            check_reference(
                edge.producer.0,
                graph.passes.len(),
                schedule_path.child("presentation_dependencies.producer"),
                "pass",
                diagnostics,
            );
            check_reference(
                edge.consumer.0,
                graph.views.len(),
                schedule_path.child("presentation_dependencies.consumer"),
                "view",
                diagnostics,
            );
        }
    }
    for contract in &graph.contracts {
        let owner = path("contracts", &contract.name);
        check_reference(
            contract.schedule.0,
            graph.schedules.len(),
            owner.child("schedule"),
            "schedule",
            diagnostics,
        );
        for clause in &contract.clauses {
            match clause {
                ContractClause::Invariant {
                    observation,
                    predicate,
                } => {
                    check_observation_references(observation, graph, &owner, diagnostics);
                    check_predicate_references(predicate, graph, &owner, diagnostics);
                }
                ContractClause::MetricLimit { observation, .. }
                | ContractClause::SteadyStateZero { observation, .. } => {
                    check_observation_references(observation, graph, &owner, diagnostics);
                }
                ContractClause::Determinism(_) => {}
            }
        }
    }
    for scenario in &graph.scenarios {
        let owner = path("scenarios", &scenario.name);
        check_reference(
            scenario.schedule.0,
            graph.schedules.len(),
            owner.child("schedule"),
            "schedule",
            diagnostics,
        );
        for expectation in &scenario.expectations {
            check_observation_references(&expectation.observation, graph, &owner, diagnostics);
            check_predicate_references(&expectation.predicate, graph, &owner, diagnostics);
        }
        for intervention in &scenario.interventions {
            check_reference(
                intervention.pass.0,
                graph.passes.len(),
                owner.child("interventions.pass"),
                "pass",
                diagnostics,
            );
            for override_ in &intervention.value_overrides {
                check_reference(
                    override_.value.0,
                    graph.resources.values.len(),
                    owner.child("interventions.value_overrides"),
                    "value",
                    diagnostics,
                );
            }
        }
    }
    for benchmark in &graph.benchmarks {
        check_reference(
            benchmark.schedule.0,
            graph.schedules.len(),
            path("benchmarks", &benchmark.name).child("schedule"),
            "schedule",
            diagnostics,
        );
    }
    for capability in &graph.capabilities {
        let streams = match &capability.kind {
            pqo_core::CapabilityKind::Inspect { streams, .. }
            | pqo_core::CapabilityKind::HostMutate { streams }
            | pqo_core::CapabilityKind::StateMutate { streams } => streams.clone(),
            pqo_core::CapabilityKind::MembershipMutate { count, members } => {
                std::iter::once(*count)
                    .chain(members.iter().copied())
                    .collect()
            }
            pqo_core::CapabilityKind::External { .. } => Vec::new(),
        };
        for stream in &streams {
            check_reference(
                stream.0,
                graph.resources.streams.len(),
                path("capabilities", &capability.name).child("streams"),
                "stream",
                diagnostics,
            );
        }
    }
}

fn check_reference(
    id: u32,
    len: usize,
    primary: SemanticPath,
    kind: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if (id as usize) < len {
        true
    } else {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::InvalidReference,
            format!("{kind} ID {id} is outside 0..{len}"),
            primary,
        ));
        false
    }
}

fn check_resource_reference(
    resource: ResourceId,
    graph: &ModuleGraph,
    primary: SemanticPath,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match resource {
        ResourceId::Value(value) => {
            check_reference(
                value.0,
                graph.resources.values.len(),
                primary,
                "value",
                diagnostics,
            );
        }
        ResourceId::Stream(stream) => {
            check_reference(
                stream.0,
                graph.resources.streams.len(),
                primary,
                "stream",
                diagnostics,
            );
        }
    }
}

fn check_observation_references(
    observation: &ObservationPoint,
    graph: &ModuleGraph,
    owner: &SemanticPath,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match observation {
        ObservationPoint::AfterPassCompletion(pass) => {
            check_reference(
                pass.0,
                graph.passes.len(),
                owner.child("observation"),
                "pass",
                diagnostics,
            );
        }
        ObservationPoint::AfterEveryPassCompletion(schedule)
        | ObservationPoint::AfterTickExecution(schedule)
        | ObservationPoint::AfterGpuCompletion(schedule) => {
            check_reference(
                schedule.0,
                graph.schedules.len(),
                owner.child("observation"),
                "schedule",
                diagnostics,
            );
        }
    }
}

fn check_predicate_references(
    predicate: &Predicate,
    graph: &ModuleGraph,
    owner: &SemanticPath,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match predicate {
        Predicate::FiniteStreams(streams) => {
            for stream in streams {
                check_reference(
                    stream.0,
                    graph.resources.streams.len(),
                    owner.child("predicate"),
                    "stream",
                    diagnostics,
                );
            }
        }
        Predicate::GroundClearance {
            position,
            radius,
            ground_height,
            ..
        } => {
            check_reference(
                position.0,
                graph.resources.streams.len(),
                owner.child("predicate.position"),
                "stream",
                diagnostics,
            );
            check_reference(
                radius.0,
                graph.resources.streams.len(),
                owner.child("predicate.radius"),
                "stream",
                diagnostics,
            );
            check_reference(
                ground_height.0,
                graph.resources.values.len(),
                owner.child("predicate.ground_height"),
                "value",
                diagnostics,
            );
        }
    }
}

fn check_ids_and_names<'a>(
    items: impl Iterator<Item = (u32, &'a String)>,
    scope: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut names = BTreeSet::new();
    let mut previous_name: Option<&String> = None;
    for (expected, (actual, name)) in items.enumerate() {
        if actual != expected as u32 {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidNodeId,
                format!("non-canonical ID {actual}; expected {expected}"),
                path(scope, name),
            ));
        }
        if previous_name.is_some_and(|previous| previous > name) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::NonCanonicalOrder,
                format!("declarations in `{scope}` must be ordered by name"),
                path(scope, name),
            ));
        }
        previous_name = Some(name);
        if !names.insert(name) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DuplicateSymbol,
                format!("duplicate symbol `{name}`"),
                path(scope, name),
            ));
        }
    }
}

fn validate_types_shapes_units(graph: CheckedGraph<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for value in &graph.resources.values {
        if let ValueKind::Constant(literal) = &value.kind
            && !literal_matches_type(literal, &value.data_type)
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidLiteral,
                format!(
                    "constant literal does not match declared type {:?}",
                    value.data_type
                ),
                path("values", &value.name).child("literal"),
            ));
        }
    }
    for stream in &graph.resources.streams {
        if let Some(initial) = &stream.initial {
            let (count, literals, arithmetic, valid_parameters) = match initial {
                pqo_core::StreamInitializer::Explicit(Literal::Array(items)) => {
                    (items.len(), items.iter().collect::<Vec<_>>(), false, true)
                }
                pqo_core::StreamInitializer::Explicit(_) => {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidInitialData,
                        "explicit stream initial data must be an array",
                        path("streams", &stream.name).child("initial"),
                    ));
                    continue;
                }
                pqo_core::StreamInitializer::Repeat { value, count } => {
                    (*count as usize, vec![value], false, true)
                }
                pqo_core::StreamInitializer::Linear { start, step, count } => {
                    (*count as usize, vec![start, step], true, true)
                }
                pqo_core::StreamInitializer::Grid2D {
                    origin,
                    column_step,
                    row_step,
                    columns,
                    count,
                } => (
                    *count as usize,
                    vec![origin, column_step, row_step],
                    true,
                    *columns > 0,
                ),
            };
            let valid_length = match stream.length {
                StreamLength::Fixed(length) => count == length as usize,
                StreamLength::Dynamic(_) => count <= stream.capacity as usize,
            };
            if !valid_length || count > stream.capacity as usize {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidInitialData,
                    format!(
                        "initial element count {} is incompatible with length and capacity {}",
                        count, stream.capacity
                    ),
                    path("streams", &stream.name).child("initial"),
                ));
            }
            if literals
                .iter()
                .any(|literal| !literal_matches_type(literal, &stream.element_type))
            {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidInitialData,
                    "one or more initial elements do not match the stream element type",
                    path("streams", &stream.name).child("initial"),
                ));
            }
            if arithmetic && !supports_initializer_arithmetic(&stream.element_type) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidInitialData,
                    "linear and grid initializers require an f32 scalar or vector element type",
                    path("streams", &stream.name).child("initial"),
                ));
            }
            if !valid_parameters {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidInitialData,
                    "grid initializer columns must be positive",
                    path("streams", &stream.name).child("initial"),
                ));
            }
        }
    }

    for pass in &graph.passes {
        let kernel = graph.kernel(pass.kernel);
        for binding in &pass.bindings {
            let slot = kernel.slot(binding.slot);
            match (&slot.resource_type, binding.resource) {
                (SlotResourceType::Value { data_type, unit }, ResourceId::Value(value)) => {
                    let resource = graph.value(value);
                    if &resource.data_type != data_type {
                        mismatch(
                            DiagnosticCode::TypeMismatch,
                            pass,
                            slot.name.as_str(),
                            "value type",
                            diagnostics,
                        );
                    }
                    if &resource.unit != unit {
                        mismatch(
                            DiagnosticCode::UnitMismatch,
                            pass,
                            slot.name.as_str(),
                            "value unit",
                            diagnostics,
                        );
                    }
                }
                (SlotResourceType::Stream { element_type, unit }, ResourceId::Stream(stream)) => {
                    let resource = graph.stream(stream);
                    if &resource.element_type != element_type {
                        mismatch(
                            DiagnosticCode::TypeMismatch,
                            pass,
                            slot.name.as_str(),
                            "stream element type",
                            diagnostics,
                        );
                    }
                    if &resource.unit != unit {
                        mismatch(
                            DiagnosticCode::UnitMismatch,
                            pass,
                            slot.name.as_str(),
                            "stream unit",
                            diagnostics,
                        );
                    }
                }
                _ => mismatch(
                    DiagnosticCode::TypeMismatch,
                    pass,
                    slot.name.as_str(),
                    "resource kind",
                    diagnostics,
                ),
            }
        }
    }

    for schedule in &graph.schedules {
        let pqo_core::ScheduleTiming::Fixed { fixed_dt, .. } = schedule.timing;
        let value = graph.value(fixed_dt);
        if value.data_type != DataType::f32()
            || value.unit != Unit::SECOND
            || value.kind
                != (ValueKind::ScheduleFixedDt {
                    schedule: schedule.id,
                })
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::TypeMismatch,
                "fixed schedule must publish its own f32 seconds fixed_dt resource",
                path("schedules", &schedule.name).child("fixed_dt"),
            ));
        }
    }

    for contract in &graph.contracts {
        for clause in &contract.clauses {
            match clause {
                ContractClause::Invariant {
                    predicate:
                        Predicate::GroundClearance {
                            position,
                            radius,
                            ground_height,
                            tolerance,
                        },
                    ..
                } => validate_ground_clearance(
                    graph,
                    *position,
                    *radius,
                    *ground_height,
                    tolerance.unit,
                    &format!("contracts.{}", contract.name),
                    diagnostics,
                ),
                ContractClause::MetricLimit {
                    metric, maximum, ..
                } => validate_metric_limit(
                    metric,
                    maximum,
                    &format!("contracts.{}", contract.name),
                    diagnostics,
                ),
                ContractClause::SteadyStateZero { metric, .. } => {
                    if !matches!(
                        metric,
                        pqo_core::Metric::HeapAllocationsPerTick
                            | pqo_core::Metric::ApplicationCopiesPerTick
                            | pqo_core::Metric::ApplicationBlitsPerTick
                            | pqo_core::Metric::OverloadEvents
                    ) {
                        diagnostics.push(Diagnostic::error(
                            DiagnosticCode::InvalidMetricUnit,
                            "this metric requires a typed limit rather than a zero-count clause",
                            path("contracts", &contract.name).child("metric"),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    for scenario in &graph.scenarios {
        for expectation in &scenario.expectations {
            if let Predicate::GroundClearance {
                position,
                radius,
                ground_height,
                tolerance,
            } = &expectation.predicate
            {
                validate_ground_clearance(
                    graph,
                    *position,
                    *radius,
                    *ground_height,
                    tolerance.unit,
                    &format!("scenarios.{}", scenario.name),
                    diagnostics,
                );
            }
        }
        let ScenarioDuration::SimulationTicks(duration_ticks) = scenario.duration;
        for intervention in &scenario.interventions {
            if intervention.tick >= duration_ticks {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidIntervention,
                    format!(
                        "intervention tick {} is outside the scenario duration of {duration_ticks} ticks",
                        intervention.tick
                    ),
                    path("scenarios", &scenario.name).child("interventions.tick"),
                ));
            }
            if graph
                .schedule(scenario.schedule)
                .execution_passes
                .contains(&intervention.pass)
            {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidIntervention,
                    "an intervention pass must be scenario-only, not part of the regular schedule",
                    path("scenarios", &scenario.name).child("interventions.pass"),
                ));
            }
            if has_duplicates(
                &intervention
                    .value_overrides
                    .iter()
                    .map(|override_| override_.value)
                    .collect::<Vec<_>>(),
            ) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidIntervention,
                    "intervention value overrides must be unique",
                    path("scenarios", &scenario.name).child("interventions.value_overrides"),
                ));
            }
            for override_ in &intervention.value_overrides {
                let value = graph.value(override_.value);
                if !matches!(value.kind, ValueKind::Constant(_))
                    || !literal_matches_type(&override_.literal, &value.data_type)
                {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidIntervention,
                        format!(
                            "override for `{}` must match the type of an immutable constant",
                            value.name
                        ),
                        path("scenarios", &scenario.name).child("interventions.value_overrides"),
                    ));
                }
            }
        }
    }

    for view in &graph.views {
        let Some(first) = view.reads.first() else {
            continue;
        };
        let domain = &graph.stream(first.stream).length;
        for read in view.reads.iter().skip(1) {
            if !compatible_lengths(domain, &graph.stream(read.stream).length) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidLogicalLength,
                    "all streams read by a view must share one logical domain",
                    path("views", &view.name).child("reads"),
                ));
            }
        }
    }
}

fn literal_matches_type(literal: &Literal, data_type: &DataType) -> bool {
    match (literal, data_type) {
        (Literal::Bool(_), DataType::Scalar(pqo_core::ScalarType::Bool))
        | (Literal::I32(_), DataType::Scalar(pqo_core::ScalarType::I32))
        | (Literal::U32(_), DataType::Scalar(pqo_core::ScalarType::U32))
        | (Literal::F16Bits(_), DataType::Scalar(pqo_core::ScalarType::F16))
        | (Literal::F32Bits(_), DataType::Scalar(pqo_core::ScalarType::F32)) => true,
        (Literal::Vector(items), DataType::Vector { scalar, lanes }) => {
            items.len() == *lanes as usize
                && items
                    .iter()
                    .all(|item| literal_matches_scalar(item, scalar))
        }
        (
            Literal::Vector(items),
            DataType::Matrix {
                scalar,
                rows,
                columns,
            },
        ) => {
            items.len() == (*rows as usize * *columns as usize)
                && items
                    .iter()
                    .all(|item| literal_matches_scalar(item, scalar))
        }
        _ => false,
    }
}

fn literal_matches_scalar(literal: &Literal, scalar: &pqo_core::ScalarType) -> bool {
    matches!(
        (literal, scalar),
        (Literal::Bool(_), pqo_core::ScalarType::Bool)
            | (Literal::I32(_), pqo_core::ScalarType::I32)
            | (Literal::U32(_), pqo_core::ScalarType::U32)
            | (Literal::F16Bits(_), pqo_core::ScalarType::F16)
            | (Literal::F32Bits(_), pqo_core::ScalarType::F32)
    )
}

fn supports_initializer_arithmetic(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Scalar(pqo_core::ScalarType::F32)
            | DataType::Vector {
                scalar: pqo_core::ScalarType::F32,
                ..
            }
    )
}

fn validate_metric_limit(
    metric: &pqo_core::Metric,
    maximum: &pqo_core::Quantity,
    primary: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expected_unit = match metric {
        pqo_core::Metric::GpuTimePerTick => Unit::SECOND,
        pqo_core::Metric::HeapAllocationsPerTick
        | pqo_core::Metric::ApplicationCopiesPerTick
        | pqo_core::Metric::ApplicationBlitsPerTick
        | pqo_core::Metric::WorkingSetBytes
        | pqo_core::Metric::OverloadEvents => Unit::DIMENSIONLESS,
    };
    let same_dimension = maximum.unit.length == expected_unit.length
        && maximum.unit.mass == expected_unit.mass
        && maximum.unit.time == expected_unit.time;
    let numeric = matches!(
        maximum.value,
        Literal::I32(_) | Literal::U32(_) | Literal::F16Bits(_) | Literal::F32Bits(_)
    );
    if !same_dimension || !numeric {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::InvalidMetricUnit,
            format!("metric `{metric:?}` has an incompatible limit value or unit"),
            SemanticPath::new(primary).child("metric_limit"),
        ));
    }
}

fn validate_ground_clearance(
    graph: CheckedGraph<'_>,
    position: StreamId,
    radius: StreamId,
    ground_height: pqo_core::ValueId,
    tolerance_unit: Unit,
    primary: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let position = graph.stream(position);
    let radius = graph.stream(radius);
    let ground = graph.value(ground_height);
    let expected_position = DataType::vec3_f32();
    let expected_radius = DataType::f32();
    if position.element_type != expected_position
        || radius.element_type != expected_radius
        || ground.data_type != expected_radius
    {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::TypeMismatch,
            "ground-clearance operands must be vec3<f32>, f32, and f32",
            SemanticPath::new(primary),
        ));
    }
    if position.unit != Unit::METER
        || radius.unit != Unit::METER
        || ground.unit != Unit::METER
        || tolerance_unit != Unit::METER
    {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::UnitMismatch,
            "ground-clearance operands and tolerance must use length units",
            SemanticPath::new(primary),
        ));
    }
    if !compatible_lengths(&position.length, &radius.length) {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::InvalidLogicalLength,
            "ground-clearance position and radius streams must share a logical domain",
            SemanticPath::new(primary),
        ));
    }
}

fn mismatch(
    code: DiagnosticCode,
    pass: &pqo_core::PassNode,
    slot: &str,
    subject: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic::error(
        code,
        format!("binding for slot `{slot}` has an incompatible {subject}"),
        SemanticPath::new(format!("passes.{}.bindings.{slot}", pass.name)),
    ));
}

fn validate_resource_access(graph: CheckedGraph<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for pass in &graph.passes {
        let kernel = graph.kernel(pass.kernel);
        for binding in &pass.bindings {
            let slot = kernel.slot(binding.slot);
            match binding.resource {
                ResourceId::Value(_) if slot.access != SlotAccess::Read => {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::AccessViolation,
                        format!("immutable value bound to writable slot `{}`", slot.name),
                        SemanticPath::new(format!("passes.{}.bindings.{}", pass.name, slot.name)),
                    ));
                }
                ResourceId::Stream(stream) => {
                    let resource = graph.stream(stream);
                    if slot.access.reads() && !resource.access.allows_read()
                        || slot.access.writes() && !resource.access.allows_write()
                    {
                        diagnostics.push(Diagnostic::error(
                            DiagnosticCode::AccessViolation,
                            format!(
                                "slot `{}` requests access not granted by stream `{}`",
                                slot.name, resource.name
                            ),
                            SemanticPath::new(format!(
                                "passes.{}.bindings.{}",
                                pass.name, slot.name
                            )),
                        ));
                    }
                    if slot.access.writes()
                        && resource
                            .write_authority
                            .is_some_and(|required| !pass.capabilities.contains(&required))
                    {
                        diagnostics.push(Diagnostic::error(
                            DiagnosticCode::MissingWriteAuthority,
                            format!(
                                "slot `{}` writes protected stream `{}` without its required capability",
                                slot.name, resource.name
                            ),
                            SemanticPath::new(format!(
                                "passes.{}.bindings.{}",
                                pass.name, slot.name
                            )),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    for pass in &graph.passes {
        if has_duplicates(&pass.capabilities) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidCapability,
                "pass capability grants must be unique",
                path("passes", &pass.name).child("capabilities"),
            ));
        }
    }
    for capability in &graph.capabilities {
        match &capability.kind {
            pqo_core::CapabilityKind::Inspect { streams, .. } => {
                if streams.is_empty() || has_duplicates(streams) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidCapability,
                        "inspection capability requires a nonempty unique stream set",
                        path("capabilities", &capability.name).child("streams"),
                    ));
                }
            }
            pqo_core::CapabilityKind::HostMutate { streams } => {
                if streams.is_empty()
                    || has_duplicates(streams)
                    || streams.iter().any(|stream| {
                        graph.stream(*stream).access != pqo_core::ResourceAccess::HostReadWrite
                    })
                {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidCapability,
                        "host mutation requires unique HostReadWrite streams",
                        path("capabilities", &capability.name).child("streams"),
                    ));
                }
            }
            pqo_core::CapabilityKind::StateMutate { streams } => {
                let valid = !streams.is_empty()
                    && !has_duplicates(streams)
                    && streams.iter().all(|stream| {
                        let stream = graph.stream(*stream);
                        stream.access.allows_write()
                            && stream.write_authority == Some(capability.id)
                    });
                if !valid {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidCapability,
                        "state mutation requires unique writable streams protected by this capability",
                        path("capabilities", &capability.name).child("streams"),
                    ));
                }
            }
            pqo_core::CapabilityKind::MembershipMutate { count, members } => {
                let count_stream = graph.stream(*count);
                let valid_count = count_stream.element_type == DataType::u32()
                    && count_stream.unit == Unit::DIMENSIONLESS
                    && count_stream.capacity == 1
                    && count_stream.length == StreamLength::Fixed(1)
                    && count_stream.access.allows_write();
                let valid_members = !members.is_empty()
                    && !has_duplicates(members)
                    && !members.contains(count)
                    && members.iter().all(|member| {
                        graph.stream(*member).length == StreamLength::Dynamic(*count)
                    });
                if !valid_count || !valid_members {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidMembershipAuthority,
                        "membership mutation requires one writable dimensionless u32 count stream and a nonempty unique set of streams using that count",
                        path("capabilities", &capability.name),
                    ));
                }
            }
            pqo_core::CapabilityKind::External { name } if name.trim().is_empty() => {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidCapability,
                    "external capability name cannot be empty",
                    path("capabilities", &capability.name).child("name"),
                ));
            }
            pqo_core::CapabilityKind::External { .. } => {}
        }
    }

    for stream in &graph.resources.streams {
        let Some(authority) = stream.write_authority else {
            continue;
        };
        let capability = &graph.capabilities[authority.0 as usize];
        if let pqo_core::CapabilityKind::MembershipMutate { count, members } = &capability.kind
            && stream.id != *count
            && !members.contains(&stream.id)
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidMembershipAuthority,
                format!(
                    "stream `{}` requires membership capability `{}` but is not in its protected set",
                    stream.name, capability.name
                ),
                path("streams", &stream.name).child("write_authority"),
            ));
        }
    }
}

fn has_duplicates<T: Ord + Copy>(items: &[T]) -> bool {
    let mut seen = BTreeSet::new();
    items.iter().any(|item| !seen.insert(*item))
}

fn validate_capacity_length_dispatch(graph: CheckedGraph<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for stream in &graph.resources.streams {
        if stream.capacity == 0 || stream.buffering == 0 {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidLogicalLength,
                "stream capacity and buffering must both be positive",
                path("streams", &stream.name),
            ));
        }
        match stream.length {
            StreamLength::Fixed(length) if length > stream.capacity => {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::CapacityExceeded,
                    format!(
                        "logical length {length} exceeds capacity {}",
                        stream.capacity
                    ),
                    path("streams", &stream.name).child("length"),
                ));
            }
            StreamLength::Dynamic(count) => {
                let count = graph.stream(count);
                if count.element_type != DataType::u32()
                    || count.unit != Unit::DIMENSIONLESS
                    || count.capacity != 1
                    || count.length != StreamLength::Fixed(1)
                    || !count.access.allows_read()
                {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidLogicalLength,
                        "dynamic stream length must use a readable, fixed-length, one-element dimensionless u32 stream",
                        path("streams", &stream.name).child("length"),
                    ));
                }
            }
            _ => {}
        }
    }

    for schedule in &graph.schedules {
        let pqo_core::ScheduleTiming::Fixed { rate_hz, .. } = schedule.timing;
        if rate_hz == 0 {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidOverloadPolicy,
                "fixed schedule rate must be greater than zero",
                path("schedules", &schedule.name).child("rate_hz"),
            ));
        }
        if schedule.in_flight.simulation_ticks == 0 || schedule.in_flight.render_frames == 0 {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidInFlightPolicy,
                "simulation ticks and render frames in flight must be positive",
                path("schedules", &schedule.name).child("in_flight"),
            ));
        }
        if schedule.overload.catch_up_limit == 0 {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidOverloadPolicy,
                "catch-up limit must be positive",
                path("schedules", &schedule.name).child("overload.catch_up_limit"),
            ));
        }
    }

    for pass in &graph.passes {
        let DispatchDomain::OverStream(domain) = pass.dispatch else {
            continue;
        };
        let domain = graph.stream(domain);
        let kernel = graph.kernel(pass.kernel);
        for binding in &pass.bindings {
            let ResourceId::Stream(stream) = binding.resource else {
                continue;
            };
            if kernel.slot(binding.slot).indexing == pqo_core::StreamIndexing::WholeResource {
                continue;
            }
            let stream = graph.stream(stream);
            if !compatible_lengths(&domain.length, &stream.length) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidLogicalLength,
                    format!(
                        "stream `{}` does not share dispatch domain length with `{}`",
                        stream.name, domain.name
                    ),
                    path("passes", &pass.name).child("dispatch"),
                ));
            }
        }
    }
}

fn compatible_lengths(left: &StreamLength, right: &StreamLength) -> bool {
    match (left, right) {
        (StreamLength::Fixed(left), StreamLength::Fixed(right)) => left == right,
        (StreamLength::Dynamic(left), StreamLength::Dynamic(right)) => left == right,
        _ => false,
    }
}

fn validate_bindings_aliasing(graph: CheckedGraph<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for pass in &graph.passes {
        let kernel = graph.kernel(pass.kernel);
        let mut bound_slots = BTreeMap::<pqo_core::SlotId, usize>::new();
        for binding in &pass.bindings {
            *bound_slots.entry(binding.slot).or_default() += 1;
        }
        for slot in &kernel.slots {
            match bound_slots.get(&slot.id).copied().unwrap_or_default() {
                0 => diagnostics.push(Diagnostic::error(
                    DiagnosticCode::MissingBinding,
                    format!("required slot `{}` is not bound", slot.name),
                    path("passes", &pass.name).child("bindings"),
                )),
                count if count > 1 => diagnostics.push(Diagnostic::error(
                    DiagnosticCode::DuplicateBinding,
                    format!("slot `{}` is bound {count} times", slot.name),
                    path("passes", &pass.name).child("bindings"),
                )),
                _ => {}
            }
        }

        let mut resources = BTreeMap::<StreamId, Vec<pqo_core::SlotId>>::new();
        for binding in &pass.bindings {
            if let ResourceId::Stream(stream) = binding.resource {
                resources.entry(stream).or_default().push(binding.slot);
            }
        }
        for (stream, slots) in resources {
            for left_index in 0..slots.len() {
                for right_index in left_index + 1..slots.len() {
                    if !alias_allowed(kernel, slots[left_index], slots[right_index]) {
                        diagnostics.push(Diagnostic::error(
                            DiagnosticCode::IllegalAlias,
                            format!(
                                "stream `{}` aliases kernel slots without ABI permission",
                                graph.stream(stream).name
                            ),
                            path("passes", &pass.name).child("bindings"),
                        ));
                    }
                }
            }
        }
    }
}

fn alias_allowed(kernel: &KernelNode, left: pqo_core::SlotId, right: pqo_core::SlotId) -> bool {
    match &kernel.abi.aliasing {
        AliasingRule::Forbidden => false,
        AliasingRule::AllowPairs(pairs) => pairs
            .iter()
            .any(|(a, b)| (*a == left && *b == right) || (*a == right && *b == left)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Access {
    item: ScheduleItemId,
    stream: StreamId,
    reads: bool,
    writes: bool,
}

fn construct_hazards(graph: CheckedGraph<'_>) -> BTreeMap<ScheduleId, Vec<Access>> {
    graph
        .schedules
        .iter()
        .map(|schedule| {
            let mut accesses = Vec::new();
            for pass_id in &schedule.execution_passes {
                let pass = graph.pass(*pass_id);
                let kernel = graph.kernel(pass.kernel);
                for binding in &pass.bindings {
                    if let ResourceId::Stream(stream) = binding.resource {
                        let slot = kernel.slot(binding.slot);
                        accesses.push(Access {
                            item: ScheduleItemId::Pass(*pass_id),
                            stream,
                            reads: slot.access.reads(),
                            writes: slot.access.writes(),
                        });
                    }
                }
            }
            for view_id in &schedule.presentation_views {
                for read in &graph.view(*view_id).reads {
                    accesses.push(Access {
                        item: ScheduleItemId::View(*view_id),
                        stream: read.stream,
                        reads: true,
                        writes: false,
                    });
                }
            }
            accesses.sort_by_key(|access| (access.item, access.stream));
            (schedule.id, accesses)
        })
        .collect()
}

fn validate_schedule_dags(
    graph: CheckedGraph<'_>,
    accesses: &BTreeMap<ScheduleId, Vec<Access>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ScheduleOrder> {
    let mut orders = Vec::new();
    for schedule in &graph.schedules {
        let nodes = schedule_nodes(schedule);
        let edges = schedule_edges(schedule);
        let order = topological_sort(&nodes, &edges);
        if let Some(items) = order {
            let reachability = transitive_reachability(&nodes, &edges);
            validate_unordered_hazards(
                graph,
                schedule.id,
                accesses.get(&schedule.id).map(Vec::as_slice).unwrap_or(&[]),
                &reachability,
                diagnostics,
            );
            orders.push(ScheduleOrder {
                schedule: schedule.id,
                items,
            });
        } else {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DependencyCycle,
                "schedule dependency graph contains a cycle",
                path("schedules", &schedule.name),
            ));
        }
    }
    orders
}

fn schedule_nodes(schedule: &pqo_core::ScheduleNode) -> BTreeSet<ScheduleItemId> {
    schedule
        .execution_passes
        .iter()
        .copied()
        .map(ScheduleItemId::Pass)
        .chain(
            schedule
                .presentation_views
                .iter()
                .copied()
                .map(ScheduleItemId::View),
        )
        .collect()
}

fn schedule_edges(schedule: &pqo_core::ScheduleNode) -> BTreeSet<(ScheduleItemId, ScheduleItemId)> {
    schedule
        .execution_dependencies
        .iter()
        .map(|edge| {
            (
                ScheduleItemId::Pass(edge.before),
                ScheduleItemId::Pass(edge.after),
            )
        })
        .chain(schedule.presentation_dependencies.iter().map(|edge| {
            (
                ScheduleItemId::Pass(edge.producer),
                ScheduleItemId::View(edge.consumer),
            )
        }))
        .collect()
}

fn topological_sort(
    nodes: &BTreeSet<ScheduleItemId>,
    edges: &BTreeSet<(ScheduleItemId, ScheduleItemId)>,
) -> Option<Vec<ScheduleItemId>> {
    let mut indegree = nodes
        .iter()
        .map(|node| (*node, 0_u32))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<ScheduleItemId, Vec<ScheduleItemId>>::new();
    for (before, after) in edges {
        if !nodes.contains(before) || !nodes.contains(after) {
            return None;
        }
        *indegree.entry(*after).or_default() += 1;
        outgoing.entry(*before).or_default().push(*after);
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(*node))
        .collect::<BTreeSet<_>>();
    let mut result = Vec::new();
    while let Some(node) = ready.pop_first() {
        result.push(node);
        for after in outgoing.get(&node).into_iter().flatten() {
            let degree = indegree.get_mut(after).expect("edge target exists");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(*after);
            }
        }
    }
    (result.len() == nodes.len()).then_some(result)
}

fn transitive_reachability(
    nodes: &BTreeSet<ScheduleItemId>,
    edges: &BTreeSet<(ScheduleItemId, ScheduleItemId)>,
) -> BTreeSet<(ScheduleItemId, ScheduleItemId)> {
    let outgoing = edges.iter().fold(
        BTreeMap::<ScheduleItemId, Vec<ScheduleItemId>>::new(),
        |mut map, (before, after)| {
            map.entry(*before).or_default().push(*after);
            map
        },
    );
    let mut result = BTreeSet::new();
    for start in nodes {
        let mut queue = VecDeque::from([*start]);
        let mut seen = BTreeSet::new();
        while let Some(current) = queue.pop_front() {
            for next in outgoing.get(&current).into_iter().flatten() {
                if seen.insert(*next) {
                    result.insert((*start, *next));
                    queue.push_back(*next);
                }
            }
        }
    }
    result
}

fn validate_unordered_hazards(
    graph: CheckedGraph<'_>,
    schedule: ScheduleId,
    accesses: &[Access],
    reachability: &BTreeSet<(ScheduleItemId, ScheduleItemId)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for left_index in 0..accesses.len() {
        for right_index in left_index + 1..accesses.len() {
            let left = accesses[left_index];
            let right = accesses[right_index];
            if left.item == right.item
                || left.stream != right.stream
                || !(left.writes || right.writes)
            {
                continue;
            }
            if !reachability.contains(&(left.item, right.item))
                && !reachability.contains(&(right.item, left.item))
            {
                let schedule_name = &graph.schedule(schedule).name;
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::UnorderedHazard,
                        format!(
                            "conflicting accesses to `{}` have no completion dependency",
                            graph.stream(left.stream).name
                        ),
                        path("schedules", schedule_name).child("dependencies"),
                    )
                    .fix(GraphEdit::AddCompletionDependency {
                        schedule,
                        before: left.item,
                        after: right.item,
                        expected_absent: true,
                    }),
                );
            }
        }
    }
}

fn validate_buffer_versions(
    graph: CheckedGraph<'_>,
    accesses: &BTreeMap<ScheduleId, Vec<Access>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<EffectiveConcurrency> {
    let mut results = Vec::new();
    for schedule in &graph.schedules {
        let requested = schedule.in_flight.simulation_ticks;
        let requested_render_frames = schedule.in_flight.render_frames;
        let mutated = accesses[&schedule.id]
            .iter()
            .filter(|access| access.writes)
            .map(|access| access.stream)
            .collect::<BTreeSet<_>>();

        let (mut effective_ticks, mut basis) = match schedule.tick_overlap {
            TickOverlapPolicy::RequireResourceVersions => {
                (requested, ConcurrencyBasis::ResourceVersions)
            }
            TickOverlapPolicy::SerializeConflictingTicks => {
                (requested.min(1), ConcurrencyBasis::SerializedConflicts)
            }
            TickOverlapPolicy::QueueOrderedReuse => {
                if schedule.queue_model == QueueModel::SingleSerialQueue {
                    (requested, ConcurrencyBasis::ProvenSerialQueue)
                } else {
                    diagnostics.push(
                        Diagnostic::error(
                            DiagnosticCode::UnprovenQueueReuse,
                            "queue-ordered reuse requires a single serial queue completion proof",
                            path("schedules", &schedule.name).child("queue_model"),
                        )
                        .fix(GraphEdit::SetTickOverlapPolicy {
                            schedule: schedule.id,
                            expected: TickOverlapPolicy::QueueOrderedReuse,
                            policy: TickOverlapPolicy::SerializeConflictingTicks,
                        }),
                    );
                    (0, ConcurrencyBasis::Invalid)
                }
            }
        };

        let presented_mutable = schedule
            .presentation_views
            .iter()
            .flat_map(|view| graph.view(*view).reads.iter())
            .map(|read| read.stream)
            .filter(|stream| mutated.contains(stream))
            .collect::<BTreeSet<_>>();

        let (mut effective_render_frames, mut presentation_basis) = match schedule
            .presentation_lifetime
        {
            PresentationLifetimePolicy::RequireResourceVersions => (
                requested_render_frames,
                PresentationConcurrencyBasis::ResourceVersions,
            ),
            PresentationLifetimePolicy::BlockNextTickUntilViewsComplete => (
                requested_render_frames.min(1),
                PresentationConcurrencyBasis::BlockUntilPresentationCompletes,
            ),
            PresentationLifetimePolicy::QueueOrderedReuse => {
                if schedule.queue_model == QueueModel::SingleSerialQueue {
                    (
                        requested_render_frames,
                        PresentationConcurrencyBasis::ProvenSerialQueue,
                    )
                } else {
                    diagnostics.push(Diagnostic::error(
                            DiagnosticCode::UnsafePresentationLifetime,
                            "queue-ordered presentation reuse requires a single serial queue completion proof",
                            path("schedules", &schedule.name)
                                .child("presentation_lifetime"),
                        ));
                    (0, PresentationConcurrencyBasis::Invalid)
                }
            }
        };

        let mut history_depth = BTreeMap::<StreamId, u32>::new();
        for view_id in &schedule.presentation_views {
            let view = graph.view(*view_id);
            let depth = match view.state {
                ViewState::CurrentCompletedTick => 1,
                ViewState::PreviousStableTick { lag } => lag.saturating_add(1),
                ViewState::Interpolated { older_lag, .. } => older_lag.saturating_add(1),
            };
            for read in &view.reads {
                history_depth
                    .entry(read.stream)
                    .and_modify(|current| *current = (*current).max(depth))
                    .or_insert(depth);
            }
        }

        let relevant_streams = mutated
            .iter()
            .copied()
            .chain(history_depth.keys().copied())
            .collect::<BTreeSet<_>>();
        let mut resource_versions = Vec::new();
        for stream_id in relevant_streams {
            let stream = graph.stream(stream_id);
            let simulation_live_versions = if mutated.contains(&stream_id) {
                match schedule.tick_overlap {
                    TickOverlapPolicy::RequireResourceVersions => requested,
                    TickOverlapPolicy::SerializeConflictingTicks
                    | TickOverlapPolicy::QueueOrderedReuse => 1,
                }
            } else {
                0
            };
            let depth = history_depth.get(&stream_id).copied().unwrap_or(0);
            let presentation_live_versions = if presented_mutable.contains(&stream_id) {
                match schedule.presentation_lifetime {
                    PresentationLifetimePolicy::RequireResourceVersions => {
                        requested_render_frames.saturating_add(depth.saturating_sub(1))
                    }
                    PresentationLifetimePolicy::BlockNextTickUntilViewsComplete
                    | PresentationLifetimePolicy::QueueOrderedReuse => depth,
                }
            } else {
                depth
            };
            let required_versions = if simulation_live_versions > 0
                && presentation_live_versions > 0
                && schedule.presentation_lifetime
                    == PresentationLifetimePolicy::RequireResourceVersions
            {
                simulation_live_versions
                    .saturating_add(presentation_live_versions)
                    .saturating_sub(1)
            } else {
                simulation_live_versions.max(presentation_live_versions)
            };

            resource_versions.push(ResourceVersionAllocation {
                stream: stream_id,
                simulation_live_versions,
                presentation_live_versions,
                required_versions,
                allocated_versions: stream.buffering,
            });

            if stream.buffering >= required_versions {
                continue;
            }

            let presentation_contributes = presentation_live_versions > 0;
            let code = if presentation_contributes {
                DiagnosticCode::UnsafePresentationLifetime
            } else {
                DiagnosticCode::InsufficientBufferVersions
            };
            diagnostics.push(
                Diagnostic::error(
                    code,
                    format!(
                        "stream `{}` has {} version(s), but its complete live range requires {required_versions} (simulation {simulation_live_versions}, presentation/history {presentation_live_versions})",
                        stream.name, stream.buffering
                    ),
                    path("streams", &stream.name).child("buffering"),
                )
                .related(path("schedules", &schedule.name).child("in_flight"))
                .fix(GraphEdit::SetStreamBuffering {
                    stream: stream.id,
                    expected: stream.buffering,
                    versions: required_versions,
                }),
            );
            if simulation_live_versions > stream.buffering {
                effective_ticks = 0;
                basis = ConcurrencyBasis::Invalid;
            }
            if presentation_contributes {
                effective_render_frames = 0;
                presentation_basis = PresentationConcurrencyBasis::Invalid;
            }
        }

        results.push(EffectiveConcurrency {
            schedule: schedule.id,
            requested_ticks: requested,
            effective_ticks,
            requested_render_frames,
            effective_render_frames,
            basis,
            presentation_basis,
            resource_versions,
        });
    }
    results
}

fn validate_backend_abi(graph: CheckedGraph<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for kernel in &graph.kernels {
        for slot in &kernel.slots {
            if matches!(slot.resource_type, SlotResourceType::Value { .. })
                && slot.indexing != pqo_core::StreamIndexing::WholeResource
            {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidKernelAbi,
                    "value slots must use whole-resource indexing",
                    path("kernels", &kernel.name).child(format!("slots.{}", slot.name)),
                ));
            }
        }
        let expected = kernel
            .slots
            .iter()
            .map(|slot| slot.id)
            .collect::<BTreeSet<_>>();
        let actual = kernel
            .abi
            .binding_order
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if expected != actual || kernel.abi.binding_order.len() != kernel.slots.len() {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidKernelAbi,
                "ABI binding order must contain every slot exactly once",
                path("kernels", &kernel.name).child("abi.binding_order"),
            ));
        }
        if let pqo_core::ThreadgroupBehavior::Fixed { x, y, z } = kernel.abi.threadgroup
            && (x == 0 || y == 0 || z == 0)
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidKernelAbi,
                "fixed threadgroup dimensions must be positive",
                path("kernels", &kernel.name).child("abi.threadgroup"),
            ));
        }
        let has_backend = match graph.target {
            pqo_core::Target::Metal => kernel.implementations.iter().any(|implementation| {
                implementation.backend == Backend::Metal
                    && !implementation.source.is_empty()
                    && !implementation.entry.is_empty()
                    && implementation
                        .source_text
                        .as_ref()
                        .is_none_or(|source| !source.trim().is_empty())
            }),
        };
        if !has_backend {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::MissingBackendImplementation,
                "kernel has no complete implementation for the selected target",
                path("kernels", &kernel.name).child("implementations"),
            ));
        }
    }
    for view in &graph.views {
        let implementation = &view.implementation;
        let complete = match graph.target {
            pqo_core::Target::Metal => {
                implementation.backend == Backend::Metal
                    && !implementation.source.trim().is_empty()
                    && !implementation.entry.trim().is_empty()
                    && implementation
                        .source_text
                        .as_ref()
                        .is_none_or(|source| !source.trim().is_empty())
            }
        };
        if !complete {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::MissingBackendImplementation,
                "view has no complete implementation for the selected target",
                path("views", &view.name).child("implementation"),
            ));
        }
    }
}

fn validate_observation_points(graph: CheckedGraph<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for view in &graph.views {
        match view.state {
            ViewState::CurrentCompletedTick => {}
            ViewState::PreviousStableTick { lag: 0 } => diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidViewState,
                "previous stable tick requires a lag of at least one",
                path("views", &view.name).child("state"),
            )),
            ViewState::Interpolated {
                older_lag,
                newer_lag,
            } if older_lag <= newer_lag => diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidViewState,
                "interpolation requires older_lag > newer_lag",
                path("views", &view.name).child("state"),
            )),
            _ => {}
        }
    }

    for contract in &graph.contracts {
        for clause in &contract.clauses {
            let observation = match clause {
                ContractClause::Invariant { observation, .. }
                | ContractClause::MetricLimit { observation, .. }
                | ContractClause::SteadyStateZero { observation, .. } => Some(observation),
                ContractClause::Determinism(_) => None,
            };
            if let Some(observation) = observation {
                validate_observation_for_schedule(
                    graph,
                    contract.schedule,
                    observation,
                    &format!("contracts.{}", contract.name),
                    diagnostics,
                );
            }
        }
    }
    for scenario in &graph.scenarios {
        for expectation in &scenario.expectations {
            validate_observation_for_schedule(
                graph,
                scenario.schedule,
                &expectation.observation,
                &format!("scenarios.{}", scenario.name),
                diagnostics,
            );
        }
    }
}

fn validate_observation_for_schedule(
    graph: CheckedGraph<'_>,
    owner_schedule: ScheduleId,
    observation: &ObservationPoint,
    primary: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let valid = match observation {
        ObservationPoint::AfterPassCompletion(pass) => graph
            .schedule(owner_schedule)
            .execution_passes
            .contains(pass),
        ObservationPoint::AfterEveryPassCompletion(schedule)
        | ObservationPoint::AfterTickExecution(schedule)
        | ObservationPoint::AfterGpuCompletion(schedule) => *schedule == owner_schedule,
    };
    if !valid {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::InvalidObservationPoint,
            "observation point is outside the owning schedule",
            SemanticPath::new(primary).child("observation"),
        ));
    }
}

fn validate_determinism_overload(graph: CheckedGraph<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for contract in &graph.contracts {
        for clause in &contract.clauses {
            let ContractClause::Determinism(determinism) = clause else {
                continue;
            };
            if determinism.tier == DeterminismTier::Tier1
                && determinism.scope != DeterminismScope::ExactExecutionFingerprint
            {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::IncompatibleDeterminismScope,
                    "Tier 1 requires the exact binary, device, OS, pipeline, layout, dispatch, schedule, and input fingerprint",
                    path("contracts", &contract.name).child("determinism"),
                ));
            }
            if matches!(determinism.scope, DeterminismScope::CrossGpu)
                && determinism.tier == DeterminismTier::Tier1
            {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::IncompatibleDeterminismScope,
                    "cross-GPU exactness cannot be claimed as Tier 1",
                    path("contracts", &contract.name).child("determinism.scope"),
                ));
            }
        }
    }

    for schedule in &graph.schedules {
        if matches!(
            schedule.overload.excess_wall_time,
            pqo_core::ExcessWallTimePolicy::Discard
        ) && schedule.overload.replay != ReplayOverloadPolicy::RecordDecisions
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::NondeterministicOverload,
                "discarded wall-time debt must be recorded as a replay input",
                path("schedules", &schedule.name).child("overload.replay"),
            ));
        }
        if matches!(
            schedule.overload.excess_wall_time,
            pqo_core::ExcessWallTimePolicy::Discard
        ) && schedule.overload.scenario_time != ScenarioTimePolicy::SimulationTicks
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::NondeterministicOverload,
                "scenarios must advance by executed simulation ticks when wall time is discarded",
                path("schedules", &schedule.name).child("overload.scenario_time"),
            ));
        }
        if schedule.overload.rendering == RenderOverloadPolicy::DropPresentationOnly {
            // Presentation nodes are terminal by graph construction: execution dependencies
            // can only connect pass → pass, never view → pass.
            for edge in &schedule.presentation_dependencies {
                if !schedule.execution_passes.contains(&edge.producer)
                    || !schedule.presentation_views.contains(&edge.consumer)
                {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::RenderDependencyAffectsSimulation,
                        "render dropping is legal only for terminal presentation dependencies",
                        path("schedules", &schedule.name).child("presentation_dependencies"),
                    ));
                }
            }
        }
    }
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == pqo_core::Severity::Error)
}

fn build_execution_plan(
    graph: CheckedGraph<'_>,
    orders: &[ScheduleOrder],
    concurrency: &[EffectiveConcurrency],
    accesses: &BTreeMap<ScheduleId, Vec<Access>>,
) -> ExecutionPlan {
    let schedules = orders
        .iter()
        .filter_map(|order| {
            let concurrency = concurrency
                .iter()
                .find(|item| item.schedule == order.schedule)?;
            let schedule = graph.schedule(order.schedule);
            let mut pass_ids = schedule.execution_passes.clone();
            pass_ids.sort();
            pass_ids.dedup();
            let passes = pass_ids
                .iter()
                .map(|pass_id| plan_pass(graph, *pass_id))
                .collect::<Vec<_>>();
            let mut view_ids = schedule.presentation_views.clone();
            view_ids.sort();
            view_ids.dedup();
            let views = view_ids
                .iter()
                .map(|view_id| {
                    let view = graph.view(*view_id);
                    let mut reads = view.reads.clone();
                    reads.sort_by(|left, right| {
                        (&left.name, left.stream).cmp(&(&right.name, right.stream))
                    });
                    PlannedView {
                        view: view.id,
                        reads,
                        state: view.state.clone(),
                        implementation: view.implementation.clone(),
                    }
                })
                .collect::<Vec<_>>();
            let accesses = accesses
                .get(&order.schedule)
                .into_iter()
                .flatten()
                .map(|access| PlannedAccess {
                    item: access.item,
                    stream: access.stream,
                    reads: access.reads,
                    writes: access.writes,
                })
                .collect::<Vec<_>>();
            let mut completion_requirements = schedule
                .execution_dependencies
                .iter()
                .map(|edge| CompletionRequirement::WithinTick {
                    before: ScheduleItemId::Pass(edge.before),
                    after: ScheduleItemId::Pass(edge.after),
                })
                .chain(schedule.presentation_dependencies.iter().map(|edge| {
                    CompletionRequirement::WithinTick {
                        before: ScheduleItemId::Pass(edge.producer),
                        after: ScheduleItemId::View(edge.consumer),
                    }
                }))
                .collect::<Vec<_>>();

            if matches!(
                schedule.tick_overlap,
                TickOverlapPolicy::SerializeConflictingTicks | TickOverlapPolicy::QueueOrderedReuse
            ) {
                let enforcement = match schedule.tick_overlap {
                    TickOverlapPolicy::SerializeConflictingTicks => CompletionEnforcement::HostWait,
                    TickOverlapPolicy::QueueOrderedReuse => CompletionEnforcement::SerialQueueOrder,
                    TickOverlapPolicy::RequireResourceVersions => unreachable!(),
                };
                let schedule_accesses = accesses
                    .iter()
                    .map(|access| (access.item, access.stream, access.writes))
                    .collect::<Vec<_>>();
                let mut last_writer = BTreeMap::<StreamId, ScheduleItemId>::new();
                for item in &order.items {
                    for (_, stream, _) in schedule_accesses
                        .iter()
                        .filter(|(candidate, _, writes)| candidate == item && *writes)
                    {
                        last_writer.insert(*stream, *item);
                    }
                }
                let mut by_item = BTreeMap::<ScheduleItemId, Vec<StreamId>>::new();
                for (stream, item) in last_writer {
                    by_item.entry(item).or_default().push(stream);
                }
                completion_requirements.extend(by_item.into_iter().map(|(after, mut streams)| {
                    streams.sort();
                    CompletionRequirement::BeforeNextTick {
                        after,
                        streams,
                        enforcement,
                    }
                }));
            }

            if matches!(
                schedule.presentation_lifetime,
                PresentationLifetimePolicy::BlockNextTickUntilViewsComplete
                    | PresentationLifetimePolicy::QueueOrderedReuse
            ) {
                let enforcement = match schedule.presentation_lifetime {
                    PresentationLifetimePolicy::BlockNextTickUntilViewsComplete => {
                        CompletionEnforcement::HostWait
                    }
                    PresentationLifetimePolicy::QueueOrderedReuse => {
                        CompletionEnforcement::SerialQueueOrder
                    }
                    PresentationLifetimePolicy::RequireResourceVersions => unreachable!(),
                };
                completion_requirements.extend(views.iter().filter_map(|view| {
                    let mut streams = view
                        .reads
                        .iter()
                        .map(|read| read.stream)
                        .filter(|stream| {
                            accesses
                                .iter()
                                .any(|access| access.stream == *stream && access.writes)
                        })
                        .collect::<Vec<_>>();
                    streams.sort();
                    streams.dedup();
                    (!streams.is_empty()).then_some(CompletionRequirement::BeforeNextTick {
                        after: ScheduleItemId::View(view.view),
                        streams,
                        enforcement,
                    })
                }));
            }

            completion_requirements.sort_by_cached_key(|requirement| {
                serde_json::to_vec(requirement).expect("completion requirement serialization")
            });
            Some(ExecutionSchedule {
                schedule: order.schedule,
                order: order.items.clone(),
                requested_ticks: concurrency.requested_ticks,
                effective_ticks: concurrency.effective_ticks,
                requested_render_frames: concurrency.requested_render_frames,
                effective_render_frames: concurrency.effective_render_frames,
                resource_versions: concurrency.resource_versions.clone(),
                passes,
                views,
                accesses,
                completion_requirements,
                dropped_presentation: DroppedPresentationPolicy::ReleaseUnsubmittedLeases,
            })
        })
        .collect();
    let scheduled_pass_ids = orders
        .iter()
        .flat_map(|order| order.items.iter())
        .filter_map(|item| match item {
            ScheduleItemId::Pass(pass) => Some(*pass),
            ScheduleItemId::View(_) => None,
        })
        .collect::<BTreeSet<_>>();
    let mut intervention_ids = graph
        .scenarios
        .iter()
        .flat_map(|scenario| {
            scenario
                .interventions
                .iter()
                .map(|intervention| intervention.pass)
        })
        .chain(
            graph
                .passes
                .iter()
                .filter(|pass| !scheduled_pass_ids.contains(&pass.id))
                .map(|pass| pass.id),
        )
        .collect::<Vec<_>>();
    intervention_ids.sort();
    intervention_ids.dedup();
    let intervention_passes = intervention_ids
        .into_iter()
        .map(|pass| plan_pass(graph, pass))
        .collect();
    ExecutionPlan {
        schema_version: 5,
        schedules,
        intervention_passes,
    }
}

fn plan_pass(graph: CheckedGraph<'_>, pass_id: pqo_core::PassId) -> PlannedPass {
    let pass = graph.pass(pass_id);
    let kernel = graph.kernel(pass.kernel);
    let implementation = kernel
        .implementations
        .iter()
        .filter(|implementation| implementation.backend == Backend::Metal)
        .min_by(|left, right| (&left.source, &left.entry).cmp(&(&right.source, &right.entry)))
        .expect("validated Metal kernel implementation")
        .clone();
    let mut bindings = pass.bindings.clone();
    bindings.sort_by_key(|binding| binding.slot);
    PlannedPass {
        pass: pass.id,
        kernel: kernel.id,
        bindings,
        dispatch: pass.dispatch.clone(),
        threads_per_threadgroup: pass.threads_per_threadgroup,
        abi: kernel.abi.clone(),
        implementation,
    }
}

fn artifact_fingerprint(graph: &ModuleGraph, plan: &ExecutionPlan) -> String {
    #[derive(Serialize)]
    struct ArtifactIdentity<'a> {
        validator_schema_version: u32,
        canonical_graph: &'a [u8],
        execution_plan: &'a ExecutionPlan,
    }

    let canonical_graph = canonicalize(graph);
    let bytes = serde_json::to_vec(&ArtifactIdentity {
        validator_schema_version: 3,
        canonical_graph: &canonical_graph.bytes,
        execution_plan: plan,
    })
    .expect("validated artifact identity serialization");
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn apply_edit(graph: &mut ModuleGraph, edit: &GraphEdit) -> Result<(), RepairError> {
    match edit {
        GraphEdit::SetStreamBuffering {
            stream,
            expected,
            versions,
        } => {
            let target = graph
                .resources
                .streams
                .get_mut(stream.0 as usize)
                .ok_or_else(|| RepairError::InvalidTarget {
                    target: SemanticPath::new(format!("streams.{}", stream.0)),
                })?;
            if target.buffering != *expected {
                return Err(RepairError::ExpectedValueMismatch {
                    target: path("streams", &target.name).child("buffering"),
                });
            }
            target.buffering = *versions;
        }
        GraphEdit::SetScheduleSimulationTicksInFlight {
            schedule,
            expected,
            ticks,
        } => {
            let target = graph
                .schedules
                .nodes
                .get_mut(schedule.0 as usize)
                .ok_or_else(|| RepairError::InvalidTarget {
                    target: SemanticPath::new(format!("schedules.{}", schedule.0)),
                })?;
            if target.in_flight.simulation_ticks != *expected {
                return Err(RepairError::ExpectedValueMismatch {
                    target: path("schedules", &target.name).child("in_flight.simulation_ticks"),
                });
            }
            target.in_flight.simulation_ticks = *ticks;
        }
        GraphEdit::SetTickOverlapPolicy {
            schedule,
            expected,
            policy,
        } => {
            let target = graph
                .schedules
                .nodes
                .get_mut(schedule.0 as usize)
                .ok_or_else(|| RepairError::InvalidTarget {
                    target: SemanticPath::new(format!("schedules.{}", schedule.0)),
                })?;
            if target.tick_overlap != *expected {
                return Err(RepairError::ExpectedValueMismatch {
                    target: path("schedules", &target.name).child("tick_overlap"),
                });
            }
            target.tick_overlap = policy.clone();
        }
        GraphEdit::AddCompletionDependency {
            schedule,
            before,
            after,
            expected_absent,
        } => {
            let target = graph
                .schedules
                .nodes
                .get_mut(schedule.0 as usize)
                .ok_or_else(|| RepairError::InvalidTarget {
                    target: SemanticPath::new(format!("schedules.{}", schedule.0)),
                })?;
            match (before, after) {
                (ScheduleItemId::Pass(before), ScheduleItemId::Pass(after)) => {
                    let exists = target
                        .execution_dependencies
                        .iter()
                        .any(|edge| edge.before == *before && edge.after == *after);
                    if exists == *expected_absent {
                        return Err(RepairError::ExpectedValueMismatch {
                            target: path("schedules", &target.name).child("execution_dependencies"),
                        });
                    }
                    target
                        .execution_dependencies
                        .push(pqo_core::ExecutionDependency {
                            before: *before,
                            after: *after,
                            semantics: pqo_core::DependencySemantics::Completion,
                        });
                    target
                        .execution_dependencies
                        .sort_by_key(|edge| (edge.before, edge.after));
                }
                (ScheduleItemId::Pass(producer), ScheduleItemId::View(consumer)) => {
                    let exists = target
                        .presentation_dependencies
                        .iter()
                        .any(|edge| edge.producer == *producer && edge.consumer == *consumer);
                    if exists == *expected_absent {
                        return Err(RepairError::ExpectedValueMismatch {
                            target: path("schedules", &target.name)
                                .child("presentation_dependencies"),
                        });
                    }
                    target
                        .presentation_dependencies
                        .push(pqo_core::PresentationDependency {
                            producer: *producer,
                            consumer: *consumer,
                            semantics: pqo_core::DependencySemantics::Completion,
                        });
                    target
                        .presentation_dependencies
                        .sort_by_key(|edge| (edge.producer, edge.consumer));
                }
                _ => return Err(RepairError::UnsupportedDependencyShape),
            }
        }
    }
    Ok(())
}

fn path(scope: &str, name: &str) -> SemanticPath {
    SemanticPath::new(format!("{scope}.{name}"))
}
