use std::{cell::RefCell, ffi::c_void, rc::Rc, time::SystemTime};

use ash::vk;
use glam::{Mat4, Vec3};
use winit::{
    dpi::PhysicalSize,
    event::VirtualKeyCode,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use vulkan_example_rs::{
    app::{FixedVulkanStuff, WindowApp},
    camera::Camera,
    mesh::Vertex,
    transforms::{Direction, MVPMatrix},
    vulkan_objects::{
        extent_helper, image_helper, Buffer, Device, ImageBuffer, InstanceBuilder, ShaderCreate,
        VulkanApiVersion,
    },
};

struct DrawTriangleApp {
    window: Window,
    window_resized: bool,

    current_frame: usize,
    last_time: SystemTime,

    model_vertices: Vec<Vertex>,
    model_indices: Vec<u32>,

    camera: Camera,

    vulkan_objs: VulkanObjects,
}

impl WindowApp for DrawTriangleApp {
    fn draw_frame(&mut self) {
        unsafe {
            let vk_objs = &mut self.vulkan_objs;

            vk_objs
                .fixed_vulkan_stuff
                .device
                .wait_for_fences(
                    &[vk_objs.fixed_vulkan_stuff.in_flight_fences[self.current_frame]],
                    true,
                    u64::MAX,
                )
                .unwrap();

            let result = vk_objs
                .fixed_vulkan_stuff
                .swapchain_batch
                .acquire_next_image(
                    vk_objs.fixed_vulkan_stuff.image_available_semaphores[self.current_frame],
                );

            match result {
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    vk_objs
                        .fixed_vulkan_stuff
                        .recreate_swapchain(&self.window)
                        .unwrap();
                    return;
                }
                Ok(_) => {}
                _ => panic!("Failed to acquire swap chain image"),
            }

            let image_index = result.unwrap().0;

            vk_objs
                .update_uniform_buffer(&self.camera, &vk_objs.uniform_buffers[self.current_frame]);

            vk_objs
                .fixed_vulkan_stuff
                .device
                .reset_fences(&[vk_objs.fixed_vulkan_stuff.in_flight_fences[self.current_frame]])
                .unwrap();

            vk_objs.record_render_commands(
                vk_objs.fixed_vulkan_stuff.command_buffers[self.current_frame],
                vk_objs.fixed_vulkan_stuff.swapchain_framebuffers[image_index as usize],
                vk_objs.descriptor_sets[self.current_frame],
                self.model_vertices.len() as u32,
                self.model_indices.len() as u32,
            );

            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&[
                    vk_objs.fixed_vulkan_stuff.image_available_semaphores[self.current_frame]
                ])
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .command_buffers(&[vk_objs.fixed_vulkan_stuff.command_buffers[self.current_frame]])
                .signal_semaphores(&[
                    vk_objs.fixed_vulkan_stuff.render_finished_semaphores[self.current_frame]
                ])
                .build();

            vk_objs
                .fixed_vulkan_stuff
                .device
                .queue_submit(
                    vk_objs.fixed_vulkan_stuff.device.graphic_queue(),
                    &[submit_info],
                    vk_objs.fixed_vulkan_stuff.in_flight_fences[self.current_frame],
                )
                .unwrap();

            let result = vk_objs.fixed_vulkan_stuff.swapchain_batch.queue_present(
                image_index,
                &[vk_objs.fixed_vulkan_stuff.render_finished_semaphores[self.current_frame]],
                &vk_objs.fixed_vulkan_stuff.device.graphic_queue(),
            );

            let need_recreate = match result {
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Ok(true) => true,
                Ok(_) => false,
                _ => panic!("Failed to present swap chain image"),
            };
            if need_recreate || self.window_resized {
                self.window_resized = false;
                vk_objs
                    .fixed_vulkan_stuff
                    .recreate_swapchain(&self.window)
                    .unwrap();
            }
        };

        self.current_frame = (self.current_frame + 1) % FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT;
        self.last_time = SystemTime::now();
    }

    fn window_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    fn on_window_resized(&mut self, _size: PhysicalSize<u32>) {
        self.window_resized = true;
    }

    fn on_keyboard_input(&mut self, key_code: VirtualKeyCode) {
        let duration = SystemTime::now()
            .duration_since(self.last_time)
            .unwrap()
            .as_secs_f32();
        match key_code {
            VirtualKeyCode::W => self.camera.translate_in_time(Direction::Up, duration),
            VirtualKeyCode::S => self.camera.translate_in_time(Direction::Down, duration),
            VirtualKeyCode::A => self.camera.translate_in_time(Direction::Left, duration),
            VirtualKeyCode::D => self.camera.translate_in_time(Direction::Right, duration),
            VirtualKeyCode::Q => self.camera.translate_in_time(Direction::Front, duration),
            VirtualKeyCode::E => self.camera.translate_in_time(Direction::Back, duration),
            VirtualKeyCode::Up => self.camera.rotate_in_time(Direction::Up, duration),
            VirtualKeyCode::Down => self.camera.rotate_in_time(Direction::Down, duration),
            VirtualKeyCode::Left => self.camera.rotate_in_time(Direction::Left, duration),
            VirtualKeyCode::Right => self.camera.rotate_in_time(Direction::Right, duration),
            _ => {}
        }
    }

    fn new(event_loop: &EventLoop<()>) -> Self {
        let window = WindowBuilder::new()
            .with_title(stringify!(DrawTriangleApp))
            .with_inner_size(PhysicalSize::new(1800, 1200))
            .build(event_loop)
            .unwrap();

        let (model_vertices, model_indices) =
            vulkan_example_rs::mesh::load_obj_model("examples/meshes/triangle/viking_room.obj")
                .unwrap();

        let vulkan_objs = VulkanObjects::new(
            stringify!(DrawTriangleApp),
            "No Engine",
            &window,
            &model_vertices,
            &model_indices,
        );

        DrawTriangleApp {
            window,
            window_resized: false,

            current_frame: 0,
            last_time: SystemTime::now(),

            model_vertices,
            model_indices,

            camera: Camera::default()
                .with_move_speed(100.)
                .with_rotate_speed(400.),

            vulkan_objs,
        }
    }

    fn run(&mut self, event_loop: &mut RefCell<EventLoop<()>>) {
        env_logger::init();
        self.render_loop(event_loop);
        unsafe {
            self.vulkan_objs
                .fixed_vulkan_stuff
                .device
                .device_wait_idle()
                .unwrap()
        };
    }
}

