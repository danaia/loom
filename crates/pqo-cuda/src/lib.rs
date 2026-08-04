#![cfg(target_os = "linux")]

use std::{
    ffi::{CStr, CString, c_char, c_int, c_uint, c_void},
    fs,
    path::{Path, PathBuf},
    process::Command,
    ptr,
    time::{Duration, Instant},
};

use pqo_core::{
    Backend, ComputeBackend, DataType, DispatchDomain, Literal, ResourceId, ScalarType,
    StreamInitializer, StreamLength, ValueKind, ViewBackend,
};
use pqo_validator::ValidatedModuleGraph;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type CuDevice = c_int;
type CuContext = *mut c_void;
type CuStream = *mut c_void;
type CuModule = *mut c_void;
type CuFunction = *mut c_void;
type CuGraph = *mut c_void;
type CuGraphExec = *mut c_void;
type CuGraphNode = *mut c_void;
type CuEvent = *mut c_void;
type CuDevicePtr = u64;
type CuResult = c_int;

const CUDA_SUCCESS: CuResult = 0;
const CU_STREAM_NON_BLOCKING: c_uint = 1;
const CU_STREAM_CAPTURE_MODE_GLOBAL: c_uint = 0;

#[link(name = "cuda")]
unsafe extern "C" {
    fn cuInit(flags: c_uint) -> CuResult;
    fn cuDriverGetVersion(version: *mut c_int) -> CuResult;
    fn cuDeviceGet(device: *mut CuDevice, ordinal: c_int) -> CuResult;
    fn cuDeviceComputeCapability(
        major: *mut c_int,
        minor: *mut c_int,
        device: CuDevice,
    ) -> CuResult;
    fn cuDeviceGetName(name: *mut c_char, len: c_int, device: CuDevice) -> CuResult;
    fn cuDeviceGetUuid_v2(uuid: *mut CuUuid, device: CuDevice) -> CuResult;
    fn cuCtxCreate_v2(context: *mut CuContext, flags: c_uint, device: CuDevice) -> CuResult;
    fn cuCtxDestroy_v2(context: CuContext) -> CuResult;
    fn cuStreamCreate(stream: *mut CuStream, flags: c_uint) -> CuResult;
    fn cuStreamDestroy_v2(stream: CuStream) -> CuResult;
    fn cuStreamSynchronize(stream: CuStream) -> CuResult;
    fn cuEventCreate(event: *mut CuEvent, flags: c_uint) -> CuResult;
    fn cuEventRecord(event: CuEvent, stream: CuStream) -> CuResult;
    fn cuEventQuery(event: CuEvent) -> CuResult;
    fn cuEventDestroy_v2(event: CuEvent) -> CuResult;
    fn cuEventElapsedTime(milliseconds: *mut f32, start: CuEvent, end: CuEvent) -> CuResult;
    fn cuMemGetInfo_v2(free: *mut usize, total: *mut usize) -> CuResult;
    fn cuMemAllocAsync(ptr: *mut CuDevicePtr, bytes: usize, stream: CuStream) -> CuResult;
    fn cuMemFreeAsync(ptr: CuDevicePtr, stream: CuStream) -> CuResult;
    fn cuMemsetD8Async(ptr: CuDevicePtr, value: u8, count: usize, stream: CuStream) -> CuResult;
    fn cuMemcpyHtoDAsync_v2(
        destination: CuDevicePtr,
        source: *const c_void,
        bytes: usize,
        stream: CuStream,
    ) -> CuResult;
    fn cuMemcpyDtoH_v2(destination: *mut c_void, source: CuDevicePtr, bytes: usize) -> CuResult;
    fn cuModuleLoadData(module: *mut CuModule, image: *const c_void) -> CuResult;
    fn cuModuleUnload(module: CuModule) -> CuResult;
    fn cuModuleGetFunction(
        function: *mut CuFunction,
        module: CuModule,
        name: *const c_char,
    ) -> CuResult;
    fn cuLaunchKernel(
        function: CuFunction,
        grid_x: c_uint,
        grid_y: c_uint,
        grid_z: c_uint,
        block_x: c_uint,
        block_y: c_uint,
        block_z: c_uint,
        shared_memory_bytes: c_uint,
        stream: CuStream,
        kernel_params: *mut *mut c_void,
        extra: *mut *mut c_void,
    ) -> CuResult;
    fn cuStreamBeginCapture(stream: CuStream, mode: c_uint) -> CuResult;
    fn cuStreamEndCapture(stream: CuStream, graph: *mut CuGraph) -> CuResult;
    fn cuGraphInstantiate_v2(
        executable: *mut CuGraphExec,
        graph: CuGraph,
        error_node: *mut CuGraphNode,
        log_buffer: *mut c_char,
        buffer_size: usize,
    ) -> CuResult;
    fn cuGraphLaunch(executable: CuGraphExec, stream: CuStream) -> CuResult;
    fn cuGraphExecDestroy(executable: CuGraphExec) -> CuResult;
    fn cuGraphDestroy(graph: CuGraph) -> CuResult;
    fn cuGetErrorName(error: CuResult, name: *mut *const c_char) -> CuResult;
    fn cuGetErrorString(error: CuResult, description: *mut *const c_char) -> CuResult;
}

