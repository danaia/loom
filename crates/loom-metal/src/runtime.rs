use std::{
    collections::BTreeMap,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

use block::ConcreteBlock;
use core_graphics_types::geometry::CGSize;
use loom_core::{
    DataType, DispatchDomain, Literal, PassId, ResourceId, ScalarType, ScheduleItemId, StreamId,
    StreamInitializer, StreamLength, ValueId, ValueKind, ViewId,
};
use loom_validator::{
    CompletionEnforcement, CompletionRequirement, ExecutionSchedule, PlannedPass, PlannedView,
    ValidatedModuleGraph,
};
use metal::{
    Buffer, CommandQueue, CompileOptions, ComputePipelineState, Device, MTLClearColor,
    MTLCommandBufferStatus, MTLLoadAction, MTLPixelFormat, MTLPrimitiveType, MTLResourceOptions,
    MTLSize, MTLStorageMode, MTLStoreAction, MTLTextureUsage, MetalLayer, RenderPassDescriptor,
    RenderPipelineDescriptor, RenderPipelineState, TextureDescriptor,
};
use objc::{msg_send, rc::autoreleasepool, runtime::YES, sel, sel_impl};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::macos::WindowExtMacOS,
    window::WindowBuilder,
};

use crate::{
    BenchmarkConfig, BenchmarkMode, BenchmarkResult, BenchmarkRunner, PacingResult,
    PipelineIdentity, ResourceMetrics, RuntimeDiagnostic, RuntimeDiagnosticCode,
    RuntimeFingerprint, ShaderIdentity, ViewportSize, sha256, summarize,
};

const INTEGRATE_SOURCE: &str = include_str!("../../../kernels/euler_integrate.metal");
const CONTACT_SOURCE: &str = include_str!("../../../kernels/ground_contact.metal");
const PARTICLE_SOURCE: &str = include_str!("../../../shaders/particle.metal");

pub struct MetalRuntime;

impl MetalRuntime {
    pub fn run(validated: ValidatedModuleGraph) -> Result<(), RuntimeDiagnostic> {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_inner_size(LogicalSize::new(960.0, 720.0))
            .with_title(format!(
                "Loom — {}",
                display_name(validated.graph().name.as_str())
            ))
            .build(&event_loop)
            .map_err(|error| {
                RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::WindowCreationFailed,
                    error.to_string(),
                )
            })?;
        let device = Device::system_default().ok_or_else(|| {
            RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::DeviceUnavailable,
                "Metal has no system-default device",
            )
        })?;
        let layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_presents_with_transaction(false);
        layer.set_maximum_drawable_count(2);
        attach_layer(&window, &layer);
        resize_layer(&window, &layer);

        let mut state = RuntimeState::new(validated, device, layer)?;
        println!(
            "runtime_fingerprint:\n{}",
            serde_json::to_string_pretty(&state.fingerprint)
                .expect("runtime fingerprint serialization")
        );

        let tick_interval = Duration::from_nanos(1_000_000_000 / state.rate_hz as u64);
        let mut next_tick = Instant::now();
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::WaitUntil(next_tick);
            autoreleasepool(|| match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        resize_layer(&window, &state.layer);
                    }
                    _ => {}
                },
                Event::MainEventsCleared => {
                    if Instant::now() >= next_tick {
                        window.request_redraw();
                    }
                }
                Event::RedrawRequested(_) => {
                    if let Err(diagnostic) = state.draw_tick() {
                        eprintln!(
                            "{}",
                            serde_json::to_string_pretty(&diagnostic)
                                .unwrap_or_else(|_| diagnostic.to_string())
                        );
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    next_tick += tick_interval;
                    let now = Instant::now();
                    if now.saturating_duration_since(next_tick) > tick_interval * 4 {
                        next_tick = now + tick_interval;
                    }
                }
                _ => {}
            });
        });
    }

    pub fn benchmark(
        validated: ValidatedModuleGraph,
        config: BenchmarkConfig,
    ) -> Result<BenchmarkResult, RuntimeDiagnostic> {
        if config.sample_ticks == 0 && config.sample_seconds.is_none() {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::UnsupportedGraph,
                "benchmark sample count must be positive",
            ));
        }
        if config.pacing_hz == Some(0) {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::UnsupportedGraph,
                "benchmark pacing rate must be positive",
            ));
        }
        if let Some(rate_hz) = config.pacing_hz
            && u64::from(config.pacing_lead_microseconds) * u64::from(rate_hz) >= 1_000_000
        {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::UnsupportedGraph,
                "benchmark pacing lead must be shorter than one tick",
            ));
        }
        let device = Device::system_default().ok_or_else(|| {
            RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::DeviceUnavailable,
                "Metal has no system-default device",
            )
        })?;
        if config.mode == BenchmarkMode::Presented {
            let event_loop = EventLoop::new();
            let window = WindowBuilder::new()
                .with_inner_size(LogicalSize::new(
                    config.viewport_width as f64,
                    config.viewport_height as f64,
                ))
                .with_title("Loom — Hello Batch Presented Benchmark")
                .build(&event_loop)
                .map_err(|error| {
                    RuntimeDiagnostic::new(
                        RuntimeDiagnosticCode::WindowCreationFailed,
                        error.to_string(),
                    )
                })?;
            let layer = MetalLayer::new();
            layer.set_device(&device);
            layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            layer.set_presents_with_transaction(false);
            layer.set_maximum_drawable_count(3);
            attach_layer(&window, &layer);
            resize_layer(&window, &layer);
            let mut state = RuntimeState::new(validated, device.clone(), layer)?;
            return autoreleasepool(|| state.run_benchmark(&device, config));
        }
        let layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        let mut state = RuntimeState::new(validated, device.clone(), layer)?;
        state.run_benchmark(&device, config)
    }
}

struct DirectMetalEncoding {
    fall: PassId,
    bounce: PassId,
    viewport: ViewId,
    position: StreamId,
    velocity: StreamId,
    radius: StreamId,
    restitution: StreamId,
    friction: StreamId,
    color: StreamId,
    gravity: ValueId,
    fixed_dt: ValueId,
    ground_height: ValueId,
}

