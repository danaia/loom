use std::{ffi::c_void, os::raw::c_char};

const ABI_VERSION: u32 = 1;
const MAX_OVERRIDES: usize = 32;
const NAME_CAPACITY: usize = 96;

const EVENT_CURSOR_MOVED: u32 = 1;
const EVENT_LEFT_MOUSE: u32 = 2;
const EVENT_SCROLL: u32 = 3;

const CAMERA_Z: f32 = 3.0;
const CAMERA_FOCAL: f32 = 1.85;
const MIN_DEPTH: f32 = -1.15;
const MAX_DEPTH: f32 = 1.15;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ProjectEventV1 {
    kind: u32,
    pressed: u32,
    key: u32,
    _reserved: u32,
    x: f32,
    y: f32,
    delta: f32,
    viewport_width: f32,
    viewport_height: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ProjectFrameContextV1 {
    viewport_width: f32,
    viewport_height: f32,
    frames_per_second: f32,
    gpu_memory_mb: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProjectControlV1 {
    name: [u8; NAME_CAPACITY],
    value: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProjectF32OverrideV1 {
    name: [u8; NAME_CAPACITY],
    value: f32,
}

impl Default for ProjectF32OverrideV1 {
    fn default() -> Self {
        Self {
            name: [0; NAME_CAPACITY],
            value: 0.0,
        }
    }
}

#[repr(C)]
pub struct ProjectFrameOutputV1 {
    override_count: u32,
    request_redraw: u32,
    overrides: [ProjectF32OverrideV1; MAX_OVERRIDES],
}

struct State {
    cursor: [f32; 2],
    viewport: [f32; 2],
    dragging: bool,
    target: [f32; 3],
    depth: f32,
    space_drag: f32,
    reset: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            cursor: [480.0, 360.0],
            viewport: [960.0, 720.0],
            dragging: false,
            target: [0.0, 0.0, 0.0],
            depth: 0.0,
            space_drag: 0.0,
            reset: false,
        }
    }
}

impl State {
    fn handle_event(&mut self, event: ProjectEventV1) {
        if event.viewport_width > 0.0 && event.viewport_height > 0.0 {
            self.viewport = [event.viewport_width, event.viewport_height];
        }

        match event.kind {
            EVENT_CURSOR_MOVED => {
                self.cursor = [event.x, event.y];
                if self.dragging {
                    self.update_target();
                }
            }
            EVENT_LEFT_MOUSE => {
                self.cursor = [event.x, event.y];
                self.dragging = event.pressed != 0;
                if self.dragging {
                    self.update_target();
                }
            }
            EVENT_SCROLL if self.dragging => {
                self.cursor = [event.x, event.y];
                self.depth =
                    (self.depth + event.delta * 0.12).clamp(MIN_DEPTH, MAX_DEPTH);
                self.update_target();
            }
            _ => {}
        }
    }

    fn set_control(&mut self, name: &str, value: f32) -> bool {
        match name {
            "interaction.space_drag" => {
                self.space_drag = value.clamp(0.0, 0.5);
            }
            "interaction.reset" => {
                self.dragging = false;
                self.depth = 0.0;
                self.target = [0.0; 3];
                self.reset = true;
            }
            _ => return false,
        }
        true
    }

    fn update_target(&mut self) {
        let [width, height] = self.viewport;
        if width <= 0.0 || height <= 0.0 {
            return;
        }

        let ndc_x = self.cursor[0] / width * 2.0 - 1.0;
        let ndc_y = 1.0 - self.cursor[1] / height * 2.0;
        let aspect = width / height;
        let camera_depth = CAMERA_Z - self.depth;
        self.target = [
            (ndc_x * aspect * camera_depth / CAMERA_FOCAL).clamp(-1.5, 1.5),
            (ndc_y * camera_depth / CAMERA_FOCAL).clamp(-0.95, 0.95),
            self.depth,
        ];
    }

    fn write_frame(
        &mut self,
        context: ProjectFrameContextV1,
        output: &mut ProjectFrameOutputV1,
    ) {
        if context.viewport_width > 0.0 && context.viewport_height > 0.0 {
            self.viewport = [context.viewport_width, context.viewport_height];
            if self.dragging {
                self.update_target();
            }
        }

        push(
            output,
            "interaction.grab_active",
            if self.dragging { 1.0 } else { 0.0 },
        );
        push(output, "interaction.grab_x", self.target[0]);
        push(output, "interaction.grab_y", self.target[1]);
        push(output, "interaction.grab_z", self.target[2]);
        push(output, "interaction.space_drag", self.space_drag);
        push(
            output,
            "interaction.reset",
            if self.reset { 1.0 } else { 0.0 },
        );
        push(
            output,
            "interaction.camera_aspect",
            self.viewport[0] / self.viewport[1].max(1.0),
        );
        push(
            output,
            "interaction.hud_fps",
            context.frames_per_second,
        );
        push(
            output,
            "interaction.hud_gpu_mb",
            context.gpu_memory_mb,
        );
        self.reset = false;
    }
}

fn push(output: &mut ProjectFrameOutputV1, name: &str, value: f32) {
    let index = output.override_count as usize;
    if index >= MAX_OVERRIDES || name.len() >= NAME_CAPACITY {
        return;
    }
    output.overrides[index].name[..name.len()].copy_from_slice(name.as_bytes());
    output.overrides[index].value = value;
    output.override_count += 1;
}

#[no_mangle]
pub extern "C" fn loom_project_abi_version_v1() -> u32 {
    ABI_VERSION
}

#[no_mangle]
pub extern "C" fn loom_project_title_v1() -> *const c_char {
    b"Loom Baseline - one zero-gravity particle\0".as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn loom_project_help_v1() -> *const c_char {
    b"drag anywhere to pull the particle; scroll while dragging to move through depth; release to preserve inertia\0"
        .as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn loom_project_create_v1() -> *mut c_void {
    Box::into_raw(Box::new(State::default())) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn loom_project_destroy_v1(state: *mut c_void) {
    if !state.is_null() {
        drop(Box::from_raw(state as *mut State));
    }
}

#[no_mangle]
pub unsafe extern "C" fn loom_project_event_v1(
    state: *mut c_void,
    event: *const ProjectEventV1,
) -> u32 {
    if state.is_null() || event.is_null() {
        return 0;
    }
    (*(state as *mut State)).handle_event(*event);
    1
}

#[no_mangle]
pub unsafe extern "C" fn loom_project_control_v1(
    state: *mut c_void,
    control: *const ProjectControlV1,
) -> u32 {
    if state.is_null() || control.is_null() {
        return 0;
    }
    let control = &*control;
    let length = control
        .name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(NAME_CAPACITY);
    let Ok(name) = std::str::from_utf8(&control.name[..length]) else {
        return 0;
    };
    u32::from((*(state as *mut State)).set_control(name, control.value))
}

#[no_mangle]
pub unsafe extern "C" fn loom_project_frame_v1(
    state: *mut c_void,
    context: *const ProjectFrameContextV1,
    output: *mut ProjectFrameOutputV1,
) -> u32 {
    if state.is_null() || context.is_null() || output.is_null() {
        return 0;
    }
    let output = &mut *output;
    output.override_count = 0;
    output.request_redraw = 0;
    output.overrides = [ProjectF32OverrideV1::default(); MAX_OVERRIDES];
    (*(state as *mut State)).write_frame(*context, output);
    1
}

#[cfg(test)]
mod tests {
    use super::{
        EVENT_CURSOR_MOVED, EVENT_LEFT_MOUSE, EVENT_SCROLL, ProjectEventV1,
        ProjectF32OverrideV1, ProjectFrameContextV1, ProjectFrameOutputV1, State,
    };

    #[test]
    fn drag_maps_pointer_to_a_three_dimensional_target() {
        let mut state = State::default();
        state.handle_event(ProjectEventV1 {
            kind: EVENT_LEFT_MOUSE,
            pressed: 1,
            x: 720.0,
            y: 180.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        assert!(state.dragging);
        assert!(state.target[0] > 0.0);
        assert!(state.target[1] > 0.0);

        state.handle_event(ProjectEventV1 {
            kind: EVENT_SCROLL,
            delta: 1.0,
            x: 720.0,
            y: 180.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        assert!(state.target[2] > 0.0);

        state.handle_event(ProjectEventV1 {
            kind: EVENT_CURSOR_MOVED,
            x: 240.0,
            y: 540.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        assert!(state.target[0] < 0.0);
        assert!(state.target[1] < 0.0);
    }

    #[test]
    fn panel_controls_are_clamped_and_reset_is_a_pulse() {
        let mut state = State::default();
        assert!(state.set_control("interaction.space_drag", 5.0));
        assert_eq!(state.space_drag, 0.5);
        assert!(state.set_control("interaction.reset", 1.0));
        assert!(state.reset);

        let context = ProjectFrameContextV1::default();
        let mut output = ProjectFrameOutputV1 {
            override_count: 0,
            request_redraw: 0,
            overrides: [ProjectF32OverrideV1::default(); 32],
        };
        state.write_frame(context, &mut output);
        assert!(!state.reset);
        assert!(output.override_count > 0);
    }
}

