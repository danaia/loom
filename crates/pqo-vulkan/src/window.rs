use std::{
    io::Cursor,
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

use ash::{Entry, vk};
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

use crate::CudaRuntime;

#[derive(Clone, Debug)]
pub struct NativeWindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub cuda_ordinal: i32,
    pub scene: NativeScene,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeScene {
    #[default]
    Crystal,
    HydrogenAtom,
    WaterMolecule,
}

impl Default for NativeWindowConfig {
    fn default() -> Self {
        Self {
            title: "Pqo — CUDA / Vulkan".to_owned(),
            width: 1180,
            height: 760,
            cuda_ordinal: 0,
            scene: NativeScene::Crystal,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VulkanControl {
    pub name: String,
    pub value: f32,
}

/// Opens a native Vulkan 1.3 swapchain on the same physical GPU selected by
/// CUDA. This is the presentation foundation for zero-copy CUDA/Vulkan views;
/// it intentionally contains no webview or browser compositor.
pub fn run_native_window(config: NativeWindowConfig) -> Result<(), String> {
    run_native_window_with_controls(config, None)
}

pub fn run_native_window_with_controls(
    config: NativeWindowConfig,
    controls: Option<Receiver<VulkanControl>>,
) -> Result<(), String> {
    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(&config.title)
        .with_inner_size(PhysicalSize::new(config.width, config.height))
        .with_resizable(false)
        .build(&event_loop)
        .map_err(|error| format!("could not create native Vulkan window: {error}"))?;
    let mut renderer = unsafe { NativeRenderer::new(&window, config.cuda_ordinal, config.scene)? };
    let mut runtime_error = None;
    let test_frames = std::env::var("PQO_VULKAN_TEST_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    let mut presented_frames = 0_u64;
    let mut orbiting = false;
    let mut disturbing_water = false;
    let mut last_cursor: Option<PhysicalPosition<f64>> = None;

    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::MainEventsCleared => {
                if let Some(controls) = &controls {
                    while let Ok(control) = controls.try_recv() {
                        renderer.set_control(&control.name, control.value);
                    }
                }
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                if let Err(error) = unsafe { renderer.draw() } {
                    runtime_error = Some(error);
                    *control_flow = ControlFlow::Exit;
                } else {
                    presented_frames += 1;
                    if test_frames.is_some_and(|limit| presented_frames >= limit) {
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state,
                        button: MouseButton::Left,
                        ..
                    },
                ..
            } => {
                if state == ElementState::Pressed {
                    let cursor = last_cursor.unwrap_or(PhysicalPosition::new(
                        config.width as f64 * 0.5,
                        config.height as f64 * 0.5,
                    ));
                    let on_water = matches!(config.scene, NativeScene::WaterMolecule)
                        && cursor.x >= config.width as f64 * 0.19
                        && cursor.x <= config.width as f64 * 0.81
                        && cursor.y >= config.height as f64 * 0.24
                        && cursor.y <= config.height as f64 * 0.72;
                    disturbing_water = on_water;
                    orbiting = !on_water;
                    if disturbing_water {
                        renderer.disturb_water(cursor, config.width, config.height);
                    }
                } else {
                    orbiting = false;
                    disturbing_water = false;
                    renderer.release_water();
                    last_cursor = None;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                if disturbing_water {
                    renderer.disturb_water(position, config.width, config.height);
                } else if orbiting {
                    if let Some(previous) = last_cursor {
                        renderer.orbit(
                            (position.x - previous.x) as f32 * 0.008,
                            (position.y - previous.y) as f32 * 0.008,
                        );
                    }
                }
                last_cursor = Some(position);
            }
            Event::WindowEvent {
                event: WindowEvent::MouseWheel { delta, .. },
                ..
            } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, vertical) => vertical * 0.12,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 * 0.001,
                };
                renderer.zoom(amount);
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(false),
                ..
            } => {
                orbiting = false;
                disturbing_water = false;
                renderer.release_water();
                last_cursor = None;
            }
            _ => {}
        }
    });

    unsafe { renderer.shutdown() };
    runtime_error.map_or(Ok(()), Err)
}

struct NativeRenderer {
    entry: Entry,
    instance: ash::Instance,
    surface_loader: ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    device: ash::Device,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    views: Vec<vk::ImageView>,
    extent: vk::Extent2D,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    frame_fence: vk::Fence,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    started: Instant,
    previous_frame: Instant,
    controls: CrystalControls,
    scene: NativeScene,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CrystalControls {
    time: f32,
    growth: f32,
    anisotropy: f32,
    temperature: f32,
    damage: f32,
    show_field: f32,
    show_particles: f32,
    particle_count: f32,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    smart_lod: f32,
    lod_bias: f32,
    instance_count: f32,
    pointer_x: f32,
    pointer_y: f32,
    pointer_down: f32,
    splash_time: f32,
    viewport_aspect: f32,
    sphere_mass_g: f32,
    sphere_count: f32,
    sphere_drop_time: f32,
}

impl Default for CrystalControls {
    fn default() -> Self {
        Self {
            time: 0.0,
            growth: 0.72,
            anisotropy: 0.68,
            temperature: 0.18,
            damage: 0.0,
            show_field: 1.0,
            show_particles: 0.0,
            particle_count: 1_000_000.0,
            yaw: -0.55,
            pitch: -0.35,
            zoom: 1.0,
            smart_lod: 1.0,
            lod_bias: 0.0,
            instance_count: 1.0,
            pointer_x: 0.0,
            pointer_y: 0.0,
            pointer_down: 0.0,
            splash_time: -10.0,
            viewport_aspect: 1180.0 / 760.0,
            sphere_mass_g: 18.0,
            sphere_count: 3.0,
            sphere_drop_time: -10.0,
        }
    }
}

impl CrystalControls {
    fn set(&mut self, name: &str, value: f32) {
        if !value.is_finite() {
            return;
        }
        match name {
            "crystal.growth" => self.growth = value.clamp(0.08, 1.0),
            "crystal.anisotropy" => self.anisotropy = value.clamp(0.0, 1.0),
            "crystal.temperature" => self.temperature = value.clamp(0.0, 1.0),
            "crystal.damage" => self.damage = value.clamp(0.0, 1.0),
            "crystal.show_field" => self.show_field = if value >= 0.5 { 1.0 } else { 0.0 },
            "crystal.show_particles" => self.show_particles = if value >= 0.5 { 1.0 } else { 0.0 },
            "crystal.particle_count" => self.particle_count = value.clamp(10_000.0, 1_000_000.0),
            "crystal.yaw" => self.yaw = value,
            "crystal.pitch" => self.pitch = value.clamp(-1.45, 1.45),
            "crystal.zoom" => self.zoom = value.clamp(0.55, 2.5),
            "crystal.orbit_delta_yaw" => self.orbit(value, 0.0),
            "crystal.orbit_delta_pitch" => self.orbit(0.0, value),
            "crystal.zoom_delta" => self.zoom(value),
            "crystal.smart_lod" => self.smart_lod = if value >= 0.5 { 1.0 } else { 0.0 },
            "crystal.lod_bias" => self.lod_bias = value.clamp(-2.0, 2.0),
            "crystal.instance_count" => self.instance_count = value.round().clamp(1.0, 1_000.0),
            "water.sphere_mass_g" => self.sphere_mass_g = value.clamp(2.0, 120.0),
            "water.sphere_count" => self.sphere_count = value.round().clamp(1.0, 5.0),
            "water.drop_spheres" => self.sphere_drop_time = self.time,
            _ => {}
        }
    }

    fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-1.45, 1.45);
    }

    fn zoom(&mut self, amount: f32) {
        self.zoom = (self.zoom * amount.exp()).clamp(0.55, 2.5);
    }

    fn zoom_water(&mut self, amount: f32) {
        self.zoom = (self.zoom * amount.exp()).clamp(0.48, 12.0);
    }
}

