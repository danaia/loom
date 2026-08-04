use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};

use pqo_core::{
    Backend, BackendRequirements, ComputeBackend, ModuleGraph, ShaderStage, SourceFormat,
    TargetProfile, ViewBackend,
};
use pqo_windowing::{PROJECT_CONFIG_PATH, WindowLayoutConfig};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const MANIFEST_NAME: &str = "pqo-package.json";
const FORMAT_NAME: &str = "pqo-package";
const FORMAT_VERSION: u32 = 2;
const PROJECT_ABI_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackageManifest {
    format: String,
    version: u32,
    module: String,
    entry: String,
    files: Vec<String>,
    #[serde(default)]
    target_profile: Option<TargetProfile>,
    #[serde(default)]
    artifacts: Vec<PackageArtifact>,
    #[serde(default)]
    requirements: Vec<BackendRequirements>,
    extension: Option<PackageExtension>,
    #[serde(default)]
    ui: Option<PackageUi>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackageArtifact {
    kind: String,
    backend: String,
    source: String,
    entries: Vec<String>,
    path: String,
    architecture: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct TargetArtifactManifest {
    format: &'static str,
    version: u32,
    backend: &'static str,
    artifacts: Vec<PackageArtifact>,
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
#[cfg_attr(target_os = "linux", allow(dead_code))]
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
    target_profile: Option<TargetProfile>,
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

    pub(crate) fn target_profile(&self) -> Option<TargetProfile> {
        self.target_profile
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
            target_profile: None,
            _extracted: None,
        })
    }
}

pub(crate) fn build(
    path: &str,
    graph: &ModuleGraph,
    target_profile: TargetProfile,
) -> Result<String, String> {
    let source_path = Path::new(path);
    if source_path.extension().and_then(|value| value.to_str()) != Some("pqo") {
        return Err("`pqo build` expects a primary .pqo source file".to_owned());
    }
    let root = project_parent(source_path)
        .canonicalize()
        .map_err(|error| format!("could not resolve project directory: {error}"))?;
    let entry_name = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "the primary .pqo file name is not valid UTF-8".to_owned())?
        .to_owned();

    let mut source_files = BTreeSet::new();
    source_files.insert(entry_name.clone());
    for implementation in graph
        .kernels
        .iter()
        .flat_map(|kernel| kernel.implementations.iter())
        .chain(
            graph
                .views
                .iter()
                .flat_map(|view| view.implementations.iter()),
        )
    {
        if implementation.source_text.is_none() && !implementation.source.starts_with("pqo://") {
            validate_relative_path(&implementation.source)?;
            let source = root.join(&implementation.source);
            if !source.is_file() {
                return Err(format!(
                    "project GPU source `{}` must live beside the primary .pqo file",
                    implementation.source
                ));
            }
            source_files.insert(implementation.source.clone());
        }
    }

    let target = host_target()?;
    let library_extension = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let extension_archive_path = format!("runtime/{target}/libpqo_project.{library_extension}");
    let extension_disk_path = compile_project_extension(&root)?;
    let (artifacts, generated_artifacts) = build_target_artifacts(&root, graph, target_profile)?;
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
    packaged_files.extend(generated_artifacts.iter().map(|(path, _)| path.clone()));
    packaged_files.sort();
    let manifest = PackageManifest {
        format: FORMAT_NAME.to_owned(),
        version: FORMAT_VERSION,
        module: graph.name.clone(),
        entry: entry_name,
        files: packaged_files,
        target_profile: Some(target_profile),
        artifacts,
        requirements: target_requirements(target_profile),
        extension,
        ui,
    };

    let output_path = root.join(format!(
        "{}.lmp",
        source_path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "the primary .pqo file stem is not valid UTF-8".to_owned())?
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
    for (archive_path, disk_path) in &generated_artifacts {
        add_file(&mut archive, disk_path, archive_path, options)?;
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
    if manifest.format != FORMAT_NAME || !(1..=FORMAT_VERSION).contains(&manifest.version) {
        return Err(format!(
            "unsupported .lmp format `{}` version {}; expected {FORMAT_NAME} version 1..={FORMAT_VERSION}",
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
        target_profile: manifest.target_profile,
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
            .unwrap_or_else(|| format!("Pqo — {}", module.replace('_', " "))),
        width: config.width,
        height: config.height,
    }))
}

fn load_source_ui(root: &Path, module: &str) -> Result<Option<LoadedUi>, String> {
    let platform_ui_root = root.join(format!("ui-{module}"));
    let ui_root = if platform_ui_root.join("pqo-ui.json").is_file() {
        platform_ui_root
    } else {
        root.join("ui")
    };
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
            .unwrap_or_else(|| format!("Pqo — {}", module.replace('_', " "))),
        width: config.width,
        height: config.height,
    }))
}

