use std::rc::Rc;

use ash::{prelude::VkResult, vk};
use winit::window::Window;

use crate::{
    error::{RenderError, RenderResult},
    vulkan_objects::{
        format_helper, DepthStencil, Device, Instance, QueueInfo, Surface, SwapChainBatch,
    },
};

pub struct FrameSyncPrimitive {
    pub in_flight_fence: vk::Fence,
    pub image_available_semaphore: vk::Semaphore,
    pub render_finished_semaphore: vk::Semaphore,
}

pub struct FixedVulkanStuff {
    pub surface: Rc<Surface>,
    pub device: Rc<Device>,
    pub swapchain_batch: SwapChainBatch,
    pub swapchain_framebuffers: Vec<vk::Framebuffer>,
    pub graphic_command_pool: vk::CommandPool,
    pub graphic_command_buffers: [vk::CommandBuffer; Self::MAX_FRAMES_IN_FLIGHT],
    pub frame_sync_primitives: [FrameSyncPrimitive; Self::MAX_FRAMES_IN_FLIGHT],
    pub depth_stencil: DepthStencil,
    pub render_pass: vk::RenderPass,
}

impl FixedVulkanStuff {
    pub const MAX_FRAMES_IN_FLIGHT: usize = 2;
    pub const DEFAULT_SURFACE_FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;

    pub fn new(window: &Window, instance: Rc<Instance>) -> RenderResult<Self> {
        let surface = Rc::new(Surface::new(
            window,
            instance.clone(),
            Self::DEFAULT_SURFACE_FORMAT,
        )?);
        let device = Rc::new(Device::new(instance, QueueInfo::new(&surface)?)?);
        let swapchain_batch = SwapChainBatch::new(surface.clone(), device.clone())?;
        let graphic_command_pool = {
            let create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(device.graphic_queue_family_index())
                .build();
            unsafe { device.create_command_pool(&create_info, None)? }
        };
        let graphic_command_buffers: [_; Self::MAX_FRAMES_IN_FLIGHT] = {
            let allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(graphic_command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(Self::MAX_FRAMES_IN_FLIGHT as u32)
                .build();
            unsafe {
                device
                    .allocate_command_buffers(&allocate_info)?
                    .try_into()
                    .unwrap()
            }
        };
        let frame_sync_primitives: [_; Self::MAX_FRAMES_IN_FLIGHT] =
            array_init::try_array_init(|_| -> Result<_, vk::Result> {
                Ok(unsafe {
                    FrameSyncPrimitive {
                        in_flight_fence: device.create_fence(
                            &vk::FenceCreateInfo::builder()
                                .flags(vk::FenceCreateFlags::SIGNALED)
                                .build(),
                            None,
                        )?,
                        render_finished_semaphore: device
                            .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                        image_available_semaphore: device
                            .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                    }
                })
            })?;
        let depth_format = format_helper::find_depth_format(&device)?;
        let depth_stencil = DepthStencil::new(surface.extent(), depth_format, device.clone())?;
        let render_pass = create_renderpass(surface.format(), depth_stencil.format(), &device)?;
        let swapchain_framebuffers = create_swapchain_frame_buffer(
            &swapchain_batch,
            &render_pass,
            surface.extent(),
            &device,
            depth_stencil.image_view(),
        )?;

        Ok(Self {
            surface,
            device,
            swapchain_batch,
            graphic_command_pool,
            graphic_command_buffers,
            frame_sync_primitives,
            depth_stencil,
            render_pass,
            swapchain_framebuffers,
        })
    }

    pub fn recreate(&mut self, window: &Window) -> RenderResult<()> {
        unsafe {
            self.device.device_wait_idle()?;
            self.surface.refit_surface_attribute(window)?;
            self.swapchain_batch.recreate()?;
            self.depth_stencil = DepthStencil::new(
                self.surface.extent(),
                self.depth_stencil.format(),
                self.device.clone(),
            )?;
            self.swapchain_framebuffers
                .iter()
                .for_each(|fb| self.device.destroy_framebuffer(*fb, None));
            self.swapchain_framebuffers = create_swapchain_frame_buffer(
                &self.swapchain_batch,
                &self.render_pass,
                self.surface.extent(),
                &self.device,
                self.depth_stencil.image_view(),
            )?;
            Ok(())
        }
    }

    pub fn frame_wait_for_fence(&self, frame_index: usize) -> VkResult<()> {
        debug_assert!(frame_index < Self::MAX_FRAMES_IN_FLIGHT);
        unsafe {
            self.device.wait_for_fences(
                &[self.frame_sync_primitives[frame_index].in_flight_fence],
                true,
                u64::MAX,
            )
        }
    }

    pub fn frame_acquire_next_image(&self, frame_index: usize) -> VkResult<(u32, bool)> {
        debug_assert!(frame_index < Self::MAX_FRAMES_IN_FLIGHT);
        self.swapchain_batch
            .acquire_next_image(self.frame_sync_primitives[frame_index].image_available_semaphore)
    }

    pub fn frame_reset_fence(&self, frame_index: usize) -> VkResult<()> {
        debug_assert!(frame_index < Self::MAX_FRAMES_IN_FLIGHT);
        unsafe {
            self.device
                .reset_fences(&[self.frame_sync_primitives[frame_index].in_flight_fence])
        }
    }

    pub fn frame_draw_queue_submit(&self, frame_index: usize) -> VkResult<()> {
        debug_assert!(frame_index < Self::MAX_FRAMES_IN_FLIGHT);
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&[self.frame_sync_primitives[frame_index].image_available_semaphore])
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
            .command_buffers(&[self.graphic_command_buffers[frame_index]])
            .signal_semaphores(&[self.frame_sync_primitives[frame_index].render_finished_semaphore])
            .build();

        unsafe {
            self.device.queue_submit(
                self.device.graphic_queue(),
                &[submit_info],
                self.frame_sync_primitives[frame_index].in_flight_fence,
            )
        }
    }

