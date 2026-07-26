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

const CAMERA_TARGET_Y: f32 = 0.02;
const DEFAULT_CAMERA_HEIGHT: f32 = 2.10;
const DEFAULT_CAMERA_DEPTH: f32 = 3.55;
const CAMERA_ORBIT_SPEED: f32 = 0.008;
const CAMERA_ZOOM_SPEED: f32 = 0.12;

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
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    dragging_marble: bool,
    orbiting_camera: bool,
    orbit_cursor: [f32; 2],
    grab_height: f32,
    grab_target: [f32; 3],
    camera_target: [f32; 3],
    camera_yaw: f32,
    camera_pitch: f32,
    camera_distance: f32,
    density: f32,
    plane_scale: f32,
    amplification: f32,
    player_speed: f32,
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
            dragging_marble: false,
            orbiting_camera: false,
            orbit_cursor: [0.0; 2],
            grab_height: 0.55,
            grab_target: [0.0, 0.625, 0.15],
            camera_target: [0.0, CAMERA_TARGET_Y, 0.0],
            camera_yaw: 0.0,
            camera_pitch: DEFAULT_CAMERA_HEIGHT.atan2(DEFAULT_CAMERA_DEPTH),
            camera_distance: (DEFAULT_CAMERA_HEIGHT * DEFAULT_CAMERA_HEIGHT
                + DEFAULT_CAMERA_DEPTH * DEFAULT_CAMERA_DEPTH)
                .sqrt(),
            density: 0.5,
            plane_scale: 1.0,
            amplification: 0.25,
            player_speed: 1.0,
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
            if self.dragging_marble {
                self.update_grab_target();
            } else if self.orbiting_camera {
                let delta = [
                    self.cursor[0] - self.orbit_cursor[0],
                    self.cursor[1] - self.orbit_cursor[1],
                ];
                self.camera_yaw += delta[0] * CAMERA_ORBIT_SPEED;
                self.camera_pitch = (self.camera_pitch
                    + delta[1] * CAMERA_ORBIT_SPEED)
                    .clamp(0.12, 1.32);
                self.orbit_cursor = self.cursor;
            }
        } else if event.kind == EVENT_LEFT_MOUSE {
            self.cursor = [event.x, event.y];
            if event.pressed != 0 {
                if self.pointer_is_on_water() {
                    self.dragging_marble = true;
                    self.orbiting_camera = false;
                    self.update_grab_target();
                } else {
                    self.dragging_marble = false;
                    self.orbiting_camera = true;
                    self.orbit_cursor = self.cursor;
                }
            } else {
                self.dragging_marble = false;
                self.orbiting_camera = false;
            }
        } else if event.kind == EVENT_SCROLL {
            self.cursor = [event.x, event.y];
            if self.dragging_marble {
                self.grab_height =
                    (self.grab_height + event.delta * 0.08).clamp(0.03, 1.75);
                self.update_grab_target();
            } else {
                self.zoom_toward_cursor(event.delta);
            }
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

    fn set_control(&mut self, name: &str, value: f32) -> bool {
        match name {
            "interaction.water_density" => {
                let value = value.clamp(0.0, 1.0);
                if (self.density - value).abs() > 0.001 {
                    self.density = value;
                    self.reset_water = true;
                }
            }
            "interaction.plane_scale" => {
                let value = value.clamp(1.0, 3.0);
                if (self.plane_scale - value).abs() > 0.001 {
                    let old_camera_scale =
                        1.0 + (self.plane_scale - 1.0) * 0.72;
                    let new_camera_scale = 1.0 + (value - 1.0) * 0.72;
                    self.camera_distance *=
                        new_camera_scale / old_camera_scale;
                    self.plane_scale = value;
                    self.reset_water = true;
                }
            }
            "interaction.water_amplification" => {
                self.amplification = value.clamp(0.0, 1.0);
            }
            "interaction.player_speed" => {
                self.player_speed = value.clamp(0.25, 3.0);
            }
            "interaction.reset_scene" => {
                self.dragging_marble = false;
                self.orbiting_camera = false;
                self.reset_scene = true;
                self.reset_water = true;
            }
            _ => return false,
        }
        true
    }

    fn update_grab_target(&mut self) {
        if let Some(target) = cursor_target(
            self.cursor,
            self.viewport,
            self.plane_scale,
            0.075 + self.grab_height,
            self.camera_position(),
            self.camera_target,
        ) {
            self.grab_target = target;
        }
    }

    fn pointer_is_on_water(&self) -> bool {
        let Some(point) = cursor_plane_point(
            self.cursor,
            self.viewport,
            self.camera_position(),
            self.camera_target,
            0.0,
        ) else {
            return false;
        };
        let scale = self.plane_scale.max(1.0);
        point[0].abs() <= 1.45 * scale
            && point[2].abs() <= 1.08 * scale
    }

    fn zoom_toward_cursor(&mut self, delta: f32) {
        if delta.abs() < f32::EPSILON {
            return;
        }
        let camera_before = self.camera_position();
        let anchor_before = cursor_plane_point(
            self.cursor,
            self.viewport,
            camera_before,
            self.camera_target,
            0.0,
        );
        let scale = self.plane_scale.max(1.0);
        self.camera_distance = (
            self.camera_distance
                * (-delta.clamp(-8.0, 8.0) * CAMERA_ZOOM_SPEED).exp()
        )
            .clamp(0.55 * scale, 10.0 * scale);
        if let Some(anchor_before) = anchor_before {
            if let Some(anchor_after) = cursor_plane_point(
                self.cursor,
                self.viewport,
                self.camera_position(),
                self.camera_target,
                0.0,
            ) {
                self.camera_target[0] +=
                    anchor_before[0] - anchor_after[0];
                self.camera_target[2] +=
                    anchor_before[2] - anchor_after[2];
                self.clamp_camera_target();
            }
        }
    }

    fn clamp_camera_target(&mut self) {
        let scale = self.plane_scale.max(1.0);
        self.camera_target[0] =
            self.camera_target[0].clamp(-1.45 * scale, 1.45 * scale);
        self.camera_target[2] =
            self.camera_target[2].clamp(-1.08 * scale, 1.08 * scale);
    }

    fn camera_position(&self) -> [f32; 3] {
        let horizontal = self.camera_pitch.cos() * self.camera_distance;
        [
            self.camera_target[0] + self.camera_yaw.sin() * horizontal,
            self.camera_target[1]
                + self.camera_pitch.sin() * self.camera_distance,
            self.camera_target[2] + self.camera_yaw.cos() * horizontal,
        ]
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
        push(output, "interaction.player_speed", self.player_speed);
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
        let camera = self.camera_position();
        push(output, "interaction.camera_x", camera[0]);
        push(output, "interaction.camera_y", camera[1]);
        push(output, "interaction.camera_z", camera[2]);
        push(
            output,
            "interaction.camera_target_x",
            self.camera_target[0],
        );
        push(
            output,
            "interaction.camera_target_y",
            self.camera_target[1],
        );
        push(
            output,
            "interaction.camera_target_z",
            self.camera_target[2],
        );
        push(
            output,
            "interaction.camera_aspect",
            self.viewport[0] / self.viewport[1].max(1.0),
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

fn cursor_plane_point(
    point: [f32; 2],
    viewport: [f32; 2],
    camera: [f32; 3],
    camera_target: [f32; 3],
    plane_y: f32,
) -> Option<[f32; 3]> {
    if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
        return None;
    }
    let ndc = pointer_ndc(point, viewport);
    let forward = normalize([
        camera_target[0] - camera[0],
        camera_target[1] - camera[1],
        camera_target[2] - camera[2],
    ]);
    let right = normalize(cross(forward, [0.0, 1.0, 0.0]));
    let camera_up = cross(right, forward);
    let aspect = viewport[0] / viewport[1];
    let focal = 1.85;
    let direction = normalize([
        forward[0]
            + right[0] * ndc[0] * aspect / focal
            + camera_up[0] * ndc[1] / focal,
        forward[1]
            + right[1] * ndc[0] * aspect / focal
            + camera_up[1] * ndc[1] / focal,
        forward[2]
            + right[2] * ndc[0] * aspect / focal
            + camera_up[2] * ndc[1] / focal,
    ]);
    if direction[1].abs() < 1.0e-5 {
        return None;
    }
    let distance = (plane_y - camera[1]) / direction[1];
    if distance <= 0.0 {
        return None;
    }
    Some([
        camera[0] + direction[0] * distance,
        plane_y,
        camera[2] + direction[2] * distance,
    ])
}

fn cursor_target(
    point: [f32; 2],
    viewport: [f32; 2],
    plane_scale: f32,
    target_y: f32,
    camera: [f32; 3],
    camera_target: [f32; 3],
) -> Option<[f32; 3]> {
    let point = cursor_plane_point(
        point,
        viewport,
        camera,
        camera_target,
        0.0,
    )?;
    let scale = plane_scale.max(1.0);
    let radius = 0.075;
    let limit_x = 1.45 * scale - radius;
    let limit_z = 1.08 * scale - radius;
    Some([
        point[0].clamp(-limit_x, limit_x),
        target_y,
        point[2].clamp(-limit_z, limit_z),
    ])
}

#[no_mangle]
pub extern "C" fn loom_project_abi_version_v1() -> u32 {
    ABI_VERSION
}

#[no_mangle]
pub extern "C" fn loom_project_title_v1() -> *const c_char {
    b"Loom - Marble Water - water drag: marble | background drag: orbit | scroll: zoom\0"
        .as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn loom_project_help_v1() -> *const c_char {
    b"left-drag the water to move the yellow marble; scroll while held to change its drop height; drag the background to orbit; scroll elsewhere to zoom toward the pointer; WASD/arrow keys steer\0"
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
        EVENT_CURSOR_MOVED, EVENT_LEFT_MOUSE, EVENT_SCROLL,
        ProjectEventV1, ProjectF32OverrideV1, ProjectFrameContextV1,
        ProjectFrameOutputV1, State, cursor_plane_point,
    };

    #[test]
    fn panel_controls_update_water_state() {
        let mut state = State::default();
        assert!(state.set_control("interaction.water_density", 0.75));
        assert!(state.set_control("interaction.plane_scale", 2.25));
        assert!(state.set_control("interaction.water_amplification", 0.6));
        assert!(state.set_control("interaction.player_speed", 2.4));
        assert_eq!(state.density, 0.75);
        assert_eq!(state.plane_scale, 2.25);
        assert_eq!(state.amplification, 0.6);
        assert_eq!(state.player_speed, 2.4);
        assert!(state.reset_water);
    }

    #[test]
    fn reset_action_resets_scene_and_water() {
        let mut state = State::default();
        state.dragging_marble = true;
        state.orbiting_camera = true;
        assert!(state.set_control("interaction.reset_scene", 1.0));
        assert!(!state.dragging_marble);
        assert!(!state.orbiting_camera);
        assert!(state.reset_scene);
        assert!(state.reset_water);
    }

    #[test]
    fn background_drag_orbits_while_water_drag_keeps_marble_control() {
        let mut state = State::default();
        state.handle_event(ProjectEventV1 {
            kind: EVENT_LEFT_MOUSE,
            pressed: 1,
            x: 10.0,
            y: 10.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        assert!(state.orbiting_camera);
        assert!(!state.dragging_marble);
        let yaw_before = state.camera_yaw;
        state.handle_event(ProjectEventV1 {
            kind: EVENT_CURSOR_MOVED,
            x: 70.0,
            y: 45.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        assert!(state.camera_yaw > yaw_before);

        state.handle_event(ProjectEventV1 {
            kind: EVENT_LEFT_MOUSE,
            pressed: 0,
            x: 70.0,
            y: 45.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        state.handle_event(ProjectEventV1 {
            kind: EVENT_LEFT_MOUSE,
            pressed: 1,
            x: 480.0,
            y: 360.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        assert!(state.dragging_marble);
        assert!(!state.orbiting_camera);
    }

    #[test]
    fn free_scroll_zooms_toward_the_pointer() {
        let mut state = State::default();
        state.cursor = [620.0, 390.0];
        let anchor_before = cursor_plane_point(
            state.cursor,
            state.viewport,
            state.camera_position(),
            state.camera_target,
            0.0,
        )
        .unwrap();
        let distance_before = state.camera_distance;
        state.handle_event(ProjectEventV1 {
            kind: EVENT_SCROLL,
            x: state.cursor[0],
            y: state.cursor[1],
            delta: 2.0,
            viewport_width: 960.0,
            viewport_height: 720.0,
            ..ProjectEventV1::default()
        });
        let anchor_after = cursor_plane_point(
            state.cursor,
            state.viewport,
            state.camera_position(),
            state.camera_target,
            0.0,
        )
        .unwrap();
        assert!(state.camera_distance < distance_before);
        assert!((anchor_after[0] - anchor_before[0]).abs() < 0.002);
        assert!((anchor_after[2] - anchor_before[2]).abs() < 0.002);
    }

    #[test]
    fn forward_and_backward_keys_emit_expected_z_axes() {
        fn z_axis(state: &mut State) -> f32 {
            let mut output = ProjectFrameOutputV1 {
                override_count: 0,
                request_redraw: 0,
                overrides: [ProjectF32OverrideV1::default(); 32],
            };
            state.write_frame(
                ProjectFrameContextV1::default(),
                &mut output,
            );
            output.overrides[..output.override_count as usize]
                .iter()
                .find_map(|entry| {
                    let length = entry
                        .name
                        .iter()
                        .position(|byte| *byte == 0)
                        .unwrap_or(entry.name.len());
                    (&entry.name[..length] == b"interaction.input_z")
                        .then_some(entry.value)
                })
                .unwrap()
        }

        let mut state = State::default();
        state.forward = true;
        assert_eq!(z_axis(&mut state), -1.0);
        state.forward = false;
        state.backward = true;
        assert_eq!(z_axis(&mut state), 1.0);
    }
}
