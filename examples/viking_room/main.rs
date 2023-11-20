use std::{cell::RefCell, ffi::c_void, rc::Rc, time::SystemTime};

use ash::vk;
use glam::{Mat4, Vec3};
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
    vulkan_objects::{extent_helper, Buffer, Device, Surface, Texture},
    window_fns,
};

struct VikingRoomApp {
    window: Window,
    window_resized: bool,

    current_frame: usize,
    last_frame_time_stamp: SystemTime,

    model_vertices: Vec<Vertex>,
    model_indices: Vec<u32>,

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
    #[allow(dead_code)]
    texture_image: Texture,
}

impl WindowApp for VikingRoomApp {
    window_fns!(VikingRoomApp);

    fn draw_frame(&mut self) {
        let image_index = {
            let ret = self
                .fixed_vulkan_stuff
                .frame_get_image_index_to_draw(self.current_frame, &self.window)
                .unwrap();
            if ret.1 {
                return;
            }
            ret.0
        };

        self.update_uniform_buffer(&self.camera, &self.uniform_buffers[self.current_frame]);

        self.record_render_commands(
            self.fixed_vulkan_stuff.command_buffers[self.current_frame],
            self.fixed_vulkan_stuff.swapchain_framebuffers[image_index as usize],
            self.descriptor_sets[self.current_frame],
            self.model_vertices.len() as u32,
            self.model_indices.len() as u32,
        );

        self.window_resized = self
            .fixed_vulkan_stuff
            .frame_queue_submit_and_present(
                self.current_frame,
                image_index,
                &self.window,
                self.window_resized,
            )
            .unwrap();

        self.current_frame = (self.current_frame + 1) % FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT;
        self.last_frame_time_stamp = SystemTime::now();
    }

    fn new(event_loop: &EventLoop<()>) -> Self {
        let window = WindowBuilder::new()
            .with_title(Self::window_title())
            .with_inner_size(PhysicalSize::new(1800, 1200))
            .build(event_loop)
            .unwrap();

        let (model_vertices, model_indices) =
            vulkan_example_rs::mesh::load_obj_model("examples/meshes/viking_room/viking_room.obj")
                .unwrap();

        let fixed_vulkan_stuff = Self::create_fixed_vulkan_stuff(&window).unwrap();
        let descriptor_set_layout = {
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
                fixed_vulkan_stuff
                    .device
                    .create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
                    .unwrap()
            }
        };
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

        let indice_buffer = Buffer::new_device_local(
            &model_indices,
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
        .unwrap();

        let uniform_buffers: [_; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT] =
            array_init::array_init(|_| {
                let mut buffer =
                    MVPMatrix::empty_uniform_buffer(fixed_vulkan_stuff.device.clone()).unwrap();
                let ptr = buffer.map_memory_all().unwrap();
                (buffer, ptr)
            });

        let mut texture_image = Texture::from_rgba8_picture(
            "examples/textures/viking_room/viking_room.png",
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
        .unwrap();
        texture_image.spawn_image_view().unwrap();
        texture_image.spawn_sampler(vk::Filter::LINEAR).unwrap();

        {
            for i in 0..FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT {
                let uniform_descritptor_write = vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(&[uniform_buffers[i].0.descriptor_default()])
                    .build();

                let image_descritptor_write = vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(1)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[texture_image.descriptor_default()])
                    .build();

                unsafe {
                    fixed_vulkan_stuff.device.update_descriptor_sets(
                        &[uniform_descritptor_write, image_descritptor_write],
                        &[],
                    )
                }
            }
        }

        VikingRoomApp {
            window,
            window_resized: false,

            current_frame: 0,
            last_frame_time_stamp: SystemTime::now(),

            model_vertices,
            model_indices,

            camera: Camera::with_translation(Vec3::new(0., 0., -3.))
                .with_move_speed(100.)
                .with_rotate_speed(400.),
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
        }
    }
}

impl VikingRoomApp {
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

impl Drop for VikingRoomApp {
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
        "examples/shaders/viking_room/shader.vert.spv".to_string()
    }

    fn frag_spv_path(&self) -> String {
        "examples/shaders/viking_room/shader.frag.spv".to_string()
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
    let mut app = VikingRoomApp::new(&event_loop.borrow());
    app.run(&mut event_loop);
}
