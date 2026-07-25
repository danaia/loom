use serde::{Deserialize, Serialize};

use crate::{
    ids::{ScheduleId, StreamId},
    model::{ScheduleItemId, TickOverlapPolicy},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DiagnosticCode {
    DuplicateSymbol,
    UnknownSymbol,
    InvalidReference,
    InvalidNodeId,
    NonCanonicalOrder,
    TypeMismatch,
    UnitMismatch,
    InvalidLiteral,
    InvalidInitialData,
    AccessViolation,
    CapacityExceeded,
    InvalidLogicalLength,
    InvalidInFlightPolicy,
    InvalidOverloadPolicy,
    MissingBinding,
    DuplicateBinding,
    IllegalAlias,
    UnorderedHazard,
    DependencyCycle,
    InsufficientBufferVersions,
    UnprovenQueueReuse,
    InvalidKernelAbi,
    MissingBackendImplementation,
    InvalidObservationPoint,
    InvalidViewState,
    UnsafePresentationLifetime,
    InvalidCapability,
    InvalidMetricUnit,
    IncompatibleDeterminismScope,
    NondeterministicOverload,
    RenderDependencyAffectsSimulation,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticPath(pub Vec<String>);

impl SemanticPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into().split('.').map(str::to_owned).collect())
    }

    pub fn child(&self, segment: impl Into<String>) -> Self {
        let mut path = self.0.clone();
        path.push(segment.into());
        Self(path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphEdit {
    SetStreamBuffering {
        stream: StreamId,
        expected: u32,
        versions: u32,
    },
    SetScheduleSimulationTicksInFlight {
        schedule: ScheduleId,
        expected: u32,
        ticks: u32,
    },
    SetTickOverlapPolicy {
        schedule: ScheduleId,
        expected: TickOverlapPolicy,
        policy: TickOverlapPolicy,
    },
    AddCompletionDependency {
        schedule: ScheduleId,
        before: ScheduleItemId,
        after: ScheduleItemId,
        expected_absent: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub primary: SemanticPath,
    pub related: Vec<SemanticPath>,
    pub suggested_fix: Option<GraphEdit>,
}

impl Diagnostic {
    pub fn error(code: DiagnosticCode, message: impl Into<String>, primary: SemanticPath) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            primary,
            related: Vec::new(),
            suggested_fix: None,
        }
    }

    pub fn related(mut self, path: SemanticPath) -> Self {
        self.related.push(path);
        self
    }

    pub fn fix(mut self, fix: GraphEdit) -> Self {
        self.suggested_fix = Some(fix);
        self
    }
}
