use ash::vk;

use super::Surface;
use crate::error::{RenderError, RenderResult};

#[derive(Default, Clone, Copy)]
pub struct QueueInfo {
    pub graphic_family_index_priority: (u32, f32),
    pub present_family_index_priority: (u32, f32),
}

#[derive(Default, Clone, Copy)]
pub struct QueueState {
    pub info: QueueInfo,
    pub graphic_queue: vk::Queue,
    pub present_queue: vk::Queue,
}

impl QueueInfo {
    pub fn new(surface: &Surface) -> RenderResult<Self> {
        let mut queue_info = QueueInfo::default();
        let mut graphic_ok = false;
        let mut present_ok = false;

        let physical_device = surface.physical_device().upgrade().unwrap();

        let family_properties = unsafe {
            surface
                .instance()
                .get_physical_device_queue_family_properties(*physical_device)
        };

        for (index, fp) in family_properties.iter().enumerate() {
            if !(fp.queue_flags | vk::QueueFlags::GRAPHICS).is_empty() {
                queue_info.graphic_family_index_priority = (index as u32, 1.0);
                graphic_ok = true;
            }
            let support_surface = unsafe {
                surface.loader().get_physical_device_surface_support(
                    *physical_device,
                    index as u32,
                    *surface.surface_khr(),
                )?
            };
            if support_surface {
                queue_info.present_family_index_priority = (index as u32, 1.0);
                present_ok = true;
            }

            if graphic_ok && present_ok {
                break;
            }
        }

        if graphic_ok && present_ok {
            Ok(queue_info)
        } else {
            Err(RenderError::QueueFamilyNotSupported(
                "Fail to find suitable queue families".to_string(),
            ))
        }
    }

    pub fn merge_queue_family_index_and_priority(&self) -> Vec<(u32, f32)> {
        let mut ret = std::collections::HashMap::new();
        [
            self.graphic_family_index_priority,
            self.present_family_index_priority,
        ]
        .iter()
        .for_each(|x| {
            ret.insert(x.0, x.1);
        });

        ret.drain().collect()
    }
}
