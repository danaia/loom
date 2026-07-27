use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};

use loom_core::ModuleGraph;
use loom_windowing::{PROJECT_CONFIG_PATH, WindowLayoutConfig};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const MANIFEST_NAME: &str = "loom-package.json";
const FORMAT_NAME: &str = "loom-package";
const FORMAT_VERSION: u32 = 1;
const PROJECT_ABI_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackageManifest {
    format: String,
    version: u32,
    module: String,
    entry: String,
    files: Vec<String>,
    extension: Option<PackageExtension>,
    #[serde(default)]
    ui: Option<PackageUi>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackageExtension {
    abi: u32,
    target: String,
    path: String,
    source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackageUi {
    framework: String,
    root: String,
    entry: String,
    title: String,
    width: f64,
    height: f64,
}

#[derive(Debug, Deserialize)]
struct UiSourceConfig {
    framework: String,
    #[serde(default = "default_ui_dist")]
    dist: String,
    #[serde(default = "default_ui_entry")]
    entry: String,
    title: Option<String>,
    #[serde(default = "default_ui_width")]
    width: f64,
    #[serde(default = "default_ui_height")]
    height: f64,
}

#[derive(Clone)]
pub(crate) struct LoadedUi {
    pub(crate) asset_root: PathBuf,
    pub(crate) entry: String,
    pub(crate) title: String,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

pub(crate) struct LoadedProgram {
    source: String,
    project_root: PathBuf,
    extension_path: Option<PathBuf>,
    ui: Option<LoadedUi>,
    _extracted: Option<TempDir>,
}

impl LoadedProgram {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn project_root(&self) -> Option<&Path> {
        Some(&self.project_root)
    }

    pub(crate) fn extension_path(&self) -> Option<&Path> {
        self.extension_path.as_deref()
    }

    pub(crate) fn ui(&self) -> Option<&LoadedUi> {
        self.ui.as_ref()
    }
}

pub(crate) fn load(path: &str, prepare_source_project: bool) -> Result<LoadedProgram, String> {
    let path = Path::new(path);
    if path.extension().and_then(|value| value.to_str()) == Some("lmp") {
        load_package(path)
    } else {
        let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
        let project_root = project_parent(path)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let (extension_path, ui) = if prepare_source_project {
            let module = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("project");
            (
                compile_project_extension(&project_root)?,
                load_source_ui(&project_root, module)?,
            )
        } else {
            (None, None)
        };
        Ok(LoadedProgram {
            source,
            project_root,
            extension_path,
            ui,
            _extracted: None,
        })
    }
}

pub(crate) fn build(path: &str, graph: &ModuleGraph) -> Result<String, String> {
    let source_path = Path::new(path);
    if source_path.extension().and_then(|value| value.to_str()) != Some("loom") {
        return Err("`loom build` expects a primary .loom source file".to_owned());
    }
    let root = project_parent(source_path)
        .canonicalize()
        .map_err(|error| format!("could not resolve project directory: {error}"))?;
    let entry_name = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "the primary .loom file name is not valid UTF-8".to_owned())?
        .to_owned();

    let mut source_files = BTreeSet::new();
    source_files.insert(entry_name.clone());
    for implementation in graph
        .kernels
        .iter()
        .flat_map(|kernel| kernel.implementations.iter())
        .chain(graph.views.iter().map(|view| &view.implementation))
    {
        if implementation.source_text.is_none() && !implementation.source.starts_with("loom://") {
            validate_relative_path(&implementation.source)?;
            let source = root.join(&implementation.source);
            if !source.is_file() {
                return Err(format!(
                    "project Metal source `{}` must live beside the primary .loom file",
                    implementation.source
                ));
            }
            source_files.insert(implementation.source.clone());
        }
    }

    let target = host_target()?;
    let extension_archive_path = format!("runtime/{target}/libloom_project.dylib");
    let extension_disk_path = compile_project_extension(&root)?;
    let extension = if extension_disk_path.is_some() {
        source_files.insert("src/runtime.rs".to_owned());
        Some(PackageExtension {
            abi: PROJECT_ABI_VERSION,
            target: target.clone(),
            path: extension_archive_path.clone(),
            source: "src/runtime.rs".to_owned(),
        })
    } else {
        None
    };
    if root.join("README.md").is_file() {
        source_files.insert("README.md".to_owned());
    }
    let window_config_path = root.join(PROJECT_CONFIG_PATH);
    if window_config_path.is_file() {
        WindowLayoutConfig::load(&root)?;
        source_files.insert(PROJECT_CONFIG_PATH.to_owned());
    }
    let ui = build_ui(&root, &graph.name, &mut source_files)?;

    let mut packaged_files = source_files.iter().cloned().collect::<Vec<_>>();
    if extension.is_some() {
        packaged_files.push(extension_archive_path.clone());
    }
    packaged_files.sort();
    let manifest = PackageManifest {
        format: FORMAT_NAME.to_owned(),
        version: FORMAT_VERSION,
        module: graph.name.clone(),
        entry: entry_name,
        files: packaged_files,
        extension,
        ui,
    };

    let output_path = root.join(format!(
        "{}.lmp",
        source_path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "the primary .loom file stem is not valid UTF-8".to_owned())?
    ));
    let output_file = File::create(&output_path)
        .map_err(|error| format!("could not create `{}`: {error}", output_path.display()))?;
    let mut archive = ZipWriter::new(output_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    archive
        .start_file(MANIFEST_NAME, options)
        .map_err(|error| error.to_string())?;
    archive
        .write_all(
            serde_json::to_string_pretty(&manifest)
                .map_err(|error| error.to_string())?
                .as_bytes(),
        )
        .map_err(|error| error.to_string())?;
    for relative in &source_files {
        add_file(&mut archive, &root.join(relative), relative, options)?;
    }
    if manifest.extension.is_some() {
        add_file(
            &mut archive,
            extension_disk_path
                .as_deref()
                .expect("package extension has a compiled library"),
            &extension_archive_path,
            options.unix_permissions(0o755),
        )?;
    }
    archive.finish().map_err(|error| error.to_string())?;
    Ok(output_path.display().to_string())
}

fn load_package(path: &Path) -> Result<LoadedProgram, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("`{}` is not a valid .lmp archive: {error}", path.display()))?;
    let manifest = {
        let mut entry = archive
            .by_name(MANIFEST_NAME)
            .map_err(|_| format!("`{}` has no {MANIFEST_NAME}", path.display()))?;
        let mut json = String::new();
        entry
            .read_to_string(&mut json)
            .map_err(|error| error.to_string())?;
        serde_json::from_str::<PackageManifest>(&json)
            .map_err(|error| format!("invalid {MANIFEST_NAME}: {error}"))?
    };
    if manifest.format != FORMAT_NAME || manifest.version != FORMAT_VERSION {
        return Err(format!(
            "unsupported .lmp format `{}` version {}; expected {FORMAT_NAME} version {FORMAT_VERSION}",
            manifest.format, manifest.version
        ));
    }
    validate_relative_path(&manifest.entry)?;
    if let Some(extension) = manifest.extension.as_ref() {
        validate_relative_path(&extension.path)?;
        if extension.abi != PROJECT_ABI_VERSION {
            return Err(format!(
                "package extension ABI {} is incompatible with runtime ABI {PROJECT_ABI_VERSION}",
                extension.abi
            ));
        }
        let host = host_target()?;
        if extension.target != host {
            return Err(format!(
                "package extension target `{}` cannot run on `{host}`; rebuild the .lmp on this platform",
                extension.target
            ));
        }
    }
    if let Some(ui) = manifest.ui.as_ref() {
        validate_relative_path(&ui.root)?;
        validate_relative_path(&ui.entry)?;
        if ui.framework != "vue3" {
            return Err(format!(
                "package UI framework `{}` is unsupported; expected `vue3`",
                ui.framework
            ));
        }
        validate_ui_dimension(ui.width, "width")?;
        validate_ui_dimension(ui.height, "height")?;
    }

    let extracted =
        tempfile::tempdir().map_err(|error| format!("could not create package cache: {error}"))?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(format!(
                "package entry `{}` escapes the .lmp archive",
                entry.name()
            ));
        };
        let destination = extracted.path().join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut output = File::create(&destination).map_err(|error| error.to_string())?;
        io::copy(&mut entry, &mut output).map_err(|error| error.to_string())?;
    }
    let source_path = extracted.path().join(&manifest.entry);
    let source = fs::read_to_string(&source_path).map_err(|error| {
        format!(
            "could not read packaged entry `{}`: {error}",
            manifest.entry
        )
    })?;
    let extension_path = manifest
        .extension
        .as_ref()
        .map(|extension| extracted.path().join(&extension.path));
    let ui = manifest.ui.as_ref().map(|ui| LoadedUi {
        asset_root: extracted.path().join(&ui.root),
        entry: ui.entry.clone(),
        title: ui.title.clone(),
        width: ui.width,
        height: ui.height,
    });
    if let Some(ui) = ui.as_ref() {
        let entry = ui.asset_root.join(&ui.entry);
        if !entry.is_file() {
            return Err(format!(
                "packaged UI entry `{}` is missing",
                entry.display()
            ));
        }
    }
    Ok(LoadedProgram {
        source,
        project_root: extracted.path().to_owned(),
        extension_path,
        ui,
        _extracted: Some(extracted),
    })
}

