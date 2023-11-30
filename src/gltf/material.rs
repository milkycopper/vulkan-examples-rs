use glam::Vec4;
use gltf_json::Index;

use crate::vulkan_wrappers::Texture;

pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

pub struct Material {
    pub alpha_mode: AlphaMode,
    pub alpha_cut_off: f32,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub base_color_factor: Vec4,

    pub base_color_texture: Option<Index<Texture>>,
    pub metallic_roughness_texture: Option<Index<Texture>>,
    pub normal_texture: Option<Index<Texture>>,
    pub occlusion_texture: Option<Index<Texture>>,
    pub emissive_texture: Option<Index<Texture>>,
    pub specular_glossiness_texture: Option<Index<Texture>>,
    pub diffuse_texture: Option<Index<Texture>>,
}

impl<'a> Default for Material {
    fn default() -> Self {
        Material {
            alpha_mode: AlphaMode::Opaque,
            alpha_cut_off: 1.,
            metallic_factor: 1.,
            roughness_factor: 1.,
            base_color_factor: Vec4::ONE,
            base_color_texture: None,
            metallic_roughness_texture: None,
            normal_texture: None,
            occlusion_texture: None,
            emissive_texture: None,
            specular_glossiness_texture: None,
            diffuse_texture: None,
        }
    }
}
