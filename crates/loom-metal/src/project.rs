use std::{
    ffi::{CStr, c_char, c_void},
    path::Path,
};

use libloading::Library;

use crate::{RuntimeDiagnostic, RuntimeDiagnosticCode};

pub(crate) const PROJECT_ABI_VERSION: u32 = 1;
pub(crate) const PROJECT_MAX_OVERRIDES: usize = 32;
pub(crate) const PROJECT_OVERRIDE_NAME_CAPACITY: usize = 96;

pub(crate) const EVENT_CURSOR_MOVED: u32 = 1;
pub(crate) const EVENT_LEFT_MOUSE: u32 = 2;
pub(crate) const EVENT_SCROLL: u32 = 3;
pub(crate) const EVENT_KEY: u32 = 4;
pub(crate) const EVENT_RESIZED: u32 = 5;

pub(crate) const KEY_W: u32 = 1;
pub(crate) const KEY_A: u32 = 2;
pub(crate) const KEY_S: u32 = 3;
pub(crate) const KEY_D: u32 = 4;
pub(crate) const KEY_UP: u32 = 5;
pub(crate) const KEY_LEFT: u32 = 6;
pub(crate) const KEY_DOWN: u32 = 7;
pub(crate) const KEY_RIGHT: u32 = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ProjectEventV1 {
    pub kind: u32,
    pub pressed: u32,
    pub key: u32,
    pub _reserved: u32,
    pub x: f32,
    pub y: f32,
    pub delta: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ProjectFrameContextV1 {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub frames_per_second: f32,
    pub gpu_memory_mb: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ProjectControlV1 {
    pub name: [u8; PROJECT_OVERRIDE_NAME_CAPACITY],
    pub value: f32,
}

impl Default for ProjectControlV1 {
    fn default() -> Self {
        Self {
            name: [0; PROJECT_OVERRIDE_NAME_CAPACITY],
            value: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ProjectF32OverrideV1 {
    pub name: [u8; PROJECT_OVERRIDE_NAME_CAPACITY],
    pub value: f32,
}

impl Default for ProjectF32OverrideV1 {
    fn default() -> Self {
        Self {
            name: [0; PROJECT_OVERRIDE_NAME_CAPACITY],
            value: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ProjectFrameOutputV1 {
    pub override_count: u32,
    pub request_redraw: u32,
    pub overrides: [ProjectF32OverrideV1; PROJECT_MAX_OVERRIDES],
}

impl Default for ProjectFrameOutputV1 {
    fn default() -> Self {
        Self {
            override_count: 0,
            request_redraw: 0,
            overrides: [ProjectF32OverrideV1::default(); PROJECT_MAX_OVERRIDES],
        }
    }
}

type AbiVersionFn = unsafe extern "C" fn() -> u32;
type TextFn = unsafe extern "C" fn() -> *const c_char;
type CreateFn = unsafe extern "C" fn() -> *mut c_void;
type DestroyFn = unsafe extern "C" fn(*mut c_void);
type EventFn = unsafe extern "C" fn(*mut c_void, *const ProjectEventV1) -> u32;
type FrameFn = unsafe extern "C" fn(
    *mut c_void,
    *const ProjectFrameContextV1,
    *mut ProjectFrameOutputV1,
) -> u32;
type ControlFn = unsafe extern "C" fn(*mut c_void, *const ProjectControlV1) -> u32;

pub(crate) struct ProjectExtension {
    _library: Library,
    state: *mut c_void,
    destroy: DestroyFn,
    event: EventFn,
    frame: FrameFn,
    control: Option<ControlFn>,
    title: String,
    help: String,
}

impl ProjectExtension {
    pub(crate) fn load(path: &Path) -> Result<Self, RuntimeDiagnostic> {
        let library = unsafe { Library::new(path) }.map_err(|error| {
            RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::ProjectExtensionFailed,
                format!(
                    "could not load project extension `{}`: {error}",
                    path.display()
                ),
            )
        })?;
        let abi_version = *unsafe { library.get::<AbiVersionFn>(b"loom_project_abi_version_v1\0") }
            .map_err(|error| missing_symbol(path, "loom_project_abi_version_v1", error))?;
        let version = unsafe { abi_version() };
        if version != PROJECT_ABI_VERSION {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::ProjectExtensionFailed,
                format!(
                    "project extension `{}` uses ABI {version}; runtime requires ABI {}",
                    path.display(),
                    PROJECT_ABI_VERSION
                ),
            ));
        }
        let title_fn = *unsafe { library.get::<TextFn>(b"loom_project_title_v1\0") }
            .map_err(|error| missing_symbol(path, "loom_project_title_v1", error))?;
        let help_fn = *unsafe { library.get::<TextFn>(b"loom_project_help_v1\0") }
            .map_err(|error| missing_symbol(path, "loom_project_help_v1", error))?;
        let create = *unsafe { library.get::<CreateFn>(b"loom_project_create_v1\0") }
            .map_err(|error| missing_symbol(path, "loom_project_create_v1", error))?;
        let destroy = *unsafe { library.get::<DestroyFn>(b"loom_project_destroy_v1\0") }
            .map_err(|error| missing_symbol(path, "loom_project_destroy_v1", error))?;
        let event = *unsafe { library.get::<EventFn>(b"loom_project_event_v1\0") }
            .map_err(|error| missing_symbol(path, "loom_project_event_v1", error))?;
        let frame = *unsafe { library.get::<FrameFn>(b"loom_project_frame_v1\0") }
            .map_err(|error| missing_symbol(path, "loom_project_frame_v1", error))?;
        let control = unsafe {
            library
                .get::<ControlFn>(b"loom_project_control_v1\0")
                .ok()
                .map(|symbol| *symbol)
        };
        let title = unsafe { copy_text(title_fn(), "title", path)? };
        let help = unsafe { copy_text(help_fn(), "help", path)? };
        let state = unsafe { create() };
        if state.is_null() {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::ProjectExtensionFailed,
                format!(
                    "project extension `{}` returned a null state",
                    path.display()
                ),
            ));
        }
        Ok(Self {
            _library: library,
            state,
            destroy,
            event,
            frame,
            control,
            title,
            help,
        })
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn help(&self) -> &str {
        &self.help
    }

    pub(crate) fn event(&mut self, event: ProjectEventV1) -> bool {
        unsafe { (self.event)(self.state, &event) != 0 }
    }

    pub(crate) fn control(&mut self, name: &str, value: f32) -> Result<bool, RuntimeDiagnostic> {
        let Some(control) = self.control else {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::ProjectExtensionFailed,
                "the project panel requires `loom_project_control_v1` in its extension",
            ));
        };
        if name.is_empty() || name.len() >= PROJECT_OVERRIDE_NAME_CAPACITY || !value.is_finite() {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::ProjectExtensionFailed,
                format!("the project panel emitted invalid control `{name}`"),
            ));
        }
        let mut input = ProjectControlV1 {
            value,
            ..ProjectControlV1::default()
        };
        input.name[..name.len()].copy_from_slice(name.as_bytes());
        Ok(unsafe { control(self.state, &input) != 0 })
    }

    pub(crate) fn frame(
        &mut self,
        context: ProjectFrameContextV1,
    ) -> Result<(Vec<(String, f32)>, bool), RuntimeDiagnostic> {
        let mut output = ProjectFrameOutputV1::default();
        let succeeded = unsafe { (self.frame)(self.state, &context, &mut output) };
        if succeeded == 0 {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::ProjectExtensionFailed,
                "the project extension failed while producing frame values",
            ));
        }
        let count = usize::try_from(output.override_count)
            .unwrap_or(usize::MAX)
            .min(PROJECT_MAX_OVERRIDES);
        let mut overrides = Vec::with_capacity(count);
        for item in &output.overrides[..count] {
            let length = item
                .name
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(PROJECT_OVERRIDE_NAME_CAPACITY);
            let name = std::str::from_utf8(&item.name[..length]).map_err(|error| {
                RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::ProjectExtensionFailed,
                    format!("project extension emitted a non-UTF-8 value name: {error}"),
                )
            })?;
            if name.is_empty() {
                return Err(RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::ProjectExtensionFailed,
                    "project extension emitted an empty value name",
                ));
            }
            overrides.push((name.to_owned(), item.value));
        }
        Ok((overrides, output.request_redraw != 0))
    }
}

impl Drop for ProjectExtension {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.state) };
    }
}

fn missing_symbol(path: &Path, symbol: &str, error: libloading::Error) -> RuntimeDiagnostic {
    RuntimeDiagnostic::new(
        RuntimeDiagnosticCode::ProjectExtensionFailed,
        format!(
            "project extension `{}` is missing `{symbol}`: {error}",
            path.display()
        ),
    )
}

unsafe fn copy_text(
    pointer: *const c_char,
    field: &str,
    path: &Path,
) -> Result<String, RuntimeDiagnostic> {
    if pointer.is_null() {
        return Err(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::ProjectExtensionFailed,
            format!(
                "project extension `{}` returned a null {field}",
                path.display()
            ),
        ));
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .map(str::to_owned)
        .map_err(|error| {
            RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::ProjectExtensionFailed,
                format!(
                    "project extension `{}` returned non-UTF-8 {field}: {error}",
                    path.display()
                ),
            )
        })
}