impl DirectMetalEncoding {
    fn resolve(graph: &loom_core::ModuleGraph) -> Option<Self> {
        let pass = |name: &str| {
            graph
                .passes
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.id)
        };
        let view = |name: &str| {
            graph
                .views
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.id)
        };
        let stream = |name: &str| {
            graph
                .resources
                .streams
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.id)
        };
        let value = |name: &str| {
            graph
                .resources
                .values
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.id)
        };
        Some(Self {
            fall: pass("fall")?,
            bounce: pass("bounce")?,
            viewport: view("viewport")?,
            position: stream("particles.position")?,
            velocity: stream("particles.velocity")?,
            radius: stream("particles.radius")?,
            restitution: stream("particles.restitution")?,
            friction: stream("particles.friction")?,
            color: stream("particles.color")?,
            gravity: value("world.gravity")?,
            fixed_dt: value("simulation.fixed_dt")?,
            ground_height: value("ground.height")?,
        })
    }
}

struct RuntimeState {
    validated: ValidatedModuleGraph,
    queue: CommandQueue,
    layer: MetalLayer,
    stream_buffers: Vec<Vec<Buffer>>,
    value_buffers: Vec<Buffer>,
    compute_pipelines: BTreeMap<PassId, ComputePipelineState>,
    render_pipelines: BTreeMap<ViewId, RenderPipelineState>,
    rate_hz: u32,
    fingerprint: RuntimeFingerprint,
    max_in_flight_command_buffers: u32,
    direct_metal: Option<DirectMetalEncoding>,
}

