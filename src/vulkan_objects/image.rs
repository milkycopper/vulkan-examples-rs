use std::path::Path;
use std::rc::Rc;

use ash::{prelude::VkResult, vk};
use ktx::KtxInfo;

use super::{Buffer, Device, OneTimeCommand};
use crate::error::{RenderError, RenderResult};

pub struct TextureBuilder {
    width: u32,
    height: u32,
    extent_depth: u32,
    layout: vk::ImageLayout,
    mip_levels: u32,
    array_layers: u32,
    format: vk::Format,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
    device: Rc<Device>,
}

impl TextureBuilder {
    pub fn new(
        width: u32,
        height: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        device: Rc<Device>,
    ) -> Self {
        Self {
            width,
            height,
            extent_depth: 1,
            layout: vk::ImageLayout::UNDEFINED,
            mip_levels: 1,
            array_layers: 1,
            format,
            tiling: vk::ImageTiling::OPTIMAL,
            usage,
            device,
        }
    }

    pub fn extent_depth(mut self, depth: u32) -> Self {
        self.extent_depth = depth;
        self
    }

    pub fn mip_levels(mut self, mip_levels: u32) -> Self {
        self.mip_levels = mip_levels;
        self
    }

    pub fn array_layers(mut self, array_layers: u32) -> Self {
        self.array_layers = array_layers;
        self
    }

    pub fn image_layout(mut self, image_layout: vk::ImageLayout) -> Self {
        self.layout = image_layout;
        self
    }

    pub fn image_tiling(mut self, image_tiling: vk::ImageTiling) -> Self {
        self.tiling = image_tiling;
        self
    }

    pub fn build(&self) -> RenderResult<Texture> {
        Texture::new(
            self.width,
            self.height,
            self.extent_depth,
            self.layout,
            self.mip_levels,
            self.array_layers,
            self.format,
            self.tiling,
            self.usage,
            self.device.clone(),
        )
    }
}

pub struct Texture {
    size_in_bytes: vk::DeviceSize,
    image: vk::Image,
    device_momory: vk::DeviceMemory,
    image_layout: vk::ImageLayout,
    extent_2d: vk::Extent2D,
    extent_depth: u32,
    mip_levels: u32,
    array_layers: u32,
    format: vk::Format,
    image_view: Option<Rc<vk::ImageView>>,
    sampler: Option<Rc<vk::Sampler>>,
    device: Rc<Device>,
}

