use std::process::Stdio;
use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode},
};

#[cfg(target_os = "linux")]
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    process::Child,
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use pqo_core::{TargetPolicy, TargetProfile};
use pqo_syntax::parse;
use pqo_validator::Validator;
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
    Execute {
        command: Command,
        path: String,
        target: Option<TargetProfile>,
    },
    Help,
    New {
        project_name: String,
    },
    Update,
    Version,
}

#[derive(Serialize)]
struct CheckSuccess<'a> {
    status: &'static str,
    module: &'a str,
    target: String,
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
    let (command, path, requested_target) = match action {
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
            println!("pqo {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        CliAction::Execute {
            command,
            path,
            target,
        } => (command, path, target),
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
    let packaged_target = loaded.target_profile();
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
    let target_profile = requested_target
        .or(packaged_target)
        .unwrap_or_else(|| default_target(&graph.target));
    let report = Validator::validate_for(&graph, target_profile);
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
        let output = match package::build(&path, &graph, target_profile) {
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
            target: target_name(target_profile).to_owned(),
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
                            implementation.source.starts_with("pqo://generated/")
                        })
                    })
                    .count(),
                external_kernels: graph
                    .kernels
                    .iter()
                    .filter(|kernel| {
                        kernel.implementations.iter().any(|implementation| {
                            !implementation.source.starts_with("pqo://generated/")
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
            target: None,
        }),
        [command, path] => {
            let command = match command.as_str() {
                "build" => Command::Build,
                "check" => Command::Check,
                "explain" => Command::Explain,
                "run" => Command::Run,
                _ => {
                    return Err(format!(
                        "unknown command `{command}`; pass a .pqo file or use check, explain, or run"
                    ));
                }
            };
            Ok(CliAction::Execute {
                command,
                path: path.clone(),
                target: None,
            })
        }
        [command, path, flag, target] if flag == "--target" => {
            let command = parse_command(command)?;
            Ok(CliAction::Execute {
                command,
                path: path.clone(),
                target: Some(parse_target(target)?),
            })
        }
        [] => Err("missing Pqo program".to_owned()),
        _ => Err("too many arguments".to_owned()),
    }
}

fn parse_command(command: &str) -> Result<Command, String> {
    match command {
        "build" => Ok(Command::Build),
        "check" => Ok(Command::Check),
        "explain" => Ok(Command::Explain),
        "run" => Ok(Command::Run),
        _ => Err(format!("unknown command `{command}`")),
    }
}

fn parse_target(target: &str) -> Result<TargetProfile, String> {
    match target {
        "metal" => Ok(TargetProfile::metal()),
        "cuda-vulkan" => Ok(TargetProfile::cuda_vulkan()),
        "cuda-headless" => Ok(TargetProfile::cuda_headless()),
        _ => Err(format!(
            "unknown target `{target}`; expected metal, cuda-vulkan, or cuda-headless"
        )),
    }
}

fn default_target(policy: &TargetPolicy) -> TargetProfile {
    if let TargetPolicy::Profiles(profiles) = policy
        && profiles.len() == 1
    {
        return profiles[0];
    }
    if cfg!(target_os = "macos") {
        TargetProfile::metal()
    } else {
        TargetProfile::cuda_vulkan()
    }
}

fn target_name(target: TargetProfile) -> &'static str {
    if target == TargetProfile::metal() {
        "metal"
    } else if target == TargetProfile::cuda_vulkan() {
        "cuda-vulkan"
    } else {
        "cuda-headless"
    }
}

