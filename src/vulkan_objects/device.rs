use std::{
    ops::Deref,
    rc::{Rc, Weak},
};

use ash::{prelude::VkResult, vk};

use super::{Instance, QueueInfo, QueueState};

pub struct Device {
    inner: ash::Device,
    instance: Rc<Instance>,
    physical_device: Weak<vk::PhysicalDevice>,
    queue_state: QueueState,
}

impl Device {
    pub fn new(instance: Rc<Instance>, queue_info: QueueInfo) -> VkResult<Self> {
        let physical_device = instance.pick_physical_device();
        let inner = {
            let queue_infos = queue_info.merge_queue_family_index_and_priority();
            let indexs = queue_infos.iter().map(|x| x.0).collect::<Vec<_>>();
            let priorities = queue_infos.iter().map(|x| x.1).collect::<Vec<_>>();
            let queue_create_infos = indexs
                .into_iter()
                .enumerate()
                .map(|(i, index)| {
                    vk::DeviceQueueCreateInfo::builder()
                        .queue_family_index(index)
                        .queue_priorities(&priorities[i..i + 1])
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
                instance.create_device(*physical_device.upgrade().unwrap(), &create_info, None)?
            }
        };
        let queue_state = unsafe {
            QueueState {
                info: queue_info,
                graphic_queue: inner
                    .get_device_queue(queue_info.graphic_family_index_priority.0, 0),
                present_queue: inner
                    .get_device_queue(queue_info.present_family_index_priority.0, 0),
            }
        };

        Ok(Self {
            inner,
            instance,
            physical_device,
            queue_state,
        })
    }

    pub fn instance(&self) -> &Rc<Instance> {
        &self.instance
    }

    pub fn physical_device(&self) -> &Weak<vk::PhysicalDevice> {
        &self.physical_device
    }

    pub fn queue_family_indices(&self) -> Vec<u32> {
        self.queue_state
            .info
            .merge_queue_family_index_and_priority()
            .iter()
            .map(|x| x.0)
            .collect()
    }

    pub fn queue_state(&self) -> &QueueState {
        &self.queue_state
    }

    pub fn graphic_queue(&self) -> vk::Queue {
        self.queue_state.graphic_queue
    }

    pub fn present_queue(&self) -> vk::Queue {
        self.queue_state.present_queue
    }

    pub fn graphic_queue_family_index(&self) -> u32 {
        self.queue_state.info.graphic_family_index_priority.0
    }

    pub fn present_queue_family_index(&self) -> u32 {
        self.queue_state.info.present_family_index_priority.0
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
