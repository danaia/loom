use std::{
    env,
    process::{Command as ProcessCommand, ExitCode, Stdio},
};

use loom_syntax::parse;
use loom_validator::Validator;
use serde::Serialize;

mod package;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Build,
    Check,
    Explain,
    Run,
}

#[derive(Debug, Eq, PartialEq)]
enum CliAction {
    Execute { command: Command, path: String },
    Help,
    Update,
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
        CliAction::Update => {
            return update_installation();
        }
        CliAction::Version => {
            println!("loom {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        CliAction::Execute { command, path } => (command, path),
    };

    let loaded = match package::load(&path) {
        Ok(loaded) => loaded,
        Err(error) => {
            print_json(&serde_json::json!({
                "status": "io_error",
                "path": path,
                "message": error,
            }));
            return Err(2);
        }
    };
    let source = loaded.source();
    let graph = match parse(source) {
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

    if command == Command::Build {
        let output = match package::build(&path, &graph) {
            Ok(output) => output,
            Err(error) => {
                print_json(&serde_json::json!({
                    "status": "package_error",
                    "path": path,
                    "message": error,
                }));
                return Err(1);
            }
        };
        print_json(&serde_json::json!({
            "status": "built",
            "module": graph.name,
            "package": output,
            "artifact_fingerprint": validated.artifact_fingerprint(),
        }));
    } else if command == Command::Run {
        return run_window(
            validated.clone(),
            loaded.project_root().map(ToOwned::to_owned),
            loaded.extension_path().map(ToOwned::to_owned),
        );
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
        [command] if command == "update" => Ok(CliAction::Update),
        [path] => Ok(CliAction::Execute {
            command: Command::Run,
            path: path.clone(),
        }),
        [command, path] => {
            let command = match command.as_str() {
                "build" => Command::Build,
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
  loom <project.lmp>           Run a compiled Loom package
  loom build <source.loom>     Build a portable .lmp package
  loom run <source.loom>       Run a Loom program explicitly
  loom check <source.loom>     Parse, validate, and fingerprint
  loom explain <source.loom>   Print the graph, plan, and generated Metal
  loom update                  Install the latest Loom release
  loom --version               Print the installed version
  loom --help                  Print this help"
    );
}

#[cfg(target_os = "macos")]
fn update_installation() -> Result<(), u8> {
    const INSTALLER_URL: &str = "https://raw.githubusercontent.com/danaia/loom/main/install.sh";
    let installer_url =
        env::var("LOOM_UPDATE_INSTALLER_URL").unwrap_or_else(|_| INSTALLER_URL.to_owned());

    println!("Updating Loom from {installer_url}...");
    let mut download = ProcessCommand::new("curl")
        .args([
            "-fsSL",
            "--retry",
            "3",
            "--proto",
            "=https,file",
            "--tlsv1.2",
            &installer_url,
        ])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| {
            print_json(&serde_json::json!({
                "status": "update_error",
                "stage": "download",
                "message": error.to_string(),
            }));
            2
        })?;
    let installer = download
        .stdout
        .take()
        .expect("piped curl output must be available");
    let install_status = ProcessCommand::new("sh")
        .stdin(Stdio::from(installer))
        .status();
    let download_status = download.wait();

    let download_status = download_status.map_err(|error| {
        print_json(&serde_json::json!({
            "status": "update_error",
            "stage": "download",
            "message": error.to_string(),
        }));
        2
    })?;
    if !download_status.success() {
        print_json(&serde_json::json!({
            "status": "update_error",
            "stage": "download",
            "message": format!("curl exited with {download_status}"),
        }));
        return Err(2);
    }

    let install_status = install_status.map_err(|error| {
        print_json(&serde_json::json!({
            "status": "update_error",
            "stage": "install",
            "message": error.to_string(),
        }));
        2
    })?;
    if !install_status.success() {
        print_json(&serde_json::json!({
            "status": "update_error",
            "stage": "install",
            "message": format!("installer exited with {install_status}"),
        }));
        return Err(2);
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn update_installation() -> Result<(), u8> {
    print_json(&serde_json::json!({
        "status": "unsupported",
        "message": "Loom updates currently require an Apple Silicon Mac",
    }));
    Err(2)
}

#[cfg(target_os = "macos")]
fn run_window(
    validated: loom_validator::ValidatedModuleGraph,
    project_root: Option<std::path::PathBuf>,
    extension_path: Option<std::path::PathBuf>,
) -> Result<(), u8> {
    loom_metal::MetalRuntime::run_project(validated, project_root, extension_path).map_err(
        |diagnostic| {
            print_json(&serde_json::json!({
                "status": "runtime_error",
                "diagnostic": diagnostic,
            }));
            1
        },
    )
}

#[cfg(not(target_os = "macos"))]
fn run_window(
    _validated: loom_validator::ValidatedModuleGraph,
    _project_root: Option<std::path::PathBuf>,
    _extension_path: Option<std::path::PathBuf>,
) -> Result<(), u8> {
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
        assert_eq!(
            parse_arguments(&args(&["build", "hello-particle.loom"])),
            Ok(CliAction::Execute {
                command: Command::Build,
                path: "hello-particle.loom".to_owned(),
            })
        );
    }

    #[test]
    fn help_version_and_update_are_host_friendly() {
        assert_eq!(parse_arguments(&args(&["--help"])), Ok(CliAction::Help));
        assert_eq!(
            parse_arguments(&args(&["--version"])),
            Ok(CliAction::Version)
        );
        assert_eq!(parse_arguments(&args(&["update"])), Ok(CliAction::Update));
    }
}