fn print_help() {
    println!(
        "Pqo — agent-native portable GPU programs

Usage:
  pqo <source.pqo>           Run a Pqo program
  pqo <project.lmp>           Run a compiled Pqo package
  pqo build <source.pqo> [--target metal|cuda-vulkan|cuda-headless]
  pqo run <source.pqo>       Run a Pqo program explicitly
  pqo check <source.pqo>     Parse, validate, and fingerprint
  pqo explain <source.pqo>   Print the graph and selected execution plan
  pqo new <project-name>      Create a project from the Baseline starter
  pqo update                  Install the latest Pqo release
  pqo --version               Print the installed version
  pqo --help                  Print this help"
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
        .prefix(".pqo-new-")
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
        let archive = staging.path().join("pqo-release.tar.gz");
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
        copy_project_tree(&unpacked.join("pqo/baseline"), &staged_project)
            .map_err(|message| new_error("copy", &message))?;
    }
    rename_baseline_source(&staged_project, project_name)
        .map_err(|message| new_error("rename", &message))?;
    verify_metal_view_project(&staged_project, project_name)
        .map_err(|message| new_error("metal_view", &message))?;
    personalize_controls_title(&staged_project, project_name)
        .map_err(|message| new_error("panel_title", &message))?;
    install_project_dependencies(&staged_project)
        .map_err(|message| new_error("npm_install", &message))?;
    fs::rename(&staged_project, &destination).map_err(|error| {
        new_error(
            "destination",
            &format!("could not create `{}`: {error}", destination.display()),
        )
    })?;
    println!("Created Pqo project at {}", destination.display());
    Ok(())
}

fn rename_baseline_source(project_root: &Path, project_name: &str) -> Result<(), String> {
    let baseline_source = project_root.join("baseline.pqo");
    if !baseline_source.is_file() {
        return Err(format!(
            "Baseline template is missing `{}`",
            baseline_source.display()
        ));
    }
    let project_source = project_root.join(format!("{project_name}.pqo"));
    fs::rename(&baseline_source, &project_source).map_err(|error| {
        format!(
            "could not rename `{}` to `{}`: {error}",
            baseline_source.display(),
            project_source.display()
        )
    })?;
    let prebuilt_package = project_root.join("baseline.lmp");
    if prebuilt_package.exists() {
        fs::remove_file(&prebuilt_package).map_err(|error| {
            format!("could not remove `{}`: {error}", prebuilt_package.display())
        })?;
    }
    Ok(())
}

fn verify_metal_view_project(project_root: &Path, project_name: &str) -> Result<(), String> {
    let source_path = project_root.join(format!("{project_name}.pqo"));
    let source = fs::read_to_string(&source_path)
        .map_err(|error| format!("could not read `{}`: {error}", source_path.display()))?;
    let graph = parse(&source).map_err(|diagnostics| {
        format!(
            "starter source `{}` is invalid: {diagnostics:?}",
            source_path.display()
        )
    })?;
    let presents_view = graph
        .schedules
        .iter()
        .any(|schedule| !schedule.presentation_views.is_empty());
    if graph.views.is_empty() || !presents_view {
        return Err(format!(
            "starter source `{}` must declare and draw a Metal view",
            source_path.display()
        ));
    }
    Ok(())
}

fn personalize_controls_title(project_root: &Path, project_name: &str) -> Result<(), String> {
    let config_path = project_root.join("ui/pqo-ui.json");
    let mut config = serde_json::from_slice::<serde_json::Value>(
        &fs::read(&config_path)
            .map_err(|error| format!("could not read `{}`: {error}", config_path.display()))?,
    )
    .map_err(|error| format!("could not parse `{}`: {error}", config_path.display()))?;
    let Some(config) = config.as_object_mut() else {
        return Err(format!(
            "`{}` must contain a JSON object",
            config_path.display()
        ));
    };
    config.insert(
        "title".to_owned(),
        serde_json::Value::String(format!("Pqo {project_name} — Controls")),
    );
    let mut serialized = serde_json::to_vec_pretty(&config)
        .map_err(|error| format!("could not serialize `{}`: {error}", config_path.display()))?;
    serialized.push(b'\n');
    fs::write(&config_path, serialized)
        .map_err(|error| format!("could not write `{}`: {error}", config_path.display()))
}

