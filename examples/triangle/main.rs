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
    app::WindowApp,
    camera::Camera,
    mesh::Vertex,
    queue_family::QueueFamilyIndices,
    transforms::{Direction, MVPMatrix},
    vulkan_objects::{
        extent_helper, format_helper, image_helper, Buffer, Device, ImageBuffer, InstanceBuilder,
        QueueInfo, ShaderCreate, Surface, SwapChainBatch,
    },
};

const MAX_FRAMES_IN_FLIGHT: usize = 2;

struct DrawTriangleApp {
    window: Window,
    vulkan_objs: VulkanObjects,
    current_frame: usize,
    window_resized: bool,
    start_time: SystemTime,
    model_vertices: Vec<Vertex>,
    model_indices: Vec<u32>,
    camera: Camera,
}

impl WindowApp for DrawTriangleApp {
    fn draw_frame(&mut self) {
        unsafe {
            let vk_objs = &mut self.vulkan_objs;

            vk_objs
                .device
                .wait_for_fences(
                    &[vk_objs.in_flight_fences[self.current_frame]],
                    true,
                    u64::MAX,
                )
                .unwrap();

            let result = vk_objs.swapchain_batch.loader().acquire_next_image(
                *vk_objs.swapchain_batch.swapchain_khr(),
                u64::MAX,
                vk_objs.image_available_semaphores[self.current_frame],
                vk::Fence::null(),
            );

            match result {
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    vk_objs.recreate_swapchain(&self.window);
                    return;
                }
                Ok(_) => {}
                _ => panic!("Failed to acquire swap chain image"),
            }

            let image_index = result.unwrap().0;

            vk_objs
                .update_uniform_buffer(&self.camera, &vk_objs.uniform_buffers[self.current_frame]);

            vk_objs
                .device
                .reset_fences(&[vk_objs.in_flight_fences[self.current_frame]])
                .unwrap();

            vk_objs.record_command(
                vk_objs.command_buffers[self.current_frame],
                vk_objs.swapchain_framebuffers[image_index as usize],
                vk_objs.descriptor_sets[self.current_frame],
                self.model_vertices.len() as u32,
                self.model_indices.len() as u32,
            );

            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&[vk_objs.image_available_semaphores[self.current_frame]])
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .command_buffers(&[vk_objs.command_buffers[self.current_frame]])
                .signal_semaphores(&[vk_objs.render_finished_semaphores[self.current_frame]])
                .build();

            vk_objs
                .device
                .queue_submit(
                    vk_objs.graphic_queue,
                    &[submit_info],
                    vk_objs.in_flight_fences[self.current_frame],
                )
                .unwrap();

            let present_info_khr = vk::PresentInfoKHR::builder()
                .wait_semaphores(&[vk_objs.render_finished_semaphores[self.current_frame]])
                .swapchains(&[*vk_objs.swapchain_batch.swapchain_khr()])
                .image_indices(&[image_index])
                .build();

            let result = vk_objs
                .swapchain_batch
                .loader()
                .queue_present(vk_objs.present_queue, &present_info_khr);

            let mut need_recreate = false;
            match result {
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Ok(true) => need_recreate = true,
                Ok(_) => {}
                _ => panic!("Failed to present swap chain image"),
            }
            if need_recreate || self.window_resized {
                self.window_resized = false;
                vk_objs.recreate_swapchain(&self.window);
            }
        };

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT
    }

    fn window_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    fn on_window_resized(&mut self, _size: PhysicalSize<u32>) {
        self.window_resized = true;
    }

    fn on_keyboard_input(&mut self, key_code: VirtualKeyCode) {
        let duration = SystemTime::now()
            .duration_since(self.start_time)
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
        let name = stringify!(DrawTriangleApp);
        let model_path = "examples/meshes/triangle/viking_room.obj";
        let window = WindowBuilder::new()
            .with_title(name)
            .with_inner_size(PhysicalSize::new(1800, 1200))
            .build(event_loop)
            .unwrap();

        let (model_vertices, model_indices) =
            vulkan_example_rs::mesh::load_obj_model(model_path).unwrap();

        let vulkan_objs =
            helper::create_vulkan_objs(name, "No Engine", &window, &model_vertices, &model_indices);

        DrawTriangleApp {
            window,
            vulkan_objs,
            current_frame: 0,
            window_resized: false,
            start_time: SystemTime::now(),
            model_vertices,
            model_indices,
            camera: Camera::default()
                .with_move_speed(0.004)
                .with_rotate_speed(4e-4),
        }
    }

    fn run(&mut self, event_loop: &mut RefCell<EventLoop<()>>) {
        env_logger::init();
        self.render_loop(event_loop);
        unsafe { self.vulkan_objs.device.device_wait_idle().unwrap() };
    }
}

