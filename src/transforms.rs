use std::rc::Rc;

use ash::vk;
use glam::Mat4;

use super::vulkan_objects::{Buffer, Device};
use crate::error::RenderResult;

#[repr(C, align(16))]
pub struct MVPMatrix {
    pub model: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
}

impl MVPMatrix {
    pub fn empty_uniform_buffer(device: Rc<Device>) -> RenderResult<Buffer<MVPMatrix>> {
        Buffer::<MVPMatrix>::new(
            1,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            device,
        )
    }
}

pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    Front,
    Back,
}