impl RuntimeState {
    fn new(
        validated: ValidatedModuleGraph,
        device: Device,
        layer: MetalLayer,
    ) -> Result<Self, RuntimeDiagnostic> {
        if validated.execution_plan().schedules.len() != 1 {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::UnsupportedGraph,
                "the first Metal slice requires exactly one schedule",
            ));
        }
        let schedule = &validated.execution_plan().schedules[0];
        if schedule
            .resource_versions
            .iter()
            .any(|allocation| allocation.required_versions > 1)
        {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::UnsupportedGraph,
                "the first Metal slice supports one physical version per stream",
            ));
        }

        let max_in_flight_command_buffers = schedule
            .requested_ticks
            .max(schedule.requested_render_frames)
            .max(1);
        let queue = device
            .new_command_queue_with_max_command_buffer_count(max_in_flight_command_buffers as u64);
        let stream_buffers = allocate_streams(&validated, &device, &queue)?;
        let value_buffers = allocate_values(&validated, &device)?;
        let (compute_pipelines, render_pipelines, shader_identities, pipeline_identities) =
            build_pipelines(&validated, &device)?;
        let rate_hz = match validated.graph().schedules[0].timing {
            loom_core::ScheduleTiming::Fixed { rate_hz, .. } => rate_hz,
        };
        let fingerprint =
            make_fingerprint(&validated, &device, shader_identities, pipeline_identities);
        let direct_metal = DirectMetalEncoding::resolve(validated.graph());

        Ok(Self {
            validated,
            queue,
            layer,
            stream_buffers,
            value_buffers,
            compute_pipelines,
            render_pipelines,
            rate_hz,
            fingerprint,
            max_in_flight_command_buffers,
            direct_metal,
        })
    }

    fn draw_tick(&mut self) -> Result<(), RuntimeDiagnostic> {
        let schedule = &self.validated.execution_plan().schedules[0];
        let command_buffer = self.queue.new_command_buffer();
        command_buffer.set_label("loom.tick");
        let drawable = self.layer.next_drawable();
        let mut wait_before_next_tick = false;

        for item in &schedule.order {
            match item {
                ScheduleItemId::Pass(pass_id) => {
                    let pass = schedule
                        .passes
                        .iter()
                        .find(|pass| pass.pass == *pass_id)
                        .expect("validated plan pass");
                    self.encode_compute(command_buffer, pass)?;
                    wait_before_next_tick |= requires_wait_after(schedule, *item);
                }
                ScheduleItemId::View(view_id) => {
                    if let Some(drawable) = drawable {
                        let view = schedule
                            .views
                            .iter()
                            .find(|view| view.view == *view_id)
                            .expect("validated plan view");
                        self.encode_render(command_buffer, view, drawable.texture())?;
                        command_buffer.present_drawable(drawable);
                        wait_before_next_tick |= requires_wait_after(schedule, *item);
                    }
                }
            }
        }

        command_buffer.commit();
        if wait_before_next_tick {
            command_buffer.wait_until_completed();
            if command_buffer.status() == MTLCommandBufferStatus::Error {
                return Err(RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::CommandBufferFailed,
                    "Metal reported a command-buffer execution error",
                )
                .at("schedules.simulation"));
            }
        }
        Ok(())
    }

    fn run_benchmark(
        &mut self,
        device: &Device,
        config: BenchmarkConfig,
    ) -> Result<BenchmarkResult, RuntimeDiagnostic> {
        if config.pacing_hz.is_some() {
            let status = unsafe { pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0) };
            if status != 0 {
                return Err(RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::UnsupportedGraph,
                    format!("failed to request interactive pacing QoS: errno {status}"),
                ));
            }
        }
        let presented = config.mode == BenchmarkMode::Presented;
        let render_target = (config.mode == BenchmarkMode::Rendered).then(|| {
            let descriptor = TextureDescriptor::new();
            descriptor.set_width(config.viewport_width as u64);
            descriptor.set_height(config.viewport_height as u64);
            descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            descriptor.set_storage_mode(MTLStorageMode::Private);
            descriptor.set_usage(MTLTextureUsage::RenderTarget);
            device.new_texture(&descriptor)
        });

        let (warmup_receiver, warmup_ticks) = self.submit_benchmark_phase(
            render_target.as_deref(),
            config.warmup_ticks,
            config.warmup_seconds,
            &config,
            presented,
        )?;
        self.drain_benchmark_ticks(warmup_receiver, warmup_ticks)?;

        let sample_start = Instant::now();
        let (sample_receiver, sample_ticks) = self.submit_benchmark_phase(
            render_target.as_deref(),
            config.sample_ticks,
            config.sample_seconds,
            &config,
            presented,
        )?;
        let timings = self.drain_benchmark_ticks(sample_receiver, sample_ticks)?;
        let sample_wall_time_seconds = sample_start.elapsed().as_secs_f64();
        let gpu_samples = timings
            .iter()
            .map(|timing| timing.gpu_ms)
            .collect::<Vec<_>>();
        let cpu_samples = timings
            .iter()
            .map(|timing| timing.cpu_orchestration_ms)
            .collect::<Vec<_>>();
        let latency_samples = timings
            .iter()
            .map(|timing| timing.end_to_end_tick_ms)
            .collect::<Vec<_>>();

        let gpu_ms = summarize(&gpu_samples);
        let cpu_orchestration_ms = summarize(&cpu_samples);
        let end_to_end_tick_ms = summarize(&latency_samples);
        let pacing = config.pacing_hz.map(|target_hz| {
            let deadline_misses = timings
                .iter()
                .filter(|timing| timing.deadline_lateness_ms > 0.0)
                .count() as u32;
            PacingResult {
                target_hz,
                tick_budget_ms: 1_000.0 / target_hz as f64,
                submission_lead_ms: config.pacing_lead_microseconds as f64 / 1_000.0,
                deadline_misses,
                deadline_miss_rate: deadline_misses as f64 / sample_ticks as f64,
                maximum_lateness_ms: timings
                    .iter()
                    .map(|timing| timing.deadline_lateness_ms)
                    .fold(0.0, f64::max),
            }
        });
        let particle_count = benchmark_dispatch_count(
            self.validated.graph(),
            &self.validated.execution_plan().schedules[0],
        )?;
        Ok(BenchmarkResult {
            experiment: self.validated.graph().name.clone(),
            particle_count,
            mode: config.mode,
            runner: config.runner,
            viewport: (config.mode != BenchmarkMode::Headless).then_some(ViewportSize {
                width: config.viewport_width,
                height: config.viewport_height,
            }),
            warmup_ticks,
            sample_ticks,
            requested_warmup_seconds: config.warmup_seconds,
            requested_sample_seconds: config.sample_seconds,
            gpu_p95_below_8_33_ms: gpu_ms.p95 < 8.33,
            gpu_ms,
            cpu_orchestration_ms,
            end_to_end_tick_ms,
            sample_wall_time_seconds,
            submitted_ticks_per_second: sample_ticks as f64 / sample_wall_time_seconds,
            synchronized_each_tick: false,
            max_in_flight_command_buffers: self.max_in_flight_command_buffers,
            pacing,
            resources: self.resource_metrics(),
            runtime: self.fingerprint.clone(),
        })
    }

    fn submit_benchmark_phase(
        &self,
        render_target: Option<&metal::TextureRef>,
        ticks: u32,
        seconds: Option<u32>,
        config: &BenchmarkConfig,
        presented: bool,
    ) -> Result<(Receiver<RawTickTiming>, u32), RuntimeDiagnostic> {
        let (sender, receiver) = mpsc::channel();
        let mut submitted = 0_u32;
        let phase_start = Instant::now();
        loop {
            if let Some(seconds) = seconds {
                let duration = Duration::from_secs(seconds as u64);
                let phase_complete = config.pacing_hz.map_or_else(
                    || submitted > 0 && phase_start.elapsed() >= duration,
                    |hz| pacing_offset(submitted, hz) >= duration,
                );
                if phase_complete {
                    break;
                }
            } else if submitted >= ticks {
                break;
            }
            let deadline = config.pacing_hz.map(|hz| {
                let nominal_admission = phase_start + pacing_offset(submitted, hz);
                let lead = Duration::from_micros(u64::from(config.pacing_lead_microseconds));
                let admission = nominal_admission.checked_sub(lead).unwrap_or(phase_start);
                wait_until(admission);
                phase_start + pacing_offset(submitted + 1, hz)
            });
            self.submit_benchmark_tick(
                render_target,
                sender.clone(),
                config.runner,
                deadline,
                presented,
            )?;
            submitted = submitted.checked_add(1).ok_or_else(|| {
                RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::UnsupportedGraph,
                    "benchmark phase submitted more ticks than can be reported",
                )
            })?;
        }
        drop(sender);
        Ok((receiver, submitted))
    }

    fn submit_benchmark_tick(
        &self,
        render_target: Option<&metal::TextureRef>,
        sender: Sender<RawTickTiming>,
        runner: BenchmarkRunner,
        deadline: Option<Instant>,
        presented: bool,
    ) -> Result<(), RuntimeDiagnostic> {
        let schedule = &self.validated.execution_plan().schedules[0];
        let cpu_start = Instant::now();
        let command_buffer = self.queue.new_command_buffer();
        command_buffer.set_label("loom.benchmark.tick");
        let drawable = if presented {
            Some(self.layer.next_drawable().ok_or_else(|| {
                RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::CommandBufferFailed,
                    "CAMetalLayer did not provide a drawable",
                )
            })?)
        } else {
            None
        };
        let texture = drawable
            .as_ref()
            .map(|drawable| drawable.texture())
            .or(render_target);

        match runner {
            BenchmarkRunner::LoomPlan => {
                for item in &schedule.order {
                    match item {
                        ScheduleItemId::Pass(pass_id) => {
                            let pass = schedule
                                .passes
                                .iter()
                                .find(|pass| pass.pass == *pass_id)
                                .expect("validated plan pass");
                            self.encode_compute(command_buffer, pass)?;
                        }
                        ScheduleItemId::View(view_id) => {
                            if let Some(texture) = texture {
                                let view = schedule
                                    .views
                                    .iter()
                                    .find(|view| view.view == *view_id)
                                    .expect("validated plan view");
                                self.encode_render(command_buffer, view, texture)?;
                            }
                        }
                    }
                }
            }
            BenchmarkRunner::DirectMetalEncoding => {
                self.encode_direct_metal(command_buffer, texture)?;
            }
        }
        if let Some(drawable) = &drawable {
            command_buffer.present_drawable(drawable);
        }

        let cpu_orchestration_bits = Arc::new(AtomicU64::new(u64::MAX));
        let callback_cpu_bits = Arc::clone(&cpu_orchestration_bits);
        let callback_start = cpu_start;
        let handler = ConcreteBlock::new(move |completed: &metal::CommandBufferRef| {
            let gpu_start: f64 = unsafe { msg_send![completed, GPUStartTime] };
            let gpu_end: f64 = unsafe { msg_send![completed, GPUEndTime] };
            let mut cpu_bits = callback_cpu_bits.load(Ordering::Acquire);
            while cpu_bits == u64::MAX {
                std::hint::spin_loop();
                cpu_bits = callback_cpu_bits.load(Ordering::Acquire);
            }
            let _ = sender.send(RawTickTiming {
                status: completed.status(),
                gpu_start,
                gpu_end,
                cpu_orchestration_ms: f64::from_bits(cpu_bits),
                end_to_end_tick_ms: callback_start.elapsed().as_secs_f64() * 1_000.0,
                deadline_lateness_ms: deadline
                    .and_then(|deadline| Instant::now().checked_duration_since(deadline))
                    .map_or(0.0, |lateness| lateness.as_secs_f64() * 1_000.0),
            });
        })
        .copy();
        command_buffer.add_completed_handler(&handler);
        command_buffer.commit();
        let cpu_orchestration_ms = cpu_start.elapsed().as_secs_f64() * 1_000.0;
        cpu_orchestration_bits.store(cpu_orchestration_ms.to_bits(), Ordering::Release);
        Ok(())
    }

    fn drain_benchmark_ticks(
        &self,
        receiver: Receiver<RawTickTiming>,
        count: u32,
    ) -> Result<Vec<TickTiming>, RuntimeDiagnostic> {
        (0..count)
            .map(|_| {
                receiver
                    .recv()
                    .map_err(|_| {
                        RuntimeDiagnostic::new(
                            RuntimeDiagnosticCode::CommandBufferFailed,
                            "Metal stopped asynchronous benchmark timestamp delivery",
                        )
                    })?
                    .finish()
            })
            .collect()
    }

    fn encode_compute(
        &self,
        command_buffer: &metal::CommandBufferRef,
        pass: &PlannedPass,
    ) -> Result<(), RuntimeDiagnostic> {
        let pipeline = &self.compute_pipelines[&pass.pass];
        let encoder = command_buffer.new_compute_command_encoder();
        encoder.set_compute_pipeline_state(pipeline);
        for (index, slot) in pass.abi.binding_order.iter().enumerate() {
            let binding = pass
                .bindings
                .iter()
                .find(|binding| binding.slot == *slot)
                .expect("validated binding");
            match binding.resource {
                ResourceId::Stream(stream) => {
                    encoder.set_buffer(
                        index as u64,
                        Some(&self.stream_buffers[stream.0 as usize][0]),
                        0,
                    );
                }
                ResourceId::Value(value) => {
                    encoder.set_buffer(
                        index as u64,
                        Some(&self.value_buffers[value.0 as usize]),
                        0,
                    );
                }
            }
        }
        let count = dispatch_count(self.validated.graph(), &pass.dispatch)?;
        let width = pipeline
            .thread_execution_width()
            .min(pipeline.max_total_threads_per_threadgroup())
            .max(1);
        encoder.dispatch_threads(MTLSize::new(count, 1, 1), MTLSize::new(width, 1, 1));
        encoder.end_encoding();
        Ok(())
    }

    fn encode_render(
        &self,
        command_buffer: &metal::CommandBufferRef,
        view: &PlannedView,
        texture: &metal::TextureRef,
    ) -> Result<(), RuntimeDiagnostic> {
        let descriptor = RenderPassDescriptor::new();
        let attachment = descriptor.color_attachments().object_at(0).unwrap();
        attachment.set_texture(Some(texture));
        attachment.set_load_action(MTLLoadAction::Clear);
        attachment.set_clear_color(MTLClearColor::new(0.025, 0.03, 0.055, 1.0));
        attachment.set_store_action(MTLStoreAction::Store);

        let encoder = command_buffer.new_render_command_encoder(descriptor);
        encoder.set_render_pipeline_state(&self.render_pipelines[&view.view]);
        for (index, read) in view.reads.iter().enumerate() {
            encoder.set_vertex_buffer(
                index as u64,
                Some(&self.stream_buffers[read.stream.0 as usize][0]),
                0,
            );
        }
        let instances = view_instance_count(self.validated.graph(), view)?;
        encoder.draw_primitives_instanced(MTLPrimitiveType::Triangle, 0, 6, instances);
        encoder.end_encoding();
        Ok(())
    }

    fn encode_direct_metal(
        &self,
        command_buffer: &metal::CommandBufferRef,
        render_target: Option<&metal::TextureRef>,
    ) -> Result<(), RuntimeDiagnostic> {
        let direct = self.direct_metal.as_ref().ok_or_else(|| {
            RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::UnsupportedGraph,
                "direct-Metal encoding is available only for the Hello Batch resource layout",
            )
        })?;
        let count = dispatch_count(
            self.validated.graph(),
            &DispatchDomain::OverStream(direct.position),
        )?;

        let fall_pipeline = &self.compute_pipelines[&direct.fall];
        let fall = command_buffer.new_compute_command_encoder();
        fall.set_compute_pipeline_state(fall_pipeline);
        fall.set_buffer(
            0,
            Some(&self.stream_buffers[direct.position.0 as usize][0]),
            0,
        );
        fall.set_buffer(
            1,
            Some(&self.stream_buffers[direct.velocity.0 as usize][0]),
            0,
        );
        fall.set_buffer(2, Some(&self.value_buffers[direct.gravity.0 as usize]), 0);
        fall.set_buffer(3, Some(&self.value_buffers[direct.fixed_dt.0 as usize]), 0);
        let width = fall_pipeline
            .thread_execution_width()
            .min(fall_pipeline.max_total_threads_per_threadgroup())
            .max(1);
        fall.dispatch_threads(MTLSize::new(count, 1, 1), MTLSize::new(width, 1, 1));
        fall.end_encoding();

        let bounce_pipeline = &self.compute_pipelines[&direct.bounce];
        let bounce = command_buffer.new_compute_command_encoder();
        bounce.set_compute_pipeline_state(bounce_pipeline);
        for (index, stream) in [
            direct.position,
            direct.velocity,
            direct.radius,
            direct.restitution,
            direct.friction,
        ]
        .iter()
        .enumerate()
        {
            bounce.set_buffer(
                index as u64,
                Some(&self.stream_buffers[stream.0 as usize][0]),
                0,
            );
        }
        bounce.set_buffer(
            5,
            Some(&self.value_buffers[direct.ground_height.0 as usize]),
            0,
        );
        let width = bounce_pipeline
            .thread_execution_width()
            .min(bounce_pipeline.max_total_threads_per_threadgroup())
            .max(1);
        bounce.dispatch_threads(MTLSize::new(count, 1, 1), MTLSize::new(width, 1, 1));
        bounce.end_encoding();

        if let Some(texture) = render_target {
            let descriptor = RenderPassDescriptor::new();
            let attachment = descriptor.color_attachments().object_at(0).unwrap();
            attachment.set_texture(Some(texture));
            attachment.set_load_action(MTLLoadAction::Clear);
            attachment.set_clear_color(MTLClearColor::new(0.025, 0.03, 0.055, 1.0));
            attachment.set_store_action(MTLStoreAction::Store);
            let render = command_buffer.new_render_command_encoder(descriptor);
            render.set_render_pipeline_state(&self.render_pipelines[&direct.viewport]);
            for (index, stream) in [direct.color, direct.position, direct.radius]
                .iter()
                .enumerate()
            {
                render.set_vertex_buffer(
                    index as u64,
                    Some(&self.stream_buffers[stream.0 as usize][0]),
                    0,
                );
            }
            render.draw_primitives_instanced(MTLPrimitiveType::Triangle, 0, 6, count);
            render.end_encoding();
        }
        Ok(())
    }

    fn resource_metrics(&self) -> ResourceMetrics {
        ResourceMetrics {
            gpu_stream_buffer_bytes: self
                .stream_buffers
                .iter()
                .flatten()
                .map(|buffer| buffer.length())
                .sum(),
            gpu_value_buffer_bytes: self
                .value_buffers
                .iter()
                .map(|buffer| buffer.length())
                .sum(),
            initialization_application_blits: self
                .stream_buffers
                .iter()
                .map(|versions| versions.len() as u64)
                .sum(),
            steady_state_application_copies_per_tick: 0,
            steady_state_application_blits_per_tick: 0,
            steady_state_heap_allocations_per_tick: None,
            peak_resident_set_bytes: peak_resident_set_bytes(),
        }
    }
}

