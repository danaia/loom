use std::{
    ffi::{CStr, CString, c_char, c_int, c_uint, c_void},
    fs,
    path::{Path, PathBuf},
    process::Command,
    ptr, thread,
    time::{Duration, Instant},
};

use libloading::Library;
use loom_core::{Target, ViewId};
use loom_validator::ValidatedModuleGraph;
use loom_windowing::WindowLayoutConfig;
use minifb::{Key, Window, WindowOptions};
use serde::Serialize;
use tempfile::TempDir;

mod panel;

pub use panel::ProjectUi;

type CUdevice = c_int;
type CUcontext = *mut c_void;
type CUmodule = *mut c_void;
type CUfunction = *mut c_void;
type CUdeviceptr = u64;
type CUresult = c_int;

const CUDA_SUCCESS: CUresult = 0;
const PARTICLE_CAPACITY: usize = 32;
const RENDER_CAPACITY: usize = 64;

#[derive(Debug, Serialize)]
pub struct CudaDiagnostic {
    pub code: &'static str,
    pub message: String,
}

impl CudaDiagnostic {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
struct Vec4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

pub struct CudaRuntime;

impl CudaRuntime {
    pub fn run_project(
        validated: ValidatedModuleGraph,
        project_root: Option<PathBuf>,
        project_ui: Option<ProjectUi>,
    ) -> Result<(), CudaDiagnostic> {
        if validated.graph().target != Target::Cuda {
            return Err(CudaDiagnostic::new(
                "unsupported_graph",
                "CUDA runtime expects a `target cuda` graph",
            ));
        }
        if validated.graph().name != "baseline" {
            return Err(CudaDiagnostic::new(
                "unsupported_graph",
                "the first CUDA runtime slice currently runs only the Baseline project",
            ));
        }
        if validated.execution_plan().schedules.len() != 1 {
            return Err(CudaDiagnostic::new(
                "unsupported_graph",
                "the first CUDA runtime slice requires exactly one schedule",
            ));
        }
        let project_root = project_root.ok_or_else(|| {
            CudaDiagnostic::new(
                "project_root_missing",
                "CUDA source execution requires a project root",
            )
        })?;
        let kernel_source = project_root.join("kernels/baseline.cu");
        if !kernel_source.is_file() {
            return Err(CudaDiagnostic::new(
                "source_missing",
                format!(
                    "CUDA baseline kernel source is missing: `{}`",
                    kernel_source.display()
                ),
            ));
        }
        let layout = WindowLayoutConfig::load(&project_root)
            .map_err(|message| CudaDiagnostic::new("window_config_invalid", message))?;
        let rate_hz = match validated.graph().schedules[0].timing {
            loom_core::ScheduleTiming::Fixed { rate_hz, .. } => rate_hz,
        };
        let ptx = compile_ptx(&kernel_source)?;
        let mut driver = CudaDriver::load()?;
        let mut state = BaselineCudaState::new(&mut driver, &ptx)?;
        let title = "Loom CUDA Baseline";
        let width = layout.viewer_width.clamp(320.0, 4096.0).round() as usize;
        let height = layout.viewer_height.clamp(240.0, 4096.0).round() as usize;
        let mut window = Window::new(title, width, height, WindowOptions::default())
            .map_err(|error| CudaDiagnostic::new("window_creation_failed", error.to_string()))?;
        let mut panel = project_ui.map(panel::PanelBridge::launch).transpose()?;
        window.set_target_fps(rate_hz.min(240) as usize);
        let mut pixels = vec![0_u32; width * height];
        let dt = 1.0_f32 / rate_hz as f32;
        let frame_interval = Duration::from_secs_f64(1.0 / rate_hz as f64);
        let mut next_frame = Instant::now();

        println!(
            "runtime_fingerprint:\n{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "backend": "cuda",
                "runtime": "baseline-v0",
                "module": validated.graph().name,
                "artifact_fingerprint": validated.artifact_fingerprint(),
                "view": view_name(validated.graph()),
            }))
            .expect("runtime fingerprint serialization")
        );

        while window.is_open() && !window.is_key_down(Key::Escape) {
            if let Some(panel) = panel.as_mut() {
                for command in panel.drain_commands().collect::<Vec<_>>() {
                    match command {
                        panel::PanelCommand::Set(control) => {
                            state.set_control(&mut driver, &control.name, control.value)?;
                        }
                        panel::PanelCommand::Quit => {
                            return Ok(());
                        }
                        panel::PanelCommand::Reload { generation } => {
                            panel.publish_reload_status(generation, &Ok(()));
                        }
                        panel::PanelCommand::WindowFrame { frame } => {
                            let _ = frame;
                        }
                    }
                }
            }
            state.step(&mut driver, dt)?;
            let frame = state.read_frame(&mut driver)?;
            draw_frame(&frame, &mut pixels, width, height);
            if let Some(panel) = panel.as_mut() {
                panel.publish(&state.panel_values());
            }
            window
                .update_with_buffer(&pixels, width, height)
                .map_err(|error| CudaDiagnostic::new("window_update_failed", error.to_string()))?;
            let now = Instant::now();
            if next_frame > now {
                thread::sleep(next_frame - now);
            }
            next_frame = next_frame.max(now) + frame_interval;
        }

        Ok(())
    }
}

