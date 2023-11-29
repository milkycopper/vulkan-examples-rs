use std::{
    borrow::Cow,
    ffi::{c_char, CStr, CString},
    ops::Deref,
    rc::{Rc, Weak},
};

use ash::{extensions::ext::DebugUtils, vk, Entry};
use raw_window_handle::HasRawDisplayHandle;
use winit::window::Window;

use crate::error::{RenderError, RenderResult};

const VALIDATION_LAYER_NAME: &CStr =
    unsafe { CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") };

#[derive(Clone, Copy, Debug)]
pub enum VulkanDebugInfoStrategy {
    Idle,
    PrintAll(vk::PFN_vkDebugUtilsMessengerCallbackEXT),
    PanicOnErrorsPrintOthers(vk::PFN_vkDebugUtilsMessengerCallbackEXT),
}

impl VulkanDebugInfoStrategy {
    pub const DEFAULT_PRINT_ALL: Self = Self::PrintAll(Some(vulkan_debug_callback_print_all));
    pub const DEFAULT_PANIC_ON_ERRORS: Self =
        Self::PanicOnErrorsPrintOthers(Some(vulkan_debug_callback_panic_on_errors_print_others));
}

#[derive(Clone, Copy, Debug)]
pub enum VulkanApiVersion {
    V1_0,
    V1_1,
    V1_2,
    V1_3,
}

pub struct InstanceBuilder<'a> {
    window: Option<&'a Window>,
    app_name: Option<&'a str>,
    engine_name: Option<&'a str>,
    app_version: u32,
    engine_version: u32,
    vulkan_api_version: VulkanApiVersion,
    debug_strategy: VulkanDebugInfoStrategy,
}

impl<'a> Default for InstanceBuilder<'a> {
    fn default() -> Self {
        Self {
            window: None,
            app_name: None,
            engine_name: None,
            app_version: 0,
            engine_version: 0,
            vulkan_api_version: VulkanApiVersion::V1_0,
            debug_strategy: VulkanDebugInfoStrategy::Idle,
        }
    }
}

impl<'a> InstanceBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window(mut self, window: &'a Window) -> Self {
        self.window = Some(window);
        self
    }

    pub fn app_name_and_version(mut self, name: &'a str, version: u32) -> Self {
        self.app_name = Some(name);
        self.app_version = version;
        self
    }

    pub fn engine_name_and_version(mut self, name: &'a str, version: u32) -> Self {
        self.engine_name = Some(name);
        self.engine_version = version;
        self
    }

    pub fn vulkan_api_version(mut self, version: VulkanApiVersion) -> Self {
        self.vulkan_api_version = version;
        self
    }

    pub fn debug_strategy(mut self, strategy: VulkanDebugInfoStrategy) -> Self {
        self.debug_strategy = strategy;
        self
    }

    pub fn build(&self) -> RenderResult<Instance> {
        let (mut extensions, support_window) = if let Some(window) = self.window {
            (
                ash_window::enumerate_required_extensions(window.raw_display_handle())?.to_vec(),
                true,
            )
        } else {
            (vec![], false)
        };

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        [
            vk::KhrPortabilityEnumerationFn::name().as_ptr(),
            // Enabling this extension is a requirement when using `VK_KHR_portability_subset`
            vk::KhrGetPhysicalDeviceProperties2Fn::name().as_ptr(),
        ]
        .into_iter()
        .for_each(|x| extensions.push(x));

        match self.debug_strategy {
            VulkanDebugInfoStrategy::Idle => {}
            _ => extensions.push(DebugUtils::name().as_ptr()),
        }

        let app_info = vk::ApplicationInfo::builder()
            .application_name(&CString::new(self.app_name.unwrap_or("")).unwrap())
            .application_version(self.app_version)
            .engine_name(&CString::new(self.engine_name.unwrap_or("No Engine")).unwrap())
            .engine_version(self.engine_version)
            .api_version(match self.vulkan_api_version {
                VulkanApiVersion::V1_0 => vk::API_VERSION_1_0,
                VulkanApiVersion::V1_1 => vk::API_VERSION_1_1,
                VulkanApiVersion::V1_2 => vk::API_VERSION_1_2,
                VulkanApiVersion::V1_3 => vk::API_VERSION_1_3,
            })
            .build();

        let layer_names: Vec<*const c_char> = match self.debug_strategy {
            VulkanDebugInfoStrategy::Idle => vec![],
            _ => vec![VALIDATION_LAYER_NAME.as_ptr()],
        };

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let create_flags = vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        let create_flags = vk::InstanceCreateFlags::default();

        let instance_create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layer_names)
            .flags(create_flags)
            .build();

        let entry = Entry::linked();
        let vk_instance = unsafe { entry.create_instance(&instance_create_info, None)? };

        let debug_worker = match self.debug_strategy {
            VulkanDebugInfoStrategy::Idle => None,
            VulkanDebugInfoStrategy::PrintAll(p_fn)
            | VulkanDebugInfoStrategy::PanicOnErrorsPrintOthers(p_fn) => {
                let debug_utils_loader = DebugUtils::new(&entry, &vk_instance);
                let messenger_create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                    .message_severity(
                        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                            | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                    )
                    .message_type(
                        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                    )
                    .pfn_user_callback(p_fn)
                    .build();
                let debug_messenger = unsafe {
                    debug_utils_loader.create_debug_utils_messenger(&messenger_create_info, None)?
                };
                Some((debug_utils_loader, debug_messenger))
            }
        };

        let physical_devices = {
            let devices = unsafe { vk_instance.enumerate_physical_devices()? };
            let mut collection = PhysicalDeviceCollection::default();
            for device in devices {
                let property = unsafe { vk_instance.get_physical_device_properties(device) };
                if property.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                    collection.discrete.push(Rc::new(device));
                } else if property.device_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
                    collection.integrated.push(Rc::new(device));
                } else if property.device_type == vk::PhysicalDeviceType::CPU {
                    collection.cpu.push(Rc::new(device));
                }
            }
            if collection.is_empty() {
                return Err(RenderError::PhysicalDeviceNotSupported(
                    "Fail to find available physical device".to_string(),
                ));
            }

            collection
        };

        Ok(Instance {
            inner: vk_instance,
            entry,
            debug_worker,
            physical_devices,
            app_name_and_version: self
                .app_name
                .map(|name| (name.to_string(), self.app_version)),
            engine_name_and_version: self
                .engine_name
                .map(|name| (name.to_string(), self.engine_version)),
            vulkan_api_version: self.vulkan_api_version,
            support_window,
        })
    }
}