struct VulkanObjects {
    fixed_vulkan_stuff: FixedVulkanStuff,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: [vk::DescriptorSet; 2],
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    vertex_buffer: Buffer<Vertex>,
    indice_buffer: Buffer<u32>,
    uniform_buffers: [(Buffer<MVPMatrix>, *mut c_void); FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT],
    #[allow(dead_code)]
    texture_image: ImageBuffer,
    texture_image_view: vk::ImageView,
    texture_image_sampler: vk::Sampler,
}

impl VulkanObjects {
    pub fn new(
        app_name: &str,
        engine_name: &str,
        window: &Window,
        vertices: &Vec<Vertex>,
        indices: &Vec<u32>,
    ) -> Self {
        let instance = Rc::new(
            InstanceBuilder::new(window)
                .with_app_name_and_version(app_name, 0)
                .with_engine_name_and_version(engine_name, 0)
                .with_vulkan_api_version(VulkanApiVersion::V1_0)
                .enable_validation_layer()
                .build()
                .unwrap(),
        );
        let fixed_vulkan_stuff = FixedVulkanStuff::new(window, instance).unwrap();

        let descriptor_set_layout =
            helper::create_descriptor_set_layout(&fixed_vulkan_stuff.device);

        let descriptor_pool = unsafe {
            let create_info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(
                    &[
                        vk::DescriptorType::UNIFORM_BUFFER,
                        vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    ]
                    .map(|ty| {
                        vk::DescriptorPoolSize::builder()
                            .ty(ty)
                            .descriptor_count(FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT as u32)
                            .build()
                    }),
                )
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

        let (pipeline_layout, pipeline) = helper::create_pipeline(
            fixed_vulkan_stuff.device.clone(),
            fixed_vulkan_stuff.surface.extent(),
            &fixed_vulkan_stuff.render_pass,
            &[descriptor_set_layout],
        );

        let vertex_buffer = Vertex::create_buffer(
            vertices,
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
        .unwrap();

        let indice_buffer = Buffer::new_indice_buffer(
            indices,
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

        let texture_image = ImageBuffer::color_image_from_file(
            "examples/textures/triangle/viking_room.png",
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
        .unwrap();
        let texture_image_sampler =
            image_helper::create_texture_sampler(&fixed_vulkan_stuff.device).unwrap();
        let texture_image_view = texture_image
            .create_image_view(vk::Format::R8G8B8A8_SRGB, vk::ImageAspectFlags::COLOR)
            .unwrap();

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

                let image_buffer_info = vk::DescriptorImageInfo::builder()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(texture_image_view)
                    .sampler(texture_image_sampler)
                    .build();
                let image_descritptor_write = vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(1)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[image_buffer_info])
                    .build();

                unsafe {
                    fixed_vulkan_stuff.device.update_descriptor_sets(
                        &[uniform_descritptor_write, image_descritptor_write],
                        &[],
                    )
                }
            }
        }

        VulkanObjects {
            fixed_vulkan_stuff,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
            pipeline_layout,
            pipeline,
            vertex_buffer,
            indice_buffer,
            uniform_buffers,
            texture_image,
            texture_image_view,
            texture_image_sampler,
        }
    }

    pub fn update_uniform_buffer(
        &self,
        camera: &Camera,
        uniform_buffer: &(Buffer<MVPMatrix>, *mut c_void),
    ) {
        let mvp_matrix = MVPMatrix {
            model: Mat4::from_axis_angle(Vec3::X, 2.3 * core::f32::consts::FRAC_PI_2)
                * Mat4::from_axis_angle(Vec3::Y, -0.46 * core::f32::consts::FRAC_PI_2)
                * Mat4::from_axis_angle(Vec3::Z, -1.1 * core::f32::consts::FRAC_PI_2),
            view: camera.view_transform(),
            projection: camera.projection_transform(),
        };

        unsafe {
            std::ptr::copy_nonoverlapping(
                &mvp_matrix as *const MVPMatrix,
                uniform_buffer.1 as *mut MVPMatrix,
                1,
            )
        }
    }

    pub fn record_render_commands(
        &self,
        command_buffer: vk::CommandBuffer,
        frame_buffer: vk::Framebuffer,
        descriptor_set: vk::DescriptorSet,
        _vertex_num: u32,
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
                &helper::create_renderpass_begin_info(
                    &self.fixed_vulkan_stuff.render_pass,
                    &frame_buffer,
                    self.fixed_vulkan_stuff.surface.extent(),
                ),
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

impl Drop for VulkanObjects {
    fn drop(&mut self) {
        unsafe {
            self.fixed_vulkan_stuff
                .device
                .destroy_image_view(self.texture_image_view, None);
            self.fixed_vulkan_stuff
                .device
                .destroy_sampler(self.texture_image_sampler, None);
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

mod helper {
    use super::*;

    pub(super) fn create_pipeline(
        device: Rc<Device>,
        swapchain_extent: vk::Extent2D,
        renderpass: &vk::RenderPass,
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
    ) -> (vk::PipelineLayout, vk::Pipeline) {
        let shader_creates = [
            ShaderCreate::with_spv_path(
                "examples/shaders/triangle/shader.vert.spv",
                vk::ShaderStageFlags::VERTEX,
                ShaderCreate::DEFAULT_SHADER_START_NAME,
                device.clone(),
            )
            .unwrap(),
            ShaderCreate::with_spv_path(
                "examples/shaders/triangle/shader.frag.spv",
                vk::ShaderStageFlags::FRAGMENT,
                ShaderCreate::DEFAULT_SHADER_START_NAME,
                device.clone(),
            )
            .unwrap(),
        ];
        let (shader_infos, _shader_modules) = (
            [
                shader_creates[0].stage_create_info,
                shader_creates[1].stage_create_info,
            ],
            [shader_creates[0].module, shader_creates[1].module],
        );

        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR])
            .build();
        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&[Vertex::binding_description()])
            .vertex_attribute_descriptions(&Vertex::attr_descriptions())
            .build();
        let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false)
            .build();
        let viewport_state_info = {
            vk::PipelineViewportStateCreateInfo::builder()
                .viewports(&[extent_helper::viewport_from_extent(swapchain_extent)])
                .scissors(&[extent_helper::scissor_from_extent(swapchain_extent)])
                .build()
        };
        let raster_state_info = vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false)
            .build();
        let multisample_state_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .build();
        let color_blend_attach_state = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)
            .build();
        let color_blend_state_info = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .attachments(&[color_blend_attach_state])
            .build();
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(descriptor_set_layouts)
            .push_constant_ranges(&[])
            .build();
        let pipeline_layout = unsafe {
            device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .expect("Fail to create pipeline layout")
        };
        let depth_stencil_state_info = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false)
            .build();

