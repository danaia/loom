use serde::{Deserialize, Serialize};
use std::ops::Deref;

use crate::ids::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGraph {
    pub schema_version: u32,
    pub name: String,
    pub target: Target,
    pub resources: ResourceGraph,
    pub kernels: KernelGraph,
    pub passes: PassGraph,
    pub views: Vec<ViewNode>,
    pub schedules: ScheduleGraph,
    pub contracts: Vec<ContractNode>,
    pub scenarios: Vec<ScenarioNode>,
    pub benchmarks: Vec<BenchmarkNode>,
    pub capabilities: Vec<CapabilityNode>,
}

impl ModuleGraph {
    pub fn value(&self, id: ValueId) -> Option<&ValueNode> {
        self.resources.values.get(id.0 as usize)
    }

    pub fn stream(&self, id: StreamId) -> Option<&StreamNode> {
        self.resources.streams.get(id.0 as usize)
    }

    pub fn kernel(&self, id: KernelId) -> Option<&KernelNode> {
        self.kernels.get(id.0 as usize)
    }

    pub fn pass(&self, id: PassId) -> Option<&PassNode> {
        self.passes.get(id.0 as usize)
    }

    pub fn view(&self, id: ViewId) -> Option<&ViewNode> {
        self.views.get(id.0 as usize)
    }