    pub fn frame_queue_present(&self, frame_index: usize, image_index: u32) -> VkResult<bool> {
        debug_assert!(frame_index < Self::MAX_FRAMES_IN_FLIGHT);
        debug_assert!((image_index as usize) < self.swapchain_batch.images().len());
        self.swapchain_batch.queue_present(
            image_index,
            &[self.frame_sync_primitives[frame_index].render_finished_semaphore],
            &self.device.graphic_queue(),
        )
    }

    pub fn frame_get_image_index_to_draw(
        &mut self,
        frame_index: usize,
        window: &Window,
    ) -> RenderResult<(u32, bool)> {
        self.frame_wait_for_fence(frame_index)?;
        let result = self.frame_acquire_next_image(frame_index);
        match result {
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate(window)?;
                return Ok((u32::MAX, true));
            }
            Ok(_) => {}
            Err(e) => return Err(RenderError::VkResult(e)),
        }
        self.frame_reset_fence(frame_index)?;
        Ok((result?.0, false))
    }

    pub fn frame_queue_submit_and_present(
        &mut self,
        frame_index: usize,
        image_index: u32,
        window: &Window,
        window_resized: bool,
    ) -> RenderResult<bool> {
        self.frame_draw_queue_submit(frame_index)?;
        let result = self.frame_queue_present(frame_index, image_index);
        let need_recreate = match result {
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Ok(true) => true,
            Ok(_) => false,
            Err(e) => return Err(RenderError::VkResult(e)),
        };
        if need_recreate || window_resized {
            self.recreate(window)?;
        };

        Ok(false)
    }
}

impl Drop for FixedVulkanStuff {
    fn drop(&mut self) {
        unsafe {
            self.frame_sync_primitives.iter().for_each(|fsp| {
                self.device
                    .destroy_semaphore(fsp.image_available_semaphore, None);
                self.device
                    .destroy_semaphore(fsp.render_finished_semaphore, None);
                self.device.destroy_fence(fsp.in_flight_fence, None)
            });
            self.device
                .destroy_command_pool(self.graphic_command_pool, None);
            self.swapchain_framebuffers
                .iter()
                .for_each(|fb| self.device.destroy_framebuffer(*fb, None));
            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

fn create_swapchain_frame_buffer(
    swapchain_batch: &SwapChainBatch,
    render_pass: &vk::RenderPass,
    extent: vk::Extent2D,
    device: &Device,
    depth_image_view: &vk::ImageView,
) -> VkResult<Vec<vk::Framebuffer>> {
    swapchain_batch
        .image_views()
        .iter()
        .map(|image_view| {
            let framebuffer_create_info = vk::FramebufferCreateInfo::builder()
                .render_pass(*render_pass)
                .attachments(&[*image_view, *depth_image_view])
                .width(extent.width)
                .height(extent.height)
                .layers(1)
                .build();
            unsafe { device.create_framebuffer(&framebuffer_create_info, None) }
        })
        .collect()
}

fn create_renderpass(
    color_format: vk::Format,
    depth_format: vk::Format,
    device: &Device,
) -> VkResult<vk::RenderPass> {
    let color_attach = vk::AttachmentDescription::builder()
        .format(color_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .build();
    let depth_attach = vk::AttachmentDescription::builder()
        .format(depth_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::LOAD)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
        .build();
    let color_attach_ref = vk::AttachmentReference::builder()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .build();
    let depth_attach_ref = vk::AttachmentReference::builder()
        .attachment(1)
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let subpass_desc = vk::SubpassDescription::builder()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&[color_attach_ref])
        .depth_stencil_attachment(&depth_attach_ref)
        .build();
    let dependency_0 = vk::SubpassDependency::builder()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(
            vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .dst_stage_mask(
            vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        )
        .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
        .dst_access_mask(
            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        )
        .build();
    let dependency_1 = vk::SubpassDependency::builder()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::default())
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        )
        .build();
    let renderpass_create_info = vk::RenderPassCreateInfo::builder()
        .attachments(&[color_attach, depth_attach])
        .subpasses(&[subpass_desc])
        .dependencies(&[dependency_0, dependency_1])
        .build();
    Ok(unsafe { device.create_render_pass(&renderpass_create_info, None)? })
}