struct TickTiming {
    gpu_ms: f64,
    cpu_orchestration_ms: f64,
    end_to_end_tick_ms: f64,
    deadline_lateness_ms: f64,
}

struct RawTickTiming {
    status: MTLCommandBufferStatus,
    gpu_start: f64,
    gpu_end: f64,
    cpu_orchestration_ms: f64,
    end_to_end_tick_ms: f64,
    deadline_lateness_ms: f64,
}

impl RawTickTiming {
    fn finish(self) -> Result<TickTiming, RuntimeDiagnostic> {
        if self.status == MTLCommandBufferStatus::Error {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::CommandBufferFailed,
                "Metal reported a benchmark command-buffer error",
            ));
        }
        if self.gpu_start <= 0.0 || self.gpu_end < self.gpu_start {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::CommandBufferFailed,
                "Metal did not provide valid command-buffer GPU timestamps",
            ));
        }
        Ok(TickTiming {
            gpu_ms: (self.gpu_end - self.gpu_start) * 1_000.0,
            cpu_orchestration_ms: self.cpu_orchestration_ms,
            end_to_end_tick_ms: self.end_to_end_tick_ms,
            deadline_lateness_ms: self.deadline_lateness_ms,
        })
    }
}

fn display_name(module_name: &str) -> String {
    module_name
        .split('_')
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().chain(characters).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn peak_resident_set_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    (status == 0).then(|| unsafe { usage.assume_init().ru_maxrss as u64 })
}

