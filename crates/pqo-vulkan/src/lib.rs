#![cfg(target_os = "linux")]

use std::{collections::BTreeSet, ffi::CStr};

use ash::{Entry, vk};
use pqo_cuda::{CudaDeviceIdentity, CudaRuntime};
use serde::{Deserialize, Serialize};

mod window;
pub use window::{
    NativeWindowConfig, VulkanControl, run_native_window, run_native_window_with_controls,
};

const REQUIRED_DEVICE_EXTENSIONS: [&str; 3] = [
    "VK_KHR_swapchain",
    "VK_KHR_external_memory_fd",
    "VK_KHR_external_semaphore_fd",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VulkanDeviceIdentity {
    pub name: String,
    pub uuid: [u8; 16],
    pub api_version: u32,
    pub driver_version: u32,
    pub vendor_id: u32,
    pub device_id: u32,
    pub graphics_queue_family: u32,
    pub timeline_semaphore: bool,
    pub synchronization2: bool,
    pub dynamic_rendering: bool,
    pub required_extensions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InteropDeviceReport {
    pub cuda: CudaDeviceIdentity,
    pub vulkan: VulkanDeviceIdentity,
    pub uuid_match: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalResourceProbeReport {
    pub device: InteropDeviceReport,
    pub allocation_size: u64,
    pub mapped_size: u64,
    pub cuda_signal_value: u64,
    pub vulkan_observed_value: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationRingPlan {
    pub slots: Vec<PresentationSlotPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationSlotPlan {
    pub slot: u32,
    pub generation: u64,
    pub released_value: u64,
    pub ready_value: u64,
}

impl PresentationRingPlan {
    pub fn for_swapchain_images(swapchain_images: u32) -> Self {
        let count = swapchain_images.saturating_add(1).clamp(2, 4);
        Self {
            slots: (0..count)
                .map(|slot| PresentationSlotPlan {
                    slot,
                    generation: 0,
                    released_value: 0,
                    ready_value: 1,
                })
                .collect(),
        }
    }

    pub fn advance(&mut self, slot: u32) -> Result<PresentationSlotPlan, String> {
        let state = self
            .slots
            .get_mut(slot as usize)
            .ok_or_else(|| format!("presentation slot {slot} does not exist"))?;
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| "presentation timeline generation overflow".to_owned())?;
        state.released_value = state
            .generation
            .checked_mul(2)
            .ok_or_else(|| "presentation release timeline overflow".to_owned())?;
        state.ready_value = state
            .released_value
            .checked_add(1)
            .ok_or_else(|| "presentation ready timeline overflow".to_owned())?;
        Ok(state.clone())
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(2..=4).contains(&self.slots.len()) {
            return Err("presentation ring must contain two through four slots".to_owned());
        }
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.slot != index as u32
                || slot.released_value != slot.generation * 2
                || slot.ready_value != slot.released_value + 1
            {
                return Err(format!("presentation slot {index} has an invalid timeline"));
            }
        }
        Ok(())
    }
}

pub fn probe_interop_device(cuda_ordinal: i32) -> Result<InteropDeviceReport, String> {
    let cuda = CudaRuntime::probe_device(cuda_ordinal)?;
    // SAFETY: the instance is local and destroyed after physical-device queries.
    unsafe {
        let entry = Entry::load().map_err(|error| format!("could not load Vulkan: {error}"))?;
        let application_name = c"pqo-vulkan-probe";
        let application = vk::ApplicationInfo::default()
            .application_name(application_name)
            .application_version(1)
            .engine_name(application_name)
            .engine_version(1)
            .api_version(vk::API_VERSION_1_3);
        let create = vk::InstanceCreateInfo::default().application_info(&application);
        let instance = entry
            .create_instance(&create, None)
            .map_err(|error| format!("could not create Vulkan 1.3 instance: {error}"))?;
        let result = probe_matching_physical_device(&instance, &cuda);
        instance.destroy_instance(None);
        result
    }
}

pub fn probe_external_resources(
    cuda_ordinal: i32,
    mapped_size: u64,
) -> Result<ExternalResourceProbeReport, String> {
    let identity = CudaRuntime::probe_device(cuda_ordinal)?;
    // SAFETY: every Vulkan handle is destroyed after CUDA releases its imports.
    unsafe {
        let entry = Entry::load().map_err(|error| format!("could not load Vulkan: {error}"))?;
        let application = vk::ApplicationInfo::default()
            .application_name(c"pqo-interop-probe")
            .api_version(vk::API_VERSION_1_3);
        let instance = entry
            .create_instance(
                &vk::InstanceCreateInfo::default().application_info(&application),
                None,
            )
            .map_err(|error| format!("could not create Vulkan instance: {error}"))?;
        let result = probe_external_resources_on_instance(
            &instance,
            &identity,
            cuda_ordinal,
            mapped_size.max(256),
        );
        instance.destroy_instance(None);
        result
    }
}

unsafe fn probe_external_resources_on_instance(
    instance: &ash::Instance,
    cuda: &CudaDeviceIdentity,
    cuda_ordinal: i32,
    mapped_size: u64,
) -> Result<ExternalResourceProbeReport, String> {
    let physical = unsafe { instance.enumerate_physical_devices() }
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|device| {
            let mut id = vk::PhysicalDeviceIDProperties::default();
            let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
            unsafe { instance.get_physical_device_properties2(*device, &mut properties) };
            id.device_uuid == cuda.uuid
        })
        .ok_or_else(|| "no Vulkan device matches the CUDA UUID".to_owned())?;
    let queue_family = unsafe { instance.get_physical_device_queue_family_properties(physical) }
        .iter()
        .position(|family| family.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .ok_or_else(|| "matching Vulkan device has no graphics queue".to_owned())?
        as u32;
    let priority = [1.0_f32];
    let queue = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family)
        .queue_priorities(&priority)];
    let extension_names = REQUIRED_DEVICE_EXTENSIONS
        .iter()
        .map(|name| CStringHolder::new(name))
        .collect::<Result<Vec<_>, _>>()?;
    let extension_pointers = extension_names
        .iter()
        .map(|name| name.0.as_ptr())
        .collect::<Vec<_>>();
    let mut vulkan12 = vk::PhysicalDeviceVulkan12Features::default().timeline_semaphore(true);
    let mut vulkan13 = vk::PhysicalDeviceVulkan13Features::default()
        .synchronization2(true)
        .dynamic_rendering(true);
    let create = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue)
        .enabled_extension_names(&extension_pointers)
        .push_next(&mut vulkan12)
        .push_next(&mut vulkan13);
    let device = unsafe { instance.create_device(physical, &create, None) }
        .map_err(|error| format!("could not create Vulkan interop device: {error}"))?;
    let result = unsafe {
        allocate_export_import_probe(instance, physical, &device, cuda_ordinal, mapped_size)
    };
    unsafe { device.destroy_device(None) };
    result
}

