use std::{env, fs, process::ExitCode};

use loom_syntax::parse;
use loom_validator::Validator;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Check,
    Explain,
    Run,
}

#[derive(Debug, Eq, PartialEq)]
enum CliAction {
    Execute { command: Command, path: String },
    Help,
    Version,
}

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
    let action = match parse_arguments(&arguments) {
        Ok(action) => action,
        Err(message) => {
            eprintln!("{message}\n");
            print_help();
            return Err(2);
        }
    };
    let (command, path) = match action {
        CliAction::Help => {
            print_help();
            return Ok(());
        }
        CliAction::Version => {
            println!("loom {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        CliAction::Execute { command, path } => (command, path),
    };

    let source = match fs::read_to_string(&path) {
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

    if command == Command::Run {
        return run_window(validated.clone());
    } else if command == Command::Check {
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

fn parse_arguments(arguments: &[String]) -> Result<CliAction, String> {
    match arguments {
        [flag] if flag == "--help" || flag == "-h" => Ok(CliAction::Help),
        [flag] if flag == "--version" || flag == "-V" => Ok(CliAction::Version),
        [path] => Ok(CliAction::Execute {
            command: Command::Run,
            path: path.clone(),
        }),
        [command, path] => {
            let command = match command.as_str() {
                "check" => Command::Check,
                "explain" => Command::Explain,
                "run" => Command::Run,
                _ => {
                    return Err(format!(
                        "unknown command `{command}`; pass a .loom file or use check, explain, or run"
                    ));
                }
            };
            Ok(CliAction::Execute {
                command,
                path: path.clone(),
            })
        }
        [] => Err("missing Loom program".to_owned()),
        _ => Err("too many arguments".to_owned()),
    }
}

fn print_help() {
    println!(
        "Loom — agent-native GPU programs for Metal

Usage:
  loom <source.loom>           Run a Loom program
  loom run <source.loom>       Run a Loom program explicitly
  loom check <source.loom>     Parse, validate, and fingerprint
  loom explain <source.loom>   Print the graph, plan, and generated Metal
  loom --version               Print the installed version
  loom --help                  Print this help"
    );
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
        "message": "running Loom programs currently requires macOS and Metal",
    }));
    Err(2)
}

fn print_json(value: &impl Serialize) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("CLI result must serialize")
    );
}

#[cfg(test)]
mod tests {
    use super::{CliAction, Command, parse_arguments};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn a_source_path_runs_without_a_subcommand() {
        assert_eq!(
            parse_arguments(&args(&["hello-particle.loom"])),
            Ok(CliAction::Execute {
                command: Command::Run,
                path: "hello-particle.loom".to_owned(),
            })
        );
    }

    #[test]
    fn explicit_commands_remain_available() {
        assert_eq!(
            parse_arguments(&args(&["check", "hello-particle.loom"])),
            Ok(CliAction::Execute {
                command: Command::Check,
                path: "hello-particle.loom".to_owned(),
            })
        );
    }

    #[test]
    fn help_and_version_are_host_friendly() {
        assert_eq!(parse_arguments(&args(&["--help"])), Ok(CliAction::Help));
        assert_eq!(
            parse_arguments(&args(&["--version"])),
            Ok(CliAction::Version)
        );
    }
}