impl Texture {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: u32,
        height: u32,
        extent_depth: u32,
        layout: vk::ImageLayout,
        mip_levels: u32,
        array_layers: u32,
        format: vk::Format,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
        device: Rc<Device>,
    ) -> RenderResult<Self> {
        let create_info = vk::ImageCreateInfo::builder()
            .image_type(if extent_depth > 1 {
                vk::ImageType::TYPE_3D
            } else {
                vk::ImageType::TYPE_2D
            })
            .extent(
                vk::Extent3D::builder()
                    .width(width)
                    .height(height)
                    .depth(extent_depth)
                    .build(),
            )
            .mip_levels(mip_levels)
            .array_layers(array_layers)
            .format(format)
            .tiling(tiling)
            .initial_layout(layout)
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
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )?)
                .build();
            let device_momory = device.allocate_memory(&memory_alloc_info, None)?;

            device.bind_image_memory(image, device_momory, 0)?;

            Ok(Texture {
                size_in_bytes: memory_requirement.size,
                image,
                device_momory,
                image_layout: layout,
                extent_2d: vk::Extent2D::builder().width(width).height(height).build(),
                extent_depth,
                mip_levels,
                array_layers,
                format,
                image_view: None,
                sampler: None,
                device,
            })
        }
    }

    pub fn builder(
        width: u32,
        height: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        device: Rc<Device>,
    ) -> TextureBuilder {
        TextureBuilder::new(width, height, format, usage, device)
    }

    pub fn size_in_bytes(&self) -> vk::DeviceSize {
        self.size_in_bytes
    }

    pub fn image(&self) -> &vk::Image {
        &self.image
    }

    pub fn device_memory(&self) -> &vk::DeviceMemory {
        &self.device_momory
    }

    pub fn format(&self) -> vk::Format {
        self.format
    }

    pub fn extent2d(&self) -> vk::Extent2D {
        self.extent_2d
    }

    pub fn layout(&self) -> vk::ImageLayout {
        self.image_layout
    }

    pub fn image_view(&self) -> Option<Rc<vk::ImageView>> {
        self.image_view.clone()
    }

    pub fn set_image_view(&mut self, image_view: Rc<vk::ImageView>) {
        self.image_view = Some(image_view)
    }

    pub fn spawn_image_view(&mut self) -> VkResult<()> {
        let image_view = {
            let image_view_type = if self.extent_depth > 1 {
                vk::ImageViewType::TYPE_3D
            } else if self.array_layers > 1 {
                vk::ImageViewType::TYPE_2D_ARRAY
            } else {
                vk::ImageViewType::TYPE_2D
            };
            let create_info = vk::ImageViewCreateInfo::builder()
                .image(self.image)
                .view_type(image_view_type)
                .format(self.format)
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(self.mip_levels)
                        .base_array_layer(0)
                        .layer_count(self.array_layers)
                        .build(),
                )
                .build();
            unsafe { self.device.create_image_view(&create_info, None)? }
        };
        self.set_image_view(Rc::new(image_view));
        Ok(())
    }

    pub fn sampler(&self) -> Option<Rc<vk::Sampler>> {
        self.sampler.clone()
    }

    pub fn set_sampler(&mut self, sampler: Rc<vk::Sampler>) {
        self.sampler = Some(sampler)
    }

    pub fn spawn_sampler(&mut self, filter: vk::Filter) -> VkResult<()> {
        self.set_sampler(Rc::new(image_helper::create_texture_sampler(
            &self.device,
            filter,
        )?));
        Ok(())
    }

    pub fn descriptor(
        &self,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> vk::DescriptorImageInfo {
        vk::DescriptorImageInfo::builder()
            .image_layout(self.layout())
            .image_view(image_view)
            .sampler(sampler)
            .build()
    }

    pub fn descriptor_default(&self) -> vk::DescriptorImageInfo {
        self.descriptor(*self.image_view().unwrap(), *self.sampler().unwrap())
    }

    pub fn transition_layout(
        &mut self,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
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
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .layer_count(self.array_layers)
                    .base_array_layer(0)
                    .level_count(self.mip_levels)
                    .build(),
            )
            .src_access_mask(src_access_mask)
            .dst_access_mask(dst_access_mask)
            .build();

        OneTimeCommand::new(self.device.clone(), command_pool)?.take_and_execute(
            |command| unsafe {
                self.device.cmd_pipeline_barrier(
                    *command.command_buffer(),
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
        self.image_layout = new_layout;

        Ok(())
    }

    pub fn from_rgba8_picture<P: AsRef<Path>>(
        path: P,
        device: Rc<Device>,
        command_pool: &vk::CommandPool,
        queue: &vk::Queue,
    ) -> RenderResult<Self> {
        let image_data = image_loader::io::Reader::open(&path)?.decode()?.to_rgba8();
        let size = image_data.len();

        let mut texture = Self::builder(
            image_data.width(),
            image_data.height(),
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            device.clone(),
        )
        .build()?;

        let staging_buffer = {
            let mut buffer = Buffer::<u8>::new(
                size,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                device.clone(),
            )?;
            buffer.load_data(&image_data)?;
            buffer
        };

        texture.transition_layout(
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
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
                    *command.command_buffer(),
                    staging_buffer.buffer(),
                    texture.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[image_copy],
                );
                Ok(())
            },
            queue,
        )?;

        texture.transition_layout(
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            command_pool,
            queue,
        )?;

        Ok(texture)
    }

    pub fn from_ktx<P: AsRef<Path>>(
        path: P,
        device: Rc<Device>,
        command_pool: &vk::CommandPool,
        queue: &vk::Queue,
    ) -> RenderResult<(Self, u32)> {
        let buf_reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let decoder = ktx::Decoder::new(buf_reader)?;
        let (width, height) = (decoder.pixel_width(), decoder.pixel_height());
        let layer_count = {
            let x = decoder.array_elements();
            if x == 0 {
                1
            } else {
                x
            }
        };
        let data: Vec<Vec<u8>> = decoder.read_textures().collect();
        assert!(data.len() == 1);
        let data = data.concat();
        let size = data.len();
        let size_per_layer = size as u32 / layer_count;
        assert!(size_per_layer * layer_count == size as u32);

        let mut texture = Texture::builder(
            width,
            height,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            device.clone(),
        )
        .array_layers(layer_count)
        .build()?;

        let staging_buffer = {
            let mut buffer = Buffer::<u8>::new(
                size,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                device.clone(),
            )?;
            buffer.load_data(&data)?;
            buffer
        };

        texture.transition_layout(
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            command_pool,
            queue,
        )?;

        let image_copies = (0..layer_count)
            .map(|layer| {
                vk::BufferImageCopy::builder()
                    .image_subresource(
                        vk::ImageSubresourceLayers::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .mip_level(0)
                            .base_array_layer(layer)
                            .layer_count(1)
                            .build(),
                    )
                    .image_offset(vk::Offset3D::default())
                    .image_extent(
                        vk::Extent3D::builder()
                            .width(width)
                            .height(height)
                            .depth(1)
                            .build(),
                    )
                    .buffer_offset((size_per_layer * layer) as u64)
                    .build()
            })
            .collect::<Vec<_>>();

        OneTimeCommand::new(device.clone(), command_pool)?.take_and_execute(
            |command| unsafe {
                device.cmd_copy_buffer_to_image(
                    *command.command_buffer(),
                    staging_buffer.buffer(),
                    texture.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &image_copies,
                );
                Ok(())
            },
            queue,
        )?;

        texture.transition_layout(
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            command_pool,
            queue,
        )?;

        Ok((texture, layer_count))
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.device_momory, None);
            if let Some(view) = &self.image_view {
                if Rc::strong_count(view) == 1 {
                    self.device.destroy_image_view(**view, None);
                }
            }
            if let Some(sampler) = &self.sampler {
                if Rc::strong_count(sampler) == 1 {
                    self.device.destroy_sampler(**sampler, None);
                }
            }
        }
    }
}

