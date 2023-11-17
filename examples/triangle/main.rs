use std::{cell::RefCell, rc::Rc};

use ash::vk;
use glam::vec3;
use winit::{
    dpi::PhysicalSize,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use vulkan_example_rs::{
    app::{FixedVulkanStuff, PipelineBuilder, WindowApp},
    mesh::Vertex,
    vulkan_objects::{extent_helper, Buffer, Device, Surface},
    window_fns,
};

struct DrawTriangleApp {
    window: Window,
    window_resized: bool,
    fixed_vulkan_stuff: FixedVulkanStuff,
    vertex_buffer: Buffer<Vertex>,
    indice_buffer: Buffer<u32>,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    current_frame: usize,
}

impl WindowApp for DrawTriangleApp {
    window_fns!(DrawTriangleApp);

    fn new(event_loop: &winit::event_loop::EventLoop<()>) -> Self {
        let window = WindowBuilder::new()
            .with_title(Self::window_title())
            .with_inner_size(PhysicalSize::new(1800, 1200))
            .build(event_loop)
            .unwrap();
        let model_vertices = vec![vec3(-0.5, -0.5, 0.), vec3(0.5, -0.5, 0.), vec3(0., 0.5, 0.)]
            .into_iter()
            .map(|v3| Vertex::new(v3).with_color(v3 + vec3(0.5, 0.4, 0.3)))
            .collect();

        let fixed_vulkan_stuff = Self::create_fixed_vulkan_stuff(&window).unwrap();
        let pipeline_creator = PipelineCreator {
            device: fixed_vulkan_stuff.device.clone(),
            surface: &fixed_vulkan_stuff.surface,
            render_pass: fixed_vulkan_stuff.render_pass,
            vertex_bindings: &[Vertex::binding_description()],
            vertex_attributes: &Vertex::attr_descriptions(),
        };
        let (pipeline_layout, pipeline) = pipeline_creator.build().unwrap();
        let vertex_buffer = Vertex::create_buffer(
            &model_vertices,
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
        .unwrap();

        let indice_buffer = Buffer::new_indice_buffer(
            &vec![1, 0, 2],
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
        .unwrap();

        DrawTriangleApp {
            window,
            window_resized: false,
            fixed_vulkan_stuff,
            vertex_buffer,
            indice_buffer,
            pipeline_layout,
            pipeline,
            current_frame: 0,
        }
    }

    fn draw_frame(&mut self) {
        unsafe {
            self.fixed_vulkan_stuff
                .device
                .wait_for_fences(
                    &[self.fixed_vulkan_stuff.in_flight_fences[self.current_frame]],
                    true,
                    u64::MAX,
                )
                .unwrap();

            let result = self.fixed_vulkan_stuff.swapchain_batch.acquire_next_image(
                self.fixed_vulkan_stuff.image_available_semaphores[self.current_frame],
            );

            match result {
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.fixed_vulkan_stuff
                        .recreate_swapchain(&self.window)
                        .unwrap();
                    return;
                }
                Ok(_) => {}
                _ => panic!("Failed to acquire swap chain image"),
            }

            let image_index = result.unwrap().0;

            self.fixed_vulkan_stuff
                .device
                .reset_fences(&[self.fixed_vulkan_stuff.in_flight_fences[self.current_frame]])
                .unwrap();

            self.record_render_commands(
                self.fixed_vulkan_stuff.command_buffers[self.current_frame],
                self.fixed_vulkan_stuff.swapchain_framebuffers[image_index as usize],
                3,
            );

            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&[
                    self.fixed_vulkan_stuff.image_available_semaphores[self.current_frame]
                ])
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .command_buffers(&[self.fixed_vulkan_stuff.command_buffers[self.current_frame]])
                .signal_semaphores(&[
                    self.fixed_vulkan_stuff.render_finished_semaphores[self.current_frame]
                ])
                .build();

            self.fixed_vulkan_stuff
                .device
                .queue_submit(
                    self.fixed_vulkan_stuff.device.graphic_queue(),
                    &[submit_info],
                    self.fixed_vulkan_stuff.in_flight_fences[self.current_frame],
                )
                .unwrap();

            let result = self.fixed_vulkan_stuff.swapchain_batch.queue_present(
                image_index,
                &[self.fixed_vulkan_stuff.render_finished_semaphores[self.current_frame]],
                &self.fixed_vulkan_stuff.device.graphic_queue(),
            );

            let need_recreate = match result {
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Ok(true) => true,
                Ok(_) => false,
                _ => panic!("Failed to present swap chain image"),
            };
            if need_recreate || self.window_resized {
                self.window_resized = false;
                self.fixed_vulkan_stuff
                    .recreate_swapchain(&self.window)
                    .unwrap();
            }
        };

        self.current_frame = (self.current_frame + 1) % FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT;
    }
}

impl DrawTriangleApp {
    fn record_render_commands(
        &self,
        command_buffer: vk::CommandBuffer,
        frame_buffer: vk::Framebuffer,
        indice_num: u32,
    ) {
        unsafe {
            self.fixed_vulkan_stuff
                .device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::default())
                .expect("Fail to reset command buffer");

            self.fixed_vulkan_stuff
                .device
                .begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::default())
                .expect("Fail to begin command buffer");

            self.fixed_vulkan_stuff.device.cmd_begin_render_pass(
                command_buffer,
                &vk::RenderPassBeginInfo::builder()
                    .render_pass(self.fixed_vulkan_stuff.render_pass)
                    .framebuffer(frame_buffer)
                    .render_area(
                        vk::Rect2D::builder()
                            .offset(vk::Offset2D { x: 0, y: 0 })
                            .extent(self.fixed_vulkan_stuff.surface.extent())
                            .build(),
                    )
                    .clear_values(&Self::clear_value().to_array())
                    .build(),
                vk::SubpassContents::INLINE,
            );

            self.fixed_vulkan_stuff.device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );

            self.fixed_vulkan_stuff.device.cmd_bind_vertex_buffers(
                command_buffer,
                0,
                &[self.vertex_buffer.buffer()],
                &[0],
            );
            self.fixed_vulkan_stuff.device.cmd_bind_index_buffer(
                command_buffer,
                self.indice_buffer.buffer(),
                0,
                vk::IndexType::UINT32,
            );

            self.fixed_vulkan_stuff.device.cmd_set_viewport(
                command_buffer,
                0,
                &[extent_helper::viewport_from_extent(
                    self.fixed_vulkan_stuff.surface.extent(),
                )],
            );

            self.fixed_vulkan_stuff.device.cmd_set_scissor(
                command_buffer,
                0,
                &[extent_helper::scissor_from_extent(
                    self.fixed_vulkan_stuff.surface.extent(),
                )],
            );

            self.fixed_vulkan_stuff
                .device
                .cmd_draw_indexed(command_buffer, indice_num, 1, 0, 0, 0);

            self.fixed_vulkan_stuff
                .device
                .cmd_end_render_pass(command_buffer);
            self.fixed_vulkan_stuff
                .device
                .end_command_buffer(command_buffer)
                .unwrap();
        }
    }
}

