use std::{
    cell::{Ref, RefCell},
    rc::{Rc, Weak},
};

use ash::{extensions::khr::Surface as SurfaceLoader, vk};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::window::Window;

use super::Instance;
use crate::error::{RenderError, RenderResult};

pub struct SurfaceAttributes {
    capabilities: vk::SurfaceCapabilitiesKHR,
    format: vk::SurfaceFormatKHR,
    present_mode: vk::PresentModeKHR,
    extent: vk::Extent2D,
}

pub struct Surface {
    attributes: RefCell<SurfaceAttributes>,
    loader: SurfaceLoader,
    inner: vk::SurfaceKHR,
    instance: Rc<Instance>,
    physical_device: Weak<vk::PhysicalDevice>,
}

impl Surface {
    pub const DEFAULT_FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;

    pub fn new(window: &Window, instance: Rc<Instance>, format: vk::Format) -> RenderResult<Self> {
        let surface_khr = unsafe {
            ash_window::create_surface(
                instance.entry(),
                &instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
            )?
        };
        let loader = SurfaceLoader::new(instance.entry(), &instance);
        let physical_device = instance.pick_physical_device();
        let attributes = RefCell::new(get_surface_attrs(
            &surface_khr,
            &loader,
            format,
            &physical_device.upgrade().unwrap(),
            window,
        )?);

        Ok(Self {
            attributes,
            loader,
            inner: surface_khr,
            physical_device,
            instance,
        })
    }

    pub fn format(&self) -> vk::Format {
        self.attributes.borrow().format.format
    }

    pub fn color_space(&self) -> vk::ColorSpaceKHR {
        self.attributes.borrow().format.color_space
    }

    pub fn extent(&self) -> vk::Extent2D {
        self.attributes.borrow().extent
    }

    pub fn capabilities(&self) -> Ref<'_, vk::SurfaceCapabilitiesKHR> {
        Ref::map(self.attributes.borrow(), |x| &x.capabilities)
    }

    pub fn present_mode(&self) -> vk::PresentModeKHR {
        self.attributes.borrow().present_mode
    }

    pub fn refit_surface_attribute(&self, window: &Window) -> RenderResult<()> {
        *self.attributes.borrow_mut() = get_surface_attrs(
            &self.inner,
            &self.loader,
            self.format(),
            &self.physical_device.upgrade().unwrap(),
            window,
        )?;
        Ok(())
    }

    pub fn surface_khr(&self) -> &vk::SurfaceKHR {
        &self.inner
    }

    pub fn instance(&self) -> &Rc<Instance> {
        &self.instance
    }

    pub fn physical_device(&self) -> &Weak<vk::PhysicalDevice> {
        &self.physical_device
    }

    pub fn loader(&self) -> &SurfaceLoader {
        &self.loader
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface(self.inner, None) };
    }
}

fn get_surface_attrs(
    surface: &vk::SurfaceKHR,
    surface_loader: &SurfaceLoader,
    format: vk::Format,
    device: &vk::PhysicalDevice,
    window: &Window,
) -> RenderResult<SurfaceAttributes> {
    unsafe {
        let capabilities =
            surface_loader.get_physical_device_surface_capabilities(*device, *surface)?;
        let extent = extent_helper::get_window_extent(&capabilities, window);
        let format = surface_loader
            .get_physical_device_surface_formats(*device, *surface)?
            .into_iter()
            .find(|f| f.format == format && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .map_or_else(
                || {
                    Err(RenderError::FormatNotSupported(
                        "Fail at find suitable surface format".to_string(),
                    ))
                },
                Ok,
            )?;
        let present_mode = surface_loader
            .get_physical_device_surface_present_modes(*device, *surface)?
            .into_iter()
            .find(|mode| *mode == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO);

        Ok(SurfaceAttributes {
            capabilities,
            format,
            present_mode,
            extent,
        })
    }
}

pub mod extent_helper {
    use super::*;

    pub fn viewport_from_extent(extent: vk::Extent2D) -> vk::Viewport {
        vk::Viewport::builder()
            .x(0.)
            .y(0.)
            .width(extent.width as f32)
            .height(extent.height as f32)
            .min_depth(0.)
            .max_depth(1.)
            .build()
    }

    pub fn scissor_from_extent(extent: vk::Extent2D) -> vk::Rect2D {
        vk::Rect2D::builder()
            .offset(vk::Offset2D::default())
            .extent(extent)
            .build()
    }

    pub(super) fn get_window_extent(
        capabilities: &vk::SurfaceCapabilitiesKHR,
        window: &Window,
    ) -> vk::Extent2D {
        if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            let window_size = window.inner_size();
            let width = window_size.width.clamp(
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
            );
            let height = window_size.height.clamp(
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
            );
            vk::Extent2D::builder().width(width).height(height).build()
        }
    }
}
