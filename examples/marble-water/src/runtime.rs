use std::{ffi::c_void, os::raw::c_char};

const ABI_VERSION: u32 = 1;
const MAX_OVERRIDES: usize = 32;
const NAME_CAPACITY: usize = 96;

const EVENT_CURSOR_MOVED: u32 = 1;
const EVENT_LEFT_MOUSE: u32 = 2;
const EVENT_SCROLL: u32 = 3;
const EVENT_KEY: u32 = 4;

const KEY_W: u32 = 1;
const KEY_A: u32 = 2;
const KEY_S: u32 = 3;
const KEY_D: u32 = 4;
const KEY_UP: u32 = 5;
const KEY_LEFT: u32 = 6;
const KEY_DOWN: u32 = 7;
const KEY_RIGHT: u32 = 8;

const HUD_CENTER: [f32; 2] = [-0.79, 0.76];
const HUD_HALF_SIZE: [f32; 2] = [0.18, 0.17];
const SLIDER_START: f32 = -0.65;
const SLIDER_END: f32 = 0.18;
const PARTICLE_SLIDER_Y: f32 = 0.70;
const PLANE_SLIDER_Y: f32 = 0.34;
const WATER_SLIDER_Y: f32 = -0.02;
const RESET_CENTER: [f32; 2] = [0.67, 0.70];

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

#[derive(Clone, Copy)]
enum Slider {
    Particles,
    Plane,
    Water,
}

#[derive(Clone, Copy)]
enum HudAction {
    Slider(Slider, f32),
    Reset,
    None,
}

struct State {
    cursor: [f32; 2],
    viewport: [f32; 2],
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    slider: Option<Slider>,
    dragging_marble: bool,
    grab_height: f32,
    grab_target: [f32; 3],
    density: f32,
    plane_scale: f32,
    amplification: f32,
    reset_scene: bool,
    reset_water: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            cursor: [0.0; 2],
            viewport: [960.0, 720.0],
            forward: false,
            backward: false,
            left: false,
            right: false,
            slider: None,
            dragging_marble: false,
            grab_height: 0.55,
            grab_target: [0.0, 0.641, 0.15],
            density: 0.0,
            plane_scale: 1.0,
            amplification: 0.0,
            reset_scene: false,
            reset_water: false,
        }
    }
}

impl State {
    fn handle_event(&mut self, event: ProjectEventV1) {
        if event.viewport_width > 0.0 && event.viewport_height > 0.0 {
            self.viewport = [event.viewport_width, event.viewport_height];
        }
        if event.kind == EVENT_CURSOR_MOVED {
            self.cursor = [event.x, event.y];
            if let Some(slider) = self.slider {
                self.set_slider(slider, slider_value(self.cursor, self.viewport));
            } else if self.dragging_marble {
                self.update_grab_target();
            }
        } else if event.kind == EVENT_LEFT_MOUSE {
            self.cursor = [event.x, event.y];
            if event.pressed != 0 {
                match hud_action(self.cursor, self.viewport) {
                    HudAction::Slider(slider, value) => {
                        self.slider = Some(slider);
                        self.set_slider(slider, value);
                    }
                    HudAction::Reset => {
                        self.dragging_marble = false;
                        self.reset_scene = true;
                        self.reset_water = true;
                    }
                    HudAction::None if !hud_contains(self.cursor, self.viewport) => {
                        self.dragging_marble = true;
                        self.update_grab_target();
                    }
                    HudAction::None => {}
                }
            } else {
                self.slider = None;
                self.dragging_marble = false;
            }
        } else if event.kind == EVENT_SCROLL && self.dragging_marble {
            self.grab_height = (self.grab_height + event.delta * 0.08).clamp(0.03, 1.75);
            self.update_grab_target();
        } else if event.kind == EVENT_KEY {
            let pressed = event.pressed != 0;
            match event.key {
                KEY_W | KEY_UP => self.forward = pressed,
                KEY_S | KEY_DOWN => self.backward = pressed,
                KEY_A | KEY_LEFT => self.left = pressed,
                KEY_D | KEY_RIGHT => self.right = pressed,
                _ => {}
            }
        }
    }

    fn set_slider(&mut self, slider: Slider, value: f32) {
        match slider {
            Slider::Particles => {
                if (self.density - value).abs() > 0.001 {
                    self.density = value;
                    self.reset_water = true;
                }
            }
            Slider::Plane => {
                let scale = 1.0 + value * 2.0;
                if (self.plane_scale - scale).abs() > 0.001 {
                    self.plane_scale = scale;
                    self.reset_water = true;
                }
            }
            Slider::Water => self.amplification = value,
        }
    }

    fn update_grab_target(&mut self) {
        if let Some(target) = cursor_target(
            self.cursor,
            self.viewport,
            self.plane_scale,
            0.016 + 0.075 + self.grab_height,
        ) {
            self.grab_target = target;
        }
    }