fn wait_until(target: Instant) {
    if let Some(remaining) = target.checked_duration_since(Instant::now()) {
        let (numerator, denominator) = mach_timebase();
        let ticks =
            remaining.as_nanos().saturating_mul(u128::from(denominator)) / u128::from(numerator);
        let deadline = unsafe { mach_absolute_time() }.saturating_add(ticks as u64);
        unsafe {
            mach_wait_until(deadline);
        }
    }
    while Instant::now() < target {
        std::hint::spin_loop();
    }
}

fn pacing_offset(tick: u32, rate_hz: u32) -> Duration {
    let nanoseconds = (u128::from(tick) * 1_000_000_000_u128) / u128::from(rate_hz);
    Duration::from_nanos(nanoseconds as u64)
}

fn mach_timebase() -> (u32, u32) {
    static TIMEBASE: std::sync::OnceLock<(u32, u32)> = std::sync::OnceLock::new();
    *TIMEBASE.get_or_init(|| {
        let mut info = std::mem::MaybeUninit::<MachTimebaseInfo>::zeroed();
        let status = unsafe { mach_timebase_info(info.as_mut_ptr()) };
        assert_eq!(status, 0, "mach_timebase_info failed");
        let info = unsafe { info.assume_init() };
        (info.numer, info.denom)
    })
}

#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;

unsafe extern "C" {
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> libc::c_int;
    fn mach_wait_until(deadline: u64) -> libc::c_int;
    fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: libc::c_int)
    -> libc::c_int;
}

