use std::rc::{Rc, Weak};

use ash::{extensions::khr::Swapchain as SwapChainLoader, prelude::VkResult, vk};

use super::{Device, Surface};

pub struct SwapChainBatch {
    loader: SwapChainLoader,
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    device: Rc<Device>,
    surface: Rc<Surface>,
}

impl SwapChainBatch {
    pub fn new(surface: Rc<Surface>, device: Rc<Device>) -> VkResult<Self> {
        assert!(Rc::ptr_eq(surface.instance(), device.instance()));
        assert!(Weak::ptr_eq(
            surface.physical_device(),
            device.physical_device()
        ));
        let loader = SwapChainLoader::new(device.instance(), &device);
        let (swapchain, images, image_views) =
            create_swapchain_image_and_views(&surface, &device, &loader)?;
        Ok(Self {
            loader,
            swapchain,
            images,
            image_views,
            device,
            surface,
        })
    }

    pub fn recreate(&mut self) -> VkResult<()> {
        self.dispose_gpu_resources();

        (self.swapchain, self.images, self.image_views) =
            create_swapchain_image_and_views(&self.surface, &self.device, &self.loader)?;

        Ok(())
    }

    pub fn acquire_next_image(&self, signal_semaphore: vk::Semaphore) -> VkResult<(u32, bool)> {
        unsafe {
            self.loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                signal_semaphore,
                vk::Fence::null(),
            )
        }
    }

    pub fn queue_present(
        &self,
        image_index: u32,
        wait_semaphores: &[vk::Semaphore],
        queue: &vk::Queue,
    ) -> VkResult<bool> {
        let present_info_khr = vk::PresentInfoKHR::builder()
            .wait_semaphores(wait_semaphores)
            .swapchains(&[self.swapchain])
            .image_indices(&[image_index])
            .build();

        unsafe { self.loader.queue_present(*queue, &present_info_khr) }
    }

    pub fn loader(&self) -> &SwapChainLoader {
        &self.loader
    }

    pub fn swapchain_khr(&self) -> &vk::SwapchainKHR {
        &self.swapchain
    }

    pub fn images(&self) -> &Vec<vk::Image> {
        &self.images
    }

    pub fn image_views(&self) -> &Vec<vk::ImageView> {
        &self.image_views
    }

    fn dispose_gpu_resources(&self) {
        unsafe {
            self.image_views
                .iter()
                .for_each(|view| self.device.destroy_image_view(*view, None));
            self.loader.destroy_swapchain(self.swapchain, None);
        };
    }
}

impl Drop for SwapChainBatch {
    fn drop(&mut self) {
        self.dispose_gpu_resources()
    }
}

fn create_swapchain_image_and_views(
    surface: &Surface,
    device: &Device,
    loader: &SwapChainLoader,
) -> VkResult<(vk::SwapchainKHR, Vec<vk::Image>, Vec<vk::ImageView>)> {
    let swapchain = create_swapchain(loader, surface, &device.queue_family_indices())?;
    let images = unsafe { loader.get_swapchain_images(swapchain)? };
    let mut image_views = vec![];
    for image in &images {
        let create_info = vk::ImageViewCreateInfo::builder()
            .image(*image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(surface.format())
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            )
            .build();
        image_views.push(unsafe { device.create_image_view(&create_info, None)? })
    }

    Ok((swapchain, images, image_views))
}

fn create_swapchain(
    swapchain_loader: &SwapChainLoader,
    surface: &Surface,
    family_indices: &Vec<u32>,
) -> VkResult<vk::SwapchainKHR> {
    let create_info = vk::SwapchainCreateInfoKHR::builder()
        .surface(*surface.surface_khr())
        .min_image_count(
            surface
                .capabilities()
                .max_image_count
                .min(surface.capabilities().min_image_count + 1),
        )
        .image_format(surface.format())
        .image_color_space(surface.color_space())
        .image_extent(surface.extent())
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(if family_indices.len() > 1 {
            vk::SharingMode::CONCURRENT
        } else {
            vk::SharingMode::EXCLUSIVE
        })
        .queue_family_indices(family_indices)
        .pre_transform(surface.capabilities().current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(surface.present_mode())
        .clipped(false)
        .old_swapchain(vk::SwapchainKHR::null())
        .build();

    Ok(unsafe { swapchain_loader.create_swapchain(&create_info, None)? })
}
