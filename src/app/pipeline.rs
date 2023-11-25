use std::{path::Path, rc::Rc};

use ash::vk;

use crate::{
    error::RenderResult,
    vulkan_objects::{extent_helper, Device, ShaderCreate, ShaderModule},
};

pub trait PipelineBuilder<'a, P: AsRef<Path>> {
    fn device(&self) -> Rc<Device>;
    fn vertex_spv_path(&self) -> P;
    fn frag_spv_path(&self) -> P;
    fn extent(&self) -> vk::Extent2D;
    fn render_pass(&self) -> vk::RenderPass;
    fn vertex_binding_descriptions(&self) -> &'a [vk::VertexInputBindingDescription];
    fn vertex_attribute_descriptions(&self) -> &'a [vk::VertexInputAttributeDescription];
    fn pipeline_layout(&self) -> vk::PipelineLayout;

    fn subpass(&self) -> u32 {
        0
    }

    fn pipeline_cache(&self) -> vk::PipelineCache {
        vk::PipelineCache::null()
    }

    fn vertex_input_state_create_info(&self) -> vk::PipelineVertexInputStateCreateInfo {
        vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(self.vertex_binding_descriptions())
            .vertex_attribute_descriptions(self.vertex_attribute_descriptions())
            .build()
    }

    fn shader_stage_create_infos(
        &self,
    ) -> RenderResult<(Vec<vk::PipelineShaderStageCreateInfo>, Vec<ShaderModule>)> {
        let shader_creates = vec![
            ShaderCreate::with_spv_path_default_start_name(
                self.vertex_spv_path(),
                vk::ShaderStageFlags::VERTEX,
                self.device(),
            )?,
            ShaderCreate::with_spv_path_default_start_name(
                self.frag_spv_path(),
                vk::ShaderStageFlags::FRAGMENT,
                self.device(),
            )?,
        ];
        let mut infos = vec![];
        let mut modules = vec![];
        shader_creates.into_iter().for_each(|sc| {
            infos.push(sc.stage_create_info);
            modules.push(sc.module)
        });
        Ok((infos, modules))
    }

    fn dynamic_state_create_info(&self) -> vk::PipelineDynamicStateCreateInfo {
        vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR])
            .build()
    }

    fn input_assembly_state_create_info(&self) -> vk::PipelineInputAssemblyStateCreateInfo {
        vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false)
            .build()
    }

    fn viewport_state_create_info(&self) -> vk::PipelineViewportStateCreateInfo {
        vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&[extent_helper::viewport_from_extent(self.extent())])
            .scissors(&[extent_helper::scissor_from_extent(self.extent())])
            .build()
    }

    fn rasterization_state_create_info(&self) -> vk::PipelineRasterizationStateCreateInfo {
        vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false)
            .build()
    }

    fn multisample_state_create_info(&self) -> vk::PipelineMultisampleStateCreateInfo {
        vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .build()
    }

    fn color_blend_attach_state(&self) -> vk::PipelineColorBlendAttachmentState {
        vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)
            .build()
    }

    fn color_blend_state_create_info(
        &self,
        attach_states: &[vk::PipelineColorBlendAttachmentState],
    ) -> vk::PipelineColorBlendStateCreateInfo {
        vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .attachments(attach_states)
            .build()
    }

    fn depth_stencil_state_create_info(&self) -> vk::PipelineDepthStencilStateCreateInfo {
        vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false)
            .build()
    }

    fn build(&self) -> RenderResult<(vk::PipelineLayout, vk::Pipeline)> {
        let layout = self.pipeline_layout();
        let (shader_infos, _shader_modules) = self.shader_stage_create_infos()?;
        let color_blend_attach_state = self.color_blend_attach_state();

        let create_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_infos)
            .vertex_input_state(&self.vertex_input_state_create_info())
            .input_assembly_state(&self.input_assembly_state_create_info())
            .viewport_state(&self.viewport_state_create_info())
            .rasterization_state(&self.rasterization_state_create_info())
            .multisample_state(&self.multisample_state_create_info())
            .color_blend_state(&self.color_blend_state_create_info(&[color_blend_attach_state]))
            .dynamic_state(&self.dynamic_state_create_info())
            .layout(layout)
            .render_pass(self.render_pass())
            .subpass(self.subpass())
            .depth_stencil_state(&self.depth_stencil_state_create_info())
            .build();

        let pipeline = unsafe {
            self.device()
                .create_graphics_pipelines(self.pipeline_cache(), &[create_info], None)
                .map_err(|e| e.1)?[0]
        };

        Ok((layout, pipeline))
    }
}

#[macro_export]
macro_rules! impl_pipeline_builder_fns {
    () => {
        fn device(&self) -> Rc<Device> {
            self.device.clone()
        }

        fn extent(&self) -> vk::Extent2D {
            self.extent
        }

        fn render_pass(&self) -> vk::RenderPass {
            self.render_pass
        }

        fn pipeline_cache(&self) -> vk::PipelineCache {
            self.pipeline_cache
        }

        fn vertex_binding_descriptions(&self) -> &'a [vk::VertexInputBindingDescription] {
            self.vertex_bindings
        }

        fn vertex_attribute_descriptions(&self) -> &'a [vk::VertexInputAttributeDescription] {
            self.vertex_attributes
        }
    };
}

pub use impl_pipeline_builder_fns;
