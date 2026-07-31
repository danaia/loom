use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeFingerprint {
    pub artifact: String,
    pub device: String,
    pub operating_system: String,
    pub host_profile: String,
    pub host_executable_sha256: String,
    pub source_revision: String,
    pub source_dirty: Option<bool>,
    pub rust_compiler: String,
    pub metal_sdk: String,
    pub shader_hashes: Vec<ShaderIdentity>,
    pub pipelines: Vec<PipelineIdentity>,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShaderIdentity {
    pub source_path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineIdentity {
    pub entry: String,
    pub kind: String,
    pub source_sha256: String,
    pub thread_execution_width: Option<u64>,
    pub max_threads_per_threadgroup: Option<u64>,
}

pub(crate) fn sha256(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
