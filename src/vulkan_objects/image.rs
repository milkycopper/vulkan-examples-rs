use std::path::Path;
use std::rc::Rc;

use ash::{prelude::VkResult, vk};

use super::{Buffer, Device, OneTimeCommand};
use crate::error::{RenderError, RenderResult};

pub struct ImageBuffer {
    size_in_bytes: vk::DeviceSize,
    image: vk::Image,
    device_momory: vk::DeviceMemory,
    format: vk::Format,
    device: Rc<Device>,
}

impl ImageBuffer {
    pub fn new(
        width: u32,
        height: u32,
        format: vk::Format,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
        memory_property: vk::MemoryPropertyFlags,
        device: Rc<Device>,
    ) -> RenderResult<Self> {
        let create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(
                vk::Extent3D::builder()
                    .width(width)
                    .height(height)
                    .depth(1)
                    .build(),
            )
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(tiling)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .build();

        unsafe {
            let image = device.create_image(&create_info, None)?;
            let memory_requirement = device.get_image_memory_requirements(image);
            let memory_alloc_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(memory_requirement.size)
                .memory_type_index(super::memory_helper::find_memory_type(
                    &device,
                    &memory_requirement,
                    memory_property,
                )?)
                .build();
            let device_momory = device.allocate_memory(&memory_alloc_info, None)?;

            device.bind_image_memory(image, device_momory, 0)?;

            Ok(ImageBuffer {
                size_in_bytes: memory_requirement.size,
                image,
                device_momory,
                format,
                device,
            })
        }
    }

    pub fn size_in_bytes(&self) -> vk::DeviceSize {
        self.size_in_bytes
    }

    pub fn vk_image(&self) -> vk::Image {
        self.image
    }

    pub fn format(&self) -> vk::Format {
        self.format
    }

    pub fn create_image_view(
        &self,
        format: vk::Format,
        aspect_flags: vk::ImageAspectFlags,
    ) -> VkResult<vk::ImageView> {
        image_helper::create_image_view(self.image, format, aspect_flags, &self.device)
    }

    pub fn transition_layout(
        &self,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        aspect_flags: vk::ImageAspectFlags,
        command_pool: &vk::CommandPool,
        queue: &vk::Queue,
    ) -> RenderResult<()> {
        let (src_access_mask, dst_access_mask, src_stage_mask, dst_stage_mask) = if old_layout
            == vk::ImageLayout::UNDEFINED
            && new_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
        {
            (
                vk::AccessFlags::NONE,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
            )
        } else if old_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL
            && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
        {
            (
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            )
        } else {
            return Err(RenderError::LayoutTransitionNotSupported(
                "Unsupported layout transition".to_string(),
            ));
        };

        let barrier = vk::ImageMemoryBarrier::builder()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.image)
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(aspect_flags)
                    .base_mip_level(0)
                    .layer_count(1)
                    .base_array_layer(0)
                    .level_count(1)
                    .build(),
            )
            .src_access_mask(src_access_mask)
            .dst_access_mask(dst_access_mask)
            .build();

        OneTimeCommand::new(self.device.clone(), command_pool)?.take_and_execute(
            |command| unsafe {
                self.device.cmd_pipeline_barrier(
                    *command.vk_command_buffer(),
                    src_stage_mask,
                    dst_stage_mask,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                );
                Ok(())
            },
            queue,
        )?;

        Ok(())
    }

    pub fn color_image_from_file<P: AsRef<Path>>(
        path: P,
        device: Rc<Device>,
        command_pool: &vk::CommandPool,
        queue: &vk::Queue,
    ) -> RenderResult<Self> {
        let image_data = image_loader::io::Reader::open(&path)?.decode()?.to_rgba8();
        let size = image_data.len();
        let image_buffer = Self::new(
            image_data.width(),
            image_data.height(),
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device.clone(),
        )?;

        let staging_buffer = {
            let buffer = Buffer::<u8>::new(
                size,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                device.clone(),
            )?;

            unsafe {
                let data_ptr = device.map_memory(
                    buffer.vk_device_momory(),
                    0,
                    buffer.size_in_bytes(),
                    vk::MemoryMapFlags::default(),
                )?;
                std::ptr::copy_nonoverlapping(
                    image_data.to_vec().as_ptr(),
                    data_ptr as *mut u8,
                    size,
                );
                device.unmap_memory(buffer.vk_device_momory());
            };
            buffer
        };

        image_buffer.transition_layout(
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageAspectFlags::COLOR,
            command_pool,
            queue,
        )?;

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
                    .width(image_data.width())
                    .height(image_data.height())
                    .depth(1)
                    .build(),
            )
            .build();

        OneTimeCommand::new(device.clone(), command_pool)?.take_and_execute(
            |command| unsafe {
                device.cmd_copy_buffer_to_image(
                    *command.vk_command_buffer(),
                    staging_buffer.vk_buffer(),
                    image_buffer.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[image_copy],
                );
                Ok(())
            },
            queue,
        )?;

        image_buffer.transition_layout(
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::ImageAspectFlags::COLOR,
            command_pool,
            queue,
        )?;

        Ok(image_buffer)
    }

    pub fn create_depth_buffer(extent: vk::Extent2D, device: Rc<Device>) -> RenderResult<Self> {
        Self::new(
            extent.width,
            extent.height,
            format_helper::find_depth_format(&device)?,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device,
        )
    }
}

