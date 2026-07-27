use std::{ffi::c_void, os::raw::c_char};

const ABI_VERSION: u32 = 1;
const MAX_OVERRIDES: usize = 32;
const NAME_CAPACITY: usize = 96;
const EVENT_CURSOR_MOVED: u32 = 1;
const EVENT_LEFT_MOUSE: u32 = 2;
const EVENT_SCROLL: u32 = 3;
const MODIFIER_COMMAND: u32 = 1;
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
    target: [f32; 3],
    depth: f32,
    click_generation: f32,
    spawn_generation: f32,
    agent_type: f32,
    agent_count: f32,
    pointer_down: bool,
    space_drag: f32,
    reset: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            cursor: [480.0, 360.0],
            viewport: [960.0, 720.0],
            target: [0.0; 3],
            depth: 0.0,
            click_generation: 0.0,
            spawn_generation: 0.0,
            agent_type: 0.0,
            agent_count: 1.0,
            pointer_down: false,
            space_drag: 0.0,
            reset: false,
        }
    }
}
impl State {
    fn update_target(&mut self) {
        let [w, h] = self.viewport;
        if w <= 0.0 || h <= 0.0 {
            return;
        };
        let depth = CAMERA_Z - self.depth;
        self.target = [
            ((self.cursor[0] / w * 2.0 - 1.0) * (w / h) * depth / CAMERA_FOCAL).clamp(-1.5, 1.5),
            ((1.0 - self.cursor[1] / h * 2.0) * depth / CAMERA_FOCAL).clamp(-0.95, 0.95),
            self.depth,
        ];
    }
    fn event(&mut self, e: ProjectEventV1) {
        if e.viewport_width > 0.0 && e.viewport_height > 0.0 {
            self.viewport = [e.viewport_width, e.viewport_height]
        };
        match e.kind {
            EVENT_CURSOR_MOVED => {
                self.cursor = [e.x, e.y];
                if self.pointer_down {
                    self.update_target();
                }
            }
            EVENT_LEFT_MOUSE if e.pressed != 0 && e._reserved & MODIFIER_COMMAND != 0 => {
                self.cursor = [e.x, e.y];
                self.update_target();
                if self.agent_count < 32.0 {
                    self.spawn_generation += 1.0;
                    self.agent_count += 1.0;
                }
            }
            EVENT_LEFT_MOUSE => {
                self.cursor = [e.x, e.y];
                self.update_target();
                self.pointer_down = e.pressed != 0;
                if self.pointer_down {
                    self.click_generation += 1.0;
                }
            }
            EVENT_SCROLL => {
                self.cursor = [e.x, e.y];
                self.depth = (self.depth + e.delta * 0.12).clamp(MIN_DEPTH, MAX_DEPTH);
                self.update_target()
            }
            _ => {}
        }
    }
    fn control(&mut self, name: &str, value: f32) -> bool {
        match name {
            "interaction.space_drag" => self.space_drag = value.clamp(0.0, 0.5),
            "interaction.agent_type" => self.agent_type = value.clamp(0.0, 2.0).round(),
            "interaction.reset" => {
                self.depth = 0.0;
                self.target = [0.0; 3];
                self.pointer_down = false;
                self.click_generation += 1.0;
                self.spawn_generation += 1.0;
                self.agent_count = 1.0;
                self.reset = true
            }
            _ => return false,
        };
        true
    }
    fn frame(&mut self, c: ProjectFrameContextV1, o: &mut ProjectFrameOutputV1) {
        if c.viewport_width > 0.0 && c.viewport_height > 0.0 {
            self.viewport = [c.viewport_width, c.viewport_height];
            self.update_target()
        }
        for (n, v) in [
            ("interaction.click_x", self.target[0]),
            ("interaction.click_y", self.target[1]),
            ("interaction.click_z", self.target[2]),
            ("interaction.click_generation", self.click_generation),
            ("interaction.spawn_x", self.target[0]),
            ("interaction.spawn_y", self.target[1]),
            ("interaction.spawn_z", self.target[2]),
            ("interaction.spawn_generation", self.spawn_generation),
            ("interaction.spawn_type", self.agent_type),
            ("interaction.agent_count", self.agent_count),
            (
                "interaction.pointer_down",
                if self.pointer_down { 1.0 } else { 0.0 },
            ),
            ("interaction.space_drag", self.space_drag),
            ("interaction.reset", if self.reset { 1.0 } else { 0.0 }),
            (
                "interaction.camera_aspect",
                self.viewport[0] / self.viewport[1].max(1.0),
            ),
            ("interaction.hud_fps", c.frames_per_second),
            ("interaction.hud_gpu_mb", c.gpu_memory_mb),
        ] {
            push(o, n, v)
        }
        self.reset = false;
    }
}
fn push(o: &mut ProjectFrameOutputV1, n: &str, v: f32) {
    let i = o.override_count as usize;
    if i < MAX_OVERRIDES && n.len() < NAME_CAPACITY {
        o.overrides[i].name[..n.len()].copy_from_slice(n.as_bytes());
        o.overrides[i].value = v;
        o.override_count += 1
    }
}
#[no_mangle]
pub extern "C" fn loom_project_abi_version_v1() -> u32 {
    ABI_VERSION
}
#[no_mangle]
pub extern "C" fn loom_project_title_v1() -> *const c_char {
    b"Loom Baseline - selectable particles\0".as_ptr() as *const c_char
}
#[no_mangle]
pub extern "C" fn loom_project_help_v1() -> *const c_char {
    b"click or drag a particle to move it, click space to set its target, Command-click space to add the chosen agent type\0".as_ptr() as *const c_char
}
#[no_mangle]
pub extern "C" fn loom_project_create_v1() -> *mut c_void {
    Box::into_raw(Box::new(State::default())) as *mut c_void
}
#[no_mangle]
pub unsafe extern "C" fn loom_project_destroy_v1(s: *mut c_void) {
    if !s.is_null() {
        drop(Box::from_raw(s as *mut State))
    }
}
#[no_mangle]
pub unsafe extern "C" fn loom_project_event_v1(s: *mut c_void, e: *const ProjectEventV1) -> u32 {
    if s.is_null() || e.is_null() {
        return 0;
    };
    (*(s as *mut State)).event(*e);
    1
}
#[no_mangle]
pub unsafe extern "C" fn loom_project_control_v1(
    s: *mut c_void,
    c: *const ProjectControlV1,
) -> u32 {
    if s.is_null() || c.is_null() {
        return 0;
    };
    let c = &*c;
    let len = c.name.iter().position(|b| *b == 0).unwrap_or(NAME_CAPACITY);
    let Ok(n) = std::str::from_utf8(&c.name[..len]) else {
        return 0;
    };
    u32::from((*(s as *mut State)).control(n, c.value))
}
#[no_mangle]
pub unsafe extern "C" fn loom_project_frame_v1(
    s: *mut c_void,
    c: *const ProjectFrameContextV1,
    o: *mut ProjectFrameOutputV1,
) -> u32 {
    if s.is_null() || c.is_null() || o.is_null() {
        return 0;
    };
    let o = &mut *o;
    o.override_count = 0;
    o.request_redraw = 0;
    o.overrides = [ProjectF32OverrideV1::default(); MAX_OVERRIDES];
    (*(s as *mut State)).frame(*c, o);
    1
}

