use std::collections::{BTreeMap, BTreeSet};

use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, SemanticPath},
    ids::*,
    model::*,
};

#[derive(Clone, Debug)]
pub struct ModuleBuilder {
    name: String,
    target: Target,
    values: Vec<ValueDraft>,
    streams: Vec<StreamDraft>,
    kernels: Vec<KernelDraft>,
    passes: Vec<PassDraft>,
    views: Vec<ViewDraft>,
    schedules: Vec<ScheduleDraft>,
    contracts: Vec<ContractDraft>,
    scenarios: Vec<ScenarioDraft>,
    benchmarks: Vec<BenchmarkDraft>,
    capabilities: Vec<CapabilityDraft>,
}

impl ModuleBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            target: Target::Metal,
            values: Vec::new(),
            streams: Vec::new(),
            kernels: Vec::new(),
            passes: Vec::new(),
            views: Vec::new(),
            schedules: Vec::new(),
            contracts: Vec::new(),
            scenarios: Vec::new(),
            benchmarks: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    pub fn target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }

    pub fn value(mut self, value: ValueDraft) -> Self {
        self.values.push(value);
        self
    }

    pub fn stream(mut self, stream: StreamDraft) -> Self {
        self.streams.push(stream);
        self
    }

    pub fn kernel(mut self, kernel: KernelDraft) -> Self {
        self.kernels.push(kernel);
        self
    }

    pub fn pass(mut self, pass: PassDraft) -> Self {
        self.passes.push(pass);
        self
    }

    pub fn view(mut self, view: ViewDraft) -> Self {
        self.views.push(view);
        self
    }

    pub fn schedule(mut self, schedule: ScheduleDraft) -> Self {
        self.schedules.push(schedule);
        self
    }

    pub fn contract(mut self, contract: ContractDraft) -> Self {
        self.contracts.push(contract);
        self
    }

    pub fn scenario(mut self, scenario: ScenarioDraft) -> Self {
        self.scenarios.push(scenario);
        self
    }

    pub fn benchmark(mut self, benchmark: BenchmarkDraft) -> Self {
        self.benchmarks.push(benchmark);
        self
    }

    pub fn capability(mut self, capability: CapabilityDraft) -> Self {
        self.capabilities.push(capability);
        self
    }

    pub fn build(mut self) -> Result<ModuleGraph, Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        sort_by_name(&mut self.schedules, |item| &item.name);
        let schedule_ids = assign_ids(&self.schedules, |item| &item.name, ScheduleId);
        check_duplicate_names(
            &self.schedules,
            |item| &item.name,
            "schedules",
            &mut diagnostics,
        );

        for schedule in &self.schedules {
            self.values.push(ValueDraft {
                name: format!("{}.fixed_dt", schedule.name),
                data_type: DataType::f32(),
                unit: Unit::SECOND,
                kind: ValueDraftKind::ScheduleFixedDt(schedule.name.clone()),
            });
        }

        sort_by_name(&mut self.values, |item| &item.name);
        sort_by_name(&mut self.streams, |item| &item.name);
        sort_by_name(&mut self.kernels, |item| &item.name);
        sort_by_name(&mut self.passes, |item| &item.name);
        sort_by_name(&mut self.views, |item| &item.name);
        sort_by_name(&mut self.contracts, |item| &item.name);
        sort_by_name(&mut self.scenarios, |item| &item.name);
        sort_by_name(&mut self.benchmarks, |item| &item.name);
        sort_by_name(&mut self.capabilities, |item| &item.name);

        check_global_duplicates(&self, &mut diagnostics);
        check_duplicate_names(&self.values, |item| &item.name, "values", &mut diagnostics);
        check_duplicate_names(
            &self.streams,
            |item| &item.name,
            "streams",
            &mut diagnostics,
        );
        check_duplicate_names(
            &self.kernels,
            |item| &item.name,
            "kernels",
            &mut diagnostics,
        );
        check_duplicate_names(&self.passes, |item| &item.name, "passes", &mut diagnostics);
        check_duplicate_names(&self.views, |item| &item.name, "views", &mut diagnostics);
        check_duplicate_names(
            &self.contracts,
            |item| &item.name,
            "contracts",
            &mut diagnostics,
        );
        check_duplicate_names(
            &self.scenarios,
            |item| &item.name,
            "scenarios",
            &mut diagnostics,
        );
        check_duplicate_names(
            &self.benchmarks,
            |item| &item.name,
            "benchmarks",
            &mut diagnostics,
        );
        check_duplicate_names(
            &self.capabilities,
            |item| &item.name,
            "capabilities",
            &mut diagnostics,
        );

        if !diagnostics.is_empty() {
            diagnostics.sort_by_key(diagnostic_key);
            return Err(diagnostics);
        }

        let value_ids = assign_ids(&self.values, |item| &item.name, ValueId);
        let stream_ids = assign_ids(&self.streams, |item| &item.name, StreamId);
        let kernel_ids = assign_ids(&self.kernels, |item| &item.name, KernelId);
        let pass_ids = assign_ids(&self.passes, |item| &item.name, PassId);
        let view_ids = assign_ids(&self.views, |item| &item.name, ViewId);
        let contract_ids = assign_ids(&self.contracts, |item| &item.name, ContractId);
        let scenario_ids = assign_ids(&self.scenarios, |item| &item.name, ScenarioId);
        let benchmark_ids = assign_ids(&self.benchmarks, |item| &item.name, BenchmarkId);
        let capability_ids = assign_ids(&self.capabilities, |item| &item.name, CapabilityId);

        let values = self
            .values
            .iter()
            .map(|draft| {
                let kind = match &draft.kind {
                    ValueDraftKind::Constant(value) => ValueKind::Constant(value.clone()),
                    ValueDraftKind::ScheduleFixedDt(schedule) => {
                        let schedule =
                            resolve(&schedule_ids, schedule, "schedules", &mut diagnostics)?;
                        ValueKind::ScheduleFixedDt { schedule }
                    }
                    ValueDraftKind::DynamicCounter => ValueKind::DynamicCounter,
                };
                Some(ValueNode {
                    id: value_ids[&draft.name],
                    name: draft.name.clone(),
                    data_type: draft.data_type.clone(),
                    unit: draft.unit,
                    kind,
                })
            })
            .collect::<Option<Vec<_>>>();

        let streams = self
            .streams
            .iter()
            .map(|draft| {
                let length = match &draft.length {
                    StreamLengthDraft::Fixed(length) => StreamLength::Fixed(*length),
                    StreamLengthDraft::Dynamic(value) => {
                        let value = resolve(&value_ids, value, "values", &mut diagnostics)?;
                        StreamLength::Dynamic(value)
                    }
                };
                Some(StreamNode {
                    id: stream_ids[&draft.name],
                    name: draft.name.clone(),
                    element_type: draft.element_type.clone(),
                    unit: draft.unit,
                    capacity: draft.capacity,
                    length,
                    buffering: draft.buffering,
                    storage: draft.storage.clone(),
                    access: draft.access.clone(),
                    initial: draft.initial.clone(),
                })
            })
            .collect::<Option<Vec<_>>>();

        let mut slot_maps = BTreeMap::<KernelId, BTreeMap<String, SlotId>>::new();
        let kernels = self
            .kernels
            .iter()
            .map(|draft| {
                let kernel_id = kernel_ids[&draft.name];
                let mut slots = draft.slots.clone();
                sort_by_name(&mut slots, |item| &item.name);
                check_duplicate_names(
                    &slots,
                    |item| &item.name,
                    &format!("kernels.{}.slots", draft.name),
                    &mut diagnostics,
                );
                let slot_ids = assign_ids(&slots, |item| &item.name, SlotId);
                let binding_order = draft
                    .abi
                    .binding_order
                    .iter()
                    .filter_map(|name| {
                        resolve(
                            &slot_ids,
                            name,
                            &format!("kernels.{}.slots", draft.name),
                            &mut diagnostics,
                        )
                    })
                    .collect();
                let aliasing = match &draft.abi.aliasing {
                    AliasingDraft::Forbidden => AliasingRule::Forbidden,
                    AliasingDraft::AllowPairs(pairs) => {
                        let mut pairs = pairs
                            .iter()
                            .filter_map(|(left, right)| {
                                Some((
                                    resolve(
                                        &slot_ids,
                                        left,
                                        &format!("kernels.{}.slots", draft.name),
                                        &mut diagnostics,
                                    )?,
                                    resolve(
                                        &slot_ids,
                                        right,
                                        &format!("kernels.{}.slots", draft.name),
                                        &mut diagnostics,
                                    )?,
                                ))
                            })
                            .collect::<Vec<_>>();
                        pairs.sort();
                        AliasingRule::AllowPairs(pairs)
                    }
                };
                let nodes = slots
                    .into_iter()
                    .map(|slot| SlotNode {
                        id: slot_ids[&slot.name],
                        name: slot.name,
                        resource_type: slot.resource_type,
                        access: slot.access,
                    })
                    .collect();
                slot_maps.insert(kernel_id, slot_ids);
                let mut implementations = draft.implementations.clone();
                implementations.sort_by(|left, right| {
                    (&left.source, &left.entry).cmp(&(&right.source, &right.entry))
                });
                KernelNode {
                    id: kernel_id,
                    name: draft.name.clone(),
                    slots: nodes,
                    abi: KernelAbi {
                        binding_order,
                        dispatch_index: draft.abi.dispatch_index.clone(),
                        threadgroup: draft.abi.threadgroup.clone(),
                        aliasing,
                    },
                    implementations,
                }
            })
            .collect::<Vec<_>>();

        let passes = self
            .passes
            .iter()
            .filter_map(|draft| {
                let kernel = resolve(&kernel_ids, &draft.kernel, "kernels", &mut diagnostics)?;
                let slots = &slot_maps[&kernel];
                let mut bindings = draft.bindings.clone();
                bindings.sort_by(|left, right| left.slot.cmp(&right.slot));
                let bindings = bindings
                    .iter()
                    .filter_map(|binding| {
                        let slot = resolve(
                            slots,
                            &binding.slot,
                            &format!("kernels.{}.slots", draft.kernel),
                            &mut diagnostics,
                        )?;
                        let resource = resolve_resource(
                            &binding.resource,
                            &value_ids,
                            &stream_ids,
                            &mut diagnostics,
                        )?;
                        Some(Binding { slot, resource })
                    })
                    .collect();
                let dispatch = match &draft.dispatch {
                    DispatchDraft::OverStream(name) => DispatchDomain::OverStream(resolve(
                        &stream_ids,
                        name,
                        "streams",
                        &mut diagnostics,
                    )?),
                    DispatchDraft::Fixed(count) => DispatchDomain::Fixed(*count),
                };
                Some(PassNode {
                    id: pass_ids[&draft.name],
                    name: draft.name.clone(),
                    kernel,
                    bindings,
                    dispatch,
                })
            })
            .collect::<Vec<_>>();

        let views = self
            .views
            .iter()
            .map(|draft| {
                let mut reads = draft.reads.clone();
                reads.sort_by(|left, right| left.name.cmp(&right.name));
                let reads = reads
                    .iter()
                    .filter_map(|read| {
                        Some(ViewRead {
                            name: read.name.clone(),
                            stream: resolve(
                                &stream_ids,
                                &read.stream,
                                "streams",
                                &mut diagnostics,
                            )?,
                        })
                    })
                    .collect();
                ViewNode {
                    id: view_ids[&draft.name],
                    name: draft.name.clone(),
                    reads,
                    state: draft.state.clone(),
                    implementation: draft.implementation.clone(),
                }
            })
            .collect::<Vec<_>>();

        let schedules = self
            .schedules
            .iter()
            .map(|draft| {
                let id = schedule_ids[&draft.name];
                let fixed_dt = value_ids[&format!("{}.fixed_dt", draft.name)];
                let mut execution_passes = draft
                    .runs
                    .iter()
                    .filter_map(|name| resolve(&pass_ids, name, "passes", &mut diagnostics))
                    .collect::<Vec<_>>();
                execution_passes.sort();
                execution_passes.dedup();
                let mut presentation_views = draft
                    .shows
                    .iter()
                    .filter_map(|name| resolve(&view_ids, name, "views", &mut diagnostics))
                    .collect::<Vec<_>>();
                presentation_views.sort();
                presentation_views.dedup();
                let mut execution_dependencies = draft
                    .execution_dependencies
                    .iter()
                    .filter_map(|dependency| {
                        Some(ExecutionDependency {
                            before: resolve(
                                &pass_ids,
                                &dependency.before,
                                "passes",
                                &mut diagnostics,
                            )?,
                            after: resolve(
                                &pass_ids,
                                &dependency.after,
                                "passes",
                                &mut diagnostics,
                            )?,
                            semantics: DependencySemantics::Completion,
                        })
                    })
                    .collect::<Vec<_>>();
                execution_dependencies.sort_by_key(|edge| (edge.before, edge.after));
                let mut presentation_dependencies = draft
                    .presentation_dependencies
                    .iter()
                    .filter_map(|dependency| {
                        Some(PresentationDependency {
                            producer: resolve(
                                &pass_ids,
                                &dependency.producer,
                                "passes",
                                &mut diagnostics,
                            )?,
                            consumer: resolve(
                                &view_ids,
                                &dependency.consumer,
                                "views",
                                &mut diagnostics,
                            )?,
                            semantics: DependencySemantics::Completion,
                        })
                    })
                    .collect::<Vec<_>>();
                presentation_dependencies.sort_by_key(|edge| (edge.producer, edge.consumer));
                ScheduleNode {
                    id,
                    name: draft.name.clone(),
                    timing: ScheduleTiming::Fixed {
                        rate_hz: draft.rate_hz,
                        fixed_dt,
                    },
                    execution_passes,
                    presentation_views,
                    execution_dependencies,
                    presentation_dependencies,
                    in_flight: draft.in_flight.clone(),
                    tick_overlap: draft.tick_overlap.clone(),
                    presentation_lifetime: draft.presentation_lifetime.clone(),
                    queue_model: draft.queue_model.clone(),
                    overload: draft.overload.clone(),
                }
            })
            .collect::<Vec<_>>();

        let contracts = self
            .contracts
            .iter()
            .filter_map(|draft| {
                let schedule = resolve(
                    &schedule_ids,
                    &draft.schedule,
                    "schedules",
                    &mut diagnostics,
                )?;
                let mut clauses = draft
                    .clauses
                    .iter()
                    .filter_map(|clause| {
                        resolve_contract_clause(
                            clause,
                            &pass_ids,
                            &schedule_ids,
                            &stream_ids,
                            &value_ids,
                            &mut diagnostics,
                        )
                    })
                    .collect::<Vec<_>>();
                sort_serializable(&mut clauses);
                Some(ContractNode {
                    id: contract_ids[&draft.name],
                    name: draft.name.clone(),
                    schedule,
                    clauses,
                })
            })
            .collect::<Vec<_>>();

        let scenarios = self
            .scenarios
            .iter()
            .filter_map(|draft| {
                let schedule = resolve(
                    &schedule_ids,
                    &draft.schedule,
                    "schedules",
                    &mut diagnostics,
                )?;
                let mut expectations = draft
                    .expectations
                    .iter()
                    .filter_map(|expectation| {
                        Some(ScenarioExpectation {
                            observation: resolve_observation(
                                &expectation.observation,
                                &pass_ids,
                                &schedule_ids,
                                &mut diagnostics,
                            )?,
                            predicate: resolve_predicate(
                                &expectation.predicate,
                                &stream_ids,
                                &value_ids,
                                &mut diagnostics,
                            )?,
                        })
                    })
                    .collect::<Vec<_>>();
                sort_serializable(&mut expectations);
                Some(ScenarioNode {
                    id: scenario_ids[&draft.name],
                    name: draft.name.clone(),
                    schedule,
                    duration: ScenarioDuration::SimulationTicks(draft.duration_ticks),
                    expectations,
                })
            })
            .collect::<Vec<_>>();

        let benchmarks = self
            .benchmarks
            .iter()
            .filter_map(|draft| {
                let mut metrics = draft.metrics.clone();
                sort_serializable(&mut metrics);
                metrics.dedup();
                Some(BenchmarkNode {
                    id: benchmark_ids[&draft.name],
                    name: draft.name.clone(),
                    schedule: resolve(
                        &schedule_ids,
                        &draft.schedule,
                        "schedules",
                        &mut diagnostics,
                    )?,
                    warmup_ticks: draft.warmup_ticks,
                    measured_ticks: draft.measured_ticks,
                    metrics,
                })
            })
            .collect::<Vec<_>>();

        let capabilities = self
            .capabilities
            .iter()
            .map(|draft| {
                let kind = match &draft.kind {
                    CapabilityDraftKind::Inspect {
                        streams,
                        delivery,
                        snapshot,
                    } => {
                        let mut streams = streams
                            .iter()
                            .filter_map(|name| {
                                resolve(&stream_ids, name, "streams", &mut diagnostics)
                            })
                            .collect::<Vec<_>>();
                        streams.sort();
                        streams.dedup();
                        CapabilityKind::Inspect {
                            streams,
                            delivery: delivery.clone(),
                            snapshot: snapshot.clone(),
                        }
                    }
                    CapabilityDraftKind::HostMutate { streams } => {
                        let mut streams = streams
                            .iter()
                            .filter_map(|name| {
                                resolve(&stream_ids, name, "streams", &mut diagnostics)
                            })
                            .collect::<Vec<_>>();
                        streams.sort();
                        streams.dedup();
                        CapabilityKind::HostMutate { streams }
                    }
                    CapabilityDraftKind::External { name } => {
                        CapabilityKind::External { name: name.clone() }
                    }
                };
                CapabilityNode {
                    id: capability_ids[&draft.name],
                    name: draft.name.clone(),
                    kind,
                }
            })
            .collect::<Vec<_>>();

        if !diagnostics.is_empty() {
            diagnostics.sort_by_key(diagnostic_key);
            return Err(diagnostics);
        }

        Ok(ModuleGraph {
            schema_version: 1,
            name: self.name,
            target: self.target,
            resources: ResourceGraph {
                values: values.expect("value resolution checked"),
                streams: streams.expect("stream resolution checked"),
            },
            kernels: KernelGraph { nodes: kernels },
            passes: PassGraph { nodes: passes },
            views,
            schedules: ScheduleGraph { nodes: schedules },
            contracts,
            scenarios,
            benchmarks,
            capabilities,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ValueDraft {
    pub name: String,
    pub data_type: DataType,
    pub unit: Unit,
    pub kind: ValueDraftKind,
}

impl ValueDraft {
    pub fn constant(
        name: impl Into<String>,
        data_type: DataType,
        unit: Unit,
        value: Literal,
    ) -> Self {
        Self {
            name: name.into(),
            data_type,
            unit,
            kind: ValueDraftKind::Constant(value),
        }
    }

    pub fn dynamic_counter(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: DataType::u32(),
            unit: Unit::DIMENSIONLESS,
            kind: ValueDraftKind::DynamicCounter,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ValueDraftKind {
    Constant(Literal),
    ScheduleFixedDt(String),
    DynamicCounter,
}

#[derive(Clone, Debug)]
pub struct StreamDraft {
    pub name: String,
    pub element_type: DataType,
    pub unit: Unit,
    pub capacity: u32,
    pub length: StreamLengthDraft,
    pub buffering: u32,
    pub storage: StorageClass,
    pub access: ResourceAccess,
    pub initial: Option<Literal>,
}

impl StreamDraft {
    pub fn new(name: impl Into<String>, element_type: DataType, unit: Unit) -> Self {
        Self {
            name: name.into(),
            element_type,
            unit,
            capacity: 1,
            length: StreamLengthDraft::Fixed(1),
            buffering: 1,
            storage: StorageClass::DevicePrivate,
            access: ResourceAccess::DeviceReadWrite,
            initial: None,
        }
    }

    pub fn capacity(mut self, capacity: u32) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn length(mut self, length: u32) -> Self {
        self.length = StreamLengthDraft::Fixed(length);
        self
    }

    pub fn dynamic_length(mut self, value: impl Into<String>) -> Self {
        self.length = StreamLengthDraft::Dynamic(value.into());
        self
    }

    pub fn buffering(mut self, buffering: u32) -> Self {
        self.buffering = buffering;
        self
    }

    pub fn storage(mut self, storage: StorageClass) -> Self {
        self.storage = storage;
        self
    }

    pub fn access(mut self, access: ResourceAccess) -> Self {
        self.access = access;
        self
    }

    pub fn initial(mut self, initial: Literal) -> Self {
        self.initial = Some(initial);
        self
    }
}

#[derive(Clone, Debug)]
pub enum StreamLengthDraft {
    Fixed(u32),
    Dynamic(String),
}

#[derive(Clone, Debug)]
pub struct SlotDraft {
    pub name: String,
    pub resource_type: SlotResourceType,
    pub access: SlotAccess,
}

impl SlotDraft {
    pub fn stream(
        name: impl Into<String>,
        element_type: DataType,
        unit: Unit,
        access: SlotAccess,
    ) -> Self {
        Self {
            name: name.into(),
            resource_type: SlotResourceType::Stream { element_type, unit },
            access,
        }
    }

    pub fn value(name: impl Into<String>, data_type: DataType, unit: Unit) -> Self {
        Self {
            name: name.into(),
            resource_type: SlotResourceType::Value { data_type, unit },
            access: SlotAccess::Read,
        }
    }
}

#[derive(Clone, Debug)]
pub struct KernelDraft {
    pub name: String,
    pub slots: Vec<SlotDraft>,
    pub abi: KernelAbiDraft,
    pub implementations: Vec<BackendImplementation>,
}

impl KernelDraft {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            slots: Vec::new(),
            abi: KernelAbiDraft::new(Vec::<String>::new()),
            implementations: Vec::new(),
        }
    }

    pub fn slot(mut self, slot: SlotDraft) -> Self {
        self.slots.push(slot);
        self
    }

    pub fn abi(mut self, abi: KernelAbiDraft) -> Self {
        self.abi = abi;
        self
    }

    pub fn implementation(mut self, implementation: BackendImplementation) -> Self {
        self.implementations.push(implementation);
        self
    }
}

#[derive(Clone, Debug)]
pub struct KernelAbiDraft {
    pub binding_order: Vec<String>,
    pub dispatch_index: DispatchIndex,
    pub threadgroup: ThreadgroupBehavior,
    pub aliasing: AliasingDraft,
}

impl KernelAbiDraft {
    pub fn new<I, S>(binding_order: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            binding_order: binding_order.into_iter().map(Into::into).collect(),
            dispatch_index: DispatchIndex::GlobalLinearU32,
            threadgroup: ThreadgroupBehavior::BackendDerived,
            aliasing: AliasingDraft::Forbidden,
        }
    }

    pub fn threadgroup(mut self, behavior: ThreadgroupBehavior) -> Self {
        self.threadgroup = behavior;
        self
    }

    pub fn allow_alias(mut self, left: impl Into<String>, right: impl Into<String>) -> Self {
        match &mut self.aliasing {
            AliasingDraft::Forbidden => {
                self.aliasing = AliasingDraft::AllowPairs(vec![(left.into(), right.into())])
            }
            AliasingDraft::AllowPairs(pairs) => pairs.push((left.into(), right.into())),
        }
        self
    }
}

#[derive(Clone, Debug)]
pub enum AliasingDraft {
    Forbidden,
    AllowPairs(Vec<(String, String)>),
}

pub fn metal_implementation(
    source: impl Into<String>,
    entry: impl Into<String>,
) -> BackendImplementation {
    BackendImplementation {
        backend: Backend::Metal,
        source: source.into(),
        entry: entry.into(),
    }
}

#[derive(Clone, Debug)]
pub struct PassDraft {
    pub name: String,
    pub kernel: String,
    pub bindings: Vec<BindingDraft>,
    pub dispatch: DispatchDraft,
}

impl PassDraft {
    pub fn new(name: impl Into<String>, kernel: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kernel: kernel.into(),
            bindings: Vec::new(),
            dispatch: DispatchDraft::Fixed(1),
        }
    }

    pub fn bind(mut self, slot: impl Into<String>, resource: impl Into<String>) -> Self {
        self.bindings.push(BindingDraft {
            slot: slot.into(),
            resource: resource.into(),
        });
        self
    }

    pub fn dispatch_over(mut self, stream: impl Into<String>) -> Self {
        self.dispatch = DispatchDraft::OverStream(stream.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct BindingDraft {
    pub slot: String,
    pub resource: String,
}

#[derive(Clone, Debug)]
pub enum DispatchDraft {
    OverStream(String),
    Fixed(u32),
}

#[derive(Clone, Debug)]
pub struct ViewDraft {
    pub name: String,
    pub reads: Vec<ViewReadDraft>,
    pub state: ViewState,
    pub implementation: BackendImplementation,
}

impl ViewDraft {
    pub fn render(name: impl Into<String>, implementation: BackendImplementation) -> Self {
        Self {
            name: name.into(),
            reads: Vec::new(),
            state: ViewState::CurrentCompletedTick,
            implementation,
        }
    }

    pub fn read(mut self, name: impl Into<String>, stream: impl Into<String>) -> Self {
        self.reads.push(ViewReadDraft {
            name: name.into(),
            stream: stream.into(),
        });
        self
    }

    pub fn state(mut self, state: ViewState) -> Self {
        self.state = state;
        self
    }
}

#[derive(Clone, Debug)]
pub struct ViewReadDraft {
    pub name: String,
    pub stream: String,
}

#[derive(Clone, Debug)]
pub struct ScheduleDraft {
    pub name: String,
    pub rate_hz: u32,
    pub runs: Vec<String>,
    pub shows: Vec<String>,
    pub execution_dependencies: Vec<ExecutionDependencyDraft>,
    pub presentation_dependencies: Vec<PresentationDependencyDraft>,
    pub in_flight: InFlightPolicy,
    pub tick_overlap: TickOverlapPolicy,
    pub presentation_lifetime: PresentationLifetimePolicy,
    pub queue_model: QueueModel,
    pub overload: OverloadPolicy,
}

impl ScheduleDraft {
    pub fn fixed(name: impl Into<String>, rate_hz: u32) -> Self {
        Self {
            name: name.into(),
            rate_hz,
            runs: Vec::new(),
            shows: Vec::new(),
            execution_dependencies: Vec::new(),
            presentation_dependencies: Vec::new(),
            in_flight: InFlightPolicy {
                simulation_ticks: 1,
                render_frames: 1,
            },
            tick_overlap: TickOverlapPolicy::RequireResourceVersions,
            presentation_lifetime: PresentationLifetimePolicy::RequireResourceVersions,
            queue_model: QueueModel::Unproven,
            overload: OverloadPolicy {
                catch_up_limit: 1,
                excess_wall_time: ExcessWallTimePolicy::Retain,
                simulation_time: SimulationTimePolicy::AdvanceExecutedTicksOnly,
                scenario_time: ScenarioTimePolicy::SimulationTicks,
                replay: ReplayOverloadPolicy::RecordDecisions,
                rendering: RenderOverloadPolicy::DropPresentationOnly,
            },
        }
    }

    pub fn run(mut self, pass: impl Into<String>) -> Self {
        self.runs.push(pass.into());
        self
    }

    pub fn run_after(mut self, pass: impl Into<String>, predecessor: impl Into<String>) -> Self {
        let pass = pass.into();
        self.runs.push(pass.clone());
        self.execution_dependencies.push(ExecutionDependencyDraft {
            before: predecessor.into(),
            after: pass,
        });
        self
    }

    pub fn show_after(mut self, view: impl Into<String>, producer: impl Into<String>) -> Self {
        let view = view.into();
        self.shows.push(view.clone());
        self.presentation_dependencies
            .push(PresentationDependencyDraft {
                producer: producer.into(),
                consumer: view,
            });
        self
    }

    pub fn in_flight(mut self, simulation_ticks: u32, render_frames: u32) -> Self {
        self.in_flight = InFlightPolicy {
            simulation_ticks,
            render_frames,
        };
        self
    }

    pub fn tick_overlap(mut self, policy: TickOverlapPolicy) -> Self {
        self.tick_overlap = policy;
        self
    }

    pub fn presentation_lifetime(mut self, policy: PresentationLifetimePolicy) -> Self {
        self.presentation_lifetime = policy;
        self
    }

    pub fn queue_model(mut self, model: QueueModel) -> Self {
        self.queue_model = model;
        self
    }

    pub fn overload(mut self, policy: OverloadPolicy) -> Self {
        self.overload = policy;
        self
    }
}

#[derive(Clone, Debug)]
pub struct ExecutionDependencyDraft {
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug)]
pub struct PresentationDependencyDraft {
    pub producer: String,
    pub consumer: String,
}

#[derive(Clone, Debug)]
pub struct ContractDraft {
    pub name: String,
    pub schedule: String,
    pub clauses: Vec<ContractClauseDraft>,
}

impl ContractDraft {
    pub fn new(name: impl Into<String>, schedule: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            schedule: schedule.into(),
            clauses: Vec::new(),
        }
    }

    pub fn clause(mut self, clause: ContractClauseDraft) -> Self {
        self.clauses.push(clause);
        self
    }
}

#[derive(Clone, Debug)]
pub enum ContractClauseDraft {
    Invariant {
        observation: ObservationDraft,
        predicate: PredicateDraft,
    },
    MetricLimit {
        observation: ObservationDraft,
        metric: Metric,
        maximum: Quantity,
    },
    SteadyStateZero {
        observation: ObservationDraft,
        metric: Metric,
        excludes_requested_inspection: bool,
    },
    Determinism(DeterminismContract),
}

#[derive(Clone, Debug)]
pub enum ObservationDraft {
    AfterPassCompletion(String),
    AfterEveryPassCompletion(String),
    AfterTickExecution(String),
    AfterGpuCompletion(String),
}

#[derive(Clone, Debug)]
pub enum PredicateDraft {
    FiniteStreams(Vec<String>),
    GroundClearance {
        position: String,
        radius: String,
        ground_height: String,
        tolerance: Quantity,
    },
}

#[derive(Clone, Debug)]
pub struct ScenarioDraft {
    pub name: String,
    pub schedule: String,
    pub duration_ticks: u64,
    pub expectations: Vec<ScenarioExpectationDraft>,
}

impl ScenarioDraft {
    pub fn new(name: impl Into<String>, schedule: impl Into<String>, duration_ticks: u64) -> Self {
        Self {
            name: name.into(),
            schedule: schedule.into(),
            duration_ticks,
            expectations: Vec::new(),
        }
    }

    pub fn expect(mut self, observation: ObservationDraft, predicate: PredicateDraft) -> Self {
        self.expectations.push(ScenarioExpectationDraft {
            observation,
            predicate,
        });
        self
    }
}

#[derive(Clone, Debug)]
pub struct ScenarioExpectationDraft {
    pub observation: ObservationDraft,
    pub predicate: PredicateDraft,
}

#[derive(Clone, Debug)]
pub struct BenchmarkDraft {
    pub name: String,
    pub schedule: String,
    pub warmup_ticks: u64,
    pub measured_ticks: u64,
    pub metrics: Vec<Metric>,
}

impl BenchmarkDraft {
    pub fn new(name: impl Into<String>, schedule: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            schedule: schedule.into(),
            warmup_ticks: 0,
            measured_ticks: 1,
            metrics: Vec::new(),
        }
    }

    pub fn ticks(mut self, warmup_ticks: u64, measured_ticks: u64) -> Self {
        self.warmup_ticks = warmup_ticks;
        self.measured_ticks = measured_ticks;
        self
    }

    pub fn measure(mut self, metric: Metric) -> Self {
        self.metrics.push(metric);
        self
    }
}

