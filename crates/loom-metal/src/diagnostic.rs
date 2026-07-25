use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeDiagnosticCode {
    DeviceUnavailable,
    UnsupportedGraph,
    ResourceAllocationFailed,
    ShaderCompilationFailed,
    PipelineCreationFailed,
    DrawableUnavailable,
    CommandBufferFailed,
    WindowCreationFailed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDiagnostic {
    pub code: RuntimeDiagnosticCode,
    pub message: String,
    pub semantic_path: Option<String>,
}

impl RuntimeDiagnostic {
    pub(crate) fn new(code: RuntimeDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            semantic_path: None,
        }
    }

    pub(crate) fn at(mut self, path: impl Into<String>) -> Self {
        self.semantic_path = Some(path.into());
        self
    }
}

impl std::fmt::Display for RuntimeDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for RuntimeDiagnostic {}
