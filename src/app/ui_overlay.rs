use std::rc::Rc;

use ash::vk;
use glam::Vec2;
use imgui::{Context, DrawCmd, DrawIdx, DrawVert, FontSource, StyleColor};

use super::{FixedVulkanStuff, PipelineBuilder};
use crate::{
    error::{RenderError, RenderResult},
    impl_pipeline_builder_fns,
    vulkan_objects::{Buffer, Device, OneTimeCommand, Texture},
};

#[derive(Clone, Copy)]
pub struct UIPushConstBlock {
    scale: Vec2,
    translation: Vec2,
}

unsafe impl bytemuck::Pod for UIPushConstBlock {}
unsafe impl bytemuck::Zeroable for UIPushConstBlock {}

impl UIPushConstBlock {
    pub fn new(scale: Vec2, translation: Vec2) -> Self {
        Self { scale, translation }
    }

    pub fn scale(&self) -> Vec2 {
        self.scale
    }

    pub fn translation(&self) -> Vec2 {
        self.translation
    }
}

pub struct UIOverlay {
    pub device: Rc<Device>,
    pub command_pool: vk::CommandPool,

    pub vertex_buffers: [Buffer<DrawVert>; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT],
    pub indice_buffers: [Buffer<DrawIdx>; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT],

    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_set: vk::DescriptorSet,
    pub pipeline_layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,

    pub font_texture: Texture,
    pub scale: f32,

    pub imgui_context: Context,
}