fn install_project_dependencies(project_root: &Path) -> Result<(), String> {
    let ui_root = project_root.join("ui");
    if !ui_root.join("package.json").is_file() {
        return Ok(());
    }
    let npm = env::var_os("PQO_NPM").unwrap_or_else(|| "npm".into());
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
    if let Some(path) = env::var_os("PQO_NEW_TEMPLATE_DIR").map(PathBuf::from) {
        return path.is_dir().then_some(path);
    }
    let executable = env::current_exe().ok()?.canonicalize().ok()?;
    let candidate = executable.parent()?.parent()?.join("baseline");
    candidate.is_dir().then_some(candidate)
}

fn download_release_baseline(archive: &Path) -> Result<(), String> {
    let default_url = format!(
        "https://github.com/danaia/pqo/releases/download/v{}/pqo-{}-{}.tar.gz",
        env!("CARGO_PKG_VERSION"),
        if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        },
        if cfg!(target_os = "macos") {
            "arm64"
        } else {
            "x86_64"
        },
    );
    let release_url = env::var("PQO_NEW_RELEASE_URL").unwrap_or(default_url);
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
            "could not download Pqo release from {release_url}: {status}"
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
            Some(".pqo" | "agentDB" | "node_modules" | "dist" | "target")
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

fn update_installation() -> Result<(), u8> {
    const INSTALLER_URL: &str = "https://raw.githubusercontent.com/danaia/pqo/main/install.sh";
    let installer_url =
        env::var("PQO_UPDATE_INSTALLER_URL").unwrap_or_else(|_| INSTALLER_URL.to_owned());

    println!("Updating Pqo from {installer_url}...");
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

#[cfg(target_os = "macos")]
fn run_window(
    validated: pqo_validator::ValidatedModuleGraph,
    project_root: Option<std::path::PathBuf>,
    extension_path: Option<std::path::PathBuf>,
    ui: Option<package::LoadedUi>,
) -> Result<(), u8> {
    let ui_project_root = project_root.clone();
    let ui = ui.map(|ui| pqo_metal::ProjectUi {
        project_root: ui_project_root.unwrap_or_else(|| ui.asset_root.clone()),
        asset_root: ui.asset_root,
        entry: ui.entry,
        title: ui.title,
        width: ui.width,
        height: ui.height,
    });
    pqo_metal::MetalRuntime::run_project_with_ui(validated, project_root, extension_path, ui)
        .map_err(|diagnostic| {
            print_json(&serde_json::json!({
                "status": "runtime_error",
                "diagnostic": diagnostic,
            }));
            1
        })
}

#[cfg(all(not(target_os = "macos"), not(target_os = "linux")))]
fn run_window(
    _validated: pqo_validator::ValidatedModuleGraph,
    _project_root: Option<std::path::PathBuf>,
    _extension_path: Option<std::path::PathBuf>,
    _ui: Option<package::LoadedUi>,
) -> Result<(), u8> {
    print_json(&serde_json::json!({
        "status": "unsupported",
        "message": "running Pqo programs currently requires macOS and Metal",
    }));
    Err(2)
}

#[cfg(target_os = "linux")]
fn run_window(
    validated: pqo_validator::ValidatedModuleGraph,
    project_root: Option<std::path::PathBuf>,
    _extension_path: Option<std::path::PathBuf>,
    ui: Option<package::LoadedUi>,
) -> Result<(), u8> {
    if validated.target_profile() == TargetProfile::cuda_vulkan() {
        let title = format!("Pqo — {} — CUDA / Vulkan", validated.graph().name);
        let config = pqo_cuda::CudaConfig {
            ticks: std::env::var("PQO_HEADLESS_TICKS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_else(|| {
                    if validated.graph().name == "hello_crystal_cuda" {
                        240
                    } else {
                        1
                    }
                }),
            ..Default::default()
        };
        let report =
            pqo_cuda::CudaRuntime::run_headless(&validated, project_root.as_deref(), config)
                .map_err(|message| {
                    print_json(&serde_json::json!({
                        "status": "runtime_error",
                        "message": message,
                    }));
                    1
                })?;
        print_json(&report);
        let controls = if let (Some(ui), Some(project_root)) = (ui, project_root) {
            let (sender, receiver) = mpsc::channel();
            std::thread::spawn(move || {
                if let Err(message) =
                    launch_linux_panel_with_controls(ui, project_root, Some(sender))
                {
                    eprintln!("Pqo controls window closed: {message}");
                }
            });
            Some(receiver)
        } else {
            None
        };
        return pqo_vulkan::run_native_window_with_controls(
            pqo_vulkan::NativeWindowConfig {
                title,
                ..Default::default()
            },
            controls,
        )
        .map_err(|message| {
            print_json(&serde_json::json!({
                "status": "vulkan_error",
                "message": message,
            }));
            1
        });
    }
    if validated.target_profile() != TargetProfile::cuda_headless() {
        print_json(&serde_json::json!({
            "status": "unsupported",
            "message": "the Linux runtime requires cuda-vulkan or cuda-headless"
        }));
        return Err(2);
    }
    let ticks = std::env::var("PQO_HEADLESS_TICKS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| {
            if validated.graph().name == "hello_crystal_cuda" {
                240
            } else {
                1
            }
        });
    let config = pqo_cuda::CudaConfig {
        ticks,
        ..Default::default()
    };
    match pqo_cuda::CudaRuntime::run_headless(&validated, project_root.as_deref(), config) {
        Ok(report) => {
            print_json(&report);
            if let (Some(ui), Some(project_root)) = (ui, project_root) {
                launch_linux_panel(ui, project_root).map_err(|message| {
                    print_json(&serde_json::json!({
                        "status": "ui_error",
                        "message": message,
                    }));
                    1
                })?;
            }
            Ok(())
        }
        Err(message) => {
            print_json(&serde_json::json!({
                "status": "runtime_error",
                "message": message,
            }));
            Err(1)
        }
    }
}