fn read_ui_config(ui_root: &Path) -> Result<Option<UiSourceConfig>, String> {
    let config_path = ui_root.join("pqo-ui.json");
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
    let npm = std::env::var_os("PQO_NPM").unwrap_or_else(|| "npm".into());
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
    let build_directory = root.join(".pqo/build");
    fs::create_dir_all(&build_directory)
        .map_err(|error| format!("could not create project build directory: {error}"))?;
    let extension = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let output_path = build_directory.join(format!("libpqo_project.{extension}"));
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

fn build_target_artifacts(
    root: &Path,
    graph: &ModuleGraph,
    target: TargetProfile,
) -> Result<(Vec<PackageArtifact>, Vec<(String, PathBuf)>), String> {
    if target.compute != pqo_core::ComputeBackend::Cuda {
        return Ok((Vec::new(), Vec::new()));
    }
    let build_root = root.join(".pqo/build/cuda");
    fs::create_dir_all(&build_root)
        .map_err(|error| format!("could not create CUDA build directory: {error}"))?;
    let mut artifacts = Vec::new();
    let mut generated = Vec::new();
    let mut seen_sources = BTreeSet::new();
    for kernel in &graph.kernels {
        let implementation = kernel
            .implementations
            .iter()
            .find(|implementation| implementation.backend == Backend::Cuda)
            .ok_or_else(|| format!("kernel `{}` has no CUDA implementation", kernel.name))?;
        if !seen_sources.insert(implementation.source.clone()) {
            continue;
        }
        let stem = sanitize_artifact_name(&kernel.name);
        let input = if let Some(source) = &implementation.source_text {
            let path = build_root.join(format!("{stem}.cu"));
            fs::write(&path, source)
                .map_err(|error| format!("could not write generated CUDA source: {error}"))?;
            path
        } else {
            root.join(&implementation.source)
        };
        let cubin_disk = build_root.join(format!("{stem}.sm_120.cubin"));
        let ptx_disk = build_root.join(format!("{stem}.compute_120.ptx"));
        run_nvcc(&input, &cubin_disk, &["-cubin", "-arch=sm_120"])?;
        run_nvcc(&input, &ptx_disk, &["-ptx", "-arch=compute_120"])?;
        let cubin_archive = format!("targets/linux-x86_64-nvidia/compute/{stem}.sm_120.cubin");
        let ptx_archive = format!("targets/linux-x86_64-nvidia/compute/{stem}.compute_120.ptx");
        let entries = implementation.entry_points.clone();
        artifacts.push(PackageArtifact {
            kind: "cubin".to_owned(),
            backend: "cuda".to_owned(),
            source: implementation.source.clone(),
            entries: entries.clone(),
            path: cubin_archive.clone(),
            architecture: Some("sm_120".to_owned()),
        });
        artifacts.push(PackageArtifact {
            kind: "ptx".to_owned(),
            backend: "cuda".to_owned(),
            source: implementation.source.clone(),
            entries,
            path: ptx_archive.clone(),
            architecture: Some("compute_120".to_owned()),
        });
        generated.push((cubin_archive, cubin_disk));
        generated.push((ptx_archive, ptx_disk));
    }
    if target.view == ViewBackend::Vulkan {
        let view_build_root = root.join(".pqo/build/vulkan");
        fs::create_dir_all(&view_build_root)
            .map_err(|error| format!("could not create Vulkan build directory: {error}"))?;
        for view in &graph.views {
            for implementation in view
                .implementations
                .iter()
                .filter(|implementation| implementation.backend == Backend::Vulkan)
            {
                let stage = implementation.stage.ok_or_else(|| {
                    format!(
                        "Vulkan view `{}` implementation has no shader stage",
                        view.name
                    )
                })?;
                let suffix = match stage {
                    ShaderStage::Vertex => "vert",
                    ShaderStage::Fragment => "frag",
                };
                let stem = format!("{}_{}", sanitize_artifact_name(&view.name), suffix);
                let disk = view_build_root.join(format!("{stem}.spv"));
                match implementation.source_format {
                    SourceFormat::Glsl => {
                        let input = root.join(&implementation.source);
                        let glslc = std::env::var_os("PQO_GLSLC").unwrap_or_else(|| "glslc".into());
                        let output = Command::new(glslc)
                            .args(["-O", "--target-env=vulkan1.3"])
                            .arg(format!("-fshader-stage={suffix}"))
                            .arg(&input)
                            .arg("-o")
                            .arg(&disk)
                            .output()
                            .map_err(|error| format!("could not invoke glslc: {error}"))?;
                        if !output.status.success() {
                            return Err(format!(
                                "Vulkan shader compilation failed for `{}`:\n{}",
                                input.display(),
                                String::from_utf8_lossy(&output.stderr).trim()
                            ));
                        }
                    }
                    SourceFormat::SpirV => {
                        fs::copy(root.join(&implementation.source), &disk).map_err(|error| {
                            format!(
                                "could not stage SPIR-V `{}`: {error}",
                                implementation.source
                            )
                        })?;
                    }
                    _ => {
                        return Err(format!(
                            "Vulkan view `{}` requires GLSL or SPIR-V, found {:?}",
                            view.name, implementation.source_format
                        ));
                    }
                }
                let archive_path = format!("targets/linux-x86_64-nvidia/view/{stem}.spv");
                artifacts.push(PackageArtifact {
                    kind: "spirv".to_owned(),
                    backend: "vulkan".to_owned(),
                    source: implementation.source.clone(),
                    entries: implementation.entry_points.clone(),
                    path: archive_path.clone(),
                    architecture: None,
                });
                generated.push((archive_path, disk));
            }
        }
    }
    let compute_artifacts = artifacts
        .iter()
        .filter(|artifact| artifact.backend == "cuda")
        .cloned()
        .collect::<Vec<_>>();
    if !compute_artifacts.is_empty() {
        let path = build_root.join("manifest.json");
        write_target_manifest(&path, "cuda", compute_artifacts)?;
        generated.push((
            "targets/linux-x86_64-nvidia/compute/manifest.json".to_owned(),
            path,
        ));
    }
    let view_artifacts = artifacts
        .iter()
        .filter(|artifact| artifact.backend == "vulkan")
        .cloned()
        .collect::<Vec<_>>();
    if !view_artifacts.is_empty() {
        let path = root.join(".pqo/build/vulkan/pipeline.json");
        write_target_manifest(&path, "vulkan", view_artifacts)?;
        generated.push((
            "targets/linux-x86_64-nvidia/view/pipeline.json".to_owned(),
            path,
        ));
    }
    Ok((artifacts, generated))
}

fn write_target_manifest(
    path: &Path,
    backend: &'static str,
    artifacts: Vec<PackageArtifact>,
) -> Result<(), String> {
    let manifest = TargetArtifactManifest {
        format: "pqo-target-artifacts",
        version: 1,
        backend,
        artifacts,
    };
    fs::write(
        path,
        serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("could not write `{}`: {error}", path.display()))
}

fn run_nvcc(input: &Path, output: &Path, mode: &[&str]) -> Result<(), String> {
    let nvcc = std::env::var_os("PQO_NVCC").unwrap_or_else(|| "nvcc".into());
    let result = Command::new(nvcc)
        .args(["-O3", "--std=c++17"])
        .args(mode)
        .arg(input)
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|error| format!("could not invoke nvcc: {error}"))?;
    if !result.status.success() {
        return Err(format!(
            "CUDA compilation failed for `{}`:\n{}",
            input.display(),
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    Ok(())
}

fn sanitize_artifact_name(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn target_requirements(target: TargetProfile) -> Vec<BackendRequirements> {
    let mut requirements = Vec::new();
    match target.compute {
        ComputeBackend::Metal => requirements.push(BackendRequirements::Metal {
            minimum_gpu_family: None,
        }),
        ComputeBackend::Cuda => requirements.push(BackendRequirements::Cuda {
            minimum_compute_capability: Some((12, 0)),
        }),
    }
    if target.view == ViewBackend::Vulkan {
        requirements.push(BackendRequirements::Vulkan {
            minimum_api_version: (1, 3),
            required_extensions: vec![
                "VK_KHR_swapchain".to_owned(),
                "VK_KHR_external_memory_fd".to_owned(),
                "VK_KHR_external_semaphore_fd".to_owned(),
            ],
            required_features: vec![
                "timelineSemaphore".to_owned(),
                "synchronization2".to_owned(),
                "dynamicRendering".to_owned(),
            ],
        });
    }
    requirements
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
            project_parent(Path::new("marble-water.pqo")),
            Path::new(".")
        );
        assert_eq!(
            project_parent(Path::new("./marble-water.pqo")),
            Path::new(".")
        );
    }

    #[test]
    fn source_run_discovers_built_project_ui() {
        let project = tempfile::tempdir().expect("temporary project");
        fs::write(
            project.path().join("demo.pqo"),
            "pqo 0.1\nmodule demo\ntarget metal\n",
        )
        .expect("source");
        fs::create_dir_all(project.path().join("ui/dist")).expect("UI directories");
        fs::write(
            project.path().join("ui/pqo-ui.json"),
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
                .join("demo.pqo")
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

    #[test]
    fn source_run_prefers_file_specific_ui() {
        let project = tempfile::tempdir().expect("temporary project");
        fs::write(
            project.path().join("crystal-cuda.pqo"),
            "pqo 0.1\nmodule crystal_cuda\ntarget cuda-headless\n",
        )
        .expect("source");
        for (directory, title) in [
            ("ui", "Generic Controls"),
            ("ui-crystal-cuda", "CUDA Crystal Controls"),
        ] {
            fs::create_dir_all(project.path().join(directory).join("dist"))
                .expect("UI directories");
            fs::write(
                project.path().join(directory).join("pqo-ui.json"),
                format!(
                    r#"{{
                      "framework": "vue3",
                      "dist": "dist",
                      "entry": "index.html",
                      "title": "{title}",
                      "width": 900,
                      "height": 700
                    }}"#
                ),
            )
            .expect("UI config");
            fs::write(
                project.path().join(directory).join("dist/index.html"),
                "<!doctype html><title>Demo</title>",
            )
            .expect("UI entry");
        }

        let program = load(
            project
                .path()
                .join("crystal-cuda.pqo")
                .to_str()
                .expect("UTF-8 path"),
            true,
        )
        .expect("source project loads");
        let ui = program.ui().expect("source UI");
        assert_eq!(ui.title, "CUDA Crystal Controls");
        assert!(ui.asset_root.ends_with("ui-crystal-cuda/dist"));
    }
}