fn view_name(graph: &loom_core::ModuleGraph) -> Option<&str> {
    graph
        .views
        .iter()
        .find(|view| view.id == ViewId(0))
        .map(|view| view.name.as_str())
}

fn compile_ptx(source: &Path) -> Result<TempPtx, CudaDiagnostic> {
    let temp = TempDir::new()
        .map_err(|error| CudaDiagnostic::new("ptx_temp_failed", error.to_string()))?;
    let ptx_path = temp.path().join("baseline.ptx");
    let output = Command::new("nvcc")
        .args(["-std=c++17", "--ptx"])
        .arg(source)
        .arg("-o")
        .arg(&ptx_path)
        .output()
        .map_err(|error| {
            CudaDiagnostic::new("nvcc_failed", format!("could not run nvcc: {error}"))
        })?;
    if !output.status.success() {
        return Err(CudaDiagnostic::new(
            "nvcc_failed",
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let bytes = fs::read(&ptx_path)
        .map_err(|error| CudaDiagnostic::new("ptx_read_failed", error.to_string()))?;
    Ok(TempPtx { _temp: temp, bytes })
}

struct TempPtx {
    _temp: TempDir,
    bytes: Vec<u8>,
}

struct BaselineCudaState {
    module: CUmodule,
    apply: CUfunction,
    move_particle: CUfunction,
    project: CUfunction,
    positions: DeviceBuffer<Vec3>,
    velocities: DeviceBuffer<Vec3>,
    active: DeviceBuffer<f32>,
    targets: DeviceBuffer<Vec3>,
    target_active: DeviceBuffer<f32>,
    particle_types: DeviceBuffer<f32>,
    selected: DeviceBuffer<f32>,
    spawn_seen: DeviceBuffer<f32>,
    click_seen: DeviceBuffer<f32>,
    select_seen: DeviceBuffer<f32>,
    remove_seen: DeviceBuffer<f32>,
    dragging: DeviceBuffer<f32>,
    render_aspect: DeviceBuffer<f32>,
    render_color: DeviceBuffer<Vec4>,
    render_position: DeviceBuffer<Vec3>,
    render_radius: DeviceBuffer<f32>,
    values: BaselineValues,
}

impl BaselineCudaState {
    fn new(driver: &mut CudaDriver, ptx: &TempPtx) -> Result<Self, CudaDiagnostic> {
        let module = driver.load_module(&ptx.bytes)?;
        let apply = driver.get_function(module, "baseline_apply_interaction")?;
        let move_particle = driver.get_function(module, "baseline_move_particle")?;
        let project = driver.get_function(module, "baseline_project_particles")?;
        let positions = DeviceBuffer::new(driver, &[Vec3::default(); PARTICLE_CAPACITY])?;
        let velocities = DeviceBuffer::new(driver, &[Vec3::default(); PARTICLE_CAPACITY])?;
        let mut active_init = [0.0_f32; PARTICLE_CAPACITY];
        active_init[0] = 1.0;
        let active = DeviceBuffer::new(driver, &active_init)?;
        let targets = DeviceBuffer::new(driver, &[Vec3::default(); PARTICLE_CAPACITY])?;
        let target_active = DeviceBuffer::new(driver, &[0.0_f32; PARTICLE_CAPACITY])?;
        let particle_types = DeviceBuffer::new(driver, &[0.0_f32; PARTICLE_CAPACITY])?;
        let selected = DeviceBuffer::new(driver, &[0.0_f32])?;
        let spawn_seen = DeviceBuffer::new(driver, &[0.0_f32])?;
        let click_seen = DeviceBuffer::new(driver, &[0.0_f32])?;
        let select_seen = DeviceBuffer::new(driver, &[0.0_f32])?;
        let remove_seen = DeviceBuffer::new(driver, &[0.0_f32])?;
        let dragging = DeviceBuffer::new(driver, &[0.0_f32])?;
        let render_aspect = DeviceBuffer::new(driver, &[0.0_f32; RENDER_CAPACITY])?;
        let render_color = DeviceBuffer::new(driver, &[Vec4::default(); RENDER_CAPACITY])?;
        let render_position = DeviceBuffer::new(driver, &[Vec3::default(); RENDER_CAPACITY])?;
        let render_radius = DeviceBuffer::new(driver, &[0.0_f32; RENDER_CAPACITY])?;
        let values = BaselineValues::new(driver)?;
        Ok(Self {
            module,
            apply,
            move_particle,
            project,
            positions,
            velocities,
            active,
            targets,
            target_active,
            particle_types,
            selected,
            spawn_seen,
            click_seen,
            select_seen,
            remove_seen,
            dragging,
            render_aspect,
            render_color,
            render_position,
            render_radius,
            values,
        })
    }

    fn step(&mut self, driver: &mut CudaDriver, dt: f32) -> Result<(), CudaDiagnostic> {
        self.values.dt.copy_from(driver, &[dt])?;
        launch!(
            driver,
            self.apply,
            1,
            1,
            self.positions,
            self.velocities,
            self.active,
            self.targets,
            self.target_active,
            self.particle_types,
            self.selected,
            self.spawn_seen,
            self.click_seen,
            self.values.click_x,
            self.values.click_y,
            self.values.click_z,
            self.values.click_generation,
            self.values.spawn_x,
            self.values.spawn_y,
            self.values.spawn_z,
            self.values.spawn_generation,
            self.values.spawn_slot,
            self.values.spawn_type,
            self.select_seen,
            self.remove_seen,
            self.values.select_command,
            self.values.remove_command,
            self.values.selection_radius,
            self.values.reset,
            self.dragging,
            self.values.pointer_down,
            self.values.click_x,
            self.values.click_y,
            self.values.click_z
        )?;
        launch!(
            driver,
            self.move_particle,
            1,
            64,
            self.positions,
            self.velocities,
            self.active,
            self.targets,
            self.target_active,
            self.values.gravity,
            self.values.space_drag,
            self.values.target_spring,
            self.values.target_damping,
            self.values.arrival_radius,
            self.values.half_extent_x,
            self.values.half_extent_y,
            self.values.half_extent_z,
            self.values.dt
        )?;
        launch!(
            driver,
            self.project,
            1,
            64,
            self.positions,
            self.active,
            self.targets,
            self.target_active,
            self.particle_types,
            self.selected,
            self.render_position,
            self.render_radius,
            self.render_color,
            self.render_aspect,
            self.values.radius,
            self.values.target_radius,
            self.values.aspect
        )?;
        driver.synchronize()
    }

    fn read_frame(&self, driver: &mut CudaDriver) -> Result<Frame, CudaDiagnostic> {
        Ok(Frame {
            positions: self.render_position.copy_to_vec(driver, RENDER_CAPACITY)?,
            radii: self.render_radius.copy_to_vec(driver, RENDER_CAPACITY)?,
            colors: self.render_color.copy_to_vec(driver, RENDER_CAPACITY)?,
        })
    }

    fn set_control(
        &mut self,
        driver: &mut CudaDriver,
        name: &str,
        value: f32,
    ) -> Result<(), CudaDiagnostic> {
        match name {
            "interaction.space_drag" => self
                .values
                .space_drag
                .copy_from(driver, &[value.clamp(0.0, 0.5)]),
            "interaction.agent_type" => self
                .values
                .spawn_type
                .copy_from(driver, &[value.clamp(0.0, 2.0).round()]),
            "interaction.reset" if value > 0.5 => self.values.reset.copy_from(driver, &[1.0]),
            "interaction.reset" => self.values.reset.copy_from(driver, &[0.0]),
            "interaction.select_particle" => self
                .values
                .select_command
                .copy_from(driver, &[value.clamp(0.0, 31.0).round()]),
            "interaction.remove_particle" => self
                .values
                .remove_command
                .copy_from(driver, &[value.clamp(0.0, 31.0).round()]),
            _ => Ok(()),
        }
    }

    fn panel_values(&self) -> Vec<(String, f32)> {
        vec![
            ("interaction.hud_fps".to_owned(), 120.0),
            ("interaction.hud_gpu_mb".to_owned(), 0.0),
            ("interaction.hud_gpu_frame_ms".to_owned(), 0.0),
            ("interaction.hud_gpu_budget_ms".to_owned(), 8.333333),
            ("interaction.hud_gpu_pressure".to_owned(), 0.0),
        ]
    }
}

impl Drop for BaselineCudaState {
    fn drop(&mut self) {
        let _ = self.module;
    }
}

struct BaselineValues {
    click_x: DeviceBuffer<f32>,
    click_y: DeviceBuffer<f32>,
    click_z: DeviceBuffer<f32>,
    click_generation: DeviceBuffer<f32>,
    spawn_x: DeviceBuffer<f32>,
    spawn_y: DeviceBuffer<f32>,
    spawn_z: DeviceBuffer<f32>,
    spawn_generation: DeviceBuffer<f32>,
    spawn_slot: DeviceBuffer<f32>,
    spawn_type: DeviceBuffer<f32>,
    select_command: DeviceBuffer<f32>,
    remove_command: DeviceBuffer<f32>,
    selection_radius: DeviceBuffer<f32>,
    reset: DeviceBuffer<f32>,
    pointer_down: DeviceBuffer<f32>,
    gravity: DeviceBuffer<Vec3>,
    space_drag: DeviceBuffer<f32>,
    target_spring: DeviceBuffer<f32>,
    target_damping: DeviceBuffer<f32>,
    arrival_radius: DeviceBuffer<f32>,
    half_extent_x: DeviceBuffer<f32>,
    half_extent_y: DeviceBuffer<f32>,
    half_extent_z: DeviceBuffer<f32>,
    dt: DeviceBuffer<f32>,
    radius: DeviceBuffer<f32>,
    target_radius: DeviceBuffer<f32>,
    aspect: DeviceBuffer<f32>,
}

impl BaselineValues {
    fn new(driver: &mut CudaDriver) -> Result<Self, CudaDiagnostic> {
        Ok(Self {
            click_x: scalar(driver, 0.0)?,
            click_y: scalar(driver, 0.0)?,
            click_z: scalar(driver, 0.0)?,
            click_generation: scalar(driver, 0.0)?,
            spawn_x: scalar(driver, 0.0)?,
            spawn_y: scalar(driver, 0.0)?,
            spawn_z: scalar(driver, 0.0)?,
            spawn_generation: scalar(driver, 0.0)?,
            spawn_slot: scalar(driver, 0.0)?,
            spawn_type: scalar(driver, 0.0)?,
            select_command: scalar(driver, 0.0)?,
            remove_command: scalar(driver, 0.0)?,
            selection_radius: scalar(driver, 0.11)?,
            reset: scalar(driver, 0.0)?,
            pointer_down: scalar(driver, 0.0)?,
            gravity: DeviceBuffer::new(
                driver,
                &[Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                }],
            )?,
            space_drag: scalar(driver, 0.0)?,
            target_spring: scalar(driver, 42.0)?,
            target_damping: scalar(driver, 11.0)?,
            arrival_radius: scalar(driver, 0.035)?,
            half_extent_x: scalar(driver, 1.55)?,
            half_extent_y: scalar(driver, 1.0)?,
            half_extent_z: scalar(driver, 1.25)?,
            dt: scalar(driver, 1.0 / 120.0)?,
            radius: scalar(driver, 0.05)?,
            target_radius: scalar(driver, 0.018)?,
            aspect: scalar(driver, 1.333333)?,
        })
    }
}

fn scalar(driver: &mut CudaDriver, value: f32) -> Result<DeviceBuffer<f32>, CudaDiagnostic> {
    DeviceBuffer::new(driver, &[value])
}

struct Frame {
    positions: Vec<Vec3>,
    radii: Vec<f32>,
    colors: Vec<Vec4>,
}

fn draw_frame(frame: &Frame, pixels: &mut [u32], width: usize, height: usize) {
    pixels.fill(0x07090f);
    for index in 0..frame.positions.len() {
        let radius = frame.radii[index];
        if radius <= 0.0 {
            continue;
        }
        let position = frame.positions[index];
        let color = frame.colors[index];
        let cx = ((position.x * 0.5 + 0.5) * width as f32).round() as i32;
        let cy = ((0.5 - position.y * 0.5) * height as f32).round() as i32;
        let r = (radius * height as f32 * 0.5).max(2.0).round() as i32;
        let rgb = color_to_u32(color);
        for y in (cy - r).max(0)..=(cy + r).min(height as i32 - 1) {
            for x in (cx - r).max(0)..=(cx + r).min(width as i32 - 1) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= r * r {
                    pixels[y as usize * width + x as usize] = rgb;
                }
            }
        }
    }
}

fn color_to_u32(color: Vec4) -> u32 {
    let r = (color.x.clamp(0.0, 1.0) * 255.0) as u32;
    let g = (color.y.clamp(0.0, 1.0) * 255.0) as u32;
    let b = (color.z.clamp(0.0, 1.0) * 255.0) as u32;
    (r << 16) | (g << 8) | b
}

macro_rules! launch {
    ($driver:expr, $function:expr, $grid:expr, $block:expr, $($buffer:expr),+ $(,)?) => {{
        let mut params = [$(($buffer.device_ptr() as *mut c_void)),+];
        $driver.launch($function, $grid, $block, &mut params)
    }};
}
use launch;

struct DeviceBuffer<T> {
    ptr: CUdeviceptr,
    len: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Copy> DeviceBuffer<T> {
    fn new(driver: &mut CudaDriver, values: &[T]) -> Result<Self, CudaDiagnostic> {
        let ptr = driver.mem_alloc(std::mem::size_of_val(values))?;
        let buffer = Self {
            ptr,
            len: values.len(),
            _marker: std::marker::PhantomData,
        };
        buffer.copy_from(driver, values)?;
        Ok(buffer)
    }

    fn copy_from(&self, driver: &mut CudaDriver, values: &[T]) -> Result<(), CudaDiagnostic> {
        driver.copy_htod(self.ptr, values)
    }

    fn copy_to_vec(&self, driver: &mut CudaDriver, len: usize) -> Result<Vec<T>, CudaDiagnostic> {
        debug_assert!(len <= self.len);
        let mut values = Vec::<T>::with_capacity(len);
        unsafe {
            values.set_len(len);
        }
        driver.copy_dtoh(&mut values, self.ptr)?;
        Ok(values)
    }

    fn device_ptr(&self) -> *mut CUdeviceptr {
        (&self.ptr as *const CUdeviceptr).cast_mut()
    }
}

struct CudaDriver {
    _library: Library,
    context: CUcontext,
    cu_init: unsafe extern "C" fn(c_uint) -> CUresult,
    cu_device_get: unsafe extern "C" fn(*mut CUdevice, c_int) -> CUresult,
    cu_ctx_create: unsafe extern "C" fn(*mut CUcontext, c_uint, CUdevice) -> CUresult,
    cu_ctx_destroy: unsafe extern "C" fn(CUcontext) -> CUresult,
    cu_module_load_data: unsafe extern "C" fn(*mut CUmodule, *const c_void) -> CUresult,
    cu_module_get_function:
        unsafe extern "C" fn(*mut CUfunction, CUmodule, *const c_char) -> CUresult,
    cu_mem_alloc: unsafe extern "C" fn(*mut CUdeviceptr, usize) -> CUresult,
    cu_memcpy_htod: unsafe extern "C" fn(CUdeviceptr, *const c_void, usize) -> CUresult,
    cu_memcpy_dtoh: unsafe extern "C" fn(*mut c_void, CUdeviceptr, usize) -> CUresult,
    cu_launch_kernel: unsafe extern "C" fn(
        CUfunction,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        c_uint,
        CUdeviceptr,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> CUresult,
    cu_ctx_synchronize: unsafe extern "C" fn() -> CUresult,
    cu_get_error_string: unsafe extern "C" fn(CUresult, *mut *const c_char) -> CUresult,
}

impl CudaDriver {
    fn load() -> Result<Self, CudaDiagnostic> {
        let library = unsafe { Library::new("libcuda.so.1") }
            .or_else(|_| unsafe { Library::new("libcuda.so") })
            .map_err(|error| CudaDiagnostic::new("cuda_driver_missing", error.to_string()))?;
        macro_rules! load_symbol {
            ($name:literal, $ty:ty) => {
                *unsafe { library.get::<$ty>($name) }.map_err(|error| {
                    CudaDiagnostic::new("cuda_symbol_missing", error.to_string())
                })?
            };
        }
        let cu_init = load_symbol!(b"cuInit\0", unsafe extern "C" fn(c_uint) -> CUresult);
        let cu_device_get = load_symbol!(
            b"cuDeviceGet\0",
            unsafe extern "C" fn(*mut CUdevice, c_int) -> CUresult
        );
        let cu_ctx_create = load_symbol!(
            b"cuCtxCreate_v2\0",
            unsafe extern "C" fn(*mut CUcontext, c_uint, CUdevice) -> CUresult
        );
        let cu_ctx_destroy = load_symbol!(
            b"cuCtxDestroy_v2\0",
            unsafe extern "C" fn(CUcontext) -> CUresult
        );
        let cu_module_load_data = load_symbol!(
            b"cuModuleLoadData\0",
            unsafe extern "C" fn(*mut CUmodule, *const c_void) -> CUresult
        );
        let cu_module_get_function = load_symbol!(
            b"cuModuleGetFunction\0",
            unsafe extern "C" fn(*mut CUfunction, CUmodule, *const c_char) -> CUresult
        );
        let cu_mem_alloc = load_symbol!(
            b"cuMemAlloc_v2\0",
            unsafe extern "C" fn(*mut CUdeviceptr, usize) -> CUresult
        );
        let cu_memcpy_htod = load_symbol!(
            b"cuMemcpyHtoD_v2\0",
            unsafe extern "C" fn(CUdeviceptr, *const c_void, usize) -> CUresult
        );
        let cu_memcpy_dtoh = load_symbol!(
            b"cuMemcpyDtoH_v2\0",
            unsafe extern "C" fn(*mut c_void, CUdeviceptr, usize) -> CUresult
        );
        let cu_launch_kernel = load_symbol!(
            b"cuLaunchKernel\0",
            unsafe extern "C" fn(
                CUfunction,
                c_uint,
                c_uint,
                c_uint,
                c_uint,
                c_uint,
                c_uint,
                c_uint,
                CUdeviceptr,
                *mut *mut c_void,
                *mut *mut c_void,
            ) -> CUresult
        );
        let cu_ctx_synchronize =
            load_symbol!(b"cuCtxSynchronize\0", unsafe extern "C" fn() -> CUresult);
        let cu_get_error_string = load_symbol!(
            b"cuGetErrorString\0",
            unsafe extern "C" fn(CUresult, *mut *const c_char) -> CUresult
        );
        let mut driver = Self {
            _library: library,
            context: ptr::null_mut(),
            cu_init,
            cu_device_get,
            cu_ctx_create,
            cu_ctx_destroy,
            cu_module_load_data,
            cu_module_get_function,
            cu_mem_alloc,
            cu_memcpy_htod,
            cu_memcpy_dtoh,
            cu_launch_kernel,
            cu_ctx_synchronize,
            cu_get_error_string,
        };
        driver.check(unsafe { (driver.cu_init)(0) }, "cuInit")?;
        let mut device = 0;
        driver.check(
            unsafe { (driver.cu_device_get)(&mut device, 0) },
            "cuDeviceGet",
        )?;
        let mut context = ptr::null_mut();
        driver.check(
            unsafe { (driver.cu_ctx_create)(&mut context, 0, device) },
            "cuCtxCreate",
        )?;
        driver.context = context;
        Ok(driver)
    }

    fn load_module(&mut self, ptx: &[u8]) -> Result<CUmodule, CudaDiagnostic> {
        let mut bytes = ptx.to_vec();
        bytes.push(0);
        let mut module = ptr::null_mut();
        self.check(
            unsafe { (self.cu_module_load_data)(&mut module, bytes.as_ptr().cast()) },
            "cuModuleLoadData",
        )?;
        Ok(module)
    }

    fn get_function(&mut self, module: CUmodule, name: &str) -> Result<CUfunction, CudaDiagnostic> {
        let name = CString::new(name).expect("kernel name has no nul bytes");
        let mut function = ptr::null_mut();
        self.check(
            unsafe { (self.cu_module_get_function)(&mut function, module, name.as_ptr()) },
            "cuModuleGetFunction",
        )?;
        Ok(function)
    }

    fn mem_alloc(&mut self, bytes: usize) -> Result<CUdeviceptr, CudaDiagnostic> {
        let mut ptr = 0;
        self.check(
            unsafe { (self.cu_mem_alloc)(&mut ptr, bytes) },
            "cuMemAlloc",
        )?;
        Ok(ptr)
    }

    fn copy_htod<T>(&mut self, device: CUdeviceptr, values: &[T]) -> Result<(), CudaDiagnostic> {
        self.check(
            unsafe {
                (self.cu_memcpy_htod)(
                    device,
                    values.as_ptr().cast(),
                    std::mem::size_of_val(values),
                )
            },
            "cuMemcpyHtoD",
        )
    }

    fn copy_dtoh<T>(
        &mut self,
        values: &mut [T],
        device: CUdeviceptr,
    ) -> Result<(), CudaDiagnostic> {
        self.check(
            unsafe {
                (self.cu_memcpy_dtoh)(
                    values.as_mut_ptr().cast(),
                    device,
                    std::mem::size_of_val(values),
                )
            },
            "cuMemcpyDtoH",
        )
    }

    fn launch(
        &mut self,
        function: CUfunction,
        grid: c_uint,
        block: c_uint,
        params: &mut [*mut c_void],
    ) -> Result<(), CudaDiagnostic> {
        self.check(
            unsafe {
                (self.cu_launch_kernel)(
                    function,
                    grid,
                    1,
                    1,
                    block,
                    1,
                    1,
                    0,
                    0,
                    params.as_mut_ptr(),
                    ptr::null_mut(),
                )
            },
            "cuLaunchKernel",
        )
    }

    fn synchronize(&mut self) -> Result<(), CudaDiagnostic> {
        self.check(unsafe { (self.cu_ctx_synchronize)() }, "cuCtxSynchronize")
    }

    fn check(&self, result: CUresult, operation: &str) -> Result<(), CudaDiagnostic> {
        if result == CUDA_SUCCESS {
            return Ok(());
        }
        let mut message = ptr::null();
        let text = if unsafe { (self.cu_get_error_string)(result, &mut message) } == CUDA_SUCCESS
            && !message.is_null()
        {
            unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned()
        } else {
            format!("CUDA error {result}")
        };
        Err(CudaDiagnostic::new(
            "cuda_driver_error",
            format!("{operation}: {text}"),
        ))
    }
}

impl Drop for CudaDriver {
    fn drop(&mut self) {
        if !self.context.is_null() {
            unsafe {
                (self.cu_ctx_destroy)(self.context);
            }
        }
    }
}
