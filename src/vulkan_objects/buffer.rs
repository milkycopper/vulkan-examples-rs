use std::{ffi::c_void, marker::PhantomData, rc::Rc};

use ash::{prelude::VkResult, vk};

use super::{Device, OneTimeCommand};
use crate::error::{RenderError, RenderResult};

pub struct Buffer<T> {
    size_in_bytes: vk::DeviceSize,
    buffer: vk::Buffer,
    device_momory: vk::DeviceMemory,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
    device: Rc<Device>,
    phantom: PhantomData<T>,
}

impl<T> Buffer<T> {
    pub fn new(
        element_num: usize,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
        device: Rc<Device>,
    ) -> RenderResult<Self> {
        unsafe {
            let size_in_bytes = element_num as vk::DeviceSize * Self::element_size_in_bytes();
            let create_info = vk::BufferCreateInfo::builder()
                .size(size_in_bytes)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .build();
            let buffer = device.create_buffer(&create_info, None)?;

            let memory_requirements = device.get_buffer_memory_requirements(buffer);
            let allocate_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(memory_requirements.size)
                .memory_type_index(memory_helper::find_memory_type(
                    &device,
                    &memory_requirements,
                    properties,
                )?)
                .build();
            let device_momory = device.allocate_memory(&allocate_info, None)?;

            device.bind_buffer_memory(buffer, device_momory, 0)?;

            Ok(Self {
                size_in_bytes,
                buffer,
                device_momory,
                usage,
                properties,
                device,
                phantom: PhantomData::<T>,
            })
        }
    }

    pub const fn element_size_in_bytes() -> vk::DeviceSize {
        std::mem::size_of::<T>() as vk::DeviceSize
    }

    pub fn element_num(&self) -> usize {
        (self.size_in_bytes / Self::element_size_in_bytes()) as usize
    }

    pub fn size_in_bytes(&self) -> vk::DeviceSize {
        self.size_in_bytes
    }

    pub fn vk_buffer(&self) -> vk::Buffer {
        self.buffer
    }

    pub fn vk_device_momory(&self) -> vk::DeviceMemory {
        self.device_momory
    }

    pub fn usage(&self) -> vk::BufferUsageFlags {
        self.usage
    }

    pub fn memory_property_flags(&self) -> vk::MemoryPropertyFlags {
        self.properties
    }

    pub fn copy_to<V>(
        &self,
        dst: &Buffer<V>,
        command_pool: &vk::CommandPool,
        queue: &vk::Queue,
    ) -> VkResult<()> {
        assert!(self.size_in_bytes == dst.size_in_bytes);

        OneTimeCommand::new(self.device.clone(), command_pool)?.take_and_execute(
            |command| unsafe {
                self.device.cmd_copy_buffer(
                    *command.vk_command_buffer(),
                    self.buffer,
                    dst.buffer,
                    &[vk::BufferCopy::builder().size(self.size_in_bytes).build()],
                );
                Ok(())
            },
            queue,
        )?;

        Ok(())
    }

    pub fn new_indice_buffer(
        indice_data: &Vec<T>,
        device: Rc<Device>,
        command_pool: &vk::CommandPool,
        queue: &vk::Queue,
    ) -> RenderResult<Self> {
        let indice_num = indice_data.len();

        let staging_buffer = Buffer::<T>::new(
            indice_num,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            device.clone(),
        )?;

        let indice_buffer = Buffer::<T>::new(
            indice_num,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
        )?;

        unsafe {
            let data_ptr = device.map_memory(
                staging_buffer.vk_device_momory(),
                0,
                staging_buffer.size_in_bytes(),
                vk::MemoryMapFlags::default(),
            )?;
            std::ptr::copy_nonoverlapping(indice_data.as_ptr(), data_ptr as *mut T, indice_num);
            device.unmap_memory(staging_buffer.vk_device_momory());
        }

        staging_buffer.copy_to(&indice_buffer, command_pool, queue)?;

        Ok(indice_buffer)
    }

    pub fn uniform_mapped_ptr(&self) -> VkResult<*mut c_void> {
        assert!(self.usage == vk::BufferUsageFlags::UNIFORM_BUFFER);
        Ok(unsafe {
            self.device.map_memory(
                self.device_momory,
                0,
                self.size_in_bytes,
                vk::MemoryMapFlags::default(),
            )?
        })
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.device_momory, None);
        }
    }
}

impl<T: core::fmt::Debug + bytemuck::Pod> core::fmt::Debug for Buffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe {
            let size = self.size_in_bytes;
            let data_ptr = self
                .device
                .map_memory(self.device_momory, 0, size, vk::MemoryMapFlags::default())
                .unwrap();
            let host_vec = vec![u8::default(); size as usize];
            std::ptr::copy_nonoverlapping(
                data_ptr as *const u8,
                host_vec.as_ptr() as *mut u8,
                size as usize,
            );
            self.device.unmap_memory(self.device_momory);
            let host_vec_casted = bytemuck::cast_slice::<u8, T>(&host_vec);
            write!(f, "{:?}", host_vec_casted)
        }
    }
}

pub mod memory_helper {
    use super::*;

    pub fn find_memory_type(
        device: &Device,
        requirement: &vk::MemoryRequirements,
        properties: vk::MemoryPropertyFlags,
    ) -> RenderResult<u32> {
        unsafe {
            let physical_mem_properties = device.instance().get_physical_device_memory_properties(
                *device.physical_device().upgrade().unwrap(),
            );
            for i in 0..physical_mem_properties.memory_type_count {
                if (requirement.memory_type_bits & (1 << i)) != 0
                    && (physical_mem_properties.memory_types[i as usize].property_flags
                        & properties)
                        == properties
                {
                    return Ok(i);
                }
            }
            Err(RenderError::MemoryTypeNotSupported(
                "Failed to find suitable memory type".to_string(),
            ))
        }
    }
}
