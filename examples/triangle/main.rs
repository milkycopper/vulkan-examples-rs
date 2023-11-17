use std::{cell::RefCell, ffi::c_void, rc::Rc, time::SystemTime};

use ash::vk;
use glam::{vec3, Mat4, Vec3};
use winit::{
    dpi::PhysicalSize,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use vulkan_example_rs::{
    app::{FixedVulkanStuff, PipelineBuilder, WindowApp},
    camera::Camera,
    mesh::Vertex,
    transforms::MVPMatrix,
    vulkan_objects::{extent_helper, Buffer, Device, Surface},
    window_fns,
};

struct DrawTriangleApp {
    window: Window,
    window_resized: bool,

    current_frame: usize,
    last_time: SystemTime,

    camera: Camera,

    fixed_vulkan_stuff: FixedVulkanStuff,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: [vk::DescriptorSet; 2],
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    vertex_buffer: Buffer<Vertex>,
    indice_buffer: Buffer<u32>,
    uniform_buffers: [(Buffer<MVPMatrix>, *mut c_void); FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT],
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
        let descriptor_set_layout = {
            let ubo_layout_binding = vk::DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .stage_flags(vk::ShaderStageFlags::VERTEX)
                .descriptor_count(1)
                .build();
            let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
                .bindings(&[ubo_layout_binding])
                .build();

            unsafe {
                fixed_vulkan_stuff
                    .device
                    .create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
                    .unwrap()
            }
        };
        let descriptor_pool = unsafe {
            let create_info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(&[vk::DescriptorType::UNIFORM_BUFFER].map(|ty| {
                    vk::DescriptorPoolSize::builder()
                        .ty(ty)
                        .descriptor_count(FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT as u32)
                        .build()
                }))
                .max_sets(FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT as u32)
                .build();
            fixed_vulkan_stuff
                .device
                .create_descriptor_pool(&create_info, None)
                .unwrap()
        };
        let descriptor_sets: [vk::DescriptorSet; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT] = unsafe {
            let allocate_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&[descriptor_set_layout; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT])
                .build();
            fixed_vulkan_stuff
                .device
                .allocate_descriptor_sets(&allocate_info)
                .unwrap()
                .try_into()
                .unwrap()
        };

        let pipeline_creator = PipelineCreator {
            device: fixed_vulkan_stuff.device.clone(),
            surface: &fixed_vulkan_stuff.surface,
            render_pass: fixed_vulkan_stuff.render_pass,
            set_layouts: &[descriptor_set_layout],
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
            &vec![0, 1, 2, 1, 0, 2],
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
        .unwrap();

        let uniform_buffers: [_; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT] =
            array_init::array_init(|_| {
                let buffer =
                    MVPMatrix::empty_uniform_buffer(fixed_vulkan_stuff.device.clone()).unwrap();
                let ptr = buffer.uniform_mapped_ptr().unwrap();
                (buffer, ptr)
            });

        {
            for i in 0..FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT {
                let uniform_buffer_info = vk::DescriptorBufferInfo::builder()
                    .buffer(uniform_buffers[i].0.buffer())
                    .offset(0)
                    .range(std::mem::size_of::<MVPMatrix>() as u64)
                    .build();
                let uniform_descritptor_write = vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&[uniform_buffer_info])
                    .build();

                unsafe {
                    fixed_vulkan_stuff
                        .device
                        .update_descriptor_sets(&[uniform_descritptor_write], &[])
                }
            }
        }

        DrawTriangleApp {
            window,
            window_resized: false,
            fixed_vulkan_stuff,
            vertex_buffer,
            indice_buffer,
            uniform_buffers,
            pipeline_layout,
            pipeline,
            current_frame: 0,
            last_time: SystemTime::now(),
            camera: Camera::with_translation(Vec3::new(0., 0., -3.))
                .with_move_speed(100.)
                .with_rotate_speed(400.),
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
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

            self.update_uniform_buffer(&self.camera, &self.uniform_buffers[self.current_frame]);

            self.fixed_vulkan_stuff
                .device
                .reset_fences(&[self.fixed_vulkan_stuff.in_flight_fences[self.current_frame]])
                .unwrap();

            self.record_render_commands(
                self.fixed_vulkan_stuff.command_buffers[self.current_frame],
                self.fixed_vulkan_stuff.swapchain_framebuffers[image_index as usize],
                self.descriptor_sets[self.current_frame],
                6,
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
        self.last_time = SystemTime::now();
    }

    fn time_stamp(&self) -> SystemTime {
        self.last_time
    }

    fn camera(&mut self) -> &mut Camera {
        &mut self.camera
    }
}

impl DrawTriangleApp {
    fn update_uniform_buffer(
        &self,
        camera: &Camera,
        uniform_buffer: &(Buffer<MVPMatrix>, *mut c_void),
    ) {
        let mvp_matrix = MVPMatrix {
            model: Mat4::IDENTITY,
            view: camera.view_mat(),
            projection: camera.perspective_mat(),
        };

        unsafe {
            std::ptr::copy_nonoverlapping(
                &mvp_matrix as *const MVPMatrix,
                uniform_buffer.1 as *mut MVPMatrix,
                1,
            )
        }
    }

    fn record_render_commands(
        &self,
        command_buffer: vk::CommandBuffer,
        frame_buffer: vk::Framebuffer,
        descriptor_set: vk::DescriptorSet,
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

            self.fixed_vulkan_stuff.device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[descriptor_set],
                &[],
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
            self.fixed_vulkan_stuff
                .device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.fixed_vulkan_stuff
                .device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        }
    }
}

struct PipelineCreator<'a> {
    device: Rc<Device>,
    surface: &'a Surface,
    render_pass: vk::RenderPass,
    set_layouts: &'a [vk::DescriptorSetLayout],
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
            .set_layouts(self.set_layouts)
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