fn build_ui(
    root: &Path,
    module: &str,
    source_files: &mut BTreeSet<String>,
) -> Result<Option<PackageUi>, String> {
    let ui_root = root.join("ui");
    let Some(config) = read_ui_config(&ui_root)? else {
        return Ok(None);
    };
    build_ui_assets(&ui_root)?;

    let asset_root = ui_root.join(&config.dist);
    let entry_path = asset_root.join(&config.entry);
    if !entry_path.is_file() {
        return Err(format!(
            "project UI entry `{}` is missing after build",
            entry_path.display()
        ));
    }
    collect_ui_files(root, &ui_root, source_files)?;
    Ok(Some(PackageUi {
        framework: config.framework,
        root: format!("ui/{}", config.dist),
        entry: config.entry,
        title: config
            .title
            .unwrap_or_else(|| format!("Loom — {}", module.replace('_', " "))),
        width: config.width,
        height: config.height,
    }))
}

fn load_source_ui(root: &Path, module: &str) -> Result<Option<LoadedUi>, String> {
    let ui_root = root.join("ui");
    let Some(config) = read_ui_config(&ui_root)? else {
        return Ok(None);
    };
    build_ui_assets(&ui_root)?;
    let asset_root = ui_root.join(&config.dist);
    let entry = asset_root.join(&config.entry);
    if !entry.is_file() {
        return Err(format!(
            "project UI entry `{}` is missing after build",
            entry.display()
        ));
    }
    Ok(Some(LoadedUi {
        asset_root,
        entry: config.entry,
        title: config
            .title
            .unwrap_or_else(|| format!("Loom — {}", module.replace('_', " "))),
        width: config.width,
        height: config.height,
    }))
}