pub struct DepthStencil(Texture);

impl DepthStencil {
    pub fn new(extent: vk::Extent2D, format: vk::Format, device: Rc<Device>) -> RenderResult<Self> {
        let mut buffer = Texture::builder(
            extent.width,
            extent.height,
            format,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            device.clone(),
        )
        .build()?;

        let image_view = {
            let create_info = vk::ImageViewCreateInfo::builder()
                .image(*buffer.image())
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(buffer.format())
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .base_mip_level(0)
                        .level_count(buffer.mip_levels)
                        .base_array_layer(0)
                        .layer_count(buffer.array_layers)
                        .build(),
                )
                .build();
            unsafe { buffer.device.create_image_view(&create_info, None)? }
        };

        buffer.set_image_view(Rc::new(image_view));
        Ok(DepthStencil(buffer))
    }

    pub fn buffer(&self) -> &Texture {
        &self.0
    }

    pub fn image_view(&self) -> Rc<vk::ImageView> {
        self.0.image_view().unwrap().clone()
    }

    pub fn format(&self) -> vk::Format {
        self.0.format()
    }
}

pub mod image_helper {
    use super::*;

    pub fn create_texture_sampler(device: &Device, filter: vk::Filter) -> VkResult<vk::Sampler> {
        let create_info = vk::SamplerCreateInfo::builder()
            .mag_filter(filter)
            .min_filter(filter)
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
            .compare_op(vk::CompareOp::NEVER)
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
