use std::{cell::RefCell, ffi::c_void, rc::Rc};

use ash::vk;
use glam::{vec3, Mat4, Vec3};
use winit::{dpi::PhysicalSize, event_loop::EventLoop, window::Window};

use vulkan_example_rs::{
    app::{FixedVulkanStuff, FrameCounter, PipelineBuilder, UIOverlay, WindowApp},
    camera::{Camera, MVPMatrix},
    impl_drop_trait, impl_window_fns,
    mesh::Vertex,
    vulkan_objects::{Buffer, Device, Surface},
};

struct DrawTriangleApp {
    window: Window,
    window_resized: bool,

    frame_counter: FrameCounter,
    ui_overlay: UIOverlay,

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
}

impl WindowApp for DrawTriangleApp {
    impl_window_fns!(DrawTriangleApp);

    fn new(event_loop: &winit::event_loop::EventLoop<()>) -> Self {
        let window = Self::build_window(event_loop);
        let model_vertices = vec![vec3(-0.5, -0.5, 0.), vec3(0.5, -0.5, 0.), vec3(0., 0.5, 0.)]
            .into_iter()
            .map(|v3| Vertex::new(v3).with_color(v3 + vec3(0.5, 0.4, 0.3)))
            .collect::<Vec<_>>();

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
            .device_local_indice_buffer(&[0, 1, 2, 1, 0, 2])
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

        {
            for i in 0..FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT {
                let uniform_buffer_info = uniform_buffers[i].0.descriptor_default();
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

        let ui_overlay = UIOverlay::new(
            fixed_vulkan_stuff.pipeline_cache,
            fixed_vulkan_stuff.render_pass,
            1.0,
            fixed_vulkan_stuff.device.clone(),
        )
        .unwrap();

        DrawTriangleApp {
            window,
            window_resized: false,
            fixed_vulkan_stuff,
            vertex_buffer,
            indice_buffer,
            uniform_buffers,
            pipeline_layout,
            pipeline,
            frame_counter: FrameCounter::default(),
            camera: Camera::builder()
                .translation(Vec3::new(0., 0., -3.))
                .move_speed(100.)
                .rotate_speed(400.)
                .build(),
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
            ui_overlay,
        }
    }

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
            6,
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

    fn descriptor_pool_sizes() -> Vec<vk::DescriptorPoolSize> {
        vec![vk::DescriptorPoolSize::builder()
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT as u32)
            .build()]
    }

    fn descriptor_set_layout_bindings() -> Vec<vk::DescriptorSetLayoutBinding> {
        let ubo_layout_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .descriptor_count(1)
            .build();

        vec![ubo_layout_binding]
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

impl_drop_trait!(DrawTriangleApp);

struct PipelineCreator<'a> {
    device: Rc<Device>,
    surface: &'a Surface,
    render_pass: vk::RenderPass,
    set_layouts: &'a [vk::DescriptorSetLayout],
    vertex_bindings: &'a [vk::VertexInputBindingDescription],
    vertex_attributes: &'a [vk::VertexInputAttributeDescription],
    pipeline_cache: vk::PipelineCache,
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

    fn pipeline_cache(&self) -> vk::PipelineCache {
        self.pipeline_cache
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
