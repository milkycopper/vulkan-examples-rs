use ash::{prelude::VkResult, vk};

use super::Device;

pub struct OneTimeCommand<'a> {
    command_buffer: vk::CommandBuffer,
    device: &'a Device,
    pool: &'a vk::CommandPool,
}

impl<'a> OneTimeCommand<'a> {
    pub fn new(device: &'a Device, pool: &'a vk::CommandPool) -> VkResult<Self> {
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_buffer_count(1)
            .command_pool(*pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .build();

        let command_buffer =
            unsafe { device.allocate_command_buffers(&command_buffer_allocate_info)?[0] };

        Ok(Self {
            command_buffer,
            device,
            pool,
        })
    }

    pub fn command_buffer(&self) -> &vk::CommandBuffer {
        &self.command_buffer
    }

    pub fn begin(&self) -> VkResult<()> {
        unsafe {
            self.device.begin_command_buffer(
                self.command_buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                    .build(),
            )?;
        }
        Ok(())
    }

    pub fn new_and_begin(device: &'a Device, pool: &'a vk::CommandPool) -> VkResult<Self> {
        let command = Self::new(device, pool)?;
        command.begin()?;
        Ok(command)
    }

    pub fn end_and_submit(&self, queue: &vk::Queue) -> VkResult<()> {
        unsafe {
            self.device.end_command_buffer(self.command_buffer)?;
            let fence = self
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)?;
            self.device.queue_submit(
                *queue,
                &[vk::SubmitInfo::builder()
                    .command_buffers(&[self.command_buffer])
                    .build()],
                fence,
            )?;
            self.device.wait_for_fences(&[fence], true, u64::MAX)?;
            self.device.destroy_fence(fence, None);
        }

        Ok(())
    }

    pub fn take_and_execute<F>(&self, mut f: F, queue: &vk::Queue) -> VkResult<()>
    where
        F: FnMut(vk::CommandBuffer) -> VkResult<()>,
    {
        self.begin()?;
        f(self.command_buffer)?;
        self.end_and_submit(queue)?;
        Ok(())
    }
}

impl<'a> Drop for OneTimeCommand<'a> {
    fn drop(&mut self) {
        unsafe {
            self.device
                .free_command_buffers(*self.pool, &[self.command_buffer]);
        }
    }
}
