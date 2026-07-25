//! Independent deterministic validation passes for normalized Loom graphs.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use loom_core::{
    AliasingRule, Backend, CanonicalGraph, ContractClause, DataType, DeterminismScope,
    DeterminismTier, Diagnostic, DiagnosticCode, DispatchDomain, GraphEdit, KernelNode,
    ModuleGraph, ObservationPoint, Predicate, QueueModel, RenderOverloadPolicy,
    ReplayOverloadPolicy, ResourceId, ScenarioTimePolicy, ScheduleId, ScheduleItemId, SemanticPath,
    SlotAccess, SlotResourceType, StreamId, StreamLength, TickOverlapPolicy, Unit, ValueKind,
    ViewState, canonicalize,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationPass {
    Symbols,
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
    CanonicalFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduleOrder {
    pub schedule: ScheduleId,
    pub items: Vec<ScheduleItemId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveConcurrency {
    pub schedule: ScheduleId,
    pub requested_ticks: u32,
    pub effective_ticks: u32,
    pub basis: ConcurrencyBasis,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConcurrencyBasis {
    ResourceVersions,
    SerializedConflicts,
    ProvenSerialQueue,
    Invalid,
}

#[derive(Clone, Debug)]
pub struct ValidationReport {
    pub completed_passes: Vec<ValidationPass>,
    pub diagnostics: Vec<Diagnostic>,
    pub topological_orders: Vec<ScheduleOrder>,
    pub effective_concurrency: Vec<EffectiveConcurrency>,
    pub canonical: CanonicalGraph,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == loom_core::Severity::Error)
    }
}

pub struct Validator;

impl Validator {
    pub fn validate(graph: &ModuleGraph) -> ValidationReport {
        let mut diagnostics = Vec::new();
        let mut completed_passes = Vec::new();

        validate_symbols(graph, &mut diagnostics);
        completed_passes.push(ValidationPass::Symbols);

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

        let canonical = canonicalize(graph);
        completed_passes.push(ValidationPass::CanonicalFingerprint);

        diagnostics.sort_by_key(|diagnostic| (diagnostic.code.clone(), diagnostic.primary.clone()));

        ValidationReport {
            completed_passes,
            diagnostics,
            topological_orders,
            effective_concurrency,
            canonical,
        }
    }
}

fn validate_symbols(graph: &ModuleGraph, diagnostics: &mut Vec<Diagnostic>) {
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
}

fn check_ids_and_names<'a>(
    items: impl Iterator<Item = (u32, &'a String)>,
    scope: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut names = BTreeSet::new();
    for (expected, (actual, name)) in items.enumerate() {
        if actual != expected as u32 {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::UnknownSymbol,
                format!("non-canonical ID {actual}; expected {expected}"),
                path(scope, name),
            ));
        }
        if !names.insert(name) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DuplicateSymbol,
                format!("duplicate symbol `{name}`"),
                path(scope, name),
            ));
        }
    }
}

fn validate_types_shapes_units(graph: &ModuleGraph, diagnostics: &mut Vec<Diagnostic>) {
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
        let loom_core::ScheduleTiming::Fixed { fixed_dt, .. } = schedule.timing;
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
            if let ContractClause::Invariant {
                predicate:
                    Predicate::GroundClearance {
                        position,
                        radius,
                        ground_height,
                        tolerance,
                    },
                ..
            } = clause
            {
                validate_ground_clearance(
                    graph,
                    *position,
                    *radius,
                    *ground_height,
                    tolerance.unit,
                    &format!("contracts.{}", contract.name),
                    diagnostics,
                );
            }
        }
    }
}

fn validate_ground_clearance(
    graph: &ModuleGraph,
    position: StreamId,
    radius: StreamId,
    ground_height: loom_core::ValueId,
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
    pass: &loom_core::PassNode,
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

fn validate_resource_access(graph: &ModuleGraph, diagnostics: &mut Vec<Diagnostic>) {
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
                }
                _ => {}
            }
        }
    }
}

