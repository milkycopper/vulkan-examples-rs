use std::{cell::RefCell, ffi::c_void, rc::Rc, time::SystemTime};

use ash::vk;
use glam::{vec3, Mat4, Quat, Vec3, Vec4};
use winit::{dpi::PhysicalSize, event_loop::EventLoop, window::Window};

use vulkan_example_rs::{
    app::{FixedVulkanStuff, PipelineBuilder, WindowApp},
    camera::Camera,
    impl_drop_trait, impl_window_fns,
    mesh::Vertex,
    vulkan_objects::{extent_helper, Buffer, Device, Surface, Texture},
};

const MAX_ARRAY_COUNT: usize = 8;

struct TextureArrayExample {
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
    descriptor_sets: [vk::DescriptorSet; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT],
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    vertex_buffer: Buffer<Vertex>,
    indice_buffer: Buffer<u32>,
    uniform_buffers: [(Buffer<Ubo>, *mut c_void); FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT],
    #[allow(dead_code)]
    texture_image: Texture,
    layer_count: u32,
}

impl WindowApp for TextureArrayExample {
    impl_window_fns!(TextureArrayExample);

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
            self.fixed_vulkan_stuff.graphic_command_buffers[self.current_frame],
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
        let vertex_buffer = Vertex::create_buffer(
            &model_vertices,
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.graphic_command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
        .unwrap();
        let indice_buffer = Buffer::new_device_local(
            &model_indices,
            fixed_vulkan_stuff.device.clone(),
            &fixed_vulkan_stuff.graphic_command_pool,
            &fixed_vulkan_stuff.device.graphic_queue(),
        )
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
                let ptr = buffer
                    .map_memory(
                        std::mem::size_of::<Mat4>() as u64 * 2,
                        (std::mem::size_of::<InstanceData>() * layer_count as usize) as u64,
                    )
                    .unwrap();
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        &ubo_data.instances as *const InstanceData,
                        ptr as *mut InstanceData,
                        layer_count as usize,
                    )
                };
                buffer.unmap_memory();
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

        TextureArrayExample {
            window,
            window_resized: false,

            current_frame: 0,
            last_frame_time_stamp: SystemTime::now(),

            model_vertices,
            model_indices,

            camera: Camera::with_translation(Vec3::new(0., 0., -6.))
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
            layer_count,
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

            self.fixed_vulkan_stuff.device.cmd_draw_indexed(
                command_buffer,
                indice_num,
                self.layer_count,
                0,
                0,
                0,
            );

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
}

impl<'a> PipelineBuilder<'a, String> for PipelineCreator<'a> {
    fn device(&self) -> Rc<Device> {
        self.device.clone()
    }
    fn vertex_spv_path(&self) -> String {
        "examples/shaders/texture_array/shader.vert.spv".to_string()
    }

    fn frag_spv_path(&self) -> String {
        "examples/shaders/texture_array/shader.frag.spv".to_string()
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
