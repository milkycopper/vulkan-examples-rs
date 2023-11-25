use std::{cell::RefCell, ffi::c_void, rc::Rc};

use ash::vk;
use glam::{vec3, Mat4, Quat, Vec3, Vec4};
use winit::{dpi::PhysicalSize, event_loop::EventLoop, window::Window};

use vulkan_example_rs::{
    app::{FixedVulkanStuff, FrameCounter, PipelineBuilder, UIOverlay, WindowApp},
    camera::Camera,
    impl_drop_trait, impl_pipeline_builder_fns, impl_window_fns,
    mesh::Vertex,
    vulkan_objects::{Buffer, Device, Surface, Texture},
};

const MAX_ARRAY_COUNT: usize = 8;

struct TextureArrayExample {
    window: Window,
    window_resized: bool,

    frame_counter: FrameCounter,

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
    uniform_buffers: [(Buffer<Ubo>, *mut c_void); FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT],
    #[allow(dead_code)]
    texture_image: Texture,
    layer_count: u32,

    ui_overlay: UIOverlay,
}

impl WindowApp for TextureArrayExample {
    impl_window_fns!(TextureArrayExample);

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

        let fixed_vulkan_stuff = Self::create_fixed_vulkan_stuff(&window).unwrap();

        let (mut texture_image, layer_count) = Texture::from_ktx(
            "examples/textures/texture_array/texturearray_rgba.ktx",
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.graphic_command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
        .unwrap();
        texture_image.spawn_image_view().unwrap();
        texture_image.set_sampler(Rc::new({
            let create_info = vk::SamplerCreateInfo::builder()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .anisotropy_enable(true)
                .max_anisotropy(unsafe {
                    fixed_vulkan_stuff
                        .device
                        .instance()
                        .get_physical_device_properties(
                            *fixed_vulkan_stuff
                                .device
                                .physical_device()
                                .upgrade()
                                .unwrap(),
                        )
                        .limits
                        .max_sampler_anisotropy
                })
                .border_color(vk::BorderColor::INT_OPAQUE_WHITE)
                .unnormalized_coordinates(false)
                .compare_enable(false)
                .compare_op(vk::CompareOp::NEVER)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .mip_lod_bias(0.)
                .max_lod(0.)
                .min_lod(0.)
                .build();

            unsafe {
                fixed_vulkan_stuff
                    .device
                    .create_sampler(&create_info, None)
                    .unwrap()
            }
        }));

        let model_vertices = vec![
            Vertex::new((-1., -1., 1.).into()).with_texture_coord((0., 0.).into()),
            Vertex::new((1., -1., 1.).into()).with_texture_coord((1., 0.).into()),
            Vertex::new((1., 1., 1.).into()).with_texture_coord((1., 1.).into()),
            Vertex::new((-1., 1., 1.).into()).with_texture_coord((0., 1.).into()),
            //
            Vertex::new((1., 1., 1.).into()).with_texture_coord((0., 0.).into()),
            Vertex::new((1., 1., -1.).into()).with_texture_coord((1., 0.).into()),
            Vertex::new((1., -1., -1.).into()).with_texture_coord((1., 1.).into()),
            Vertex::new((1., -1., 1.).into()).with_texture_coord((0., 1.).into()),
            //
            Vertex::new((-1., -1., -1.).into()).with_texture_coord((0., 0.).into()),
            Vertex::new((1., -1., -1.).into()).with_texture_coord((1., 0.).into()),
            Vertex::new((1., 1., -1.).into()).with_texture_coord((1., 1.).into()),
            Vertex::new((-1., 1., -1.).into()).with_texture_coord((0., 1.).into()),
            //
            Vertex::new((-1., -1., -1.).into()).with_texture_coord((0., 0.).into()),
            Vertex::new((-1., -1., 1.).into()).with_texture_coord((1., 0.).into()),
            Vertex::new((-1., 1., 1.).into()).with_texture_coord((1., 1.).into()),
            Vertex::new((-1., 1., -1.).into()).with_texture_coord((0., 1.).into()),
            //
            Vertex::new((1., 1., 1.).into()).with_texture_coord((0., 0.).into()),
            Vertex::new((-1., 1., 1.).into()).with_texture_coord((1., 0.).into()),
            Vertex::new((-1., 1., -1.).into()).with_texture_coord((1., 1.).into()),
            Vertex::new((1., 1., -1.).into()).with_texture_coord((0., 1.).into()),
            //
            Vertex::new((-1., -1., -1.).into()).with_texture_coord((0., 0.).into()),
            Vertex::new((1., -1., -1.).into()).with_texture_coord((1., 0.).into()),
            Vertex::new((1., -1., 1.).into()).with_texture_coord((1., 1.).into()),
            Vertex::new((-1., -1., 1.).into()).with_texture_coord((0., 1.).into()),
        ];
        let model_indices = vec![
            0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15, 16,
            17, 18, 16, 18, 19, 20, 21, 22, 20, 22, 23,
        ];
        let vertex_buffer = fixed_vulkan_stuff
            .device_local_vertex_buffer(&model_vertices)
            .unwrap();
        let indice_buffer = fixed_vulkan_stuff
            .device_local_indice_buffer(&model_indices)
            .unwrap();

        let uniform_buffers: [(Buffer<Ubo>, *mut c_void); FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT] =
            array_init::array_init(|_| {
                let mut buffer = Buffer::<Ubo>::new(
                    1,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                    fixed_vulkan_stuff.device.clone(),
                )
                .unwrap();
                let mut ubo_data = Ubo::default();

                let offset = -1.5;
                let center = (layer_count as f32 * offset) / 2.0 - (offset * 0.5);
                for i in 0..layer_count as usize {
                    // Instance model matrix
                    ubo_data.instances[i].model = Mat4::from_scale_rotation_translation(
                        Vec3::ONE * 0.5,
                        Quat::IDENTITY,
                        vec3(i as f32 * offset - center, 0., 0.),
                    );
                    // Instance texture array index
                    ubo_data.instances[i].array_index.x = i as f32;
                }
                buffer
                    .load_data(&ubo_data.instances, std::mem::size_of::<Mat4>() as u64 * 2)
                    .unwrap();
                let ptr = buffer
                    .map_memory(0, std::mem::size_of::<Mat4>() as u64 * 2)
                    .unwrap();
                (buffer, ptr)
            });

        let descriptor_set_layout =
            Self::create_descriptor_set_layout(&fixed_vulkan_stuff.device).unwrap();
        let descriptor_pool = Self::create_descriptor_pool(&fixed_vulkan_stuff.device).unwrap();

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

        let descriptor_sets = Self::create_descriptor_sets(
            descriptor_pool,
            descriptor_set_layout,
            &fixed_vulkan_stuff.device,
        )
        .unwrap();

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

        TextureArrayExample {
            window,
            window_resized: false,

            frame_counter: FrameCounter::default(),

            model_vertices,
            model_indices,

            camera: Camera::builder()
                .translation(Vec3::new(0., 0., -6.))
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
            layer_count,
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

impl TextureArrayExample {
    fn update_uniform_buffer(&self, camera: &Camera, uniform_buffer: &(Buffer<Ubo>, *mut c_void)) {
        let pv_matrix = [camera.perspective_mat(), camera.view_mat()];

        unsafe {
            std::ptr::copy_nonoverlapping(
                &pv_matrix as *const Mat4,
                uniform_buffer.1 as *mut Mat4,
                2,
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

            self.fixed_vulkan_stuff.device.cmd_draw_indexed(
                command_buffer,
                indice_num,
                self.layer_count,
                0,
                0,
                0,
            );

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

impl_drop_trait!(TextureArrayExample);

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
        "examples/shaders/texture_array/shader.vert.spv"
    }

    fn frag_spv_path(&self) -> &'a str {
        "examples/shaders/texture_array/shader.frag.spv"
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

    fn rasterization_state_create_info(&self) -> vk::PipelineRasterizationStateCreateInfo {
        vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false)
            .build()
    }
}

#[derive(Default)]
struct InstanceData {
    pub model: Mat4,
    pub array_index: Vec4,
}

#[derive(Default)]
struct Ubo {
    #[allow(dead_code)]
    pub projection: Mat4,
    #[allow(dead_code)]
    pub view: Mat4,
    pub instances: [InstanceData; MAX_ARRAY_COUNT],
}

fn main() {
    let mut event_loop = RefCell::new(EventLoop::new());
    let mut app = TextureArrayExample::new(&event_loop.borrow());
    app.run(&mut event_loop);
}