impl UIOverlay {
    pub fn new(
        pipeline_cache: vk::PipelineCache,
        render_pass: vk::RenderPass,
        scale: f32,
        device: Rc<Device>,
    ) -> RenderResult<Self> {
        let mut imgui = Context::create();
        {
            let style = imgui.style_mut();
            style[StyleColor::TitleBg] = [1., 0., 0., 1.];
            style[StyleColor::TitleBgActive] = [1., 0., 0., 1.];
            style[StyleColor::TitleBgCollapsed] = [1., 0., 0., 0.1];
            style[StyleColor::MenuBarBg] = [1., 0., 0., 0.4];
            style[StyleColor::Header] = [0.8, 0., 0., 0.4];
            style[StyleColor::HeaderActive] = [1., 0., 0., 0.4];
            style[StyleColor::HeaderHovered] = [1., 0., 0., 0.4];
            style[StyleColor::FrameBg] = [0., 0., 0., 0.8];
            style[StyleColor::CheckMark] = [1., 0., 0., 0.8];
            style[StyleColor::SliderGrab] = [1., 0., 0., 0.4];
            style[StyleColor::SliderGrabActive] = [1., 0., 0., 0.8];
            style[StyleColor::FrameBgHovered] = [1., 1., 1., 0.1];
            style[StyleColor::FrameBgActive] = [1., 1., 1., 0.2];
            style[StyleColor::Button] = [1., 0., 0., 0.4];
            style[StyleColor::ButtonHovered] = [1., 0., 0., 0.6];
            style[StyleColor::ButtonActive] = [1., 0., 0., 0.8];
        }
        {
            let io = imgui.io_mut();
            io.font_global_scale = scale;
        }

        let (tex_width, tex_height, tex_data) = {
            let fonts = imgui.fonts();
            fonts.add_font(&[FontSource::TtfData {
                data: include_bytes!("fonts/Roboto-Medium.ttf"),
                size_pixels: 16. * scale,
                config: None,
            }]);
            let font_atlas_texture = fonts.build_rgba32_texture();
            assert!(
                font_atlas_texture.width * font_atlas_texture.height * 4
                    == font_atlas_texture.data.len() as u32
            );
            (
                font_atlas_texture.width,
                font_atlas_texture.height,
                font_atlas_texture.data.to_vec(),
            )
        };

        {
            let style = imgui.style_mut();
            style.scale_all_sizes(scale);
        }

        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(device.graphic_queue_family_index())
                    .build(),
                None,
            )?
        };

        let font_texture = {
            let mut texture = Texture::builder(
                tex_width,
                tex_height,
                vk::Format::R8G8B8A8_UNORM,
                vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
                device.clone(),
            )
            .build()?;

            let mut staging_buffer = Buffer::<u8>::new(
                tex_data.len(),
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                device.clone(),
            )?;
            staging_buffer.load_data(&tex_data, 0)?;

            OneTimeCommand::new(&device, &command_pool)?.take_and_execute(
                |command_buffer| {
                    texture.transition_layout(
                        command_buffer,
                        vk::ImageLayout::UNDEFINED,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        vk::PipelineStageFlags::HOST,
                        vk::PipelineStageFlags::TRANSFER,
                    );

                    let image_copy = vk::BufferImageCopy::builder()
                        .image_subresource(
                            vk::ImageSubresourceLayers::builder()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .mip_level(0)
                                .base_array_layer(0)
                                .layer_count(1)
                                .build(),
                        )
                        .image_offset(vk::Offset3D::default())
                        .image_extent(
                            vk::Extent3D::builder()
                                .width(tex_width)
                                .height(tex_height)
                                .depth(1)
                                .build(),
                        )
                        .build();
                    unsafe {
                        device.cmd_copy_buffer_to_image(
                            command_buffer,
                            staging_buffer.buffer(),
                            *texture.image(),
                            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                            &[image_copy],
                        );
                    }

                    texture.transition_layout(
                        command_buffer,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                    );

                    Ok(())
                },
                &device.graphic_queue(),
            )?;

            texture.spawn_image_view()?;

            let sampler = {
                let create_info = vk::SamplerCreateInfo::builder()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
                    .build();
                Rc::new(unsafe { device.create_sampler(&create_info, None)? })
            };
            texture.set_sampler(sampler);

            texture
        };

        let descriptor_pool = {
            let create_info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(&[vk::DescriptorPoolSize::builder()
                    .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .descriptor_count(1)
                    .build()])
                .max_sets(FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT as u32)
                .build();
            unsafe { device.create_descriptor_pool(&create_info, None)? }
        };

        let descriptor_set_layout = {
            let bindings = [vk::DescriptorSetLayoutBinding::builder()
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                .binding(0)
                .descriptor_count(1)
                .build()];
            let create_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings);
            unsafe { device.create_descriptor_set_layout(&create_info, None)? }
        };

        let descriptor_set = unsafe {
            let allocate_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&[descriptor_set_layout])
                .build();
            device.allocate_descriptor_sets(&allocate_info)?[0]
        };

        let image_descritptor_write = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(0)
            .dst_array_element(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&[font_texture.descriptor_default()])
            .build();

        unsafe { device.update_descriptor_sets(&[image_descritptor_write], &[]) };

        let pipeline_builder = PipelineCreator {
            device: device.clone(),
            render_pass,
            extent: vk::Extent2D {
                width: tex_width,
                height: tex_height,
            },
            set_layouts: &[descriptor_set_layout],
            vertex_bindings: &[vk::VertexInputBindingDescription::builder()
                .binding(0)
                .stride(std::mem::size_of::<DrawVert>() as u32)
                .input_rate(vk::VertexInputRate::VERTEX)
                .build()],
            vertex_attributes: &[
                vk::VertexInputAttributeDescription::builder()
                    .binding(0)
                    .location(0)
                    .format(vk::Format::R32G32_SFLOAT)
                    .offset(memoffset::offset_of!(DrawVert, pos) as u32)
                    .build(),
                vk::VertexInputAttributeDescription::builder()
                    .binding(0)
                    .location(1)
                    .format(vk::Format::R32G32_SFLOAT)
                    .offset(memoffset::offset_of!(DrawVert, uv) as u32)
                    .build(),
                vk::VertexInputAttributeDescription::builder()
                    .binding(0)
                    .location(2)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .offset(memoffset::offset_of!(DrawVert, col) as u32)
                    .build(),
            ],
            pipeline_cache,
        };

        let (pipeline_layout, pipeline) = pipeline_builder.build()?;

        let vertex_buffers = array_init::try_array_init(|_| -> Result<_, RenderError> {
            Self::vertex_buffer(device.clone(), 1)
        })?;
        let indice_buffers = array_init::try_array_init(|_| -> Result<_, RenderError> {
            Self::indice_buffer(device.clone(), 1)
        })?;

        Ok(Self {
            device: device.clone(),
            command_pool,
            vertex_buffers,
            indice_buffers,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_set,
            pipeline_layout,
            pipeline,
            font_texture,
            scale,
            imgui_context: imgui,
        })
    }

    pub fn from_fixed_vulkan_stuff(s: &FixedVulkanStuff, scale: f32) -> RenderResult<Self> {
        Self::new(s.pipeline_cache, s.render_pass, scale, s.device.clone())
    }

    pub fn update(&mut self, frame_index: usize) -> RenderResult<bool> {
        assert!(frame_index < FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT);

        let mut update_command_buffers = false;

        let draw_data = self.imgui_context.render();

        if draw_data.total_vtx_count == 0 || draw_data.total_idx_count == 0 {
            return Ok(false);
        }

        if self.vertex_buffers[frame_index].element_num() != draw_data.total_vtx_count as usize {
            self.vertex_buffers[frame_index] =
                Self::vertex_buffer(self.device.clone(), draw_data.total_vtx_count as usize)?;
            update_command_buffers = true;
        }
        if self.indice_buffers[frame_index].element_num() != draw_data.total_idx_count as usize {
            self.indice_buffers[frame_index] =
                Self::indice_buffer(self.device.clone(), draw_data.total_idx_count as usize)?;
            update_command_buffers = true;
        }

        self.vertex_buffers[frame_index].map_memory_all()?;
        self.indice_buffers[frame_index].map_memory_all()?;

        let (mut vertex_offset, mut indice_offset) = (0, 0);
        for draw_list in draw_data.draw_lists() {
            self.vertex_buffers[frame_index]
                .load_data_when_mapped(draw_list.vtx_buffer(), vertex_offset);
            vertex_offset += draw_list.vtx_buffer().len() as u64;
            self.indice_buffers[frame_index]
                .load_data_when_mapped(draw_list.idx_buffer(), indice_offset);
            indice_offset += draw_list.idx_buffer().len() as u64
        }

        self.vertex_buffers[frame_index].flush()?;
        self.indice_buffers[frame_index].flush()?;
        self.vertex_buffers[frame_index].unmap_memory();
        self.indice_buffers[frame_index].unmap_memory();

        Ok(update_command_buffers)
    }

    pub fn draw(&mut self, command_buffer: vk::CommandBuffer, frame_index: usize) {
        let display_size = self.imgui_context.io().display_size;
        let draw_data = self.imgui_context.render();

        if draw_data.draw_lists_count() == 0 {
            return;
        }

        unsafe {
            self.device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );
            self.device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.descriptor_set],
                &[],
            );
            self.device.cmd_push_constants(
                command_buffer,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                bytemuck::bytes_of(&UIPushConstBlock::new(
                    Vec2::ONE * 2.0 / Vec2::from(display_size),
                    Vec2::NEG_ONE,
                )),
            );
            self.device.cmd_bind_vertex_buffers(
                command_buffer,
                0,
                &[self.vertex_buffers[frame_index].buffer()],
                &[0],
            );
            self.device.cmd_bind_index_buffer(
                command_buffer,
                self.indice_buffers[frame_index].buffer(),
                0,
                vk::IndexType::UINT16,
            );
        }

        let (mut vertex_offset, mut indice_offset) = (0, 0);
        for draw_list in draw_data.draw_lists() {
            for cmd in draw_list.commands() {
                if let DrawCmd::Elements {
                    count,
                    cmd_params: paras,
                } = cmd
                {
                    let scissor_rect = vk::Rect2D::builder()
                        .extent(
                            vk::Extent2D::builder()
                                .width((paras.clip_rect[2] - paras.clip_rect[0]) as u32)
                                .height((paras.clip_rect[3] - paras.clip_rect[1]) as u32)
                                .build(),
                        )
                        .offset(
                            vk::Offset2D::builder()
                                .x((paras.clip_rect[0] as i32).max(0))
                                .y((paras.clip_rect[1] as i32).max(0))
                                .build(),
                        )
                        .build();
                    unsafe {
                        self.device
                            .cmd_set_scissor(command_buffer, 0, &[scissor_rect]);
                        self.device.cmd_draw_indexed(
                            command_buffer,
                            count as u32,
                            1,
                            indice_offset,
                            vertex_offset as i32,
                            0,
                        )
                    }
                    indice_offset += count as u32;
                }
            }
            vertex_offset += draw_list.vtx_buffer().len() as u32;
        }
    }

    fn vertex_buffer(device: Rc<Device>, elem_num: usize) -> RenderResult<Buffer<DrawVert>> {
        Buffer::<DrawVert>::new(
            elem_num,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE,
            device,
        )
    }

    fn indice_buffer(device: Rc<Device>, indice_num: usize) -> RenderResult<Buffer<DrawIdx>> {
        Buffer::<DrawIdx>::new(
            indice_num,
            vk::BufferUsageFlags::INDEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE,
            device,
        )
    }
}

