use serde::{Deserialize, Serialize};

use crate::{
    ids::{PassId, ScheduleId, StreamId},
    model::{ScheduleItemId, TickOverlapPolicy},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DiagnosticCode {
    DuplicateSymbol,
    UnknownSymbol,
    TypeMismatch,
    UnitMismatch,
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
        versions: u32,
    },
    SetScheduleSimulationTicksInFlight {
        schedule: ScheduleId,
        ticks: u32,
    },
    SetTickOverlapPolicy {
        schedule: ScheduleId,
        policy: TickOverlapPolicy,
    },
    AddCompletionDependency {
        schedule: ScheduleId,
        before: ScheduleItemId,
        after: ScheduleItemId,
    },
    BindMissingSlot {
        pass: PassId,
        slot_name: String,
    },
    RenameSymbol {
        path: SemanticPath,
        suggested_name: String,
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
