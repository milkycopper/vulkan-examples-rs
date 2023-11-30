use std::rc::Rc;

use ash::vk;
use glam::{Mat4, Vec3};

use crate::{
    error::RenderResult,
    vulkan_wrappers::{Buffer, Device},
};

use super::Material;

pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn center(&self) -> Vec3 {
        (self.max + self.min) / 2.
    }

    pub fn radius(&self) -> f32 {
        self.min.distance(self.max) / 2.
    }
}

impl Default for AABB {
    fn default() -> Self {
        AABB {
            min: Vec3::ZERO,
            max: Vec3::ZERO,
        }
    }
}

pub struct Primitive<'a> {
    pub first_index: u32,
    pub index_count: u32,
    pub first_vertex: u32,
    pub vertex_count: u32,
    pub material: &'a Material<'a>,
    pub aabb: AABB,
}

impl<'a> Primitive<'a> {
    pub fn new(first_index: u32, index_count: u32, material: &'a Material<'a>) -> Self {
        Self {
            first_index,
            index_count,
            first_vertex: 0,
            vertex_count: 0,
            material,
            aabb: AABB::default(),
        }
    }
}

pub struct MeshUniformBlock {
    pub matrix: Mat4,
    pub joint_matrix: [Mat4; 64],
    pub joint_count: f32,
}

pub struct Mesh<'a> {
    pub primitives: Vec<Primitive<'a>>,
    pub name: String,
    pub uniform_block: MeshUniformBlock,
    pub uniform_buffer: Buffer<MeshUniformBlock>,
}

impl<'a> Mesh<'a> {
    pub fn new(device: Rc<Device>, matrix: Mat4) -> RenderResult<Self> {
        let mut buffer = Buffer::<MeshUniformBlock>::new(
            1,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            device,
        )?;
        buffer.map_memory_all()?;

        Ok(Self {
            primitives: vec![],
            name: "".to_string(),
            uniform_block: MeshUniformBlock {
                matrix,
                joint_matrix: [Mat4::IDENTITY; 64],
                joint_count: 0.,
            },
            uniform_buffer: buffer,
        })
    }
}
