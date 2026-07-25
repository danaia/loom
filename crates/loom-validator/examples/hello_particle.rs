use loom_core::conformance::{HelloParticleConfig, hello_particle_builder};
use loom_validator::Validator;

fn main() {
    let graph = hello_particle_builder(HelloParticleConfig::unsafe_unproven_overlap())
        .build()
        .expect("Hello Particle symbols should resolve");
    let report = Validator::validate(&graph);

    println!(
        "{}",
        serde_json::to_string_pretty(&graph).expect("serialize normalized graph")
    );
    println!("\nfingerprint: {}", report.canonical.fingerprint);
    println!(
        "validation:\n{}",
        serde_json::to_string_pretty(&report.diagnostics).expect("serialize diagnostics")
    );
}