pub struct Instance {
    inner: ash::Instance,
    entry: Entry,
    physical_devices: PhysicalDeviceCollection,
    app_name_and_version: Option<(String, u32)>,
    engine_name_and_version: Option<(String, u32)>,
    debug_worker: Option<(DebugUtils, vk::DebugUtilsMessengerEXT)>,
    vulkan_api_version: VulkanApiVersion,
    support_window: bool,
}

impl Instance {
    pub fn builder<'a>() -> InstanceBuilder<'a> {
        InstanceBuilder::new()
    }

    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    pub fn pick_physical_device(&self) -> Weak<vk::PhysicalDevice> {
        Rc::downgrade(&self.physical_devices.pick_first().unwrap())
    }

    pub fn app_name_and_version(&self) -> &Option<(String, u32)> {
        &self.app_name_and_version
    }

    pub fn engine_name_and_version(&self) -> &Option<(String, u32)> {
        &self.engine_name_and_version
    }

    pub fn validation_layer_enabled(&self) -> bool {
        self.debug_worker.is_some()
    }

    pub fn vulkan_api_version(&self) -> VulkanApiVersion {
        self.vulkan_api_version
    }

    pub fn support_window(&self) -> bool {
        self.support_window
    }
}

impl Deref for Instance {
    type Target = ash::Instance;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.physical_devices.check_can_be_freed();
        unsafe {
            if let Some((debug_utils, debug_messenger)) = self.debug_worker.as_ref() {
                debug_utils.destroy_debug_utils_messenger(*debug_messenger, None)
            }
            self.destroy_instance(None);
        }
    }
}

#[derive(Default)]
struct PhysicalDeviceCollection {
    discrete: Vec<Rc<vk::PhysicalDevice>>,
    integrated: Vec<Rc<vk::PhysicalDevice>>,
    cpu: Vec<Rc<vk::PhysicalDevice>>,
}

impl PhysicalDeviceCollection {
    fn is_empty(&self) -> bool {
        self.discrete.is_empty() && self.integrated.is_empty() && self.cpu.is_empty()
    }

    fn pick_first(&self) -> Option<Rc<vk::PhysicalDevice>> {
        self.chained_iter().next().map(Clone::clone)
    }

    fn check_can_be_freed(&self) {
        self.chained_iter()
            .for_each(|pd| assert!(Rc::strong_count(pd) == 1));
    }

    fn chained_iter(&self) -> impl Iterator<Item = &Rc<vk::PhysicalDevice>> {
        self.discrete
            .iter()
            .chain(self.integrated.iter())
            .chain(self.cpu.iter())
    }
}

unsafe extern "system" fn vulkan_debug_callback_print_all(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}

unsafe extern "system" fn vulkan_debug_callback_panic_on_errors_print_others(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    if message_severity == vk::DebugUtilsMessageSeverityFlagsEXT::ERROR {
        panic!(
            "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
        );
    } else {
        println!(
            "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
        );
    }

    vk::FALSE
}