fn validate_capacity_length_dispatch(graph: &ModuleGraph, diagnostics: &mut Vec<Diagnostic>) {
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
            StreamLength::Dynamic(value) => {
                let count = graph.value(value);
                if count.data_type != DataType::u32()
                    || count.unit != Unit::DIMENSIONLESS
                    || !matches!(count.kind, ValueKind::DynamicCounter)
                {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::InvalidLogicalLength,
                        "dynamic stream length must use a dimensionless u32 counter resource",
                        path("streams", &stream.name).child("length"),
                    ));
                }
            }
            _ => {}
        }
    }

    for schedule in &graph.schedules {
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
        for binding in &pass.bindings {
            let ResourceId::Stream(stream) = binding.resource else {
                continue;
            };
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

fn validate_bindings_aliasing(graph: &ModuleGraph, diagnostics: &mut Vec<Diagnostic>) {
    for pass in &graph.passes {
        let kernel = graph.kernel(pass.kernel);
        let mut bound_slots = BTreeMap::<loom_core::SlotId, usize>::new();
        for binding in &pass.bindings {
            *bound_slots.entry(binding.slot).or_default() += 1;
        }
        for slot in &kernel.slots {
            match bound_slots.get(&slot.id).copied().unwrap_or_default() {
                0 => diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::MissingBinding,
                        format!("required slot `{}` is not bound", slot.name),
                        path("passes", &pass.name).child("bindings"),
                    )
                    .fix(GraphEdit::BindMissingSlot {
                        pass: pass.id,
                        slot_name: slot.name.clone(),
                    }),
                ),
                count if count > 1 => diagnostics.push(Diagnostic::error(
                    DiagnosticCode::DuplicateBinding,
                    format!("slot `{}` is bound {count} times", slot.name),
                    path("passes", &pass.name).child("bindings"),
                )),
                _ => {}
            }
        }

        let mut resources = BTreeMap::<StreamId, Vec<loom_core::SlotId>>::new();
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

fn alias_allowed(kernel: &KernelNode, left: loom_core::SlotId, right: loom_core::SlotId) -> bool {
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

fn construct_hazards(graph: &ModuleGraph) -> BTreeMap<ScheduleId, Vec<Access>> {
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
    graph: &ModuleGraph,
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

fn schedule_nodes(schedule: &loom_core::ScheduleNode) -> BTreeSet<ScheduleItemId> {
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

fn schedule_edges(
    schedule: &loom_core::ScheduleNode,
) -> BTreeSet<(ScheduleItemId, ScheduleItemId)> {
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
    graph: &ModuleGraph,
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
                    }),
                );
            }
        }
    }
}