fn view_instance_count(
    graph: &loom_core::ModuleGraph,
    view: &PlannedView,
) -> Result<u64, RuntimeDiagnostic> {
    let mut count = None;
    for read in &view.reads {
        let length = match graph.stream(read.stream).unwrap().length {
            StreamLength::Fixed(length) => length,
            StreamLength::Dynamic(_) => {
                return Err(RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::UnsupportedGraph,
                    "dynamic view lengths are not in the first Metal slice",
                ));
            }
        };
        if count.is_some_and(|expected| expected != length) {
            return Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::UnsupportedGraph,
                "view streams have incompatible logical lengths",
            )
            .at(format!("views.{}", view.view.0)));
        }
        count = Some(length);
    }
    Ok(count.unwrap_or(0) as u64)
}

fn requires_wait_after(schedule: &ExecutionSchedule, submitted: ScheduleItemId) -> bool {
    schedule.completion_requirements.iter().any(|requirement| {
        matches!(
            requirement,
            CompletionRequirement::BeforeNextTick {
                after,
                enforcement: CompletionEnforcement::HostWait,
                ..
            } if *after == submitted
        )
    })
}

fn attach_layer(window: &winit::window::Window, layer: &MetalLayer) {
    unsafe {
        let view = window.ns_view() as *mut objc::runtime::Object;
        let _: () = msg_send![view, setWantsLayer: YES];
        let layer_object =
            layer.as_ref() as *const metal::MetalLayerRef as *mut objc::runtime::Object;
        let _: () = msg_send![view, setLayer: layer_object];
    }
}

fn resize_layer(window: &winit::window::Window, layer: &MetalLayer) {
    let size = window.inner_size();
    layer.set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
}

fn allocate_streams(
    validated: &ValidatedModuleGraph,
    device: &Device,
    queue: &CommandQueue,
) -> Result<Vec<Vec<Buffer>>, RuntimeDiagnostic> {
    let command_buffer = queue.new_command_buffer();
    command_buffer.set_label("loom.initialize");
    let blit = command_buffer.new_blit_command_encoder();
    let mut staging = Vec::new();
    let mut result = Vec::new();

    for stream in &validated.graph().resources.streams {
        let length = element_size(&stream.element_type)?
            .checked_mul(stream.capacity as usize)
            .ok_or_else(|| {
                RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::ResourceAllocationFailed,
                    "stream allocation size overflow",
                )
                .at(format!("streams.{}", stream.name))
            })?;
        let mut initial = encode_stream_initial(stream)?;
        initial.resize(length, 0);
        let upload = device.new_buffer_with_data(
            initial.as_ptr().cast(),
            length as u64,
            MTLResourceOptions::StorageModeShared,
        );
        let mut versions = Vec::new();
        for _ in 0..stream.buffering {
            let buffer = device.new_buffer(length as u64, MTLResourceOptions::StorageModePrivate);
            blit.copy_from_buffer(&upload, 0, &buffer, 0, length as u64);
            versions.push(buffer);
        }
        staging.push(upload);
        result.push(versions);
    }
    blit.end_encoding();
    command_buffer.commit();
    command_buffer.wait_until_completed();
    if command_buffer.status() == MTLCommandBufferStatus::Error {
        return Err(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::CommandBufferFailed,
            "initial resource upload failed",
        ));
    }
    drop(staging);
    Ok(result)
}

