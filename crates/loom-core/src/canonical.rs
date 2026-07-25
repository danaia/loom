use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::model::{AliasingRule, CapabilityKind, ModuleGraph, Predicate};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalGraph {
    pub bytes: Vec<u8>,
    pub fingerprint: String,
}

pub fn canonicalize(graph: &ModuleGraph) -> CanonicalGraph {
    let mut normalized = graph.clone();

    for kernel in &mut normalized.kernels.nodes {
        sort_serializable(&mut kernel.implementations);
        if let AliasingRule::AllowPairs(pairs) = &mut kernel.abi.aliasing {
            for (left, right) in pairs.iter_mut() {
                if right < left {
                    std::mem::swap(left, right);
                }
            }
            pairs.sort();
            pairs.dedup();
        }
    }
    for pass in &mut normalized.passes.nodes {
        pass.bindings.sort_by_key(|binding| binding.slot);
    }
    for view in &mut normalized.views {
        view.reads
            .sort_by(|left, right| (&left.name, left.stream).cmp(&(&right.name, right.stream)));
    }
    for schedule in &mut normalized.schedules.nodes {
        schedule.execution_passes.sort();
        schedule.execution_passes.dedup();
        schedule.presentation_views.sort();
        schedule.presentation_views.dedup();
        schedule
            .execution_dependencies
            .sort_by_key(|edge| (edge.before, edge.after));
        schedule.execution_dependencies.dedup();
        schedule
            .presentation_dependencies
            .sort_by_key(|edge| (edge.producer, edge.consumer));
        schedule.presentation_dependencies.dedup();
    }
    for contract in &mut normalized.contracts {
        for clause in &mut contract.clauses {
            normalize_contract_clause(clause);
        }
        sort_serializable(&mut contract.clauses);
    }
    for scenario in &mut normalized.scenarios {
        for expectation in &mut scenario.expectations {
            normalize_predicate(&mut expectation.predicate);
        }
        sort_serializable(&mut scenario.expectations);
    }
    for benchmark in &mut normalized.benchmarks {
        sort_serializable(&mut benchmark.metrics);
        benchmark.metrics.dedup();
    }
    for capability in &mut normalized.capabilities {
        match &mut capability.kind {
            CapabilityKind::Inspect { streams, .. } | CapabilityKind::HostMutate { streams } => {
                streams.sort();
                streams.dedup();
            }
            CapabilityKind::External { .. } => {}
        }
    }

    let bytes = serde_json::to_vec(&normalized).expect("ModuleGraph serialization is infallible");
    let digest = Sha256::digest(&bytes);
    let fingerprint = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    CanonicalGraph { bytes, fingerprint }
}

fn normalize_contract_clause(clause: &mut crate::model::ContractClause) {
    if let crate::model::ContractClause::Invariant { predicate, .. } = clause {
        normalize_predicate(predicate);
    }
}

fn normalize_predicate(predicate: &mut Predicate) {
    if let Predicate::FiniteStreams(streams) = predicate {
        streams.sort();
        streams.dedup();
    }
}

fn sort_serializable<T: Serialize>(items: &mut [T]) {
    items.sort_by_cached_key(|item| {
        serde_json::to_vec(item).expect("canonical child serialization is infallible")
    });
}