impl Drop for ImageBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.device_momory, None)
        }
    }
}

pub mod image_helper {
    use super::*;

    pub fn create_image_view(
        image: vk::Image,
        format: vk::Format,
        aspect_flags: vk::ImageAspectFlags,
        device: &Device,
    ) -> VkResult<vk::ImageView> {
        let create_info = vk::ImageViewCreateInfo::builder()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(aspect_flags)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            )
            .build();
        unsafe { device.create_image_view(&create_info, None) }
    }

    pub fn create_texture_sampler(device: &Device) -> VkResult<vk::Sampler> {
        let create_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .anisotropy_enable(true)
            .max_anisotropy(unsafe {
                device
                    .instance()
                    .get_physical_device_properties(*device.physical_device().upgrade().unwrap())
                    .limits
                    .max_sampler_anisotropy
            })
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.)
            .max_lod(0.)
            .min_lod(0.)
            .build();

        unsafe { device.create_sampler(&create_info, None) }
    }
}

pub mod format_helper {
    use super::*;

    pub fn filter_supported_format(
        candidates: &Vec<vk::Format>,
        tiling: vk::ImageTiling,
        features: vk::FormatFeatureFlags,
        device: &Device,
    ) -> RenderResult<vk::Format> {
        unsafe {
            for format in candidates {
                let format_property = device.instance().get_physical_device_format_properties(
                    *device.physical_device().upgrade().unwrap(),
                    *format,
                );

                if (tiling == vk::ImageTiling::LINEAR
                    && (format_property.linear_tiling_features & features) == features)
                    || (tiling == vk::ImageTiling::OPTIMAL
                        && (format_property.optimal_tiling_features & features) == features)
                {
                    return Ok(*format);
                }
            }

            Err(RenderError::FormatNotSupported(
                "Failed to find supported format".to_string(),
            ))
        }
    }

    pub fn find_depth_format(device: &Device) -> RenderResult<vk::Format> {
        filter_supported_format(
            &vec![
                vk::Format::D32_SFLOAT,
                vk::Format::D32_SFLOAT_S8_UINT,
                vk::Format::D24_UNORM_S8_UINT,
            ],
            vk::ImageTiling::OPTIMAL,
            vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
            device,
        )
    }

    pub fn has_stencil_component(format: vk::Format) -> bool {
        format == vk::Format::D32_SFLOAT_S8_UINT || format == vk::Format::D24_UNORM_S8_UINT
    }
}
