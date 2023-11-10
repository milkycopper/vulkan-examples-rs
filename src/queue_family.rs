use ash::{prelude::VkResult, vk};

use crate::vulkan_objects::Surface;

#[derive(Default, Clone, Copy)]
pub struct QueueFamilyIndices {
    pub graphic_family: Option<u32>,
    pub present_family: Option<u32>,
}

impl QueueFamilyIndices {
    pub fn is_ready(&self) -> bool {
        self.graphic_family.is_some() && self.present_family.is_some()
    }

    pub fn merge(&self) -> Option<Vec<u32>> {
        let mut ret = std::collections::HashSet::new();
        if self.is_ready() {
            ret.insert(self.graphic_family.unwrap());
            ret.insert(self.present_family.unwrap());
            Some(ret.iter().copied().collect())
        } else {
            None
        }
    }

    pub fn from_surface(surface: &Surface) -> VkResult<Self> {
        let mut family_indices = QueueFamilyIndices::default();
        let physical_device = surface.physical_device().upgrade().unwrap();

        let family_properties = unsafe {
            surface
                .instance()
                .get_physical_device_queue_family_properties(*physical_device)
        };

        for (index, fp) in family_properties.iter().enumerate() {
            if !(fp.queue_flags | vk::QueueFlags::GRAPHICS).is_empty() {
                family_indices.graphic_family = Some(index as u32);
            }
            let support_surface = unsafe {
                surface.loader().get_physical_device_surface_support(
                    *physical_device,
                    index as u32,
                    *surface.surface_khr(),
                )?
            };
            if support_surface {
                family_indices.present_family = Some(index as u32);
            }
            if family_indices.is_ready() {
                break;
            }
        }

        Ok(family_indices)
    }
}
