use std::{path::Path, rc::Rc};

use ash::vk;
use glam::{Vec2, Vec3};

use super::vulkan_objects::{Buffer, Device};
use crate::error::RenderResult;

#[repr(C)]
#[derive(Debug)]
pub struct Vertex {
    pos: Vec3,
    color: Vec3,
    texture_coord: Vec2,
}

impl Vertex {
    pub fn new(pos: Vec3) -> Self {
        Vertex {
            pos,
            color: Vec3::ONE,
            texture_coord: Vec2::ZERO,
        }
    }

    pub fn with_color(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    pub fn with_texture_coord(mut self, texture_coord: Vec2) -> Self {
        self.texture_coord = texture_coord;
        self
    }

    pub fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
            .build()
    }

    pub fn attr_descriptions() -> [vk::VertexInputAttributeDescription; 3] {
        [
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(memoffset::offset_of!(Vertex, pos) as u32)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(memoffset::offset_of!(Vertex, color) as u32)
                .build(),
            vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(2)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(memoffset::offset_of!(Vertex, texture_coord) as u32)
                .build(),
        ]
    }

    pub fn create_buffer(
        data: &Vec<Self>,
        device: Rc<Device>,
        command_pool: &vk::CommandPool,
        queue: &vk::Queue,
    ) -> RenderResult<Buffer<Self>> {
        let vertex_num = data.len();

        let staging_buffer = Buffer::<Self>::new(
            vertex_num,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            device.clone(),
        )?;

        let vertex_buffer = Buffer::<Self>::new(
            vertex_num,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
        )?;

        unsafe {
            let data_ptr = device.map_memory(
                staging_buffer.device_momory(),
                0,
                staging_buffer.size_in_bytes(),
                vk::MemoryMapFlags::empty(),
            )?;
            std::ptr::copy_nonoverlapping(data.as_ptr(), data_ptr as *mut Vertex, vertex_num);
            device.unmap_memory(staging_buffer.device_momory());
        }

        staging_buffer.copy_to(&vertex_buffer, command_pool, queue)?;

        Ok(vertex_buffer)
    }
}

// TODO: eliminate duplicated vertices
pub fn load_obj_model<P: AsRef<Path> + core::fmt::Debug>(
    path: P,
) -> RenderResult<(Vec<Vertex>, Vec<u32>)> {
    let load_options = tobj::LoadOptions::default();

    let (models, _) = tobj::load_obj(&path, &load_options)?;

    let mut vertices = vec![];
    let mut indices = vec![];

    for m in models.iter() {
        let vertex_indices_num = m.mesh.indices.len();
        for i in 0..vertex_indices_num {
            let vertex_index = m.mesh.indices[i];
            let texture_coord_index = m.mesh.texcoord_indices[i];
            vertices.push(
                Vertex::new(Vec3::new(
                    m.mesh.positions[3 * (vertex_index as usize)],
                    m.mesh.positions[3 * (vertex_index as usize) + 1],
                    -m.mesh.positions[3 * (vertex_index as usize) + 2],
                ))
                .with_texture_coord(Vec2::new(
                    m.mesh.texcoords[2 * (texture_coord_index as usize)],
                    1.0 - m.mesh.texcoords[2 * (texture_coord_index as usize) + 1],
                )),
            );
            indices.push(indices.len() as u32);
        }
    }

    Ok((vertices, indices))
}
