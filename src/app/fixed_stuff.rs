use std::rc::Rc;

use ash::{prelude::VkResult, vk};
use winit::window::Window;

use crate::{
    error::RenderResult,
    vulkan_objects::{
        format_helper, DepthStencilImageAndView, Device, Instance, QueueInfo, Surface,
        SwapChainBatch,
    },
};

pub struct FixedVulkanStuff {
    pub surface: Rc<Surface>,
    pub device: Rc<Device>,
    pub swapchain_batch: SwapChainBatch,
    pub swapchain_framebuffers: Vec<vk::Framebuffer>,
    pub command_pool: vk::CommandPool,
    pub command_buffers: [vk::CommandBuffer; Self::MAX_FRAMES_IN_FLIGHT],
    pub image_available_semaphores: [vk::Semaphore; Self::MAX_FRAMES_IN_FLIGHT],
    pub render_finished_semaphores: [vk::Semaphore; Self::MAX_FRAMES_IN_FLIGHT],
    pub in_flight_fences: [vk::Fence; Self::MAX_FRAMES_IN_FLIGHT],
    pub depth_image_and_view: DepthStencilImageAndView,
    pub render_pass: vk::RenderPass,
    pub depth_format: vk::Format,
}

impl FixedVulkanStuff {
    pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

    pub fn new(window: &Window, instance: Rc<Instance>) -> RenderResult<Self> {
        let surface = Rc::new(Surface::new(
            window,
            instance.clone(),
            Surface::DEFAULT_FORMAT,
        )?);
        let queue_info = QueueInfo::from_surface(&surface)?;
        let device = Rc::new(Device::new_with_queue_loaded(instance, queue_info)?);
        let swapchain_batch = SwapChainBatch::new(surface.clone(), device.clone())?;
        let command_pool = {
            let create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(device.graphic_queue_family_index())
                .build();
            unsafe { device.create_command_pool(&create_info, None)? }
        };
        let command_buffers: [_; Self::MAX_FRAMES_IN_FLIGHT] = {
            let allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
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
        let image_available_semaphores: [_; Self::MAX_FRAMES_IN_FLIGHT] =
            array_init::try_array_init(|_| unsafe {
                device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
            })?;

        let render_finished_semaphores: [_; Self::MAX_FRAMES_IN_FLIGHT] =
            array_init::try_array_init(|_| unsafe {
                device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
            })?;

        let in_flight_fences: [_; Self::MAX_FRAMES_IN_FLIGHT] =
            array_init::try_array_init(|_| unsafe {
                device.create_fence(
                    &vk::FenceCreateInfo::builder()
                        .flags(vk::FenceCreateFlags::SIGNALED)
                        .build(),
                    None,
                )
            })?;
        let depth_format = format_helper::find_depth_format(&device)?;
        let depth_image_and_view =
            DepthStencilImageAndView::new(surface.extent(), depth_format, device.clone())?;
        let render_pass = create_renderpass(surface.format(), depth_format, &device)?;
        let swapchain_framebuffers = create_swapchain_frame_buffer(
            &swapchain_batch,
            &render_pass,
            surface.extent(),
            &device,
            depth_image_and_view.image_view(),
        )?;

        Ok(Self {
            surface,
            device,
            swapchain_batch,
            command_pool,
            command_buffers,
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            depth_image_and_view,
            render_pass,
            swapchain_framebuffers,
            depth_format,
        })
    }

    pub fn recreate_swapchain(&mut self, window: &Window) -> RenderResult<()> {
        unsafe {
            self.device.device_wait_idle()?;
            self.surface.refit_surface_attribute(window)?;
            self.swapchain_batch.recreate()?;
            self.depth_image_and_view = DepthStencilImageAndView::new(
                self.surface.extent(),
                self.depth_format,
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
                self.depth_image_and_view.image_view(),
            )?;
            Ok(())
        }
    }
}

impl Drop for FixedVulkanStuff {
    fn drop(&mut self) {
        unsafe {
            [
                self.image_available_semaphores,
                self.render_finished_semaphores,
            ]
            .concat()
            .into_iter()
            .for_each(|sm| self.device.destroy_semaphore(sm, None));
            self.in_flight_fences
                .into_iter()
                .for_each(|fence| self.device.destroy_fence(fence, None));
            self.device.destroy_command_pool(self.command_pool, None);
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
