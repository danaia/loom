use std::{env, fs, process::ExitCode};

use loom_syntax::parse;
use loom_validator::Validator;
use serde::Serialize;

#[derive(Serialize)]
struct CheckSuccess<'a> {
    status: &'static str,
    module: &'a str,
    target: &'static str,
    source_graph_hash: &'a str,
    artifact_fingerprint: &'a str,
    summary: GraphSummary,
}

#[derive(Serialize)]
struct GraphSummary {
    values: usize,
    streams: usize,
    kernels: usize,
    native_kernels: usize,
    external_kernels: usize,
    passes: usize,
    views: usize,
    flows: usize,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run() -> Result<(), u8> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let [command, path] = arguments.as_slice() else {
        eprintln!("usage: loom <check|explain|run> <source.loom>");
        return Err(2);
    };
    if command != "check" && command != "explain" && command != "run" {
        eprintln!("unknown command `{command}`; expected `check`, `explain`, or `run`");
        return Err(2);
    }

    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            print_json(&serde_json::json!({
                "status": "io_error",
                "path": path,
                "message": error.to_string(),
            }));
            return Err(2);
        }
    };
    let graph = match parse(&source) {
        Ok(graph) => graph,
        Err(diagnostics) => {
            print_json(&serde_json::json!({
                "status": "source_invalid",
                "path": path,
                "diagnostics": diagnostics,
            }));
            return Err(1);
        }
    };
    let report = Validator::validate(&graph);
    let Some(validated) = report.validated.as_ref() else {
        print_json(&serde_json::json!({
            "status": "graph_invalid",
            "path": path,
            "source_graph_hash": report.source_graph.fingerprint,
            "diagnostics": report.diagnostics,
        }));
        return Err(1);
    };

    if command == "run" {
        return run_window(validated.clone());
    } else if command == "check" {
        print_json(&CheckSuccess {
            status: "valid",
            module: &graph.name,
            target: "metal",
            source_graph_hash: &report.source_graph.fingerprint,
            artifact_fingerprint: validated.artifact_fingerprint(),
            summary: GraphSummary {
                values: graph.resources.values.len(),
                streams: graph.resources.streams.len(),
                kernels: graph.kernels.len(),
                native_kernels: graph
                    .kernels
                    .iter()
                    .filter(|kernel| {
                        kernel.implementations.iter().any(|implementation| {
                            implementation.source.starts_with("loom://generated/")
                        })
                    })
                    .count(),
                external_kernels: graph
                    .kernels
                    .iter()
                    .filter(|kernel| {
                        kernel.implementations.iter().any(|implementation| {
                            !implementation.source.starts_with("loom://generated/")
                        })
                    })
                    .count(),
                passes: graph.passes.len(),
                views: graph.views.len(),
                flows: graph.schedules.len(),
            },
        });
    } else {
        print_json(&serde_json::json!({
            "status": "valid",
            "source_graph_hash": report.source_graph.fingerprint,
            "artifact_fingerprint": validated.artifact_fingerprint(),
            "graph": graph,
            "execution_plan": validated.execution_plan(),
        }));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_window(validated: loom_validator::ValidatedModuleGraph) -> Result<(), u8> {
    loom_metal::MetalRuntime::run(validated).map_err(|diagnostic| {
        print_json(&serde_json::json!({
            "status": "runtime_error",
            "diagnostic": diagnostic,
        }));
        1
    })
}

#[cfg(not(target_os = "macos"))]
fn run_window(_validated: loom_validator::ValidatedModuleGraph) -> Result<(), u8> {
    print_json(&serde_json::json!({
        "status": "unsupported",
        "message": "`loom run` currently requires macOS and Metal",
    }));
    Err(2)
}

fn print_json(value: &impl Serialize) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("CLI result must serialize")
    );
}