unsafe fn allocate_export_import_probe(
    instance: &ash::Instance,
    physical: vk::PhysicalDevice,
    device: &ash::Device,
    cuda_ordinal: i32,
    mapped_size: u64,
) -> Result<ExternalResourceProbeReport, String> {
    let handle_type = vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD;
    let mut external_buffer =
        vk::ExternalMemoryBufferCreateInfo::default().handle_types(handle_type);
    let buffer = unsafe {
        device.create_buffer(
            &vk::BufferCreateInfo::default()
                .size(mapped_size)
                .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::VERTEX_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .push_next(&mut external_buffer),
            None,
        )
    }
    .map_err(|error| format!("could not create external Vulkan buffer: {error}"))?;
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memory_properties = unsafe { instance.get_physical_device_memory_properties(physical) };
    let memory_type = (0..memory_properties.memory_type_count)
        .find(|index| {
            requirements.memory_type_bits & (1 << index) != 0
                && memory_properties.memory_types[*index as usize]
                    .property_flags
                    .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
        })
        .ok_or_else(|| "no device-local external memory type is available".to_owned())?;
    let mut export_memory = vk::ExportMemoryAllocateInfo::default().handle_types(handle_type);
    let memory = unsafe {
        device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type)
                .push_next(&mut export_memory),
            None,
        )
    }
    .map_err(|error| format!("could not allocate exported Vulkan memory: {error}"))?;
    unsafe { device.bind_buffer_memory(buffer, memory, 0) }
        .map_err(|error| format!("could not bind exported Vulkan memory: {error}"))?;

    let semaphore_handle_type = vk::ExternalSemaphoreHandleTypeFlags::OPAQUE_FD;
    let mut export_semaphore =
        vk::ExportSemaphoreCreateInfo::default().handle_types(semaphore_handle_type);
    let mut timeline = vk::SemaphoreTypeCreateInfo::default()
        .semaphore_type(vk::SemaphoreType::TIMELINE)
        .initial_value(0);
    let semaphore = unsafe {
        device.create_semaphore(
            &vk::SemaphoreCreateInfo::default()
                .push_next(&mut export_semaphore)
                .push_next(&mut timeline),
            None,
        )
    }
    .map_err(|error| format!("could not create exported timeline semaphore: {error}"))?;

    let memory_fd_loader = ash::khr::external_memory_fd::Device::new(instance, device);
    let semaphore_fd_loader = ash::khr::external_semaphore_fd::Device::new(instance, device);
    let memory_fd = unsafe {
        memory_fd_loader.get_memory_fd(
            &vk::MemoryGetFdInfoKHR::default()
                .memory(memory)
                .handle_type(handle_type),
        )
    }
    .map_err(|error| format!("could not export Vulkan memory fd: {error}"))?;
    let semaphore_fd = unsafe {
        semaphore_fd_loader.get_semaphore_fd(
            &vk::SemaphoreGetFdInfoKHR::default()
                .semaphore(semaphore)
                .handle_type(semaphore_handle_type),
        )
    }
    .map_err(|error| format!("could not export Vulkan semaphore fd: {error}"))?;
    let signal_value = 1;
    let interop_result = CudaRuntime::probe_external_resources(
        cuda_ordinal,
        memory_fd,
        requirements.size,
        mapped_size,
        semaphore_fd,
        signal_value,
    );
    let observed = if interop_result.is_ok() {
        unsafe { device.get_semaphore_counter_value(semaphore) }
            .map_err(|error| format!("could not query shared timeline semaphore: {error}"))?
    } else {
        0
    };
    unsafe {
        device.destroy_semaphore(semaphore, None);
        device.destroy_buffer(buffer, None);
        device.free_memory(memory, None);
    }
    interop_result?;
    if observed != signal_value {
        return Err(format!(
            "CUDA signaled timeline value {signal_value}, Vulkan observed {observed}"
        ));
    }
    let device_report = probe_interop_device(cuda_ordinal)?;
    Ok(ExternalResourceProbeReport {
        device: device_report,
        allocation_size: requirements.size,
        mapped_size,
        cuda_signal_value: signal_value,
        vulkan_observed_value: observed,
    })
}

