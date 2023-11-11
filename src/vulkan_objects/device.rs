use std::{
    ops::Deref,
    rc::{Rc, Weak},
};

use ash::{prelude::VkResult, vk};

use super::{Instance, QueueInfo};

pub struct Device {
    inner: ash::Device,
    instance: Rc<Instance>,
    physical_device: Weak<vk::PhysicalDevice>,
    queue_family_indices: Vec<QueueInfo>,
}

impl Device {
    pub fn new(instance: Rc<Instance>, queue_family_indices: Vec<QueueInfo>) -> VkResult<Self> {
        assert!(!queue_family_indices.is_empty());

        let physical_device = instance.pick_physical_device();
        Ok(Self {
            inner: {
                let priorities = queue_family_indices
                    .iter()
                    .map(|q| q.priority)
                    .collect::<Vec<_>>();
                let queue_create_infos = queue_family_indices
                    .iter()
                    .enumerate()
                    .map(|(i, info)| {
                        vk::DeviceQueueCreateInfo::builder()
                            .queue_family_index(info.index)
                            .queue_priorities(&priorities[i..(i + 1)])
                            .build()
                    })
                    .collect::<Vec<_>>();

                let device_extension_names = [
                    #[cfg(any(target_os = "macos", target_os = "ios"))]
                    vk::KhrPortabilitySubsetFn::name().as_ptr(),
                    vk::KhrSwapchainFn::name().as_ptr(),
                ];

                let create_info = vk::DeviceCreateInfo::builder()
                    .queue_create_infos(&queue_create_infos)
                    .enabled_features(unsafe {
                        &instance.get_physical_device_features(*physical_device.upgrade().unwrap())
                    })
                    .enabled_extension_names(&device_extension_names)
                    .build();

                unsafe {
                    instance.create_device(
                        *physical_device.upgrade().unwrap(),
                        &create_info,
                        None,
                    )?
                }
            },
            instance,
            physical_device,
            queue_family_indices,
        })
    }

    pub fn instance(&self) -> &Rc<Instance> {
        &self.instance
    }

    pub fn physical_device(&self) -> &Weak<vk::PhysicalDevice> {
        &self.physical_device
    }

    pub fn queue_family_indices(&self) -> Vec<u32> {
        self.queue_family_indices
            .iter()
            .map(|info| info.index)
            .collect()
    }
}

impl Deref for Device {
    type Target = ash::Device;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.destroy_device(None);
        }
    }
}
