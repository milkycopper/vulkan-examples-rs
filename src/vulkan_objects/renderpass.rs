use ash::vk;

pub mod renderpass_helper {
    use super::*;

    pub fn create_renderpass_begin_info(
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