fn read_ui_config(ui_root: &Path) -> Result<Option<UiSourceConfig>, String> {
    let config_path = ui_root.join("loom-ui.json");
    if !config_path.is_file() {
        return Ok(None);
    }
    let config = serde_json::from_slice::<UiSourceConfig>(
        &fs::read(&config_path)
            .map_err(|error| format!("could not read `{}`: {error}", config_path.display()))?,
    )
    .map_err(|error| format!("invalid `{}`: {error}", config_path.display()))?;
    if config.framework != "vue3" {
        return Err(format!(
            "UI framework `{}` is unsupported; expected `vue3`",
            config.framework
        ));
    }
    validate_relative_path(&config.dist)?;
    validate_relative_path(&config.entry)?;
    validate_ui_dimension(config.width, "width")?;
    validate_ui_dimension(config.height, "height")?;
    Ok(Some(config))
}

fn build_ui_assets(ui_root: &Path) -> Result<(), String> {
    let package_json = ui_root.join("package.json");
    if !package_json.is_file() {
        return Ok(());
    }
    let npm = std::env::var_os("LOOM_NPM").unwrap_or_else(|| "npm".into());
    let output = Command::new(npm)
        .args(["run", "build", "--prefix"])
        .arg(ui_root)
        .output()
        .map_err(|error| format!("could not build project UI with npm: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "project UI failed to build:\n{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn compile_project_extension(root: &Path) -> Result<Option<PathBuf>, String> {
    let source = root.join("src/runtime.rs");
    if !source.is_file() {
        return Ok(None);
    }
    let build_directory = root.join(".loom/build");
    fs::create_dir_all(&build_directory)
        .map_err(|error| format!("could not create project build directory: {error}"))?;
    let output_path = build_directory.join("libloom_project.dylib");
    let output = Command::new("rustc")
        .args(["--crate-type", "cdylib", "--edition", "2021"])
        .arg("-C")
        .arg("opt-level=2")
        .arg(&source)
        .arg("-o")
        .arg(&output_path)
        .output()
        .map_err(|error| format!("could not invoke rustc for the project extension: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "project Rust extension failed to compile:\n{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(Some(output_path))
}

fn collect_ui_files(
    project_root: &Path,
    directory: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("could not inspect `{}`: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        if name == "node_modules" || name == ".DS_Store" {
            continue;
        }
        if path.is_dir() {
            collect_ui_files(project_root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(project_root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            validate_relative_path(&relative)?;
            files.insert(relative);
        }
    }
    Ok(())
}

fn validate_ui_dimension(value: f64, field: &str) -> Result<(), String> {
    if !value.is_finite() || !(200.0..=4096.0).contains(&value) {
        return Err(format!("project UI {field} must be between 200 and 4096"));
    }
    Ok(())
}

fn default_ui_dist() -> String {
    "dist".to_owned()
}

fn default_ui_entry() -> String {
    "index.html".to_owned()
}

fn default_ui_width() -> f64 {
    380.0
}

fn default_ui_height() -> f64 {
    720.0
}

fn add_file(
    archive: &mut ZipWriter<File>,
    disk_path: &Path,
    archive_path: &str,
    options: SimpleFileOptions,
) -> Result<(), String> {
    validate_relative_path(archive_path)?;
    archive
        .start_file(archive_path, options)
        .map_err(|error| error.to_string())?;
    let bytes = fs::read(disk_path)
        .map_err(|error| format!("could not read `{}`: {error}", disk_path.display()))?;
    archive.write_all(&bytes).map_err(|error| error.to_string())
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!(
            "package paths must stay inside the project directory: `{}`",
            path.display()
        ));
    }
    Ok(())
}

fn project_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn host_target() -> Result<String, String> {
    let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|error| format!("could not inspect the Rust host target: {error}"))?;
    if !output.status.success() {
        return Err("`rustc -vV` failed while determining the package target".to_owned());
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "`rustc -vV` did not report a host target".to_owned())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::{load, project_parent};

    #[test]
    fn bare_source_filename_uses_current_directory() {
        assert_eq!(
            project_parent(Path::new("marble-water.loom")),
            Path::new(".")
        );
        assert_eq!(
            project_parent(Path::new("./marble-water.loom")),
            Path::new(".")
        );
    }

    #[test]
    fn source_run_discovers_built_project_ui() {
        let project = tempfile::tempdir().expect("temporary project");
        fs::write(
            project.path().join("demo.loom"),
            "loom 0.1\nmodule demo\ntarget metal\n",
        )
        .expect("source");
        fs::create_dir_all(project.path().join("ui/dist")).expect("UI directories");
        fs::write(
            project.path().join("ui/loom-ui.json"),
            r#"{
              "framework": "vue3",
              "dist": "dist",
              "entry": "index.html",
              "title": "Demo Controls",
              "width": 360,
              "height": 650
            }"#,
        )
        .expect("UI config");
        fs::write(
            project.path().join("ui/dist/index.html"),
            "<!doctype html><title>Demo</title>",
        )
        .expect("UI entry");

        let program = load(
            project
                .path()
                .join("demo.loom")
                .to_str()
                .expect("UTF-8 path"),
            true,
        )
        .expect("source project loads");
        let ui = program.ui().expect("source UI");
        assert_eq!(ui.title, "Demo Controls");
        assert_eq!(ui.width, 360.0);
        assert_eq!(ui.height, 650.0);
        assert!(ui.asset_root.join(&ui.entry).is_file());
    }
}
