use std::path::Path;
use std::rc::Rc;
use std::{ffi::CStr, fs};

use ash::vk;

use super::Device;
use crate::error::RenderResult;

pub struct ShaderCreate {
    pub stage_create_info: vk::PipelineShaderStageCreateInfo,
    pub module: vk::ShaderModule,
    device: Rc<Device>,
}

impl ShaderCreate {
    pub const DEFAULT_SHADER_START_NAME: &CStr =
        unsafe { CStr::from_bytes_with_nul_unchecked(b"main\0") };

    pub fn with_spv_path<P: AsRef<Path>>(
        shader_spv_path: P,
        stage_flag: vk::ShaderStageFlags,
        start_name: &CStr,
        device: Rc<Device>,
    ) -> RenderResult<Self> {
        let binary = ash::util::read_spv(&mut fs::File::open(&shader_spv_path)?)?;
        let module = unsafe {
            device.create_shader_module(
                &vk::ShaderModuleCreateInfo::builder().code(&binary).build(),
                None,
            )?
        };
        let stage_create_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(stage_flag)
            .module(module)
            .name(start_name)
            .build();
        Ok(Self {
            stage_create_info,
            module,
            device,
        })
    }
}

impl Drop for ShaderCreate {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_shader_module(self.module, None);
        }
    }
}