    pub fn schedule(&self, id: ScheduleId) -> Option<&ScheduleNode> {
        self.schedules.get(id.0 as usize)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Metal,
    Cuda,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceGraph {
    pub values: Vec<ValueNode>,
    pub streams: Vec<StreamNode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelGraph {
    pub nodes: Vec<KernelNode>,
}

impl Deref for KernelGraph {
    type Target = [KernelNode];

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl<'a> IntoIterator for &'a KernelGraph {
    type Item = &'a KernelNode;
    type IntoIter = std::slice::Iter<'a, KernelNode>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.iter()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassGraph {
    pub nodes: Vec<PassNode>,
}

impl Deref for PassGraph {
    type Target = [PassNode];

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl<'a> IntoIterator for &'a PassGraph {
    type Item = &'a PassNode;
    type IntoIter = std::slice::Iter<'a, PassNode>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.iter()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleGraph {
    pub nodes: Vec<ScheduleNode>,
}

impl Deref for ScheduleGraph {
    type Target = [ScheduleNode];

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl<'a> IntoIterator for &'a ScheduleGraph {
    type Item = &'a ScheduleNode;
    type IntoIter = std::slice::Iter<'a, ScheduleNode>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.iter()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScalarType {
    Bool,
    I32,
    U32,
    F16,
    F32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DataType {
    Scalar(ScalarType),
    Vector {
        scalar: ScalarType,
        lanes: u8,
    },
    Matrix {
        scalar: ScalarType,
        rows: u8,
        columns: u8,
    },
    Struct {
        name: String,
        fields: Vec<StructField>,
    },
    TextureHandle,
    ViewHandle,
}

impl DataType {
    pub fn f32() -> Self {
        Self::Scalar(ScalarType::F32)
    }

    pub fn u32() -> Self {
        Self::Scalar(ScalarType::U32)
    }

    pub fn vec3_f32() -> Self {
        Self::Vector {
            scalar: ScalarType::F32,
            lanes: 3,
        }
    }

    pub fn vec4_f32() -> Self {
        Self::Vector {
            scalar: ScalarType::F32,
            lanes: 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StructField {
    pub name: String,
    pub data_type: DataType,
    pub unit: Unit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Unit {
    pub length: i8,
    pub mass: i8,
    pub time: i8,
    pub scale10: i8,
}

impl Unit {
    pub const DIMENSIONLESS: Self = Self::new(0, 0, 0, 0);
    pub const METER: Self = Self::new(1, 0, 0, 0);
    pub const SECOND: Self = Self::new(0, 0, 1, 0);
    pub const KILOGRAM: Self = Self::new(0, 1, 0, 0);
    pub const HERTZ: Self = Self::new(0, 0, -1, 0);
    pub const METERS_PER_SECOND: Self = Self::new(1, 0, -1, 0);
    pub const METERS_PER_SECOND_SQUARED: Self = Self::new(1, 0, -2, 0);

    pub const fn new(length: i8, mass: i8, time: i8, scale10: i8) -> Self {
        Self {
            length,
            mass,
            time,
            scale10,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Literal {
    Bool(bool),
    I32(i32),
    U32(u32),
    F16Bits(u16),
    F32Bits(u32),
    Vector(Vec<Literal>),
    Array(Vec<Literal>),
}

impl Literal {
    pub fn f32(value: f32) -> Self {
        Self::F32Bits(value.to_bits())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueNode {
    pub id: ValueId,
    pub name: String,
    pub data_type: DataType,
    pub unit: Unit,
    pub kind: ValueKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueKind {
    Constant(Literal),
    ScheduleFixedDt { schedule: ScheduleId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamNode {
    pub id: StreamId,
    pub name: String,
    pub element_type: DataType,
    pub unit: Unit,
    pub capacity: u32,
    pub length: StreamLength,
    pub buffering: u32,
    pub storage: StorageClass,
    pub access: ResourceAccess,
    pub write_authority: Option<CapabilityId>,
    pub initial: Option<StreamInitializer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamInitializer {
    Explicit(Literal),
    Repeat {
        value: Literal,
        count: u32,
    },
    Linear {
        start: Literal,
        step: Literal,
        count: u32,
    },
    Grid2D {
        origin: Literal,
        column_step: Literal,
        row_step: Literal,
        columns: u32,
        count: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamLength {
    Fixed(u32),
    Dynamic(StreamId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageClass {
    DevicePrivate,
    HostShared,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceAccess {
    DeviceRead,
    DeviceReadWrite,
    HostReadWrite,
}

impl ResourceAccess {
    pub fn allows_read(&self) -> bool {
        matches!(
            self,
            Self::DeviceRead | Self::DeviceReadWrite | Self::HostReadWrite
        )
    }

    pub fn allows_write(&self) -> bool {
        matches!(self, Self::DeviceReadWrite | Self::HostReadWrite)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelNode {
    pub id: KernelId,
    pub name: String,
    pub slots: Vec<SlotNode>,
    pub abi: KernelAbi,
    pub implementations: Vec<BackendImplementation>,
}

impl KernelNode {
    pub fn slot(&self, id: SlotId) -> &SlotNode {
        &self.slots[id.0 as usize]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotNode {
    pub id: SlotId,
    pub name: String,
    pub resource_type: SlotResourceType,
    pub access: SlotAccess,
    pub indexing: StreamIndexing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamIndexing {
    PerInvocation,
    WholeResource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotResourceType {
    Value { data_type: DataType, unit: Unit },
    Stream { element_type: DataType, unit: Unit },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotAccess {
    Read,
    Write,
    ReadWrite,
    Atomic,
}

impl SlotAccess {
    pub fn reads(&self) -> bool {
        matches!(self, Self::Read | Self::ReadWrite | Self::Atomic)
    }

    pub fn writes(&self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite | Self::Atomic)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelAbi {
    pub binding_order: Vec<SlotId>,
    pub dispatch_index: DispatchIndex,
    pub threadgroup: ThreadgroupBehavior,
    pub aliasing: AliasingRule,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchIndex {
    GlobalLinearU32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadgroupBehavior {
    BackendDerived,
    Fixed { x: u32, y: u32, z: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AliasingRule {
    Forbidden,
    AllowPairs(Vec<(SlotId, SlotId)>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendImplementation {
    pub backend: Backend,
    pub source: String,
    pub entry: String,
    pub source_text: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    Metal,
    Cuda,
    Optix,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassNode {
    pub id: PassId,
    pub name: String,
    pub kernel: KernelId,
    pub bindings: Vec<Binding>,
    pub dispatch: DispatchDomain,
    pub threads_per_threadgroup: Option<u32>,
    pub capabilities: Vec<CapabilityId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub slot: SlotId,
    pub resource: ResourceId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourceId {
    Value(ValueId),
    Stream(StreamId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchDomain {
    OverStream(StreamId),
    Fixed(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewNode {
    pub id: ViewId,
    pub name: String,
    pub reads: Vec<ViewRead>,
    pub state: ViewState,
    pub implementation: BackendImplementation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewRead {
    pub name: String,
    pub stream: StreamId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewState {
    CurrentCompletedTick,
    PreviousStableTick { lag: u32 },
    Interpolated { older_lag: u32, newer_lag: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleNode {
    pub id: ScheduleId,
    pub name: String,
    pub timing: ScheduleTiming,
    pub execution_passes: Vec<PassId>,
    pub presentation_views: Vec<ViewId>,
    pub execution_dependencies: Vec<ExecutionDependency>,
    pub presentation_dependencies: Vec<PresentationDependency>,
    pub in_flight: InFlightPolicy,
    pub tick_overlap: TickOverlapPolicy,
    pub presentation_lifetime: PresentationLifetimePolicy,
    pub queue_model: QueueModel,
    pub overload: OverloadPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleTiming {
    Fixed { rate_hz: u32, fixed_dt: ValueId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionDependency {
    pub before: PassId,
    pub after: PassId,
    pub semantics: DependencySemantics,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationDependency {
    pub producer: PassId,
    pub consumer: ViewId,
    pub semantics: DependencySemantics,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencySemantics {
    Completion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InFlightPolicy {
    pub simulation_ticks: u32,
    pub render_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TickOverlapPolicy {
    RequireResourceVersions,
    SerializeConflictingTicks,
    QueueOrderedReuse,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationLifetimePolicy {
    RequireResourceVersions,
    BlockNextTickUntilViewsComplete,
    QueueOrderedReuse,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueModel {
    SingleSerialQueue,
    Unproven,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverloadPolicy {
    pub catch_up_limit: u32,
    pub excess_wall_time: ExcessWallTimePolicy,
    pub simulation_time: SimulationTimePolicy,
    pub scenario_time: ScenarioTimePolicy,
    pub replay: ReplayOverloadPolicy,
    pub rendering: RenderOverloadPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExcessWallTimePolicy {
    Discard,
    Retain,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimulationTimePolicy {
    AdvanceExecutedTicksOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioTimePolicy {
    SimulationTicks,
    WallTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayOverloadPolicy {
    RecordDecisions,
    Unrecorded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderOverloadPolicy {
    DropPresentationOnly,
    BlockSimulation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractNode {
    pub id: ContractId,
    pub name: String,
    pub schedule: ScheduleId,
    pub clauses: Vec<ContractClause>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractClause {
    Invariant {
        observation: ObservationPoint,
        predicate: Predicate,
    },
    MetricLimit {
        observation: ObservationPoint,
        metric: Metric,
        maximum: Quantity,
    },
    SteadyStateZero {
        observation: ObservationPoint,
        metric: Metric,
        excludes_requested_inspection: bool,
    },
    Determinism(DeterminismContract),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationPoint {
    AfterPassCompletion(PassId),
    AfterEveryPassCompletion(ScheduleId),
    AfterTickExecution(ScheduleId),
    AfterGpuCompletion(ScheduleId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Predicate {
    FiniteStreams(Vec<StreamId>),
    GroundClearance {
        position: StreamId,
        radius: StreamId,
        ground_height: ValueId,
        tolerance: Quantity,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quantity {
    pub value: Literal,
    pub unit: Unit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Metric {
    HeapAllocationsPerTick,
    ApplicationCopiesPerTick,
    ApplicationBlitsPerTick,
    GpuTimePerTick,
    WorkingSetBytes,
    OverloadEvents,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterminismContract {
    pub tier: DeterminismTier,
    pub scope: DeterminismScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeterminismTier {
    Tier1,
    Tier2,
    Tier3,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeterminismScope {
    ExactExecutionFingerprint,
    SameGpuFamily,
    CrossGpu,
    Tolerance(Quantity),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioNode {
    pub id: ScenarioId,
    pub name: String,
    pub schedule: ScheduleId,
    pub duration: ScenarioDuration,
    pub interventions: Vec<ScenarioIntervention>,
    pub expectations: Vec<ScenarioExpectation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioIntervention {
    pub tick: u64,
    pub pass: PassId,
    pub value_overrides: Vec<ScenarioValueOverride>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioValueOverride {
    pub value: ValueId,
    pub literal: Literal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioDuration {
    SimulationTicks(u64),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioExpectation {
    pub observation: ObservationPoint,
    pub predicate: Predicate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkNode {
    pub id: BenchmarkId,
    pub name: String,
    pub schedule: ScheduleId,
    pub warmup_ticks: u64,
    pub measured_ticks: u64,
    pub metrics: Vec<Metric>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityNode {
    pub id: CapabilityId,
    pub name: String,
    pub kind: CapabilityKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityKind {
    Inspect {
        streams: Vec<StreamId>,
        delivery: InspectionDelivery,
        snapshot: SnapshotSemantics,
    },
    HostMutate {
        streams: Vec<StreamId>,
    },
    StateMutate {
        streams: Vec<StreamId>,
    },
    MembershipMutate {
        count: StreamId,
        members: Vec<StreamId>,
    },
    External {
        name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InspectionDelivery {
    Asynchronous,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotSemantics {
    NextGpuCompletedTickAfterRequest,
    LatestGpuCompletedTickAtRequest,
    ExactCompletedTick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScheduleItemId {
    Pass(PassId),
    View(ViewId),
}
