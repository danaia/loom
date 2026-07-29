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

    let loaded = match package::load(&path, command == Command::Run && can_prepare_source_run()) {
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
            target: target_name(&graph.target),
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
    rename_baseline_source(&staged_project, project_name)
        .map_err(|message| new_error("rename", &message))?;
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
    println!("Created Loom project at {}", destination.display());
    Ok(())
}

fn rename_baseline_source(project_root: &Path, project_name: &str) -> Result<(), String> {
    let baseline_source = project_root.join("baseline.loom");
    if !baseline_source.is_file() {
        return Err(format!(
            "Baseline template is missing `{}`",
            baseline_source.display()
        ));
    }
    let project_source = project_root.join(format!("{project_name}.loom"));
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

fn personalize_controls_title(project_root: &Path, project_name: &str) -> Result<(), String> {
    let config_path = project_root.join("ui/loom-ui.json");
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
        serde_json::Value::String(format!("Loom {project_name} — Controls")),
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
        "https://github.com/danaia/loom/releases/download/v{}/{}.tar.gz",
        env!("CARGO_PKG_VERSION"),
        release_asset_name()
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
            Some(".loom" | "agentDB" | "node_modules" | "dist" | "target")
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
    const INSTALLER_URL: &str = "https://raw.githubusercontent.com/danaia/loom/main/install.sh";
    let installer_url =
        env::var("LOOM_UPDATE_INSTALLER_URL").unwrap_or_else(|_| INSTALLER_URL.to_owned());
    let loom_home = current_install_home();
    let backend = installed_manifest_value("backend").unwrap_or_else(current_backend_name);

    println!("Updating Loom {backend} from {installer_url}...");
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
    let mut install = ProcessCommand::new("sh");
    install
        .stdin(Stdio::from(installer))
        .env("LOOM_BACKEND", backend);
    if let Some(home) = loom_home {
        install.env("LOOM_HOME", home);
    }
    let install_status = install.status();
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

fn current_install_home() -> Option<PathBuf> {
    env::current_exe()
        .ok()?
        .parent()?
        .parent()
        .map(Path::to_path_buf)
}

fn installed_manifest_value(key: &str) -> Option<String> {
    let manifest = current_install_home()?.join("install-manifest");
    let contents = fs::read_to_string(manifest).ok()?;
    contents.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name == key).then(|| value.to_owned())
    })
}

fn current_backend_name() -> String {
    env::var("LOOM_BACKEND").unwrap_or_else(|_| {
        if cfg!(target_os = "linux") {
            "cuda".to_owned()
        } else {
            "metal".to_owned()
        }
    })
}

fn release_asset_name() -> String {
    let backend = installed_manifest_value("backend").unwrap_or_else(current_backend_name);
    let platform = installed_manifest_value("platform").unwrap_or_else(|| {
        if cfg!(target_os = "macos") {
            "darwin".to_owned()
        } else if cfg!(target_os = "linux") {
            "linux".to_owned()
        } else {
            env::consts::OS.to_owned()
        }
    });
    let architecture = installed_manifest_value("architecture").unwrap_or_else(|| {
        if cfg!(target_arch = "aarch64") {
            "arm64".to_owned()
        } else {
            env::consts::ARCH.to_owned()
        }
    });
    format!("loom-{backend}-{platform}-{architecture}")
}

fn target_name(target: &loom_core::Target) -> &'static str {
    match target {
        loom_core::Target::Metal => "metal",
        loom_core::Target::Cuda => "cuda",
    }
}

fn can_prepare_source_run() -> bool {
    cfg!(target_os = "macos")
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
        "message": "CUDA execution is not available in this CLI yet; use `loom-cuda check` or `loom-cuda explain` for CUDA-target validation until the loom-cuda runtime backend lands",
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
    use std::fs;

    use super::{
        CliAction, Command, copy_project_tree, parse_arguments, personalize_controls_title,
        rename_baseline_source,
    };

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

    #[test]
    fn new_renames_the_baseline_source_and_removes_its_package() {
        let project = tempfile::tempdir().expect("project directory");
        fs::write(project.path().join("baseline.loom"), "module baseline")
            .expect("baseline source");
        fs::write(project.path().join("baseline.lmp"), "stale package").expect("baseline package");

        rename_baseline_source(project.path(), "network").expect("rename starter source");

        assert!(project.path().join("network.loom").is_file());
        assert!(!project.path().join("baseline.loom").exists());
        assert!(!project.path().join("baseline.lmp").exists());
    }

    #[test]
    fn new_personalizes_the_controls_window_title() {
        let project = tempfile::tempdir().expect("project directory");
        let ui = project.path().join("ui");
        fs::create_dir(&ui).expect("UI directory");
        fs::write(
            ui.join("loom-ui.json"),
            r#"{"title":"Loom Baseline — Controls","width":340}"#,
        )
        .expect("UI config");

        personalize_controls_title(project.path(), "network").expect("personalize title");

        let config: serde_json::Value =
            serde_json::from_slice(&fs::read(ui.join("loom-ui.json")).expect("UI config"))
                .expect("valid UI config");
        assert_eq!(config["title"], "Loom network — Controls");
    }

    #[test]
    fn new_does_not_copy_runtime_or_build_state() {
        let source = tempfile::tempdir().expect("source directory");
        let destination = tempfile::tempdir().expect("destination parent");
        for excluded in [".loom", "agentDB", "dist", "node_modules", "target"] {
            let directory = source.path().join(excluded);
            fs::create_dir(&directory).expect("excluded directory");
            fs::write(directory.join("state.json"), "{}").expect("excluded state");
        }
        fs::write(source.path().join("baseline.loom"), "module baseline")
            .expect("baseline source");
        let project = destination.path().join("clean-project");

        copy_project_tree(source.path(), &project).expect("copy clean project");

        assert!(project.join("baseline.loom").is_file());
        for excluded in [".loom", "agentDB", "dist", "node_modules", "target"] {
            assert!(!project.join(excluded).exists());
        }
    }
}