struct VulkanObjects {
    surface: Rc<Surface>,
    device: Rc<Device>,
    graphic_queue: vk::Queue,
    present_queue: vk::Queue,
    swapchain_batch: SwapChainBatch,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: [vk::DescriptorSet; 2],
    pipeline_layout: vk::PipelineLayout,
    renderpass: vk::RenderPass,
    pipeline: vk::Pipeline,
    swapchain_framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: [vk::CommandBuffer; MAX_FRAMES_IN_FLIGHT],
    image_available_semaphores: [vk::Semaphore; MAX_FRAMES_IN_FLIGHT],
    render_finished_semaphores: [vk::Semaphore; MAX_FRAMES_IN_FLIGHT],
    in_flight_fences: [vk::Fence; MAX_FRAMES_IN_FLIGHT],
    vertex_buffer: Buffer<Vertex>,
    indice_buffer: Buffer<u32>,
    uniform_buffers: [(Buffer<MVPMatrix>, *mut c_void); MAX_FRAMES_IN_FLIGHT],
    #[allow(dead_code)]
    texture_image: ImageBuffer,
    texture_image_view: vk::ImageView,
    texture_image_sampler: vk::Sampler,
    #[allow(dead_code)]
    depth_buffer: ImageBuffer,
    depth_image_view: vk::ImageView,
}

impl VulkanObjects {
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

    pub fn record_command(
        &self,
        command_buffer: vk::CommandBuffer,
        frame_buffer: vk::Framebuffer,
        descriptor_set: vk::DescriptorSet,
        _vertex_num: u32,
        indice_num: u32,
    ) {
        unsafe {
            self.device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::default())
                .expect("Fail to reset command buffer");

            self.device
                .begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::default())
                .expect("Fail to begin command buffer");

            let renderpass_begin_info = vk::RenderPassBeginInfo::builder()
                .render_pass(self.renderpass)
                .framebuffer(frame_buffer)
                .render_area(
                    vk::Rect2D::builder()
                        .offset(vk::Offset2D { x: 0, y: 0 })
                        .extent(self.surface.extent())
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
                .build();

            self.device.cmd_begin_render_pass(
                command_buffer,
                &renderpass_begin_info,
                vk::SubpassContents::INLINE,
            );

            self.device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );

            self.device.cmd_bind_vertex_buffers(
                command_buffer,
                0,
                &[self.vertex_buffer.vk_buffer()],
                &[0],
            );
            self.device.cmd_bind_index_buffer(
                command_buffer,
                self.indice_buffer.vk_buffer(),
                0,
                vk::IndexType::UINT32,
            );

            self.device.cmd_set_viewport(
                command_buffer,
                0,
                &[extent_helper::viewport_from_extent(self.surface.extent())],
            );

            self.device.cmd_set_scissor(
                command_buffer,
                0,
                &[extent_helper::scissor_from_extent(self.surface.extent())],
            );

            self.device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[descriptor_set],
                &[],
            );

            self.device
                .cmd_draw_indexed(command_buffer, indice_num, 1, 0, 0, 0);

            self.device.cmd_end_render_pass(command_buffer);
            self.device.end_command_buffer(command_buffer).unwrap();
        }
    }

    pub fn recreate_swapchain(&mut self, window: &Window) {
        unsafe {
            self.device.device_wait_idle().unwrap();

            self.surface.refit_surface_attribute(window).unwrap();

            self.swapchain_framebuffers
                .iter()
                .for_each(|fb| self.device.destroy_framebuffer(*fb, None));

            self.device.destroy_image_view(self.depth_image_view, None);
            self.swapchain_batch.recreate().unwrap();

            let depth_format = format_helper::find_depth_format(&self.device).unwrap();
            self.depth_buffer = ImageBuffer::new(
                self.surface.extent().width,
                self.surface.extent().height,
                depth_format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                self.device.clone(),
            )
            .unwrap();

            self.depth_image_view = image_helper::create_image_view(
                self.depth_buffer.vk_image(),
                depth_format,
                vk::ImageAspectFlags::DEPTH,
                &self.device,
            )
            .unwrap();

            self.swapchain_framebuffers = self
                .swapchain_batch
                .image_views()
                .iter()
                .map(|image_view| {
                    let framebuffer_create_info = vk::FramebufferCreateInfo::builder()
                        .render_pass(self.renderpass)
                        .attachments(&[*image_view, self.depth_image_view])
                        .width(self.surface.extent().width)
                        .height(self.surface.extent().height)
                        .layers(1)
                        .build();

                    self.device
                        .create_framebuffer(&framebuffer_create_info, None)
                        .expect("Fail to create framebuffer")
                })
                .collect::<Vec<_>>();
        }
    }
}

