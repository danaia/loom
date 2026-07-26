use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};

use loom_core::ModuleGraph;
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackageExtension {
    abi: u32,
    target: String,
    path: String,
    source: String,
}

pub(crate) struct LoadedProgram {
    source: String,
    project_root: PathBuf,
    extension_path: Option<PathBuf>,
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
}

pub(crate) fn load(path: &str) -> Result<LoadedProgram, String> {
    let path = Path::new(path);
    if path.extension().and_then(|value| value.to_str()) == Some("lmp") {
        load_package(path)
    } else {
        let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
        let project_root = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()
            .map_err(|error| error.to_string())?;
        Ok(LoadedProgram {
            source,
            project_root,
            extension_path: None,
            _extracted: None,
        })
    }
}

pub(crate) fn build(path: &str, graph: &ModuleGraph) -> Result<String, String> {
    let source_path = Path::new(path);
    if source_path.extension().and_then(|value| value.to_str()) != Some("loom") {
        return Err("`loom build` expects a primary .loom source file".to_owned());
    }
    let root = source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
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

    let extension_source = root.join("src/runtime.rs");
    let build_directory = root.join(".loom/build");
    fs::create_dir_all(&build_directory)
        .map_err(|error| format!("could not create project build directory: {error}"))?;
    let target = host_target()?;
    let extension_archive_path = format!("runtime/{target}/libloom_project.dylib");
    let extension_disk_path = build_directory.join("libloom_project.dylib");
    let extension = if extension_source.is_file() {
        let output = Command::new("rustc")
            .args(["--crate-type", "cdylib", "--edition", "2021"])
            .arg("-C")
            .arg("opt-level=2")
            .arg(&extension_source)
            .arg("-o")
            .arg(&extension_disk_path)
            .output()
            .map_err(|error| {
                format!("could not invoke rustc for the project extension: {error}")
            })?;
        if !output.status.success() {
            return Err(format!(
                "project Rust extension failed to compile:\n{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
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
            &extension_disk_path,
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
    Ok(LoadedProgram {
        source,
        project_root: extracted.path().to_owned(),
        extension_path,
        _extracted: Some(extracted),
    })
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