unsafe extern "C" {
    fn pqo_cuda_import_probe(
        ordinal: c_int,
        memory_fd: c_int,
        allocation_size: u64,
        mapped_size: u64,
        semaphore_fd: c_int,
        signal_value: u64,
    ) -> CuResult;
}

#[repr(C)]
struct CuUuid {
    bytes: [u8; 16],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionProfile {
    Balanced,
    MaximumThroughput,
    MinimumLatency,
    DeterministicDebug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryMode {
    Development,
    Benchmark,
    Production,
}

impl ExecutionProfile {
    fn fraction(self) -> f64 {
        match self {
            Self::Balanced => 0.70,
            Self::MaximumThroughput => 0.82,
            Self::MinimumLatency => 0.65,
            Self::DeterministicDebug => 0.50,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CudaConfig {
    pub ticks: u64,
    pub profile: ExecutionProfile,
    pub maximum_vram_bytes: Option<u64>,
    pub reserved_vram_bytes: u64,
    pub nvcc: PathBuf,
    pub inspect_stream: Option<String>,
    pub telemetry: TelemetryMode,
    pub shutdown_timeout: Duration,
    pub warmup_ticks: u64,
}

impl Default for CudaConfig {
    fn default() -> Self {
        let reserved_vram_bytes = std::env::var("PQO_RESERVE_VRAM_GIB")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(8)
            * 1024
            * 1024
            * 1024;
        Self {
            ticks: 1,
            profile: ExecutionProfile::Balanced,
            maximum_vram_bytes: None,
            reserved_vram_bytes,
            nvcc: std::env::var_os("PQO_NVCC")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("nvcc")),
            inspect_stream: std::env::var("PQO_INSPECT_STREAM").ok(),
            telemetry: match std::env::var("PQO_TELEMETRY").as_deref() {
                Ok("benchmark") => TelemetryMode::Benchmark,
                Ok("production") => TelemetryMode::Production,
                _ => TelemetryMode::Development,
            },
            shutdown_timeout: Duration::from_millis(
                std::env::var("PQO_SHUTDOWN_TIMEOUT_MS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(5_000),
            ),
            warmup_ticks: std::env::var("PQO_WARMUP_TICKS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CudaRunReport {
    pub status: String,
    pub device_name: String,
    pub device_uuid: String,
    pub compute_capability: String,
    pub artifact_path: String,
    pub logical_ticks: u64,
    pub free_vram_bytes_at_launch: u64,
    pub total_vram_bytes: u64,
    pub pqo_budget_bytes: u64,
    pub graph_launches: u64,
    pub inspection: Option<CudaInspection>,
    pub telemetry: TelemetryMode,
    pub semantic_fingerprint: String,
    pub artifact_fingerprint: String,
    pub backend_fingerprint: String,
    pub hardware_fingerprint: String,
    pub tuning: TuningRecord,
    pub timing: Option<CudaTimingSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CudaTimingSummary {
    pub samples: usize,
    pub p50_ms: f32,
    pub p95_ms: f32,
    pub p99_ms: f32,
    pub maximum_ms: f32,
    pub ticks_over_8_333_ms: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TuningRecord {
    pub candidates: Vec<TuningCandidate>,
    pub selected: usize,
    pub warmup_launches: u64,
    pub measured_launches: u64,
    pub metric: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TuningCandidate {
    pub block_size: u32,
    pub selected: bool,
    pub rejection_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CudaInspection {
    pub stream: String,
    pub bytes: usize,
    pub fnv1a64: String,
    pub first_f32: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CudaDeviceIdentity {
    pub ordinal: i32,
    pub name: String,
    pub uuid: [u8; 16],
    pub compute_capability: (i32, i32),
    pub driver_version: i32,
}

pub struct CudaRuntime;

impl CudaRuntime {
    /// Imports Vulkan-owned opaque file descriptors. CUDA assumes ownership of
    /// both descriptors on successful import, writes the shared buffer, and
    /// signals the imported timeline semaphore.
    pub fn probe_external_resources(
        ordinal: i32,
        memory_fd: i32,
        allocation_size: u64,
        mapped_size: u64,
        semaphore_fd: i32,
        signal_value: u64,
    ) -> Result<(), String> {
        cuda(
            // SAFETY: ownership of valid Vulkan-exported descriptors transfers
            // to the CUDA imports performed by the C ABI shim.
            unsafe {
                pqo_cuda_import_probe(
                    ordinal,
                    memory_fd,
                    allocation_size,
                    mapped_size,
                    semaphore_fd,
                    signal_value,
                )
            },
            "CUDA/Vulkan external-resource probe",
        )
    }

    pub fn probe_device(ordinal: i32) -> Result<CudaDeviceIdentity, String> {
        // SAFETY: this only queries driver-owned device metadata.
        unsafe {
            cuda(cuInit(0), "cuInit")?;
            let mut device = 0;
            cuda(cuDeviceGet(&mut device, ordinal), "cuDeviceGet")?;
            let mut major = 0;
            let mut minor = 0;
            cuda(
                cuDeviceComputeCapability(&mut major, &mut minor, device),
                "cuDeviceComputeCapability",
            )?;
            let mut name = [0_i8; 256];
            cuda(
                cuDeviceGetName(name.as_mut_ptr(), name.len() as c_int, device),
                "cuDeviceGetName",
            )?;
            let mut uuid = CuUuid { bytes: [0; 16] };
            cuda(cuDeviceGetUuid_v2(&mut uuid, device), "cuDeviceGetUuid_v2")?;
            let mut driver_version = 0;
            cuda(
                cuDriverGetVersion(&mut driver_version),
                "cuDriverGetVersion",
            )?;
            Ok(CudaDeviceIdentity {
                ordinal,
                name: CStr::from_ptr(name.as_ptr()).to_string_lossy().into_owned(),
                uuid: uuid.bytes,
                compute_capability: (major, minor),
                driver_version,
            })
        }
    }

    pub fn run_headless(
        validated: &ValidatedModuleGraph,
        project_root: Option<&Path>,
        config: CudaConfig,
    ) -> Result<CudaRunReport, String> {
        let target = validated.target_profile();
        if target.compute != ComputeBackend::Cuda
            || !matches!(target.view, ViewBackend::Headless | ViewBackend::Vulkan)
        {
            return Err("CUDA runtime requires a cuda-headless or cuda-vulkan target".to_owned());
        }
        let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
        let packaged = packaged_single_kernel_cubin(validated, project_root);
        let (cubin, artifact_path) = if let Some(cubin) = packaged {
            (cubin, "packaged native sm_120 cubin".to_owned())
        } else {
            (
                compile_combined_cubin(validated, project_root, temporary.path(), &config.nvcc)?,
                "development native sm_120 cubin".to_owned(),
            )
        };
        // SAFETY: the CUDA handles created below are owned by this scope and are
        // destroyed after all work on the owned stream has completed.
        unsafe { execute(validated, &cubin, artifact_path, config) }
    }
}

fn packaged_single_kernel_cubin(
    validated: &ValidatedModuleGraph,
    project_root: Option<&Path>,
) -> Option<PathBuf> {
    let kernels = &validated.graph().kernels;
    if kernels.len() != 1 {
        return None;
    }
    let root = project_root?;
    let stem = kernels[0]
        .name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let path = root.join(format!(
        "targets/linux-x86_64-nvidia/compute/{stem}.sm_120.cubin"
    ));
    path.is_file().then_some(path)
}

fn compile_combined_cubin(
    validated: &ValidatedModuleGraph,
    project_root: Option<&Path>,
    directory: &Path,
    nvcc: &Path,
) -> Result<PathBuf, String> {
    let mut source = String::new();
    for kernel in &validated.graph().kernels {
        let implementation = kernel
            .implementations
            .iter()
            .find(|implementation| implementation.backend == Backend::Cuda)
            .ok_or_else(|| format!("kernel `{}` has no CUDA implementation", kernel.name))?;
        let text = if let Some(text) = &implementation.source_text {
            text.clone()
        } else {
            let root = project_root.ok_or_else(|| {
                format!(
                    "external CUDA source `{}` has no project root",
                    implementation.source
                )
            })?;
            fs::read_to_string(root.join(&implementation.source)).map_err(|error| {
                format!(
                    "could not read CUDA source `{}`: {error}",
                    implementation.source
                )
            })?
        };
        source.push_str(&text);
        source.push('\n');
    }
    let input = directory.join("program.cu");
    let output = directory.join("program.sm_120.cubin");
    fs::write(&input, source).map_err(|error| error.to_string())?;
    let result = Command::new(nvcc)
        .args(["-O3", "--std=c++17", "-cubin", "-arch=sm_120"])
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .map_err(|error| format!("could not invoke nvcc: {error}"))?;
    if !result.status.success() {
        return Err(format!(
            "CUDA compilation failed:\n{}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    Ok(output)
}

unsafe fn execute(
    validated: &ValidatedModuleGraph,
    cubin: &Path,
    artifact_path: String,
    config: CudaConfig,
) -> Result<CudaRunReport, String> {
    cuda(unsafe { cuInit(0) }, "cuInit")?;
    let mut device = 0;
    cuda(unsafe { cuDeviceGet(&mut device, 0) }, "cuDeviceGet")?;
    let mut major = 0;
    let mut minor = 0;
    cuda(
        unsafe { cuDeviceComputeCapability(&mut major, &mut minor, device) },
        "cuDeviceComputeCapability",
    )?;
    if (major, minor) < (12, 0) {
        return Err(format!(
            "CUDA target requires compute capability 12.0, found {major}.{minor}"
        ));
    }
    let mut name = [0_i8; 256];
    cuda(
        unsafe { cuDeviceGetName(name.as_mut_ptr(), name.len() as c_int, device) },
        "cuDeviceGetName",
    )?;
    let device_name = unsafe { CStr::from_ptr(name.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    let mut uuid = CuUuid { bytes: [0; 16] };
    cuda(
        unsafe { cuDeviceGetUuid_v2(&mut uuid, device) },
        "cuDeviceGetUuid_v2",
    )?;

    let mut context = ptr::null_mut();
    cuda(
        unsafe { cuCtxCreate_v2(&mut context, 0, device) },
        "cuCtxCreate",
    )?;
    let result = unsafe {
        execute_in_context(
            validated,
            cubin,
            artifact_path,
            &config,
            device_name,
            uuid,
            major,
            minor,
        )
    };
    let destroy = cuda(unsafe { cuCtxDestroy_v2(context) }, "cuCtxDestroy");
    match (result, destroy) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error),
    }
}

unsafe fn execute_in_context(
    validated: &ValidatedModuleGraph,
    cubin: &Path,
    artifact_path: String,
    config: &CudaConfig,
    device_name: String,
    uuid: CuUuid,
    major: i32,
    minor: i32,
) -> Result<CudaRunReport, String> {
    let mut stream = ptr::null_mut();
    cuda(
        unsafe { cuStreamCreate(&mut stream, CU_STREAM_NON_BLOCKING) },
        "cuStreamCreate",
    )?;
    let mut free = 0;
    let mut total = 0;
    cuda(
        unsafe { cuMemGetInfo_v2(&mut free, &mut total) },
        "cuMemGetInfo",
    )?;
    let free_bytes = free as u64;
    let fraction_budget = (free as f64 * config.profile.fraction()) as u64;
    // Keep the requested reserve when it fits, but do not turn an otherwise
    // valid small workload into a zero-budget launch merely because another
    // process has reduced currently free VRAM below the default 8 GiB reserve.
    // A quarter-gibibyte floor leaves room for CUDA driver bookkeeping while
    // still allowing compact simulations such as the CUDA crystal to start.
    let effective_reserve = config
        .reserved_vram_bytes
        .min(free_bytes.saturating_sub(256 * 1024 * 1024));
    let headroom_budget = free_bytes.saturating_sub(effective_reserve);
    let mut budget = fraction_budget.min(headroom_budget);
    if let Some(maximum) = config.maximum_vram_bytes {
        budget = budget.min(maximum);
    }

    let image = fs::read(cubin).map_err(|error| error.to_string())?;
    let mut module = ptr::null_mut();
    cuda(
        unsafe { cuModuleLoadData(&mut module, image.as_ptr().cast()) },
        "cuModuleLoadData",
    )?;

    let graph = validated.graph();
    let mut stream_ptrs = Vec::with_capacity(graph.resources.streams.len());
    let mut allocated_bytes = 0_u64;
    for resource in &graph.resources.streams {
        let bytes = u64::from(resource.capacity) * element_size(&resource.element_type)? as u64;
        allocated_bytes = allocated_bytes.saturating_add(bytes * u64::from(resource.buffering));
        if allocated_bytes > budget {
            return Err(format!(
                "declared CUDA streams require {allocated_bytes} bytes; budget is {budget}"
            ));
        }
        let mut pointer = 0;
        cuda(
            unsafe { cuMemAllocAsync(&mut pointer, bytes as usize, stream) },
            "cuMemAllocAsync(stream)",
        )?;
        cuda(
            unsafe { cuMemsetD8Async(pointer, 0, bytes as usize, stream) },
            "cuMemsetD8Async",
        )?;
        if let Some(initial) = &resource.initial {
            let encoded = encode_initializer(initial, &resource.element_type, resource.capacity)?;
            cuda(
                unsafe {
                    cuMemcpyHtoDAsync_v2(pointer, encoded.as_ptr().cast(), encoded.len(), stream)
                },
                "cuMemcpyHtoDAsync(stream)",
            )?;
        }
        stream_ptrs.push(pointer);
    }
    let mut value_ptrs = Vec::with_capacity(graph.resources.values.len());
    for value in &graph.resources.values {
        let encoded = encode_value(value, graph)?;
        let mut pointer = 0;
        cuda(
            unsafe { cuMemAllocAsync(&mut pointer, encoded.len(), stream) },
            "cuMemAllocAsync(value)",
        )?;
        cuda(
            unsafe {
                cuMemcpyHtoDAsync_v2(pointer, encoded.as_ptr().cast(), encoded.len(), stream)
            },
            "cuMemcpyHtoDAsync(value)",
        )?;
        value_ptrs.push(pointer);
    }
    cuda(unsafe { cuStreamSynchronize(stream) }, "initialization")?;

    cuda(
        unsafe { cuStreamBeginCapture(stream, CU_STREAM_CAPTURE_MODE_GLOBAL) },
        "cuStreamBeginCapture",
    )?;
    for pass in &validated.execution_plan().schedules[0].passes {
        let entry =
            CString::new(pass.implementation.entry.as_str()).map_err(|error| error.to_string())?;
        let mut function = ptr::null_mut();
        cuda(
            unsafe { cuModuleGetFunction(&mut function, module, entry.as_ptr()) },
            "cuModuleGetFunction",
        )?;
        let dispatch = dispatch_geometry(graph, &stream_ptrs, &pass.dispatch)?;
        let block = pass.threads_per_threadgroup.unwrap_or(256).clamp(1, 1024);
        let grid = dispatch.launch_count.div_ceil(block);
        let mut pointer_values = pass
            .abi
            .binding_order
            .iter()
            .map(|slot| {
                let binding = pass
                    .bindings
                    .iter()
                    .find(|binding| binding.slot == *slot)
                    .unwrap();
                match binding.resource {
                    ResourceId::Stream(id) => stream_ptrs[id.0 as usize],
                    ResourceId::Value(id) => value_ptrs[id.0 as usize],
                }
            })
            .collect::<Vec<_>>();
        let mut dynamic_count_pointer = dispatch.dynamic_count_pointer;
        let mut maximum_count = dispatch.maximum_count;
        let mut params = pointer_values
            .iter_mut()
            .map(|value| (value as *mut CuDevicePtr).cast::<c_void>())
            .collect::<Vec<_>>();
        params.push((&mut dynamic_count_pointer as *mut CuDevicePtr).cast());
        params.push((&mut maximum_count as *mut u32).cast());
        cuda(
            unsafe {
                cuLaunchKernel(
                    function,
                    grid,
                    1,
                    1,
                    block,
                    1,
                    1,
                    0,
                    stream,
                    params.as_mut_ptr(),
                    ptr::null_mut(),
                )
            },
            "cuLaunchKernel(capture)",
        )?;
    }
    let mut graph_handle = ptr::null_mut();
    cuda(
        unsafe { cuStreamEndCapture(stream, &mut graph_handle) },
        "cuStreamEndCapture",
    )?;
    let mut executable = ptr::null_mut();
    cuda(
        unsafe {
            cuGraphInstantiate_v2(
                &mut executable,
                graph_handle,
                ptr::null_mut(),
                ptr::null_mut(),
                0,
            )
        },
        "cuGraphInstantiate",
    )?;
    for _ in 0..config.warmup_ticks {
        cuda(
            unsafe { cuGraphLaunch(executable, stream) },
            "cuGraphLaunch",
        )?;
    }
    let timing_events = if config.telemetry == TelemetryMode::Benchmark {
        let mut events = Vec::with_capacity(config.ticks as usize);
        for _ in 0..config.ticks {
            let mut start = ptr::null_mut();
            let mut end = ptr::null_mut();
            cuda(
                unsafe { cuEventCreate(&mut start, 0) },
                "cuEventCreate(start)",
            )?;
            cuda(unsafe { cuEventCreate(&mut end, 0) }, "cuEventCreate(end)")?;
            events.push((start, end));
        }
        for (start, end) in &events {
            cuda(
                unsafe { cuEventRecord(*start, stream) },
                "cuEventRecord(start)",
            )?;
            cuda(
                unsafe { cuGraphLaunch(executable, stream) },
                "cuGraphLaunch",
            )?;
            cuda(unsafe { cuEventRecord(*end, stream) }, "cuEventRecord(end)")?;
        }
        Some(events)
    } else {
        for _ in 0..config.ticks {
            cuda(
                unsafe { cuGraphLaunch(executable, stream) },
                "cuGraphLaunch",
            )?;
        }
        None
    };
    unsafe { synchronize_with_deadline(stream, config.shutdown_timeout, "headless completion") }?;

    let timing = if let Some(events) = timing_events {
        let mut samples = Vec::with_capacity(events.len());
        for (start, end) in events {
            let mut milliseconds = 0.0;
            cuda(
                unsafe { cuEventElapsedTime(&mut milliseconds, start, end) },
                "cuEventElapsedTime",
            )?;
            samples.push(milliseconds);
            cuda(unsafe { cuEventDestroy_v2(start) }, "cuEventDestroy(start)")?;
            cuda(unsafe { cuEventDestroy_v2(end) }, "cuEventDestroy(end)")?;
        }
        Some(summarize_timings(samples))
    } else {
        None
    };

    let inspection = if let Some(name) = &config.inspect_stream {
        let (index, resource) = graph
            .resources
            .streams
            .iter()
            .enumerate()
            .find(|(_, stream)| &stream.name == name)
            .ok_or_else(|| format!("inspection stream `{name}` does not exist"))?;
        let bytes = resource.capacity as usize * element_size(&resource.element_type)?;
        let mut snapshot = vec![0_u8; bytes];
        cuda(
            unsafe { cuMemcpyDtoH_v2(snapshot.as_mut_ptr().cast(), stream_ptrs[index], bytes) },
            "requested post-run inspection",
        )?;
        let hash = snapshot.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
        let first_f32 = (resource.element_type == DataType::f32() && snapshot.len() >= 4)
            .then(|| f32::from_le_bytes(snapshot[0..4].try_into().unwrap()));
        Some(CudaInspection {
            stream: name.clone(),
            bytes,
            fnv1a64: format!("{hash:016x}"),
            first_f32,
        })
    } else {
        None
    };

    cuda(
        unsafe { cuGraphExecDestroy(executable) },
        "cuGraphExecDestroy",
    )?;
    cuda(unsafe { cuGraphDestroy(graph_handle) }, "cuGraphDestroy")?;
    for pointer in value_ptrs.into_iter().chain(stream_ptrs) {
        cuda(unsafe { cuMemFreeAsync(pointer, stream) }, "cuMemFreeAsync")?;
    }
    unsafe { synchronize_with_deadline(stream, config.shutdown_timeout, "memory retirement") }?;
    cuda(unsafe { cuModuleUnload(module) }, "cuModuleUnload")?;
    cuda(unsafe { cuStreamDestroy_v2(stream) }, "cuStreamDestroy")?;

    let semantic_fingerprint = pqo_core::canonicalize(graph).fingerprint;
    let backend_fingerprint = fingerprint(&(
        validated.target_profile(),
        "cuda-graph",
        "sm_120-cubin",
        config.profile,
        256_u32,
    ));
    let hardware_fingerprint = fingerprint(&(uuid.bytes, major, minor, device_name.as_str()));
    Ok(CudaRunReport {
        status: "completed".to_owned(),
        device_name,
        device_uuid: uuid_string(&uuid.bytes),
        compute_capability: format!("{major}.{minor}"),
        artifact_path,
        logical_ticks: config.ticks,
        free_vram_bytes_at_launch: free as u64,
        total_vram_bytes: total as u64,
        pqo_budget_bytes: budget,
        graph_launches: config.warmup_ticks + config.ticks,
        inspection,
        telemetry: config.telemetry,
        semantic_fingerprint,
        artifact_fingerprint: validated.artifact_fingerprint().to_owned(),
        backend_fingerprint,
        hardware_fingerprint,
        tuning: TuningRecord {
            candidates: vec![TuningCandidate {
                block_size: 256,
                selected: true,
                rejection_reason: None,
            }],
            selected: 0,
            warmup_launches: config.warmup_ticks,
            measured_launches: config.ticks,
            metric: "not tuned; initial launch configuration".to_owned(),
        },
        timing,
    })
}

fn summarize_timings(mut samples: Vec<f32>) -> CudaTimingSummary {
    samples.sort_by(|left, right| left.total_cmp(right));
    let percentile = |percent: f32| {
        let index = ((samples.len().saturating_sub(1)) as f32 * percent).ceil() as usize;
        samples.get(index).copied().unwrap_or(0.0)
    };
    CudaTimingSummary {
        samples: samples.len(),
        p50_ms: percentile(0.50),
        p95_ms: percentile(0.95),
        p99_ms: percentile(0.99),
        maximum_ms: samples.last().copied().unwrap_or(0.0),
        ticks_over_8_333_ms: samples.iter().filter(|sample| **sample > 8.333).count(),
    }
}

fn fingerprint(value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).expect("fingerprint serialization");
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

struct DispatchGeometry {
    launch_count: u32,
    dynamic_count_pointer: CuDevicePtr,
    maximum_count: u32,
}

fn dispatch_geometry(
    graph: &pqo_core::ModuleGraph,
    stream_ptrs: &[CuDevicePtr],
    dispatch: &DispatchDomain,
) -> Result<DispatchGeometry, String> {
    match dispatch {
        DispatchDomain::Fixed(count) => Ok(DispatchGeometry {
            launch_count: *count,
            dynamic_count_pointer: 0,
            maximum_count: *count,
        }),
        DispatchDomain::OverStream(id) => {
            let stream = graph.stream(*id).unwrap();
            match stream.length {
                StreamLength::Fixed(count) => Ok(DispatchGeometry {
                    launch_count: count,
                    dynamic_count_pointer: 0,
                    maximum_count: count,
                }),
                StreamLength::Dynamic(count) => Ok(DispatchGeometry {
                    launch_count: stream.capacity,
                    dynamic_count_pointer: stream_ptrs[count.0 as usize],
                    maximum_count: stream.capacity,
                }),
            }
        }
    }
}

fn element_size(data_type: &DataType) -> Result<usize, String> {
    pqo_core::layout_of(data_type, pqo_core::AbiLayoutClass::PackedStream)
        .map(|layout| layout.size as usize)
}

fn encode_value(
    value: &pqo_core::ValueNode,
    graph: &pqo_core::ModuleGraph,
) -> Result<Vec<u8>, String> {
    match &value.kind {
        ValueKind::Constant(literal) => encode_literal(&value.data_type, literal),
        ValueKind::ScheduleFixedDt { schedule } => {
            let schedule = graph
                .schedule(*schedule)
                .ok_or_else(|| "missing schedule".to_owned())?;
            let pqo_core::ScheduleTiming::Fixed { rate_hz, .. } = schedule.timing;
            Ok((1.0_f32 / rate_hz as f32).to_le_bytes().to_vec())
        }
    }
}

fn encode_initializer(
    initial: &StreamInitializer,
    data_type: &DataType,
    capacity: u32,
) -> Result<Vec<u8>, String> {
    match initial {
        StreamInitializer::Explicit(Literal::Array(values)) => {
            let mut bytes = Vec::new();
            for value in values {
                bytes.extend(encode_literal(data_type, value)?);
            }
            if values.len() > capacity as usize {
                return Err("initializer exceeds stream capacity".to_owned());
            }
            Ok(bytes)
        }
        _ => Err("CUDA v0 supports explicit stream initializers only".to_owned()),
    }
}

fn encode_literal(data_type: &DataType, literal: &Literal) -> Result<Vec<u8>, String> {
    match (data_type, literal) {
        (DataType::Scalar(ScalarType::F32), Literal::F32Bits(value)) => {
            Ok(value.to_le_bytes().to_vec())
        }
        (DataType::Scalar(ScalarType::U32), Literal::U32(value)) => {
            Ok(value.to_le_bytes().to_vec())
        }
        (DataType::Scalar(ScalarType::I32), Literal::I32(value)) => {
            Ok(value.to_le_bytes().to_vec())
        }
        (DataType::Scalar(ScalarType::Bool), Literal::Bool(value)) => Ok(vec![u8::from(*value)]),
        (
            DataType::Vector {
                scalar: ScalarType::F32,
                lanes,
            },
            Literal::Vector(values),
        ) if values.len() == *lanes as usize => {
            let mut bytes = Vec::new();
            for value in values {
                let Literal::F32Bits(value) = value else {
                    return Err("invalid f32 vector literal".to_owned());
                };
                bytes.extend(value.to_le_bytes());
            }
            Ok(bytes)
        }
        _ => Err(format!(
            "unsupported CUDA literal {literal:?} for {data_type:?}"
        )),
    }
}

fn cuda(result: CuResult, operation: &str) -> Result<(), String> {
    if result == CUDA_SUCCESS {
        return Ok(());
    }
    let mut name = ptr::null();
    let mut description = ptr::null();
    // SAFETY: CUDA owns the returned static strings.
    unsafe {
        let _ = cuGetErrorName(result, &mut name);
        let _ = cuGetErrorString(result, &mut description);
    }
    let name = if name.is_null() {
        "CUDA_ERROR".into()
    } else {
        unsafe { CStr::from_ptr(name) }.to_string_lossy()
    };
    let description = if description.is_null() {
        "unknown CUDA error".into()
    } else {
        unsafe { CStr::from_ptr(description) }.to_string_lossy()
    };
    Err(format!(
        "{operation} failed: {name} ({result}): {description}"
    ))
}

unsafe fn synchronize_with_deadline(
    stream: CuStream,
    timeout: Duration,
    operation: &str,
) -> Result<(), String> {
    const CUDA_ERROR_NOT_READY: CuResult = 600;
    let mut event = ptr::null_mut();
    cuda(unsafe { cuEventCreate(&mut event, 0) }, "cuEventCreate")?;
    cuda(unsafe { cuEventRecord(event, stream) }, "cuEventRecord")?;
    let deadline = Instant::now() + timeout;
    loop {
        match unsafe { cuEventQuery(event) } {
            CUDA_SUCCESS => break,
            CUDA_ERROR_NOT_READY if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(1));
            }
            CUDA_ERROR_NOT_READY => {
                let _ = unsafe { cuEventDestroy_v2(event) };
                return Err(format!(
                    "{operation} exceeded shutdown deadline of {} ms",
                    timeout.as_millis()
                ));
            }
            error => {
                let _ = unsafe { cuEventDestroy_v2(event) };
                return cuda(error, operation);
            }
        }
    }
    cuda(unsafe { cuEventDestroy_v2(event) }, "cuEventDestroy")
}

fn uuid_string(bytes: &[u8; 16]) -> String {
    let mut output = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            output.push('-');
        }
        output.push_str(&format!("{byte:02x}"));
    }
    output
}
