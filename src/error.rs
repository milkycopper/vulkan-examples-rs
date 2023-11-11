use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum RenderError {
    VkResult(ash::vk::Result),
    WindowCreateError(winit::error::OsError),
    IOError(std::io::Error),
    ImageError(image_loader::error::ImageError),
    ObjLoadError(tobj::LoadError),
    PhysicalDeviceNotSupported(String),
    FormatNotSupported(String),
    MemoryTypeNotSupported(String),
    LayoutTransitionNotSupported(String),
}

impl From<ash::vk::Result> for RenderError {
    fn from(value: ash::vk::Result) -> Self {
        Self::VkResult(value)
    }
}

impl From<winit::error::OsError> for RenderError {
    fn from(value: winit::error::OsError) -> Self {
        Self::WindowCreateError(value)
    }
}

impl From<std::io::Error> for RenderError {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}

impl From<image_loader::error::ImageError> for RenderError {
    fn from(value: image_loader::error::ImageError) -> Self {
        Self::ImageError(value)
    }
}

impl From<tobj::LoadError> for RenderError {
    fn from(value: tobj::LoadError) -> Self {
        Self::ObjLoadError(value)
    }
}

impl Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VkResult(res) => write!(f, "{res}"),
            Self::WindowCreateError(e) => write!(f, "{e}"),
            Self::IOError(e) => write!(f, "{e}"),
            Self::ImageError(e) => write!(f, "{e}"),
            Self::ObjLoadError(e) => write!(f, "{e}"),
            Self::PhysicalDeviceNotSupported(s) => write!(f, "PHYSICAL DEVICE NOT SUPPORTED: {s}"),
            Self::FormatNotSupported(s) => write!(f, "FORMAT NOT SUPPORTED: {s}"),
            Self::MemoryTypeNotSupported(s) => write!(f, "MEMORY TYPE NOT SUPPORTED: {s}"),
            Self::LayoutTransitionNotSupported(s) => {
                write!(f, "LAYOUT TRANSITION NOT SUPPORTED: {s}")
            }
        }
    }
}
impl Error for RenderError {}

pub type RenderResult<T> = std::result::Result<T, RenderError>;
