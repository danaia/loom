use sha2::{Digest, Sha256};

use crate::model::ModuleGraph;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalGraph {
    pub bytes: Vec<u8>,
    pub fingerprint: String,
}

pub fn canonicalize(graph: &ModuleGraph) -> CanonicalGraph {
    let bytes = serde_json::to_vec(graph).expect("ModuleGraph serialization is infallible");
    let digest = Sha256::digest(&bytes);
    let fingerprint = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    CanonicalGraph { bytes, fingerprint }
}