#[derive(Clone, Debug)]
pub struct CapabilityDraft {
    pub name: String,
    pub kind: CapabilityDraftKind,
}

impl CapabilityDraft {
    pub fn inspect<I, S>(name: impl Into<String>, streams: I, snapshot: SnapshotSemantics) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            name: name.into(),
            kind: CapabilityDraftKind::Inspect {
                streams: streams.into_iter().map(Into::into).collect(),
                delivery: InspectionDelivery::Asynchronous,
                snapshot,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum CapabilityDraftKind {
    Inspect {
        streams: Vec<String>,
        delivery: InspectionDelivery,
        snapshot: SnapshotSemantics,
    },
    HostMutate {
        streams: Vec<String>,
    },
    External {
        name: String,
    },
}

fn resolve_contract_clause(
    draft: &ContractClauseDraft,
    pass_ids: &BTreeMap<String, PassId>,
    schedule_ids: &BTreeMap<String, ScheduleId>,
    stream_ids: &BTreeMap<String, StreamId>,
    value_ids: &BTreeMap<String, ValueId>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ContractClause> {
    Some(match draft {
        ContractClauseDraft::Invariant {
            observation,
            predicate,
        } => ContractClause::Invariant {
            observation: resolve_observation(observation, pass_ids, schedule_ids, diagnostics)?,
            predicate: resolve_predicate(predicate, stream_ids, value_ids, diagnostics)?,
        },
        ContractClauseDraft::MetricLimit {
            observation,
            metric,
            maximum,
        } => ContractClause::MetricLimit {
            observation: resolve_observation(observation, pass_ids, schedule_ids, diagnostics)?,
            metric: metric.clone(),
            maximum: maximum.clone(),
        },
        ContractClauseDraft::SteadyStateZero {
            observation,
            metric,
            excludes_requested_inspection,
        } => ContractClause::SteadyStateZero {
            observation: resolve_observation(observation, pass_ids, schedule_ids, diagnostics)?,
            metric: metric.clone(),
            excludes_requested_inspection: *excludes_requested_inspection,
        },
        ContractClauseDraft::Determinism(contract) => ContractClause::Determinism(contract.clone()),
    })
}

fn resolve_observation(
    draft: &ObservationDraft,
    pass_ids: &BTreeMap<String, PassId>,
    schedule_ids: &BTreeMap<String, ScheduleId>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ObservationPoint> {
    Some(match draft {
        ObservationDraft::AfterPassCompletion(name) => {
            ObservationPoint::AfterPassCompletion(resolve(pass_ids, name, "passes", diagnostics)?)
        }
        ObservationDraft::AfterEveryPassCompletion(name) => {
            ObservationPoint::AfterEveryPassCompletion(resolve(
                schedule_ids,
                name,
                "schedules",
                diagnostics,
            )?)
        }
        ObservationDraft::AfterTickExecution(name) => ObservationPoint::AfterTickExecution(
            resolve(schedule_ids, name, "schedules", diagnostics)?,
        ),
        ObservationDraft::AfterGpuCompletion(name) => ObservationPoint::AfterGpuCompletion(
            resolve(schedule_ids, name, "schedules", diagnostics)?,
        ),
    })
}

fn resolve_predicate(
    draft: &PredicateDraft,
    stream_ids: &BTreeMap<String, StreamId>,
    value_ids: &BTreeMap<String, ValueId>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Predicate> {
    Some(match draft {
        PredicateDraft::FiniteStreams(streams) => Predicate::FiniteStreams(
            streams
                .iter()
                .filter_map(|name| resolve(stream_ids, name, "streams", diagnostics))
                .collect(),
        ),
        PredicateDraft::GroundClearance {
            position,
            radius,
            ground_height,
            tolerance,
        } => Predicate::GroundClearance {
            position: resolve(stream_ids, position, "streams", diagnostics)?,
            radius: resolve(stream_ids, radius, "streams", diagnostics)?,
            ground_height: resolve(value_ids, ground_height, "values", diagnostics)?,
            tolerance: tolerance.clone(),
        },
    })
}

fn resolve_resource(
    name: &str,
    values: &BTreeMap<String, ValueId>,
    streams: &BTreeMap<String, StreamId>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResourceId> {
    match (values.get(name), streams.get(name)) {
        (Some(value), None) => Some(ResourceId::Value(*value)),
        (None, Some(stream)) => Some(ResourceId::Stream(*stream)),
        _ => {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::UnknownSymbol,
                format!("resource `{name}` is not defined"),
                SemanticPath::new(name),
            ));
            None
        }
    }
}

fn resolve<T: Copy>(
    symbols: &BTreeMap<String, T>,
    name: &str,
    scope: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<T> {
    symbols.get(name).copied().or_else(|| {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::UnknownSymbol,
            format!("`{name}` is not defined in {scope}"),
            SemanticPath::new(format!("{scope}.{name}")),
        ));
        None
    })
}

fn sort_by_name<T, F>(items: &mut [T], name: F)
where
    F: Fn(&T) -> &String,
{
    items.sort_by(|left, right| name(left).cmp(name(right)));
}

fn assign_ids<T, I, F, C>(items: &[T], name: F, constructor: C) -> BTreeMap<String, I>
where
    F: Fn(&T) -> &String,
    C: Fn(u32) -> I,
{
    items
        .iter()
        .enumerate()
        .map(|(index, item)| (name(item).clone(), constructor(index as u32)))
        .collect()
}

fn check_duplicate_names<T, F>(items: &[T], name: F, scope: &str, diagnostics: &mut Vec<Diagnostic>)
where
    F: Fn(&T) -> &String,
{
    let mut seen = BTreeSet::new();
    for item in items {
        if !seen.insert(name(item)) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DuplicateSymbol,
                format!("duplicate declaration `{}`", name(item)),
                SemanticPath::new(format!("{scope}.{}", name(item))),
            ));
        }
    }
}