#[cfg(target_os = "linux")]
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LinuxPanelMessage {
    Hello { token: String },
    Set { name: String, value: f32 },
    Reload { generation: u64 },
    WindowFrame { frame: serde_json::Value },
    Quit,
}

#[cfg(target_os = "linux")]
fn launch_linux_panel(ui: package::LoadedUi, project_root: PathBuf) -> Result<(), String> {
    launch_linux_panel_with_controls(ui, project_root, None)
}

#[cfg(target_os = "linux")]
fn launch_linux_panel_with_controls(
    ui: package::LoadedUi,
    project_root: PathBuf,
    controls: Option<mpsc::Sender<pqo_vulkan::VulkanControl>>,
) -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("could not bind Linux UI bridge: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("could not inspect Linux UI bridge: {error}"))?;
    let token = format!(
        "{:x}-{:x}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let executable = linux_panel_executable()?;
    let mut child = ProcessCommand::new(&executable)
        .args([
            "--address",
            &address.to_string(),
            "--token",
            &token,
            "--root",
            &ui.asset_root.to_string_lossy(),
            "--project-root",
            &project_root.to_string_lossy(),
            "--entry",
            &ui.entry,
            "--title",
            &ui.title,
            "--width",
            &ui.width.to_string(),
            "--height",
            &ui.height.to_string(),
        ])
        .spawn()
        .map_err(|error| format!("could not launch `{}`: {error}", executable.display()))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("could not configure Linux UI bridge: {error}"))?;
    let stream = accept_linux_panel(&listener, &mut child)?;
    serve_linux_panel(stream, &token, controls.as_ref())?;
    let _ = child.wait();
    Ok(())
}