impl Drop for VulkanObjects {
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
            self.device
                .destroy_image_view(self.texture_image_view, None);
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device
                .destroy_sampler(self.texture_image_sampler, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.swapchain_framebuffers
                .iter()
                .for_each(|fb| self.device.destroy_framebuffer(*fb, None));
            self.device.destroy_pipeline(self.pipeline, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.renderpass, None);
        }
    }
}

mod helper {

    use super::*;

    pub(super) fn create_vulkan_objs(
        app_name: &str,
        engine_name: &str,
        window: &Window,
        vertices: &Vec<Vertex>,
        indices: &Vec<u32>,
    ) -> VulkanObjects {
        let instance = Rc::new(
            InstanceBuilder::new(window)
                .with_app_name_and_version(app_name, 0)
                .with_engine_name_and_version(engine_name, 0)
                .build()
                .unwrap(),
        );
        let surface = Rc::new(Surface::new(window, instance.clone()).unwrap());
        let queue_families = QueueFamilyIndices::from_surface(&surface).unwrap();
        let device = Rc::new(
            Device::new(
                instance,
                queue_families
                    .merge()
                    .unwrap()
                    .iter()
                    .map(|i| QueueInfo {
                        index: *i,
                        priority: 1.0,
                    })
                    .collect(),
            )
            .unwrap(),
        );
        let graphic_queue =
            unsafe { device.get_device_queue(queue_families.graphic_family.unwrap(), 0) };
        let present_queue =
            unsafe { device.get_device_queue(queue_families.present_family.unwrap(), 0) };
        let swapchain_batch = SwapChainBatch::new(surface.clone(), device.clone()).unwrap();

        let renderpass = helper::create_renderpass(
            surface.format(),
            format_helper::find_depth_format(&device).unwrap(),
            &device,
        );
        let descriptor_set_layout = helper::create_descriptor_set_layout(&device);

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
                            .descriptor_count(MAX_FRAMES_IN_FLIGHT as u32)
                            .build()
                    }),
                )
                .max_sets(MAX_FRAMES_IN_FLIGHT as u32)
                .build();
            device.create_descriptor_pool(&create_info, None).unwrap()
        };

        let descriptor_sets: [vk::DescriptorSet; MAX_FRAMES_IN_FLIGHT] = unsafe {
            let allocate_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&[descriptor_set_layout; MAX_FRAMES_IN_FLIGHT])
                .build();
            device
                .allocate_descriptor_sets(&allocate_info)
                .unwrap()
                .try_into()
                .unwrap()
        };

        let (pipeline_layout, pipeline) = helper::create_pipeline(
            device.clone(),
            surface.extent(),
            &renderpass,
            &[descriptor_set_layout],
        );

        let command_pool = {
            let create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_families.graphic_family.unwrap())
                .build();
            unsafe {
                device
                    .create_command_pool(&create_info, None)
                    .expect("Fail to create command pool")
            }
        };

        let command_buffers: [_; MAX_FRAMES_IN_FLIGHT] = {
            let allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(MAX_FRAMES_IN_FLIGHT as u32)
                .build();
            unsafe {
                device
                    .allocate_command_buffers(&allocate_info)
                    .expect("Fail to alloc command buffer")
                    .try_into()
                    .unwrap()
            }
        };

        let image_available_semaphores: [_; MAX_FRAMES_IN_FLIGHT] =
            array_init::array_init(|_| unsafe {
                device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .unwrap()
            });

        let render_finished_semaphores: [_; MAX_FRAMES_IN_FLIGHT] =
            array_init::array_init(|_| unsafe {
                device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .unwrap()
            });

        let in_flight_fences: [_; MAX_FRAMES_IN_FLIGHT] = array_init::array_init(|_| unsafe {
            device
                .create_fence(
                    &vk::FenceCreateInfo::builder()
                        .flags(vk::FenceCreateFlags::SIGNALED)
                        .build(),
                    None,
                )
                .unwrap()
        });

        let vertex_buffer =
            Vertex::create_buffer(vertices, device.clone(), &command_pool, &graphic_queue).unwrap();

        let indice_buffer =
            Buffer::new_indice_buffer(indices, device.clone(), &command_pool, &graphic_queue)
                .unwrap();

        let uniform_buffers: [_; MAX_FRAMES_IN_FLIGHT] = array_init::array_init(|_| {
            let buffer = MVPMatrix::empty_uniform_buffer(device.clone()).unwrap();
            let ptr = buffer.uniform_mapped_ptr().unwrap();
            (buffer, ptr)
        });

        let texture_image = ImageBuffer::color_image_from_file(
            "examples/textures/triangle/viking_room.png",
            device.clone(),
            &command_pool,
            &graphic_queue,
        )
        .unwrap();
        let texture_image_sampler = image_helper::create_texture_sampler(&device).unwrap();
        let texture_image_view = texture_image
            .create_image_view(vk::Format::R8G8B8A8_SRGB, vk::ImageAspectFlags::COLOR)
            .unwrap();

        let depth_format = format_helper::find_depth_format(&device).unwrap();
        let depth_buffer = ImageBuffer::new(
            surface.extent().width,
            surface.extent().height,
            depth_format,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
        )
        .unwrap();
        let depth_image_view = image_helper::create_image_view(
            depth_buffer.vk_image(),
            depth_format,
            vk::ImageAspectFlags::DEPTH,
            &device,
        )
        .unwrap();

        let framebuffers = swapchain_batch
            .image_views()
            .iter()
            .map(|image_view| {
                let framebuffer_create_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(renderpass)
                    .attachments(&[*image_view, depth_image_view])
                    .width(surface.extent().width)
                    .height(surface.extent().height)
                    .layers(1)
                    .build();
                unsafe {
                    device
                        .create_framebuffer(&framebuffer_create_info, None)
                        .expect("Fail to create framebuffer")
                }
            })
            .collect::<Vec<_>>();

        {
            for i in 0..MAX_FRAMES_IN_FLIGHT {
                let uniform_buffer_info = vk::DescriptorBufferInfo::builder()
                    .buffer(uniform_buffers[i].0.vk_buffer())
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
                    device.update_descriptor_sets(
                        &[uniform_descritptor_write, image_descritptor_write],
                        &[],
                    )
                }
            }
        }

        VulkanObjects {
            device,
            graphic_queue,
            present_queue,
            surface,
            swapchain_batch,
            renderpass,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
            pipeline_layout,
            pipeline,
            swapchain_framebuffers: framebuffers,
            command_pool,
            command_buffers,
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            vertex_buffer,
            indice_buffer,
            uniform_buffers,
            texture_image,
            texture_image_view,
            texture_image_sampler,
            depth_buffer,
            depth_image_view,
        }
    }

    pub(super) fn create_pipeline(
        device: Rc<Device>,
        swapchain_extent: vk::Extent2D,
        renderpass: &vk::RenderPass,
        descriptor_set_layouts: &[vk::DescriptorSetLayout],
    ) -> (vk::PipelineLayout, vk::Pipeline) {
        let shader_creates = [
            ShaderCreate::with_path(
                "examples/shaders/triangle/shader.vert.spv",
                vk::ShaderStageFlags::VERTEX,
                device.clone(),
            )
            .unwrap(),
            ShaderCreate::with_path(
                "examples/shaders/triangle/shader.frag.spv",
                vk::ShaderStageFlags::FRAGMENT,
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

    pub(super) fn create_renderpass(
        color_format: vk::Format,
        depth_format: vk::Format,
        device: &Device,
    ) -> vk::RenderPass {
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
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
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
        let dependency = vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .src_access_mask(vk::AccessFlags::default())
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            )
            .build();
        let renderpass_create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&[color_attach, depth_attach])
            .subpasses(&[subpass_desc])
            .dependencies(&[dependency])
            .build();
        unsafe {
            device
                .create_render_pass(&renderpass_create_info, None)
                .expect("Fail to create renderpass")
        }
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
}

fn main() {
    println!("Hello vulkan triangle demo");
    let mut event_loop = RefCell::new(EventLoop::new());
    let mut app = DrawTriangleApp::new(&event_loop.borrow());
    app.run(&mut event_loop);
}
