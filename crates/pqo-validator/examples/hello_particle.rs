use pqo_core::conformance::{HelloParticleConfig, hello_particle_builder};
use pqo_validator::{RepairPlan, Validator};

fn main() {
    let graph = hello_particle_builder(HelloParticleConfig::unsafe_unproven_overlap())
        .build()
        .expect("Hello Particle symbols should resolve");
    let report = Validator::validate(&graph);

    println!(
        "{}",
        serde_json::to_string_pretty(&graph).expect("serialize normalized graph")
    );
    println!("\nsource_graph_hash: {}", report.source_graph.fingerprint);
    println!(
        "artifact_before_repair: {}",
        report.artifact_fingerprint().unwrap_or("none")
    );
    println!(
        "validation:\n{}",
        serde_json::to_string_pretty(&report.diagnostics).expect("serialize diagnostics")
    );

    let repair = RepairPlan::from_report(&report).expect("mechanical repair plan");
    let validated = repair
        .apply_and_validate(&graph)
        .expect("atomic repair must produce a validated graph");
    println!("repair_edits: {}", repair.edits.len());
    println!("artifact_fingerprint: {}", validated.artifact_fingerprint());
}
