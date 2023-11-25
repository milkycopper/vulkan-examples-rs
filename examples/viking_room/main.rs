use std::{cell::RefCell, ffi::c_void, rc::Rc};

use ash::vk;
use glam::{Mat4, Vec3};
use winit::{dpi::PhysicalSize, event_loop::EventLoop, window::Window};

use vulkan_example_rs::{
    app::{FixedVulkanStuff, FrameCounter, PipelineBuilder, UIOverlay, WindowApp},
    camera::{Camera, MVPMatrix},
    impl_drop_trait, impl_pipeline_builder_fns, impl_window_fns,
    mesh::Vertex,
    vulkan_objects::{Buffer, Device, Surface, Texture},
};

struct VikingRoomApp {
    window: Window,
    window_resized: bool,

    frame_counter: FrameCounter,
    ui_overlay: UIOverlay,

    #[allow(dead_code)]
    model_vertices: Vec<Vertex>,
    model_indices: Vec<u32>,

    camera: Camera,

    fixed_vulkan_stuff: FixedVulkanStuff,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_sets: [vk::DescriptorSet; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT],
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    vertex_buffer: Buffer<Vertex>,
    indice_buffer: Buffer<u32>,
    uniform_buffers: [(Buffer<MVPMatrix>, *mut c_void); FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT],
    #[allow(dead_code)]
    texture_image: Texture,
}

impl WindowApp for VikingRoomApp {
    impl_window_fns!(VikingRoomApp);

    fn draw_frame(&mut self) {
        let image_index = {
            let ret = self
                .fixed_vulkan_stuff
                .frame_get_image_index_to_draw(self.frame_counter.double_buffer_frame, &self.window)
                .unwrap();
            if ret.1 {
                return;
            }
            ret.0
        };

        self.update_uniform_buffer(
            &self.camera,
            &self.uniform_buffers[self.frame_counter.double_buffer_frame],
        );

        let name = self
            .fixed_vulkan_stuff
            .device
            .physical_device_name()
            .to_owned();
        self.update_ui(&[name]);

        self.record_render_commands(
            self.frame_counter.double_buffer_frame,
            image_index,
            self.descriptor_sets[self.frame_counter.double_buffer_frame],
            self.model_indices.len() as u32,
        );

        self.window_resized = self
            .fixed_vulkan_stuff
            .frame_queue_submit_and_present(
                self.frame_counter.double_buffer_frame,
                image_index,
                &self.window,
                self.window_resized,
            )
            .unwrap();

        self.frame_counter.update();
    }

    fn new(event_loop: &EventLoop<()>) -> Self {
        let window = Self::build_window(event_loop);

        let (model_vertices, model_indices) =
            vulkan_example_rs::mesh::load_obj_model("examples/meshes/viking_room/viking_room.obj")
                .unwrap();

        let fixed_vulkan_stuff = Self::create_fixed_vulkan_stuff(&window).unwrap();
        let descriptor_set_layout =
            Self::create_descriptor_set_layout(&fixed_vulkan_stuff.device).unwrap();
        let descriptor_pool = Self::create_descriptor_pool(&fixed_vulkan_stuff.device).unwrap();
        let descriptor_sets = Self::create_descriptor_sets(
            descriptor_pool,
            descriptor_set_layout,
            &fixed_vulkan_stuff.device,
        )
        .unwrap();

        let pipeline_creator = PipelineCreator {
            device: fixed_vulkan_stuff.device.clone(),
            surface: &fixed_vulkan_stuff.surface,
            render_pass: fixed_vulkan_stuff.render_pass,
            set_layouts: &[descriptor_set_layout],
            vertex_bindings: &[Vertex::binding_description()],
            vertex_attributes: &Vertex::attr_descriptions(),
            pipeline_cache: fixed_vulkan_stuff.pipeline_cache,
        };

        let (pipeline_layout, pipeline) = pipeline_creator.build().unwrap();

        let vertex_buffer = fixed_vulkan_stuff
            .device_local_vertex_buffer(&model_vertices)
            .unwrap();
        let indice_buffer = fixed_vulkan_stuff
            .device_local_indice_buffer(&model_indices)
            .unwrap();

        let uniform_buffers: [_; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT] =
            array_init::array_init(|_| {
                let mut buffer = Buffer::<MVPMatrix>::new(
                    1,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                    fixed_vulkan_stuff.device.clone(),
                )
                .unwrap();
                let ptr = buffer.map_memory_all().unwrap();
                (buffer, ptr)
            });

        let mut texture_image = Texture::from_rgba8_picture(
            "examples/textures/viking_room/viking_room.png",
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.graphic_command_pool,
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

        let ui_overlay = UIOverlay::new(
            fixed_vulkan_stuff.pipeline_cache,
            fixed_vulkan_stuff.render_pass,
            1.0,
            fixed_vulkan_stuff.device.clone(),
        )
        .unwrap();

        VikingRoomApp {
            window,
            window_resized: false,

            frame_counter: FrameCounter::default(),

            model_vertices,
            model_indices,

            camera: Camera::builder()
                .translation(Vec3::new(0., 0., -3.))
                .move_speed(100.)
                .rotate_speed(400.)
                .build(),
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
            ui_overlay,
        }
    }

    fn descriptor_pool_sizes() -> Vec<vk::DescriptorPoolSize> {
        vec![
            vk::DescriptorType::UNIFORM_BUFFER,
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        ]
        .into_iter()
        .map(|ty| {
            vk::DescriptorPoolSize::builder()
                .ty(ty)
                .descriptor_count(FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT as u32)
                .build()
        })
        .collect()
    }

    fn descriptor_set_layout_bindings() -> Vec<vk::DescriptorSetLayoutBinding> {
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
        vec![ubo_layout_binding, sampler_layout_binding]
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
            perspective: camera.perspective_mat(),
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
        &mut self,
        frame_index: usize,
        image_index: usize,
        descriptor_set: vk::DescriptorSet,
        indice_num: u32,
    ) {
        let command_buffer = self.fixed_vulkan_stuff.graphic_command_buffers[frame_index];
        unsafe {
            self.fixed_vulkan_stuff
                .device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::default())
                .expect("Fail to reset command buffer");

            self.fixed_vulkan_stuff
                .device
                .begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::default())
                .expect("Fail to begin command buffer");

            self.fixed_vulkan_stuff.cmd_begin_renderpass(
                frame_index,
                image_index,
                &Self::clear_value(),
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

            self.fixed_vulkan_stuff
                .cmd_set_viewport_and_scissor(frame_index);

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

            self.ui_overlay
                .draw(command_buffer, self.frame_counter.double_buffer_frame);

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

impl_drop_trait!(VikingRoomApp);

struct PipelineCreator<'a> {
    device: Rc<Device>,
    surface: &'a Surface,
    render_pass: vk::RenderPass,
    set_layouts: &'a [vk::DescriptorSetLayout],
    vertex_bindings: &'a [vk::VertexInputBindingDescription],
    vertex_attributes: &'a [vk::VertexInputAttributeDescription],
    pipeline_cache: vk::PipelineCache,
}

impl<'a> PipelineBuilder<'a, &'a str> for PipelineCreator<'a> {
    impl_pipeline_builder_fns!();

    fn vertex_spv_path(&self) -> &'a str {
        "examples/shaders/viking_room/shader.vert.spv"
    }

    fn frag_spv_path(&self) -> &'a str {
        "examples/shaders/viking_room/shader.frag.spv"
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
}

fn main() {
    let mut event_loop = RefCell::new(EventLoop::new());
    let mut app = VikingRoomApp::new(&event_loop.borrow());
    app.run(&mut event_loop);
}