struct CStringHolder(std::ffi::CString);

impl CStringHolder {
    fn new(value: &str) -> Result<Self, String> {
        std::ffi::CString::new(value)
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

unsafe fn probe_matching_physical_device(
    instance: &ash::Instance,
    cuda: &CudaDeviceIdentity,
) -> Result<InteropDeviceReport, String> {
    let devices = unsafe { instance.enumerate_physical_devices() }
        .map_err(|error| format!("could not enumerate Vulkan devices: {error}"))?;
    let mut observed = Vec::new();
    for device in devices {
        let mut id = vk::PhysicalDeviceIDProperties::default();
        let base_properties = {
            let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
            unsafe { instance.get_physical_device_properties2(device, &mut properties) };
            properties.properties
        };
        let name = unsafe { CStr::from_ptr(base_properties.device_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        observed.push(format!("{name} ({})", hex_uuid(&id.device_uuid)));
        if id.device_uuid != cuda.uuid {
            continue;
        }

        let extensions = unsafe { instance.enumerate_device_extension_properties(device) }
            .map_err(|error| format!("could not enumerate Vulkan device extensions: {error}"))?
            .into_iter()
            .map(|extension| {
                unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) }
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<BTreeSet<_>>();
        let missing = REQUIRED_DEVICE_EXTENSIONS
            .iter()
            .filter(|required| !extensions.contains(**required))
            .copied()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "matching Vulkan device is missing required extensions: {}",
                missing.join(", ")
            ));
        }

        let mut vulkan12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut vulkan13 = vk::PhysicalDeviceVulkan13Features::default();
        let mut features = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut vulkan12)
            .push_next(&mut vulkan13);
        unsafe { instance.get_physical_device_features2(device, &mut features) };
        if vulkan12.timeline_semaphore != vk::TRUE
            || vulkan13.synchronization2 != vk::TRUE
            || vulkan13.dynamic_rendering != vk::TRUE
        {
            return Err(
                "matching Vulkan device lacks timeline semaphores, synchronization2, or dynamic rendering"
                    .to_owned(),
            );
        }
        let graphics_queue_family =
            unsafe { instance.get_physical_device_queue_family_properties(device) }
                .iter()
                .position(|family| family.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .ok_or_else(|| "matching Vulkan device has no graphics queue".to_owned())?
                as u32;
        let vulkan = VulkanDeviceIdentity {
            name,
            uuid: id.device_uuid,
            api_version: base_properties.api_version,
            driver_version: base_properties.driver_version,
            vendor_id: base_properties.vendor_id,
            device_id: base_properties.device_id,
            graphics_queue_family,
            timeline_semaphore: true,
            synchronization2: true,
            dynamic_rendering: true,
            required_extensions: REQUIRED_DEVICE_EXTENSIONS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
        };
        return Ok(InteropDeviceReport {
            cuda: cuda.clone(),
            uuid_match: vulkan.uuid == cuda.uuid,
            vulkan,
        });
    }
    Err(format!(
        "no Vulkan device matches CUDA UUID {}; observed {}",
        hex_uuid(&cuda.uuid),
        observed.join(", ")
    ))
}

fn hex_uuid(uuid: &[u8; 16]) -> String {
    let mut output = String::with_capacity(36);
    for (index, byte) in uuid.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            output.push('-');
        }
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_size_is_swapchain_plus_one_with_supported_bounds() {
        assert_eq!(PresentationRingPlan::for_swapchain_images(1).slots.len(), 2);
        assert_eq!(PresentationRingPlan::for_swapchain_images(2).slots.len(), 3);
        assert_eq!(PresentationRingPlan::for_swapchain_images(3).slots.len(), 4);
        assert_eq!(PresentationRingPlan::for_swapchain_images(8).slots.len(), 4);
    }

    #[test]
    fn per_slot_timeline_values_are_strictly_monotonic() {
        let mut ring = PresentationRingPlan::for_swapchain_images(2);
        assert_eq!(ring.advance(1).unwrap().ready_value, 3);
        assert_eq!(ring.advance(1).unwrap().ready_value, 5);
        ring.validate().unwrap();
    }
}