fn allocate_values(
    validated: &ValidatedModuleGraph,
    device: &Device,
) -> Result<Vec<Buffer>, RuntimeDiagnostic> {
    let bytes = validated
        .graph()
        .resources
        .values
        .iter()
        .map(|value| match &value.kind {
            ValueKind::Constant(literal) => encode_literal(&value.data_type, literal),
            ValueKind::ScheduleFixedDt { schedule } => {
                let rate_hz = match validated.graph().schedule(*schedule).unwrap().timing {
                    loom_core::ScheduleTiming::Fixed { rate_hz, .. } => rate_hz,
                };
                Ok((1.0_f32 / rate_hz as f32).to_le_bytes().to_vec())
            }
            ValueKind::DynamicCounter => Ok(0_u32.to_le_bytes().to_vec()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(bytes
        .iter()
        .map(|bytes| {
            device.new_buffer_with_data(
                bytes.as_ptr().cast(),
                bytes.len() as u64,
                MTLResourceOptions::StorageModeShared,
            )
        })
        .collect())
}

fn encode_stream_initial(stream: &loom_core::StreamNode) -> Result<Vec<u8>, RuntimeDiagnostic> {
    match &stream.initial {
        Some(StreamInitializer::Explicit(Literal::Array(items))) => {
            let mut bytes = Vec::new();
            for item in items {
                bytes.extend(encode_literal(&stream.element_type, item)?);
            }
            Ok(bytes)
        }
        Some(StreamInitializer::Repeat { value, count }) => {
            let element = encode_literal(&stream.element_type, value)?;
            let mut bytes = Vec::with_capacity(element.len() * *count as usize);
            for _ in 0..*count {
                bytes.extend_from_slice(&element);
            }
            Ok(bytes)
        }
        Some(StreamInitializer::Linear { start, step, count }) => {
            encode_f32_initializer(&stream.element_type, start, step, None, *count, *count)
        }
        Some(StreamInitializer::Grid2D {
            origin,
            column_step,
            row_step,
            columns,
            count,
        }) => encode_f32_initializer(
            &stream.element_type,
            origin,
            column_step,
            Some(row_step),
            *columns,
            *count,
        ),
        Some(_) => Err(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsupportedGraph,
            "validated stream initializer cannot be lowered by Metal v0",
        )
        .at(format!("streams.{}.initial", stream.name))),
        None => Ok(Vec::new()),
    }
}

fn encode_f32_initializer(
    data_type: &DataType,
    origin: &Literal,
    column_step: &Literal,
    row_step: Option<&Literal>,
    columns: u32,
    count: u32,
) -> Result<Vec<u8>, RuntimeDiagnostic> {
    let origin = f32_components(data_type, origin)?;
    let column_step = f32_components(data_type, column_step)?;
    let row_step = row_step
        .map(|literal| f32_components(data_type, literal))
        .transpose()?;
    let mut bytes = Vec::with_capacity(origin.len() * count as usize * std::mem::size_of::<f32>());
    for index in 0..count {
        let column = if row_step.is_some() {
            index % columns
        } else {
            index
        } as f32;
        let row = if row_step.is_some() {
            index / columns
        } else {
            0
        } as f32;
        for lane in 0..origin.len() {
            let value = origin[lane]
                + column_step[lane] * column
                + row_step.as_ref().map_or(0.0, |step| step[lane] * row);
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
    }
    Ok(bytes)
}

fn f32_components(data_type: &DataType, literal: &Literal) -> Result<Vec<f32>, RuntimeDiagnostic> {
    match (data_type, literal) {
        (DataType::Scalar(ScalarType::F32), Literal::F32Bits(value)) => {
            Ok(vec![f32::from_bits(*value)])
        }
        (
            DataType::Vector {
                scalar: ScalarType::F32,
                lanes,
            },
            Literal::Vector(items),
        ) if items.len() == *lanes as usize => items
            .iter()
            .map(|item| match item {
                Literal::F32Bits(value) => Ok(f32::from_bits(*value)),
                _ => Err(RuntimeDiagnostic::new(
                    RuntimeDiagnosticCode::UnsupportedGraph,
                    "f32 initializer vector contains a non-f32 lane",
                )),
            })
            .collect(),
        _ => Err(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsupportedGraph,
            "linear and grid initializers require f32 scalar or vector data",
        )),
    }
}

fn encode_literal(data_type: &DataType, literal: &Literal) -> Result<Vec<u8>, RuntimeDiagnostic> {
    match (data_type, literal) {
        (DataType::Scalar(ScalarType::Bool), Literal::Bool(value)) => Ok(vec![u8::from(*value)]),
        (DataType::Scalar(ScalarType::I32), Literal::I32(value)) => {
            Ok(value.to_le_bytes().to_vec())
        }
        (DataType::Scalar(ScalarType::U32), Literal::U32(value)) => {
            Ok(value.to_le_bytes().to_vec())
        }
        (DataType::Scalar(ScalarType::F16), Literal::F16Bits(value)) => {
            Ok(value.to_le_bytes().to_vec())
        }
        (DataType::Scalar(ScalarType::F32), Literal::F32Bits(value)) => {
            Ok(value.to_le_bytes().to_vec())
        }
        (DataType::Vector { scalar, lanes }, Literal::Vector(items))
            if items.len() == *lanes as usize =>
        {
            let mut bytes = Vec::new();
            for item in items {
                bytes.extend(encode_literal(&DataType::Scalar(scalar.clone()), item)?);
            }
            Ok(bytes)
        }
        _ => Err(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsupportedGraph,
            format!("Metal v0 cannot encode literal {literal:?} as {data_type:?}"),
        )),
    }
}

fn element_size(data_type: &DataType) -> Result<usize, RuntimeDiagnostic> {
    match data_type {
        DataType::Scalar(ScalarType::Bool) => Ok(1),
        DataType::Scalar(ScalarType::F16) => Ok(2),
        DataType::Scalar(_) => Ok(4),
        DataType::Vector { scalar, lanes } => {
            element_size(&DataType::Scalar(scalar.clone())).map(|size| size * *lanes as usize)
        }
        _ => Err(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsupportedGraph,
            format!("Metal v0 has no storage layout for {data_type:?}"),
        )),
    }
}

type Pipelines = (
    BTreeMap<PassId, ComputePipelineState>,
    BTreeMap<ViewId, RenderPipelineState>,
    Vec<ShaderIdentity>,
    Vec<PipelineIdentity>,
);

fn build_pipelines(
    validated: &ValidatedModuleGraph,
    device: &Device,
) -> Result<Pipelines, RuntimeDiagnostic> {
    let schedule = &validated.execution_plan().schedules[0];
    let options = CompileOptions::new();
    let mut compute = BTreeMap::new();
    let mut render = BTreeMap::new();
    let mut shaders = BTreeMap::<String, String>::new();
    let mut pipeline_identities = Vec::new();

    for pass in &schedule.passes {
        let source = shader_source(&pass.implementation.source)?;
        let library = device
            .new_library_with_source(source, &options)
            .map_err(|error| {
                RuntimeDiagnostic::new(RuntimeDiagnosticCode::ShaderCompilationFailed, error)
                    .at(format!("passes.{}", pass.pass.0))
            })?;
        let function = library
            .get_function(&pass.implementation.entry, None)
            .map_err(|error| {
                RuntimeDiagnostic::new(RuntimeDiagnosticCode::PipelineCreationFailed, error)
                    .at(format!("passes.{}", pass.pass.0))
            })?;
        let pipeline = device
            .new_compute_pipeline_state_with_function(&function)
            .map_err(|error| {
                RuntimeDiagnostic::new(RuntimeDiagnosticCode::PipelineCreationFailed, error)
                    .at(format!("passes.{}", pass.pass.0))
            })?;
        pipeline_identities.push(PipelineIdentity {
            entry: pass.implementation.entry.clone(),
            kind: "compute".to_owned(),
            source_sha256: sha256(source.as_bytes()),
            thread_execution_width: Some(pipeline.thread_execution_width()),
            max_threads_per_threadgroup: Some(pipeline.max_total_threads_per_threadgroup()),
        });
        shaders.insert(
            pass.implementation.source.clone(),
            sha256(source.as_bytes()),
        );
        compute.insert(pass.pass, pipeline);
    }

    for view in &schedule.views {
        let source = shader_source(&view.implementation.source)?;
        let library = device
            .new_library_with_source(source, &options)
            .map_err(|error| {
                RuntimeDiagnostic::new(RuntimeDiagnosticCode::ShaderCompilationFailed, error)
                    .at(format!("views.{}", view.view.0))
            })?;
        let vertex_name = format!("{}_vertex", view.implementation.entry);
        let fragment_name = format!("{}_fragment", view.implementation.entry);
        let vertex = library.get_function(&vertex_name, None).map_err(|error| {
            RuntimeDiagnostic::new(RuntimeDiagnosticCode::PipelineCreationFailed, error)
                .at(format!("views.{}", view.view.0))
        })?;
        let fragment = library
            .get_function(&fragment_name, None)
            .map_err(|error| {
                RuntimeDiagnostic::new(RuntimeDiagnosticCode::PipelineCreationFailed, error)
                    .at(format!("views.{}", view.view.0))
            })?;
        let descriptor = RenderPipelineDescriptor::new();
        descriptor.set_vertex_function(Some(&vertex));
        descriptor.set_fragment_function(Some(&fragment));
        descriptor
            .color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        let pipeline = device
            .new_render_pipeline_state(&descriptor)
            .map_err(|error| {
                RuntimeDiagnostic::new(RuntimeDiagnosticCode::PipelineCreationFailed, error)
                    .at(format!("views.{}", view.view.0))
            })?;
        pipeline_identities.push(PipelineIdentity {
            entry: view.implementation.entry.clone(),
            kind: "render".to_owned(),
            source_sha256: sha256(source.as_bytes()),
            thread_execution_width: None,
            max_threads_per_threadgroup: None,
        });
        shaders.insert(
            view.implementation.source.clone(),
            sha256(source.as_bytes()),
        );
        render.insert(view.view, pipeline);
    }

    let shaders = shaders
        .into_iter()
        .map(|(source_path, sha256)| ShaderIdentity {
            source_path,
            sha256,
        })
        .collect();
    pipeline_identities
        .sort_by(|left, right| (&left.kind, &left.entry).cmp(&(&right.kind, &right.entry)));
    Ok((compute, render, shaders, pipeline_identities))
}

