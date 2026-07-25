use loom_core::conformance::{HelloParticleConfig, hello_particle_builder};
use loom_metal::MetalRuntime;
use loom_validator::Validator;

fn main() {
    if let Err(diagnostic) = run() {
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&diagnostic).unwrap_or_else(|_| diagnostic.to_string())
        );
        std::process::exit(1);
    }
}

fn run() -> Result<(), loom_metal::RuntimeDiagnostic> {
    let graph = hello_particle_builder(HelloParticleConfig::default())
        .build()
        .map_err(|diagnostics| loom_metal::RuntimeDiagnostic {
            code: loom_metal::RuntimeDiagnosticCode::UnsupportedGraph,
            message: serde_json::to_string(&diagnostics)
                .unwrap_or_else(|_| "Hello Particle graph build failed".to_owned()),
            semantic_path: None,
        })?;
    let report = Validator::validate(&graph);
    let validated = report
        .validated
        .ok_or_else(|| loom_metal::RuntimeDiagnostic {
            code: loom_metal::RuntimeDiagnosticCode::UnsupportedGraph,
            message: serde_json::to_string(&report.diagnostics)
                .unwrap_or_else(|_| "Hello Particle validation failed".to_owned()),
            semantic_path: None,
        })?;
    MetalRuntime::run(validated)
}