        let pipeline_create_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_infos)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly_info)
            .viewport_state(&viewport_state_info)
            .rasterization_state(&raster_state_info)
            .multisample_state(&multisample_state_info)
            .color_blend_state(&color_blend_state_info)
            .dynamic_state(&dynamic_state_info)
            .layout(pipeline_layout)
            .render_pass(*renderpass)
            .subpass(0)
            .depth_stencil_state(&depth_stencil_state_info)
            .build();

        let pipeline = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_create_info], None)
                .expect("Fail to create graphics pipeline")[0]
        };

        (pipeline_layout, pipeline)
    }

    pub(super) fn create_descriptor_set_layout(device: &Device) -> vk::DescriptorSetLayout {
        let ubo_layout_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .descriptor_count(1)
            .build();

        let sampler_layout_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(1)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();

        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&[ubo_layout_binding, sampler_layout_binding])
            .build();

        unsafe {
            device
                .create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
                .unwrap()
        }
    }

    pub(super) fn create_renderpass_begin_info(
        render_pass: &vk::RenderPass,
        frame_buffer: &vk::Framebuffer,
        extent: vk::Extent2D,
    ) -> vk::RenderPassBeginInfo {
        vk::RenderPassBeginInfo::builder()
            .render_pass(*render_pass)
            .framebuffer(*frame_buffer)
            .render_area(
                vk::Rect2D::builder()
                    .offset(vk::Offset2D { x: 0, y: 0 })
                    .extent(extent)
                    .build(),
            )
            .clear_values(&[
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0., 0., 0., 1.],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.,
                        stencil: 0,
                    },
                },
            ])
            .build()
    }
}

fn main() {
    println!("Hello vulkan triangle demo");
    let mut event_loop = RefCell::new(EventLoop::new());
    let mut app = DrawTriangleApp::new(&event_loop.borrow());
    app.run(&mut event_loop);
}