fn validate_buffer_versions(
    graph: &ModuleGraph,
    accesses: &BTreeMap<ScheduleId, Vec<Access>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<EffectiveConcurrency> {
    let mut results = Vec::new();
    for schedule in &graph.schedules {
        let requested = schedule.in_flight.simulation_ticks;
        match schedule.tick_overlap {
            TickOverlapPolicy::RequireResourceVersions => {
                let required = requested.max(schedule.in_flight.render_frames);
                let mutated = accesses[&schedule.id]
                    .iter()
                    .filter(|access| access.writes)
                    .map(|access| access.stream)
                    .collect::<BTreeSet<_>>();
                let mut valid = true;
                for stream_id in mutated {
                    let stream = graph.stream(stream_id);
                    if stream.buffering < required {
                        valid = false;
                        diagnostics.push(
                            Diagnostic::error(
                                DiagnosticCode::InsufficientBufferVersions,
                                format!(
                                    "stream `{}` has {} version(s), but schedule `{}` permits {required} overlapping consumers",
                                    stream.name, stream.buffering, schedule.name
                                ),
                                path("streams", &stream.name).child("buffering"),
                            )
                            .related(
                                path("schedules", &schedule.name).child("in_flight"),
                            )
                            .fix(GraphEdit::SetStreamBuffering {
                                stream: stream.id,
                                versions: required,
                            }),
                        );
                    }
                }
                results.push(EffectiveConcurrency {
                    schedule: schedule.id,
                    requested_ticks: requested,
                    effective_ticks: if valid { requested } else { 0 },
                    basis: if valid {
                        ConcurrencyBasis::ResourceVersions
                    } else {
                        ConcurrencyBasis::Invalid
                    },
                });
            }
            TickOverlapPolicy::SerializeConflictingTicks => {
                results.push(EffectiveConcurrency {
                    schedule: schedule.id,
                    requested_ticks: requested,
                    effective_ticks: requested.min(1),
                    basis: ConcurrencyBasis::SerializedConflicts,
                });
            }
            TickOverlapPolicy::QueueOrderedReuse => {
                if schedule.queue_model == QueueModel::SingleSerialQueue {
                    results.push(EffectiveConcurrency {
                        schedule: schedule.id,
                        requested_ticks: requested,
                        effective_ticks: requested,
                        basis: ConcurrencyBasis::ProvenSerialQueue,
                    });
                } else {
                    diagnostics.push(
                        Diagnostic::error(
                            DiagnosticCode::UnprovenQueueReuse,
                            "queue-ordered reuse requires a single serial queue completion proof",
                            path("schedules", &schedule.name).child("queue_model"),
                        )
                        .fix(GraphEdit::SetTickOverlapPolicy {
                            schedule: schedule.id,
                            policy: TickOverlapPolicy::SerializeConflictingTicks,
                        }),
                    );
                    results.push(EffectiveConcurrency {
                        schedule: schedule.id,
                        requested_ticks: requested,
                        effective_ticks: 0,
                        basis: ConcurrencyBasis::Invalid,
                    });
                }
            }
        }

        for view_id in &schedule.presentation_views {
            let view = graph.view(*view_id);
            let required_history = match view.state {
                ViewState::CurrentCompletedTick => 1,
                ViewState::PreviousStableTick { lag } => lag.saturating_add(1),
                ViewState::Interpolated { older_lag, .. } => older_lag.saturating_add(1),
            };
            for read in &view.reads {
                let stream = graph.stream(read.stream);
                if stream.buffering < required_history {
                    diagnostics.push(
                        Diagnostic::error(
                            DiagnosticCode::InsufficientBufferVersions,
                            format!(
                                "view `{}` requires {required_history} historical versions of `{}`",
                                view.name, stream.name
                            ),
                            path("views", &view.name).child("state"),
                        )
                        .related(path("streams", &stream.name).child("buffering"))
                        .fix(GraphEdit::SetStreamBuffering {
                            stream: stream.id,
                            versions: required_history,
                        }),
                    );
                }
            }
        }
    }
    results
}

fn validate_backend_abi(graph: &ModuleGraph, diagnostics: &mut Vec<Diagnostic>) {
    for kernel in &graph.kernels {
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
        if let loom_core::ThreadgroupBehavior::Fixed { x, y, z } = kernel.abi.threadgroup
            && (x == 0 || y == 0 || z == 0)
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidKernelAbi,
                "fixed threadgroup dimensions must be positive",
                path("kernels", &kernel.name).child("abi.threadgroup"),
            ));
        }
        let has_backend = match graph.target {
            loom_core::Target::Metal => kernel.implementations.iter().any(|implementation| {
                implementation.backend == Backend::Metal
                    && !implementation.source.is_empty()
                    && !implementation.entry.is_empty()
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
}

fn validate_observation_points(graph: &ModuleGraph, diagnostics: &mut Vec<Diagnostic>) {
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
    graph: &ModuleGraph,
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

fn validate_determinism_overload(graph: &ModuleGraph, diagnostics: &mut Vec<Diagnostic>) {
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
            loom_core::ExcessWallTimePolicy::Discard
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
            loom_core::ExcessWallTimePolicy::Discard
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

fn path(scope: &str, name: &str) -> SemanticPath {
    SemanticPath::new(format!("{scope}.{name}"))
}