impl Drop for UIOverlay {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_pipeline(self.pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        }
    }
}

struct PipelineCreator<'a> {
    device: Rc<Device>,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
    set_layouts: &'a [vk::DescriptorSetLayout],
    vertex_bindings: &'a [vk::VertexInputBindingDescription],
    vertex_attributes: &'a [vk::VertexInputAttributeDescription],
    pipeline_cache: vk::PipelineCache,
}

impl<'a> PipelineBuilder<'a, &'a str> for PipelineCreator<'a> {
    impl_pipeline_builder_fns!();

    fn vertex_spv_path(&self) -> &'a str {
        "src/app/shaders/uioverlay.vert.spv"
    }

    fn frag_spv_path(&self) -> &'a str {
        "src/app/shaders/uioverlay.frag.spv"
    }

    fn pipeline_layout(&self) -> vk::PipelineLayout {
        let push_constant_range = vk::PushConstantRange::builder()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .size(std::mem::size_of::<UIPushConstBlock>() as u32)
            .build();
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(self.set_layouts)
            .push_constant_ranges(&[push_constant_range])
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

    fn color_blend_attach_state(&self) -> vk::PipelineColorBlendAttachmentState {
        vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .src_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .color_blend_op(vk::BlendOp::ADD)
            .alpha_blend_op(vk::BlendOp::ADD)
            .blend_enable(true)
            .build()
    }

    fn depth_stencil_state_create_info(&self) -> vk::PipelineDepthStencilStateCreateInfo {
        vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(false)
            .depth_write_enable(false)
            .depth_compare_op(vk::CompareOp::ALWAYS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false)
            .build()
    }
}