impl NativeRenderer {
    unsafe fn new(
        window: &winit::window::Window,
        cuda_ordinal: i32,
        scene: NativeScene,
    ) -> Result<Self, String> {
        let cuda = CudaRuntime::probe_device(cuda_ordinal)?;
        let entry =
            unsafe { Entry::load() }.map_err(|error| format!("could not load Vulkan: {error}"))?;
        let surface_extension = match window.raw_display_handle() {
            RawDisplayHandle::Xlib(_) => ash::khr::xlib_surface::NAME,
            RawDisplayHandle::Wayland(_) => ash::khr::wayland_surface::NAME,
            other => return Err(format!("unsupported Linux display handle: {other:?}")),
        };
        let extension_names = [ash::khr::surface::NAME.as_ptr(), surface_extension.as_ptr()];
        let application = vk::ApplicationInfo::default()
            .application_name(c"pqo-native-vulkan")
            .engine_name(c"pqo")
            .api_version(vk::API_VERSION_1_3);
        let instance = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo::default()
                    .application_info(&application)
                    .enabled_extension_names(&extension_names),
                None,
            )
        }
        .map_err(|error| format!("could not create Vulkan instance: {error}"))?;
        let surface = unsafe { create_surface(&entry, &instance, window) }?;
        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);

        let (physical, queue_family) = unsafe { instance.enumerate_physical_devices() }
            .map_err(|error| format!("could not enumerate Vulkan devices: {error}"))?
            .into_iter()
            .find_map(|physical| {
                let mut id = vk::PhysicalDeviceIDProperties::default();
                let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
                unsafe { instance.get_physical_device_properties2(physical, &mut properties) };
                if id.device_uuid != cuda.uuid {
                    return None;
                }
                unsafe { instance.get_physical_device_queue_family_properties(physical) }
                    .iter()
                    .enumerate()
                    .find(|(index, family)| {
                        family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                            && unsafe {
                                surface_loader
                                    .get_physical_device_surface_support(
                                        physical,
                                        *index as u32,
                                        surface,
                                    )
                                    .unwrap_or(false)
                            }
                    })
                    .map(|(index, _)| (physical, index as u32))
            })
            .ok_or_else(|| {
                "the CUDA-selected GPU has no Vulkan graphics/present queue".to_owned()
            })?;

        let priorities = [1.0_f32];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priorities)];
        let device_extensions = [
            ash::khr::swapchain::NAME.as_ptr(),
            ash::khr::external_memory_fd::NAME.as_ptr(),
            ash::khr::external_semaphore_fd::NAME.as_ptr(),
        ];
        let mut vulkan12 = vk::PhysicalDeviceVulkan12Features::default().timeline_semaphore(true);
        let mut vulkan13 = vk::PhysicalDeviceVulkan13Features::default()
            .synchronization2(true)
            .dynamic_rendering(true);
        let device = unsafe {
            instance.create_device(
                physical,
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queues)
                    .enabled_extension_names(&device_extensions)
                    .push_next(&mut vulkan12)
                    .push_next(&mut vulkan13),
                None,
            )
        }
        .map_err(|error| format!("could not create Vulkan device: {error}"))?;
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
        let capabilities =
            unsafe { surface_loader.get_physical_device_surface_capabilities(physical, surface) }
                .map_err(|error| format!("could not query surface capabilities: {error}"))?;
        let formats =
            unsafe { surface_loader.get_physical_device_surface_formats(physical, surface) }
                .map_err(|error| format!("could not query surface formats: {error}"))?;
        let format = formats
            .iter()
            .copied()
            .find(|candidate| {
                candidate.format == vk::Format::B8G8R8A8_SRGB
                    && candidate.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or(formats[0]);
        let extent = if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            vk::Extent2D {
                width: config_extent(
                    configured(window.inner_size().width),
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ),
                height: config_extent(
                    configured(window.inner_size().height),
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ),
            }
        };
        let image_count =
            (capabilities.min_image_count + 1).min(if capabilities.max_image_count == 0 {
                u32::MAX
            } else {
                capabilities.max_image_count
            });
        let swapchain = unsafe {
            swapchain_loader.create_swapchain(
                &vk::SwapchainCreateInfoKHR::default()
                    .surface(surface)
                    .min_image_count(image_count)
                    .image_format(format.format)
                    .image_color_space(format.color_space)
                    .image_extent(extent)
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(capabilities.current_transform)
                    .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                    .present_mode(vk::PresentModeKHR::FIFO)
                    .clipped(true),
                None,
            )
        }
        .map_err(|error| format!("could not create Vulkan swapchain: {error}"))?;
        let images = unsafe { swapchain_loader.get_swapchain_images(swapchain) }
            .map_err(|error| format!("could not get swapchain images: {error}"))?;
        let views = images
            .iter()
            .map(|image| unsafe {
                device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(*image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format.format)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(1)
                                .layer_count(1),
                        ),
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("could not create swapchain image view: {error}"))?;
        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }
        .map_err(|error| format!("could not create Vulkan command pool: {error}"))?;
        let command_buffers = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(images.len() as u32),
            )
        }
        .map_err(|error| format!("could not allocate Vulkan command buffers: {error}"))?;
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let image_available = unsafe { device.create_semaphore(&semaphore_info, None) }
            .map_err(|error| error.to_string())?;
        let render_finished = unsafe { device.create_semaphore(&semaphore_info, None) }
            .map_err(|error| error.to_string())?;
        let frame_fence = unsafe {
            device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )
        }
        .map_err(|error| error.to_string())?;
        let push_range = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(std::mem::size_of::<CrystalControls>() as u32)];
        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&push_range),
                None,
            )
        }
        .map_err(|error| format!("could not create Vulkan pipeline layout: {error}"))?;
        let vertex_code = ash::util::read_spv(&mut Cursor::new(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/crystal.vert.spv"
        ))))
        .map_err(|error| format!("invalid embedded vertex SPIR-V: {error}"))?;
        let fragment_bytes: &[u8] = match scene {
            NativeScene::Crystal => include_bytes!(concat!(env!("OUT_DIR"), "/crystal.frag.spv")),
            NativeScene::HydrogenAtom => {
                include_bytes!(concat!(env!("OUT_DIR"), "/atom.frag.spv"))
            }
            NativeScene::WaterMolecule => {
                include_bytes!(concat!(env!("OUT_DIR"), "/water.frag.spv"))
            }
        };
        let fragment_code = ash::util::read_spv(&mut Cursor::new(fragment_bytes))
            .map_err(|error| format!("invalid embedded fragment SPIR-V: {error}"))?;
        let vertex_module = unsafe {
            device.create_shader_module(
                &vk::ShaderModuleCreateInfo::default().code(&vertex_code),
                None,
            )
        }
        .map_err(|error| format!("could not create vertex shader: {error}"))?;
        let fragment_module = unsafe {
            device.create_shader_module(
                &vk::ShaderModuleCreateInfo::default().code(&fragment_code),
                None,
            )
        }
        .map_err(|error| format!("could not create fragment shader: {error}"))?;
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vertex_module)
                .name(c"main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fragment_module)
                .name(c"main"),
        ];
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let blend_attachments = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let blend =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachments);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let color_formats = [format.format];
        let mut rendering =
            vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&color_formats);
        let pipeline = unsafe {
            device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo::default()
                    .stages(&stages)
                    .vertex_input_state(&vertex_input)
                    .input_assembly_state(&assembly)
                    .viewport_state(&viewport_state)
                    .rasterization_state(&rasterization)
                    .multisample_state(&multisample)
                    .color_blend_state(&blend)
                    .dynamic_state(&dynamic)
                    .layout(pipeline_layout)
                    .push_next(&mut rendering)],
                None,
            )
        }
        .map_err(|(_, error)| format!("could not create graphics pipeline: {error}"))?[0];
        unsafe {
            device.destroy_shader_module(fragment_module, None);
            device.destroy_shader_module(vertex_module, None);
        }
        let mut controls = CrystalControls::default();
        if scene == NativeScene::HydrogenAtom {
            controls.zoom = 2.0;
        } else if scene == NativeScene::WaterMolecule {
            controls.yaw = -0.10;
            controls.pitch = -0.10;
            controls.zoom = 0.72;
            controls.viewport_aspect = extent.width as f32 / extent.height.max(1) as f32;
            if let Ok(value) = std::env::var("PQO_WATER_START_ZOOM") {
                if let Ok(value) = value.parse::<f32>() {
                    controls.zoom = value.clamp(0.48, 12.0);
                }
            }
        }
        Ok(Self {
            entry,
            instance,
            surface_loader,
            surface,
            device,
            swapchain_loader,
            swapchain,
            images,
            views,
            extent,
            queue,
            command_pool,
            command_buffers,
            image_available,
            render_finished,
            frame_fence,
            pipeline_layout,
            pipeline,
            started: Instant::now(),
            previous_frame: Instant::now(),
            controls,
            scene,
        })
    }

    fn set_control(&mut self, name: &str, value: f32) {
        self.controls.set(name, value);
    }

    fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.controls.orbit(delta_yaw, delta_pitch);
    }

    fn zoom(&mut self, amount: f32) {
        if matches!(self.scene, NativeScene::WaterMolecule) {
            self.controls.zoom_water(amount);
        } else {
            self.controls.zoom(amount);
        }
    }

    fn disturb_water(&mut self, position: PhysicalPosition<f64>, width: u32, height: u32) {
        self.controls.pointer_x = position.x as f32 / width.max(1) as f32 * 2.0 - 1.0;
        self.controls.pointer_y = 1.0 - position.y as f32 / height.max(1) as f32 * 2.0;
        self.controls.pointer_down = 1.0;
        self.controls.splash_time = self.started.elapsed().as_secs_f32();
    }

    fn release_water(&mut self) {
        self.controls.pointer_down = 0.0;
    }

    unsafe fn draw(&mut self) -> Result<(), String> {
        unsafe {
            self.device
                .wait_for_fences(&[self.frame_fence], true, u64::MAX)
        }
        .map_err(|error| error.to_string())?;
        let (image_index, _) = unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            )
        }
        .map_err(|error| format!("could not acquire Vulkan image: {error}"))?;
        unsafe { self.device.reset_fences(&[self.frame_fence]) }
            .map_err(|error| error.to_string())?;
        let command = self.command_buffers[image_index as usize];
        unsafe {
            self.device
                .reset_command_buffer(command, vk::CommandBufferResetFlags::empty())
                .map_err(|error| error.to_string())?;
            self.device
                .begin_command_buffer(command, &vk::CommandBufferBeginInfo::default())
                .map_err(|error| error.to_string())?;
        }
        let image = self.images[image_index as usize];
        transition(
            &self.device,
            command,
            image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::PipelineStageFlags2::NONE,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::NONE,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        );
        let now = Instant::now();
        let delta = now
            .duration_since(self.previous_frame)
            .min(Duration::from_millis(50));
        self.previous_frame = now;
        let elapsed = self.started.elapsed().as_secs_f32();
        self.controls.time = elapsed;
        self.controls.damage = (self.controls.damage - delta.as_secs_f32() * 0.075).max(0.0);
        let pulse = 0.5 + 0.5 * (elapsed * 0.7).sin();
        let attachment = [vk::RenderingAttachmentInfo::default()
            .image_view(self.views[image_index as usize])
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [
                        0.008 + pulse * 0.012,
                        0.025 + pulse * 0.02,
                        0.04 + pulse * 0.035,
                        1.0,
                    ],
                },
            })];
        unsafe {
            self.device.cmd_begin_rendering(
                command,
                &vk::RenderingInfo::default()
                    .render_area(vk::Rect2D::default().extent(self.extent))
                    .layer_count(1)
                    .color_attachments(&attachment),
            );
            self.device
                .cmd_bind_pipeline(command, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            self.device.cmd_set_viewport(
                command,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: self.extent.width as f32,
                    height: self.extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.device
                .cmd_set_scissor(command, 0, &[vk::Rect2D::default().extent(self.extent)]);
            let control_bytes = std::slice::from_raw_parts(
                (&self.controls as *const CrystalControls).cast::<u8>(),
                std::mem::size_of::<CrystalControls>(),
            );
            self.device.cmd_push_constants(
                command,
                self.pipeline_layout,
                vk::ShaderStageFlags::FRAGMENT,
                0,
                control_bytes,
            );
            self.device.cmd_draw(command, 3, 1, 0, 0);
            self.device.cmd_end_rendering(command);
        }
        transition(
            &self.device,
            command,
            image,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags2::NONE,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            vk::AccessFlags2::NONE,
        );
        unsafe { self.device.end_command_buffer(command) }.map_err(|error| error.to_string())?;
        let wait = [self.image_available];
        let signal = [self.render_finished];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let commands = [command];
        unsafe {
            self.device.queue_submit(
                self.queue,
                &[vk::SubmitInfo::default()
                    .wait_semaphores(&wait)
                    .wait_dst_stage_mask(&wait_stages)
                    .command_buffers(&commands)
                    .signal_semaphores(&signal)],
                self.frame_fence,
            )
        }
        .map_err(|error| format!("could not submit Vulkan frame: {error}"))?;
        unsafe {
            self.swapchain_loader.queue_present(
                self.queue,
                &vk::PresentInfoKHR::default()
                    .wait_semaphores(&signal)
                    .swapchains(&[self.swapchain])
                    .image_indices(&[image_index]),
            )
        }
        .map_err(|error| format!("could not present Vulkan frame: {error}"))?;
        Ok(())
    }

    unsafe fn shutdown(&mut self) {
        let _ = unsafe { self.device.device_wait_idle() };
        unsafe {
            self.device.destroy_fence(self.frame_fence, None);
            self.device.destroy_semaphore(self.render_finished, None);
            self.device.destroy_semaphore(self.image_available, None);
            self.device.destroy_pipeline(self.pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_command_pool(self.command_pool, None);
            for view in self.views.drain(..) {
                self.device.destroy_image_view(view, None);
            }
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
        let _ = &self.entry;
    }
}

fn configured(value: u32) -> u32 {
    value.max(1)
}
fn config_extent(value: u32, minimum: u32, maximum: u32) -> u32 {
    value.clamp(minimum, maximum)
}

unsafe fn create_surface(
    entry: &Entry,
    instance: &ash::Instance,
    window: &winit::window::Window,
) -> Result<vk::SurfaceKHR, String> {
    match (window.raw_display_handle(), window.raw_window_handle()) {
        (RawDisplayHandle::Xlib(display), RawWindowHandle::Xlib(window)) => unsafe {
            ash::khr::xlib_surface::Instance::new(entry, instance)
                .create_xlib_surface(
                    &vk::XlibSurfaceCreateInfoKHR::default()
                        .dpy(display.display.cast())
                        .window(window.window),
                    None,
                )
                .map_err(|error| format!("could not create Xlib Vulkan surface: {error}"))
        },
        (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(window)) => unsafe {
            ash::khr::wayland_surface::Instance::new(entry, instance)
                .create_wayland_surface(
                    &vk::WaylandSurfaceCreateInfoKHR::default()
                        .display(display.display.cast())
                        .surface(window.surface.cast()),
                    None,
                )
                .map_err(|error| format!("could not create Wayland Vulkan surface: {error}"))
        },
        (display, window) => Err(format!(
            "unsupported Linux display/window handles: {display:?} / {window:?}"
        )),
    }
}

fn transition(
    device: &ash::Device,
    command: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
    source_stage: vk::PipelineStageFlags2,
    destination_stage: vk::PipelineStageFlags2,
    source_access: vk::AccessFlags2,
    destination_access: vk::AccessFlags2,
) {
    let barriers = [vk::ImageMemoryBarrier2::default()
        .src_stage_mask(source_stage)
        .src_access_mask(source_access)
        .dst_stage_mask(destination_stage)
        .dst_access_mask(destination_access)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .image(image)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .level_count(1)
                .layer_count(1),
        )];
    unsafe {
        device.cmd_pipeline_barrier2(
            command,
            &vk::DependencyInfo::default().image_memory_barriers(&barriers),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::CrystalControls;

    #[test]
    fn embedded_hydrogen_shader_preserves_the_baseline_orbital_contract() {
        let embedded = include_str!("../shaders/atom.frag");
        let baseline = include_str!("../../../baseline/shaders/atom.frag");
        for required in [
            "exp(-2.0 * length(position_bohr)) / 3.141592653589793",
            "const int sample_count = 192",
            "intersect_sphere(ray_origin, ray_direction, 0.075",
        ] {
            assert!(embedded.contains(required));
            assert!(baseline.contains(required));
        }
    }

    #[test]
    fn embedded_water_shader_preserves_the_rigid_three_site_contract() {
        let embedded = include_str!("../shaders/water.frag");
        let example = include_str!("../../../examples/water-molecule/shaders/water.frag");
        for required in [
            "const float OH_DISTANCE_ANGSTROM = 0.9572",
            "const float HOH_ANGLE_DEGREES = 104.52",
            "const float OXYGEN_CHARGE_E = -0.834",
            "const float HYDROGEN_CHARGE_E = 0.417",
            "water_geometry(oxygen, hydrogen_1, hydrogen_2)",
        ] {
            assert!(embedded.contains(required));
            assert!(example.contains(required));
        }
    }

    #[test]
    fn panel_controls_map_to_bounded_vulkan_push_constants() {
        let mut controls = CrystalControls::default();
        controls.set("crystal.growth", 0.41);
        controls.set("crystal.anisotropy", 2.0);
        controls.set("crystal.temperature", -1.0);
        controls.set("crystal.damage", 0.73);
        controls.set("crystal.show_field", 0.0);
        controls.set("crystal.show_particles", 1.0);
        controls.set("crystal.particle_count", 250_000.0);
        controls.set("crystal.yaw", 1.2);
        controls.set("crystal.pitch", 9.0);
        controls.set("crystal.zoom", 0.1);
        controls.set("crystal.smart_lod", 0.0);
        controls.set("crystal.lod_bias", 3.0);
        controls.set("crystal.instance_count", 537.4);
        assert_eq!(controls.growth, 0.41);
        assert_eq!(controls.anisotropy, 1.0);
        assert_eq!(controls.temperature, 0.0);
        assert_eq!(controls.damage, 0.73);
        assert_eq!(controls.show_field, 0.0);
        assert_eq!(controls.show_particles, 1.0);
        assert_eq!(controls.particle_count, 250_000.0);
        assert_eq!(controls.yaw, 1.2);
        assert_eq!(controls.pitch, 1.45);
        assert_eq!(controls.zoom, 0.55);
        assert_eq!(controls.smart_lod, 0.0);
        assert_eq!(controls.lod_bias, 2.0);
        assert_eq!(controls.instance_count, 537.0);
        controls.set("crystal.orbit_delta_yaw", 0.25);
        controls.set("crystal.orbit_delta_pitch", -4.0);
        controls.set("crystal.zoom_delta", 4.0);
        assert_eq!(controls.yaw, 1.45);
        assert_eq!(controls.pitch, -1.45);
        assert_eq!(controls.zoom, 2.5);
    }

    #[test]
    fn water_zoom_crosses_from_cup_to_molecule_scale() {
        let mut controls = CrystalControls::default();
        controls.zoom = 0.72;
        controls.zoom_water(10.0);
        assert_eq!(controls.zoom, 12.0);
        controls.zoom_water(-10.0);
        assert_eq!(controls.zoom, 0.48);
    }

    #[test]
    fn water_sphere_controls_bound_mass_count_and_trigger_drop() {
        let mut controls = CrystalControls::default();
        controls.time = 4.25;
        controls.set("water.sphere_mass_g", 240.0);
        controls.set("water.sphere_count", 3.6);
        controls.set("water.drop_spheres", 1.0);
        assert_eq!(controls.sphere_mass_g, 120.0);
        assert_eq!(controls.sphere_count, 4.0);
        assert_eq!(controls.sphere_drop_time, 4.25);
    }
}