impl Drop for DrawTriangleApp {
    fn drop(&mut self) {
        unsafe {
            self.fixed_vulkan_stuff.device.device_wait_idle().unwrap();
            self.fixed_vulkan_stuff
                .device
                .destroy_pipeline(self.pipeline, None);
            self.fixed_vulkan_stuff
                .device
                .destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}

struct PipelineCreator<'a> {
    device: Rc<Device>,
    surface: &'a Surface,
    render_pass: vk::RenderPass,
    vertex_bindings: &'a [vk::VertexInputBindingDescription],
    vertex_attributes: &'a [vk::VertexInputAttributeDescription],
}

impl<'a> PipelineBuilder<'a, String> for PipelineCreator<'a> {
    fn device(&self) -> Rc<Device> {
        self.device.clone()
    }
    fn vertex_spv_path(&self) -> String {
        "examples/shaders/triangle/shader.vert.spv".to_string()
    }

    fn frag_spv_path(&self) -> String {
        "examples/shaders/triangle/shader.frag.spv".to_string()
    }

    fn extent(&self) -> vk::Extent2D {
        self.surface.extent()
    }

    fn render_pass(&self) -> vk::RenderPass {
        self.render_pass
    }

    fn subpass(&self) -> u32 {
        0
    }

    fn pipeline_layout(&self) -> vk::PipelineLayout {
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&[])
            .push_constant_ranges(&[])
            .build();
        unsafe {
            self.device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .unwrap()
        }
    }

    fn vertex_binding_descriptions(&self) -> &'a [vk::VertexInputBindingDescription] {
        self.vertex_bindings
    }

    fn vertex_attribute_descriptions(&self) -> &'a [vk::VertexInputAttributeDescription] {
        self.vertex_attributes
    }
}

fn main() {
    let mut event_loop = RefCell::new(EventLoop::new());
    let mut app = DrawTriangleApp::new(&event_loop.borrow());
    app.run(&mut event_loop);
}
