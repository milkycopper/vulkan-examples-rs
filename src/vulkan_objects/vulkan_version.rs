use ash::vk;

#[cfg(all(
    feature = "vk_api_version_1_0",
    not(any(
        feature = "vk_api_version_1_1",
        feature = "vk_api_version_1_2",
        feature = "vk_api_version_1_3"
    ))
))]
pub const VULKAN_API_VERSION: u32 = vk::API_VERSION_1_0;

#[cfg(all(
    feature = "vk_api_version_1_1",
    not(any(
        feature = "vk_api_version_1_0",
        feature = "vk_api_version_1_2",
        feature = "vk_api_version_1_3"
    ))
))]
pub const VULKAN_API_VERSION: u32 = vk::API_VERSION_1_1;

#[cfg(all(
    feature = "vk_api_version_1_2",
    not(any(
        feature = "vk_api_version_1_0",
        feature = "vk_api_version_1_1",
        feature = "vk_api_version_1_3"
    ))
))]
pub const VULKAN_API_VERSION: u32 = vk::API_VERSION_1_2;

#[cfg(all(
    feature = "vk_api_version_1_3",
    not(any(
        feature = "vk_api_version_1_0",
        feature = "vk_api_version_1_1",
        feature = "vk_api_version_1_2"
    ))
))]
pub const VULKAN_API_VERSION: u32 = vk::API_VERSION_1_3;