#[cfg(test)]
mod tests {
    use super::{ProjectEventV1, State, EVENT_CURSOR_MOVED, EVENT_LEFT_MOUSE, MODIFIER_COMMAND};

    #[test]
    fn general_is_the_default_agent_type() {
        assert_eq!(State::default().agent_type, 0.0);
    }

    #[test]
    fn command_click_spawns_the_selected_agent_type() {
        let mut state = State::default();
        assert!(state.control("interaction.agent_type", 2.0));
        state.event(ProjectEventV1 {
            kind: EVENT_LEFT_MOUSE,
            pressed: 1,
            _reserved: MODIFIER_COMMAND,
            x: 720.0,
            y: 180.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });

        assert_eq!(state.spawn_generation, 1.0);
        assert_eq!(state.agent_count, 2.0);
        assert_eq!(state.agent_type, 2.0);
        assert!(state.target[0] > 0.0);
        assert!(state.target[1] > 0.0);
    }

    #[test]
    fn left_drag_tracks_the_pointer_until_release() {
        let mut state = State::default();
        state.event(ProjectEventV1 {
            kind: EVENT_LEFT_MOUSE,
            pressed: 1,
            x: 480.0,
            y: 360.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        assert!(state.pointer_down);

        state.event(ProjectEventV1 {
            kind: EVENT_CURSOR_MOVED,
            x: 720.0,
            y: 180.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        assert!(state.target[0] > 0.0);
        assert!(state.target[1] > 0.0);

        state.event(ProjectEventV1 {
            kind: EVENT_LEFT_MOUSE,
            pressed: 0,
            x: 720.0,
            y: 180.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        assert!(!state.pointer_down);
    }
}
