use std::{
    env, fs,
    path::{Component, Path, PathBuf},
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
    New { project_name: String },
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
        CliAction::New { project_name } => {
            return create_project(&project_name);
        }
        CliAction::Version => {
            println!("loom {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        CliAction::Execute { command, path } => (command, path),
    };

    let loaded = match package::load(&path, command == Command::Run) {
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
            loaded.ui().cloned(),
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
        [command, project_name] if command == "new" => Ok(CliAction::New {
            project_name: project_name.clone(),
        }),
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
  loom new <project-name>      Create a project from the Baseline starter
  loom update                  Install the latest Loom release
  loom --version               Print the installed version
  loom --help                  Print this help"
    );
}

fn create_project(project_name: &str) -> Result<(), u8> {
    if !is_project_name(project_name) {
        return Err(new_error(
            "name",
            "project name must be one non-empty directory name without path separators",
        ));
    }
    let working_directory = env::current_dir().map_err(|error| {
        print_json(&serde_json::json!({
            "status": "new_error",
            "stage": "working_directory",
            "message": error.to_string(),
        }));
        2
    })?;
    let destination = working_directory.join(project_name);
    if destination.exists() {
        return Err(new_error(
            "destination",
            &format!("destination `{}` already exists", destination.display()),
        ));
    }

    let staging = tempfile::Builder::new()
        .prefix(".loom-new-")
        .tempdir_in(&working_directory)
        .map_err(|error| {
            print_json(&serde_json::json!({
                "status": "new_error",
                "stage": "staging",
                "message": error.to_string(),
            }));
            2
        })?;
    let staged_project = staging.path().join(project_name);

    if let Some(template) = installed_baseline() {
        copy_project_tree(&template, &staged_project)
            .map_err(|message| new_error("copy", &message))?;
    } else {
        let archive = staging.path().join("loom-release.tar.gz");
        download_release_baseline(&archive).map_err(|message| new_error("download", &message))?;
        let unpacked = staging.path().join("release");
        fs::create_dir(&unpacked).map_err(|error| new_error("unpack", &error.to_string()))?;
        let status = ProcessCommand::new("tar")
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(&unpacked)
            .status()
            .map_err(|error| new_error("unpack", &format!("could not run tar: {error}")))?;
        if !status.success() {
            return Err(new_error("unpack", &format!("tar exited with {status}")));
        }
        copy_project_tree(&unpacked.join("loom/baseline"), &staged_project)
            .map_err(|message| new_error("copy", &message))?;
    }
    install_project_dependencies(&staged_project)
        .map_err(|message| new_error("npm_install", &message))?;
    fs::rename(&staged_project, &destination).map_err(|error| {
        new_error(
            "destination",
            &format!("could not create `{}`: {error}", destination.display()),
        )
    })?;
    println!("Created Loom project at {}", destination.display());
    Ok(())
}

fn install_project_dependencies(project_root: &Path) -> Result<(), String> {
    let ui_root = project_root.join("ui");
    if !ui_root.join("package.json").is_file() {
        return Ok(());
    }
    let npm = env::var_os("LOOM_NPM").unwrap_or_else(|| "npm".into());
    println!("Installing UI dependencies...");
    let status = ProcessCommand::new(npm)
        .args(["install", "--no-audit", "--no-fund"])
        .current_dir(&ui_root)
        .status()
        .map_err(|error| format!("could not run npm: {error}"))?;
    if !status.success() {
        return Err(format!("npm install exited with {status}"));
    }
    Ok(())
}

fn is_project_name(name: &str) -> bool {
    let path = Path::new(name);
    !name.is_empty()
        && path.components().count() == 1
        && matches!(path.components().next(), Some(Component::Normal(_)))
}

fn installed_baseline() -> Option<PathBuf> {
    if let Some(path) = env::var_os("LOOM_NEW_TEMPLATE_DIR").map(PathBuf::from) {
        return path.is_dir().then_some(path);
    }
    let executable = env::current_exe().ok()?.canonicalize().ok()?;
    let candidate = executable.parent()?.parent()?.join("baseline");
    candidate.is_dir().then_some(candidate)
}

fn download_release_baseline(archive: &Path) -> Result<(), String> {
    let default_url = format!(
        "https://github.com/danaia/loom/releases/download/v{}/loom-darwin-arm64.tar.gz",
        env!("CARGO_PKG_VERSION")
    );
    let release_url = env::var("LOOM_NEW_RELEASE_URL").unwrap_or(default_url);
    let status = ProcessCommand::new("curl")
        .args([
            "-fsSL",
            "--retry",
            "3",
            "--proto",
            "=https,file",
            "--tlsv1.2",
            &release_url,
            "-o",
        ])
        .arg(archive)
        .status()
        .map_err(|error| format!("could not run curl: {error}"))?;
    if !status.success() {
        return Err(format!(
            "could not download Loom release from {release_url}: {status}"
        ));
    }
    Ok(())
}

fn copy_project_tree(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err(format!(
            "Baseline template `{}` is missing",
            source.display()
        ));
    }
    fs::create_dir(destination)
        .map_err(|error| format!("could not create `{}`: {error}", destination.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("could not read `{}`: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        if matches!(
            entry.file_name().to_str(),
            Some("node_modules" | "dist" | "target")
        ) {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let destination_path = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_project_tree(&entry.path(), &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &destination_path)
                .map_err(|error| format!("could not copy `{}`: {error}", entry.path().display()))?;
        } else if !file_type.is_symlink() {
            return Err(format!(
                "Baseline template contains unsupported entry `{}`",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn new_error(stage: &str, message: &str) -> u8 {
    print_json(&serde_json::json!({
        "status": "new_error",
        "stage": stage,
        "message": message,
    }));
    2
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
    ui: Option<package::LoadedUi>,
) -> Result<(), u8> {
    let ui_project_root = project_root.clone();
    let ui = ui.map(|ui| loom_metal::ProjectUi {
        project_root: ui_project_root.unwrap_or_else(|| ui.asset_root.clone()),
        asset_root: ui.asset_root,
        entry: ui.entry,
        title: ui.title,
        width: ui.width,
        height: ui.height,
    });
    loom_metal::MetalRuntime::run_project_with_ui(validated, project_root, extension_path, ui)
        .map_err(|diagnostic| {
            print_json(&serde_json::json!({
                "status": "runtime_error",
                "diagnostic": diagnostic,
            }));
            1
        })
}

#[cfg(not(target_os = "macos"))]
fn run_window(
    _validated: loom_validator::ValidatedModuleGraph,
    _project_root: Option<std::path::PathBuf>,
    _extension_path: Option<std::path::PathBuf>,
    _ui: Option<package::LoadedUi>,
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

    #[test]
    fn new_accepts_a_single_project_name() {
        assert_eq!(
            parse_arguments(&args(&["new", "my-project"])),
            Ok(CliAction::New {
                project_name: "my-project".to_owned()
            })
        );
    }
}