#[cfg(target_os = "linux")]
fn accept_linux_panel(listener: &TcpListener, child: &mut Child) -> Result<TcpStream, String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(12);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(format!("Linux UI bridge failed: {error}")),
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("could not inspect Linux UI process: {error}"))?
        {
            return Err(format!("Linux UI exited before connecting: {status}"));
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            return Err("Linux UI did not connect within 12 seconds".to_owned());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(target_os = "linux")]
fn serve_linux_panel(
    mut stream: TcpStream,
    token: &str,
    controls: Option<&mpsc::Sender<pqo_vulkan::VulkanControl>>,
) -> Result<(), String> {
    stream
        .set_nodelay(true)
        .map_err(|error| format!("could not configure Linux UI stream: {error}"))?;
    let reader_stream = stream
        .try_clone()
        .map_err(|error| format!("could not clone Linux UI stream: {error}"))?;
    let mut lines = BufReader::new(reader_stream).lines();
    let hello = lines
        .next()
        .ok_or("Linux UI disconnected during handshake")?
        .map_err(|error| format!("could not read Linux UI handshake: {error}"))?;
    match serde_json::from_str::<LinuxPanelMessage>(&hello) {
        Ok(LinuxPanelMessage::Hello { token: received }) if received == token => {}
        _ => return Err("Linux UI handshake was rejected".to_owned()),
    }
    let mut values = BTreeMap::from([
        ("crystal.growth".to_owned(), 0.72_f32),
        ("crystal.anisotropy".to_owned(), 0.68_f32),
        ("crystal.temperature".to_owned(), 0.18_f32),
        ("crystal.damage".to_owned(), 0.0_f32),
        ("crystal.show_field".to_owned(), 1.0_f32),
        ("crystal.show_particles".to_owned(), 0.0_f32),
        ("crystal.particle_count".to_owned(), 1_000_000.0_f32),
    ]);
    write_linux_snapshot(&mut stream, &values)?;
    for line in lines {
        let line = line.map_err(|error| format!("Linux UI bridge read failed: {error}"))?;
        match serde_json::from_str::<LinuxPanelMessage>(&line) {
            Ok(LinuxPanelMessage::Set { name, value })
                if !name.is_empty() && name.len() < 96 && value.is_finite() =>
            {
                values.insert(name.clone(), value);
                if let Some(controls) = controls {
                    let _ = controls.send(pqo_vulkan::VulkanControl { name, value });
                }
                write_linux_snapshot(&mut stream, &values)?;
            }
            Ok(LinuxPanelMessage::Quit) => break,
            Ok(LinuxPanelMessage::Reload { generation }) => {
                let response = serde_json::json!({
                    "type": "reload_status",
                    "generation": generation,
                    "ok": true,
                    "message": "CUDA crystal UI is current",
                });
                writeln!(stream, "{response}").map_err(|error| error.to_string())?;
                stream.flush().map_err(|error| error.to_string())?;
            }
            Ok(LinuxPanelMessage::WindowFrame { frame }) => {
                let _ = frame;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn write_linux_snapshot(
    stream: &mut TcpStream,
    values: &BTreeMap<String, f32>,
) -> Result<(), String> {
    let response = serde_json::json!({ "type": "snapshot", "values": values });
    writeln!(stream, "{response}").map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn linux_panel_executable() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("PQO_UI_PANEL_BIN").map(PathBuf::from) {
        return path
            .is_file()
            .then_some(path.clone())
            .ok_or_else(|| format!("PQO_UI_PANEL_BIN points to missing `{}`", path.display()));
    }
    let current = env::current_exe().map_err(|error| error.to_string())?;
    let sibling = current.with_file_name("pqo-ui-panel");
    if sibling.is_file() {
        return Ok(sibling);
    }
    let resolved = current
        .canonicalize()
        .ok()
        .map(|path| path.with_file_name("pqo-ui-panel"));
    resolved
        .filter(|path| path.is_file())
        .ok_or_else(|| format!("pqo-ui-panel is missing beside `{}`", current.display()))
}

fn print_json(value: &impl Serialize) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("CLI result must serialize")
    );
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        CliAction, Command, copy_project_tree, parse_arguments, personalize_controls_title,
        rename_baseline_source, verify_metal_view_project,
    };

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn a_source_path_runs_without_a_subcommand() {
        assert_eq!(
            parse_arguments(&args(&["hello-particle.pqo"])),
            Ok(CliAction::Execute {
                command: Command::Run,
                path: "hello-particle.pqo".to_owned(),
                target: None,
            })
        );
    }

    #[test]
    fn explicit_commands_remain_available() {
        assert_eq!(
            parse_arguments(&args(&["check", "hello-particle.pqo"])),
            Ok(CliAction::Execute {
                command: Command::Check,
                path: "hello-particle.pqo".to_owned(),
                target: None,
            })
        );
        assert_eq!(
            parse_arguments(&args(&["build", "hello-particle.pqo"])),
            Ok(CliAction::Execute {
                command: Command::Build,
                path: "hello-particle.pqo".to_owned(),
                target: None,
            })
        );
    }

    #[test]
    fn explicit_build_target_is_parsed() {
        assert_eq!(
            parse_arguments(&args(&[
                "build",
                "hello-cuda.pqo",
                "--target",
                "cuda-headless",
            ])),
            Ok(CliAction::Execute {
                command: Command::Build,
                path: "hello-cuda.pqo".to_owned(),
                target: Some(pqo_core::TargetProfile::cuda_headless()),
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

    #[test]
    fn new_renames_the_baseline_source_and_removes_its_package() {
        let project = tempfile::tempdir().expect("project directory");
        fs::write(project.path().join("baseline.pqo"), "module baseline").expect("baseline source");
        fs::write(project.path().join("baseline.lmp"), "stale package").expect("baseline package");

        rename_baseline_source(project.path(), "network").expect("rename starter source");

        assert!(project.path().join("network.pqo").is_file());
        assert!(!project.path().join("baseline.pqo").exists());
        assert!(!project.path().join("baseline.lmp").exists());
    }

    #[test]
    fn new_personalizes_the_controls_window_title() {
        let project = tempfile::tempdir().expect("project directory");
        let ui = project.path().join("ui");
        fs::create_dir(&ui).expect("UI directory");
        fs::write(
            ui.join("pqo-ui.json"),
            r#"{"title":"Pqo Baseline — Controls","width":340}"#,
        )
        .expect("UI config");

        personalize_controls_title(project.path(), "network").expect("personalize title");

        let config: serde_json::Value =
            serde_json::from_slice(&fs::read(ui.join("pqo-ui.json")).expect("UI config"))
                .expect("valid UI config");
        assert_eq!(config["title"], "Pqo network — Controls");
    }

    #[test]
    fn new_requires_a_presented_metal_view() {
        let project = tempfile::tempdir().expect("project directory");
        fs::write(
            project.path().join("network.pqo"),
            include_str!("../../../baseline/baseline.pqo"),
        )
        .expect("baseline source");

        verify_metal_view_project(project.path(), "network").expect("presented Metal view");

        let headless = include_str!("../../../baseline/baseline.pqo")
            .replace(
                "view viewport(\n  color=render.color\n  position=render.position\n  radius=render.radius\n  scale=render.aspect\n) extern metal {\n  source=\"shaders/baseline.metal\"\n  entry=\"baseline_pipeline\"\n}\n\n",
                "",
            )
            .replace("  draw viewport after project\n", "");
        fs::write(project.path().join("headless.pqo"), headless).expect("headless source");
        let error = verify_metal_view_project(project.path(), "headless")
            .expect_err("headless starter must be rejected");
        assert!(error.contains("must declare and draw a Metal view"));
    }

    #[test]
    fn new_does_not_copy_runtime_or_build_state() {
        let source = tempfile::tempdir().expect("source directory");
        let destination = tempfile::tempdir().expect("destination parent");
        for excluded in [".pqo", "agentDB", "dist", "node_modules", "target"] {
            let directory = source.path().join(excluded);
            fs::create_dir(&directory).expect("excluded directory");
            fs::write(directory.join("state.json"), "{}").expect("excluded state");
        }
        fs::write(source.path().join("baseline.pqo"), "module baseline").expect("baseline source");
        let project = destination.path().join("clean-project");

        copy_project_tree(source.path(), &project).expect("copy clean project");

        assert!(project.join("baseline.pqo").is_file());
        for excluded in [".pqo", "agentDB", "dist", "node_modules", "target"] {
            assert!(!project.join(excluded).exists());
        }
    }
}