fn shader_source(path: &str) -> Result<&'static str, RuntimeDiagnostic> {
    match path {
        "kernels/euler_integrate.metal" => Ok(INTEGRATE_SOURCE),
        "kernels/ground_contact.metal" => Ok(CONTACT_SOURCE),
        "shaders/particle.metal" => Ok(PARTICLE_SOURCE),
        _ => Err(RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsupportedGraph,
            format!("no packaged Metal source for `{path}`"),
        )),
    }
}

fn dispatch_count(
    graph: &loom_core::ModuleGraph,
    domain: &DispatchDomain,
) -> Result<u64, RuntimeDiagnostic> {
    match domain {
        DispatchDomain::Fixed(count) => Ok(*count as u64),
        DispatchDomain::OverStream(stream) => match graph.stream(*stream).unwrap().length {
            StreamLength::Fixed(length) => Ok(length as u64),
            StreamLength::Dynamic(_) => Err(RuntimeDiagnostic::new(
                RuntimeDiagnosticCode::UnsupportedGraph,
                "dynamic dispatch lengths are not in the first Metal slice",
            )),
        },
    }
}

fn benchmark_dispatch_count(
    graph: &loom_core::ModuleGraph,
    schedule: &ExecutionSchedule,
) -> Result<u32, RuntimeDiagnostic> {
    let pass = schedule.passes.first().ok_or_else(|| {
        RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsupportedGraph,
            "benchmark schedule has no compute dispatch domain",
        )
    })?;
    u32::try_from(dispatch_count(graph, &pass.dispatch)?).map_err(|_| {
        RuntimeDiagnostic::new(
            RuntimeDiagnosticCode::UnsupportedGraph,
            "benchmark dispatch domain exceeds the supported count",
        )
    })
}

fn make_fingerprint(
    validated: &ValidatedModuleGraph,
    device: &Device,
    shader_hashes: Vec<ShaderIdentity>,
    pipelines: Vec<PipelineIdentity>,
) -> RuntimeFingerprint {
    #[derive(serde::Serialize)]
    struct Identity<'a> {
        artifact: &'a str,
        device: &'a str,
        operating_system: &'a str,
        host_profile: &'a str,
        host_executable_sha256: &'a str,
        source_revision: &'a str,
        source_dirty: Option<bool>,
        rust_compiler: &'a str,
        metal_sdk: &'a str,
        shader_hashes: &'a [ShaderIdentity],
        pipelines: &'a [PipelineIdentity],
    }

    let device_name = device.name().to_owned();
    let operating_system = command_output("sw_vers", &["-productVersion"])
        .unwrap_or_else(|| "macOS unknown".to_owned());
    let host_profile = if cfg!(debug_assertions) {
        "debug".to_owned()
    } else {
        "release".to_owned()
    };
    let host_executable_sha256 = std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .map(sha256)
        .unwrap_or_else(|| "unknown".to_owned());
    let source_revision =
        command_output("git", &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    let source_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty());
    let rust_compiler =
        command_output("rustc", &["--version"]).unwrap_or_else(|| "unknown".to_owned());
    let metal_sdk = command_output("xcrun", &["--sdk", "macosx", "--show-sdk-version"])
        .unwrap_or_else(|| "unknown".to_owned());
    let identity = Identity {
        artifact: validated.artifact_fingerprint(),
        device: &device_name,
        operating_system: &operating_system,
        host_profile: &host_profile,
        host_executable_sha256: &host_executable_sha256,
        source_revision: &source_revision,
        source_dirty,
        rust_compiler: &rust_compiler,
        metal_sdk: &metal_sdk,
        shader_hashes: &shader_hashes,
        pipelines: &pipelines,
    };
    let bytes = serde_json::to_vec(&identity).expect("runtime identity serialization");
    RuntimeFingerprint {
        artifact: validated.artifact_fingerprint().to_owned(),
        device: device_name,
        operating_system,
        host_profile,
        host_executable_sha256,
        source_revision,
        source_dirty,
        rust_compiler,
        metal_sdk,
        shader_hashes,
        pipelines,
        fingerprint: sha256(bytes),
    }
}

fn command_output(program: &str, arguments: &[&str]) -> Option<String> {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|output| !output.is_empty())
}

#[cfg(test)]
mod tests {
    use super::pacing_offset;
    use std::time::Duration;

    #[test]
    fn rational_pacing_has_no_one_second_drift() {
        assert_eq!(pacing_offset(120, 120), Duration::from_secs(1));
        assert_eq!(pacing_offset(240, 120), Duration::from_secs(2));
    }
}
