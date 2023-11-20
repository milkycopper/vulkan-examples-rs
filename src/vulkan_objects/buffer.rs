use std::{ffi::c_void, marker::PhantomData, rc::Rc};

use ash::{prelude::VkResult, vk};

use super::{Device, OneTimeCommand};
use crate::error::{RenderError, RenderResult};

pub struct Buffer<T> {
    buffer: vk::Buffer,
    device_momory: vk::DeviceMemory,
    size_in_bytes: vk::DeviceSize,
    alignment: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
    mapped_ptr: Option<*mut c_void>,
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
                buffer,
                device_momory,
                size_in_bytes,
                alignment: memory_requirements.alignment,
                usage,
                properties,
                mapped_ptr: None,
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

    pub fn buffer(&self) -> vk::Buffer {
        self.buffer
    }

    pub fn device_momory(&self) -> vk::DeviceMemory {
        self.device_momory
    }

    pub fn usage(&self) -> vk::BufferUsageFlags {
        self.usage
    }

    pub fn memory_property_flags(&self) -> vk::MemoryPropertyFlags {
        self.properties
    }

    pub fn alignment(&self) -> vk::DeviceSize {
        self.alignment
    }

    pub fn mapped(&self) -> bool {
        self.mapped_ptr.is_some()
    }

    pub fn descriptor(
        &self,
        offset: vk::DeviceSize,
        range: vk::DeviceSize,
    ) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo::builder()
            .buffer(self.buffer)
            .offset(offset)
            .range(range)
            .build()
    }

    pub fn descriptor_default(&self) -> vk::DescriptorBufferInfo {
        self.descriptor(0, self.size_in_bytes)
    }

    pub fn map_memory(
        &mut self,
        offset: vk::DeviceSize,
        size_in_bytes: vk::DeviceSize,
    ) -> VkResult<*mut c_void> {
        assert!(!self.mapped());
        assert!(offset + size_in_bytes <= self.size_in_bytes);
        unsafe {
            let ptr = self.device.map_memory(
                self.device_momory,
                offset,
                size_in_bytes,
                vk::MemoryMapFlags::default(),
            )?;
            self.mapped_ptr = Some(ptr);
            Ok(ptr)
        }
    }

    pub fn map_memory_all(&mut self) -> VkResult<*mut c_void> {
        self.map_memory(0, self.size_in_bytes)
    }

    pub fn unmap_memory(&mut self) {
        assert!(self.mapped());
        unsafe { self.device.unmap_memory(self.device_momory) };
        self.mapped_ptr.take();
    }

    pub fn load_data<D>(&mut self, data: &[D], offset: vk::DeviceSize) -> VkResult<()> {
        debug_assert!(offset % self.alignment == 0);
        let data_size = std::mem::size_of_val(data) as vk::DeviceSize;
        assert!(data_size + offset <= self.size_in_bytes);
        unsafe {
            let mapped_ptr = self.map_memory(offset, data_size)?;
            std::ptr::copy_nonoverlapping(data.as_ptr(), mapped_ptr as *mut D, data.len());
            self.unmap_memory();
        }
        Ok(())
    }

    pub fn copy_to<V>(
        &self,
        dst: &Buffer<V>,
        command_pool: &vk::CommandPool,
        queue: &vk::Queue,
    ) -> VkResult<()> {
        assert!(self.size_in_bytes == dst.size_in_bytes);

        OneTimeCommand::new(&self.device, command_pool)?.take_and_execute(
            |command| unsafe {
                self.device.cmd_copy_buffer(
                    *command.command_buffer(),
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

    pub fn new_device_local(
        data: &[T],
        device: Rc<Device>,
        command_pool: &vk::CommandPool,
        queue: &vk::Queue,
    ) -> RenderResult<Self> {
        let element_num = data.len();

        let staging_buffer = {
            let mut buffer = Buffer::<T>::new(
                element_num,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                device.clone(),
            )?;
            buffer.load_data(data, 0)?;
            buffer
        };

        let device_local_buffer = Buffer::<T>::new(
            element_num,
            vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
        )?;
        staging_buffer.copy_to(&device_local_buffer, command_pool, queue)?;

        Ok(device_local_buffer)
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