    fn write_frame(
        &mut self,
        context: ProjectFrameContextV1,
        output: &mut ProjectFrameOutputV1,
    ) {
        let axis_x = (self.right as u8 as f32) - (self.left as u8 as f32);
        let axis_z = (self.backward as u8 as f32) - (self.forward as u8 as f32);
        push(output, "interaction.input_x", axis_x);
        push(output, "interaction.input_z", axis_z);
        push(
            output,
            "interaction.grab_active",
            if self.dragging_marble { 1.0 } else { 0.0 },
        );
        push(output, "interaction.grab_x", self.grab_target[0]);
        push(output, "interaction.grab_y", self.grab_target[1]);
        push(output, "interaction.grab_z", self.grab_target[2]);
        push(output, "interaction.water_density", self.density);
        push(output, "interaction.plane_scale", self.plane_scale);
        push(
            output,
            "interaction.water_amplification",
            self.amplification,
        );
        push(
            output,
            "interaction.reset_scene",
            if self.reset_scene { 1.0 } else { 0.0 },
        );
        push(
            output,
            "interaction.reset_water",
            if self.reset_water { 1.0 } else { 0.0 },
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
        self.reset_scene = false;
        self.reset_water = false;
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

fn pointer_ndc(point: [f32; 2], viewport: [f32; 2]) -> [f32; 2] {
    [
        point[0] / viewport[0] * 2.0 - 1.0,
        1.0 - point[1] / viewport[1] * 2.0,
    ]
}

fn hud_local(point: [f32; 2], viewport: [f32; 2]) -> [f32; 2] {
    let ndc = pointer_ndc(point, viewport);
    [
        (ndc[0] - HUD_CENTER[0]) / HUD_HALF_SIZE[0],
        (ndc[1] - HUD_CENTER[1]) / HUD_HALF_SIZE[1],
    ]
}

fn slider_value(point: [f32; 2], viewport: [f32; 2]) -> f32 {
    let local_x = hud_local(point, viewport)[0];
    ((local_x - SLIDER_START) / (SLIDER_END - SLIDER_START)).clamp(0.0, 1.0)
}

fn hud_action(point: [f32; 2], viewport: [f32; 2]) -> HudAction {
    if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
        return HudAction::None;
    }
    let local = hud_local(point, viewport);
    let reset_delta = [local[0] - RESET_CENTER[0], local[1] - RESET_CENTER[1]];
    if reset_delta[0] * reset_delta[0] + reset_delta[1] * reset_delta[1] <= 0.24 * 0.24 {
        HudAction::Reset
    } else if (local[1] - PARTICLE_SLIDER_Y).abs() <= 0.15
        && (SLIDER_START - 0.10..=SLIDER_END + 0.10).contains(&local[0])
    {
        HudAction::Slider(Slider::Particles, slider_value(point, viewport))
    } else if (local[1] - PLANE_SLIDER_Y).abs() <= 0.15
        && (SLIDER_START - 0.10..=SLIDER_END + 0.10).contains(&local[0])
    {
        HudAction::Slider(Slider::Plane, slider_value(point, viewport))
    } else if (local[1] - WATER_SLIDER_Y).abs() <= 0.15
        && (SLIDER_START - 0.10..=SLIDER_END + 0.10).contains(&local[0])
    {
        HudAction::Slider(Slider::Water, slider_value(point, viewport))
    } else {
        HudAction::None
    }
}

fn hud_contains(point: [f32; 2], viewport: [f32; 2]) -> bool {
    if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
        return false;
    }
    let local = hud_local(point, viewport);
    local[0].abs() <= 1.0 && local[1].abs() <= 1.0
}

fn cursor_target(
    point: [f32; 2],
    viewport: [f32; 2],
    plane_scale: f32,
    target_y: f32,
) -> Option<[f32; 3]> {
    if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
        return None;
    }
    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn normalize(value: [f32; 3]) -> [f32; 3] {
        let length = dot(value, value).sqrt().max(1.0e-6);
        [value[0] / length, value[1] / length, value[2] / length]
    }
    let ndc = pointer_ndc(point, viewport);
    let scale = plane_scale.max(1.0);
    let camera_scale = 1.0 + (scale - 1.0) * 0.72;
    let camera = [0.0, 2.10 * camera_scale, 3.55 * camera_scale];
    let forward = normalize([-camera[0], 0.02 - camera[1], -camera[2]]);
    let right = normalize(cross(forward, [0.0, 1.0, 0.0]));
    let camera_up = cross(right, forward);
    let aspect = viewport[0] / viewport[1];
    let focal = 1.85;
    let direction = normalize([
        forward[0] + right[0] * ndc[0] * aspect / focal + camera_up[0] * ndc[1] / focal,
        forward[1] + right[1] * ndc[0] * aspect / focal + camera_up[1] * ndc[1] / focal,
        forward[2] + right[2] * ndc[0] * aspect / focal + camera_up[2] * ndc[1] / focal,
    ]);
    if direction[1].abs() < 1.0e-5 {
        return None;
    }
    let distance = (0.02 - camera[1]) / direction[1];
    if distance <= 0.0 {
        return None;
    }
    let radius = 0.075;
    let limit_x = 1.45 * scale - radius;
    let limit_z = 1.08 * scale - radius;
    Some([
        (camera[0] + direction[0] * distance).clamp(-limit_x, limit_x),
        target_y,
        (camera[2] + direction[2] * distance).clamp(-limit_z, limit_z),
    ])
}

#[no_mangle]
pub extern "C" fn loom_project_abi_version_v1() -> u32 {
    ABI_VERSION
}

#[no_mangle]
pub extern "C" fn loom_project_title_v1() -> *const c_char {
    b"Loom - Marble Water - drag/drop | scroll: height | WASD: steer\0".as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn loom_project_help_v1() -> *const c_char {
    b"left-drag the yellow marble in X/Z, scroll while held to change height, and release to drop; WASD/arrow keys steer; cyan: particles; purple: plane; amber: ripple intensity; reset restarts\0"
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