fn check_global_duplicates(builder: &ModuleBuilder, diagnostics: &mut Vec<Diagnostic>) {
    let mut symbols = BTreeMap::<String, &'static str>::new();
    let groups = [
        (
            "values",
            builder
                .values
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            "streams",
            builder
                .streams
                .iter()
                .map(|item| item.name.as_str())
                .collect(),
        ),
        (
            "kernels",
            builder
                .kernels
                .iter()
                .map(|item| item.name.as_str())
                .collect(),
        ),
        (
            "passes",
            builder
                .passes
                .iter()
                .map(|item| item.name.as_str())
                .collect(),
        ),
        (
            "views",
            builder
                .views
                .iter()
                .map(|item| item.name.as_str())
                .collect(),
        ),
        (
            "schedules",
            builder
                .schedules
                .iter()
                .map(|item| item.name.as_str())
                .collect(),
        ),
        (
            "contracts",
            builder
                .contracts
                .iter()
                .map(|item| item.name.as_str())
                .collect(),
        ),
        (
            "scenarios",
            builder
                .scenarios
                .iter()
                .map(|item| item.name.as_str())
                .collect(),
        ),
        (
            "benchmarks",
            builder
                .benchmarks
                .iter()
                .map(|item| item.name.as_str())
                .collect(),
        ),
        (
            "capabilities",
            builder
                .capabilities
                .iter()
                .map(|item| item.name.as_str())
                .collect(),
        ),
    ];
    for (kind, names) in groups {
        for name in names {
            if let Some(previous) = symbols.insert(name.to_owned(), kind) {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::DuplicateSymbol,
                        format!("`{name}` is declared as both {previous} and {kind}"),
                        SemanticPath::new(format!("{kind}.{name}")),
                    )
                    .related(SemanticPath::new(format!("{previous}.{name}"))),
                );
            }
        }
    }
}

fn diagnostic_key(diagnostic: &Diagnostic) -> (DiagnosticCode, SemanticPath) {
    (diagnostic.code.clone(), diagnostic.primary.clone())
}

fn sort_serializable<T: serde::Serialize>(items: &mut [T]) {
    items.sort_by_cached_key(|item| {
        serde_json::to_vec(item).expect("canonical draft item serialization")
    });
}
