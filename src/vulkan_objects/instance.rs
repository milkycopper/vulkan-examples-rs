use std::{
    ffi::{c_char, CString},
    ops::Deref,
    rc::{Rc, Weak},
};

#[cfg(feature = "vk_validation_layer")]
use std::borrow::Cow;
#[cfg(feature = "vk_validation_layer")]
use std::ffi::CStr;

#[cfg(feature = "vk_validation_layer")]
use ash::extensions::ext::DebugUtils;
use ash::{vk, Entry};
use raw_window_handle::HasRawDisplayHandle;
use winit::window::Window;

use crate::error::{RenderError, RenderResult};

pub struct InstanceBuilder<'a> {
    window: &'a Window,
    app_name: Option<&'a str>,
    engine_name: Option<&'a str>,
    app_version: u32,
    engine_version: u32,
}

impl<'a> InstanceBuilder<'a> {
    pub fn new(window: &'a Window) -> Self {
        Self {
            window,
            app_name: None,
            engine_name: None,
            app_version: 0,
            engine_version: 0,
        }
    }

    pub fn with_app_name_and_version(mut self, name: &'a str, version: u32) -> Self {
        self.app_name = Some(name);
        self.app_version = version;
        self
    }

    pub fn with_engine_name_and_version(mut self, name: &'a str, version: u32) -> Self {
        self.engine_name = Some(name);
        self.engine_version = version;
        self
    }

    pub fn build(&self) -> RenderResult<Instance> {
        let extensions = [
            ash_window::enumerate_required_extensions(self.window.raw_display_handle())?.to_vec(),
            #[cfg(feature = "vk_validation_layer")]
            vec![DebugUtils::name().as_ptr()],
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            vec![
                vk::KhrPortabilityEnumerationFn::name().as_ptr(),
                // Enabling this extension is a requirement when using `VK_KHR_portability_subset`
                vk::KhrGetPhysicalDeviceProperties2Fn::name().as_ptr(),
            ],
        ]
        .concat();

        Instance::new(
            self.app_name.unwrap_or(""),
            self.app_version,
            self.engine_name.unwrap_or(""),
            self.engine_version,
            &extensions,
        )
    }
}

pub struct Instance {
    inner: ash::Instance,
    entry: Entry,
    physical_devices: PhysicalDeviceCollection,
    app_name_and_version: (String, u32),
    engine_name_and_version: (String, u32),
    #[cfg(feature = "vk_validation_layer")]
    debug_utils: DebugUtils,
    #[cfg(feature = "vk_validation_layer")]
    debug_messenger: vk::DebugUtilsMessengerEXT,
}

impl Instance {
    fn new(
        application_name: &str,
        application_version: u32,
        engine_name: &str,
        engine_version: u32,
        enabled_extensions: &[*const c_char],
    ) -> RenderResult<Self> {
        let entry = Entry::linked();

        let app_info = vk::ApplicationInfo::builder()
            .application_name(&CString::new(application_name).unwrap())
            .application_version(application_version)
            .engine_name(&CString::new(engine_name).unwrap())
            .engine_version(engine_version)
            .api_version(super::VULKAN_API_VERSION)
            .build();

        #[cfg(feature = "vk_validation_layer")]
        let layer_names: [*const c_char; 1] = unsafe {
            [CStr::from_bytes_with_nul_unchecked(
                b"VK_LAYER_KHRONOS_validation\0",
            )]
            .map(|raw_name| raw_name.as_ptr())
        };
        #[cfg(not(feature = "vk_validation_layer"))]
        let layer_names: [*const c_char; 0] = [];

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let create_flags = vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        let create_flags = vk::InstanceCreateFlags::default();

        let instance_create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(enabled_extensions)
            .enabled_layer_names(&layer_names)
            .flags(create_flags)
            .build();

        let vk_instance = unsafe { entry.create_instance(&instance_create_info, None)? };

        #[cfg(feature = "vk_validation_layer")]
        let (debug_utils, debug_messenger) = {
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
                .pfn_user_callback(Some(vulkan_debug_callback))
                .build();
            let debug_messenger = unsafe {
                debug_utils_loader.create_debug_utils_messenger(&messenger_create_info, None)?
            };
            (debug_utils_loader, debug_messenger)
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
                    "Fail to find suitable physical device".to_string(),
                ));
            }

            collection
        };

        Ok(Instance {
            inner: vk_instance,
            entry,
            #[cfg(feature = "vk_validation_layer")]
            debug_utils,
            #[cfg(feature = "vk_validation_layer")]
            debug_messenger,
            physical_devices,
            app_name_and_version: (application_name.to_string(), application_version),
            engine_name_and_version: (engine_name.to_string(), engine_version),
        })
    }

    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    pub fn pick_physical_device(&self) -> Weak<vk::PhysicalDevice> {
        Rc::downgrade(&self.physical_devices.pick_first().unwrap())
    }

    pub fn app_name_and_version(&self) -> (String, u32) {
        self.app_name_and_version.clone()
    }

    pub fn engine_name_and_version(&self) -> (String, u32) {
        self.engine_name_and_version.clone()
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
        unsafe {
            self.physical_devices.check_can_be_freed();
            #[cfg(feature = "vk_validation_layer")]
            self.debug_utils
                .destroy_debug_utils_messenger(self.debug_messenger, None);
            self.destroy_instance(None);
        }
    }
}

#[cfg(feature = "vk_validation_layer")]
unsafe extern "system" fn vulkan_debug_callback(
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

#[derive(Default)]
struct PhysicalDeviceCollection {
    discrete: Vec<Rc<vk::PhysicalDevice>>,
    integrated: Vec<Rc<vk::PhysicalDevice>>,
    cpu: Vec<Rc<vk::PhysicalDevice>>,
}

impl PhysicalDeviceCollection {
    pub fn is_empty(&self) -> bool {
        self.discrete.is_empty() && self.integrated.is_empty() && self.cpu.is_empty()
    }

    pub fn pick_first(&self) -> Option<Rc<vk::PhysicalDevice>> {
        self.chained_iter().next().map(Clone::clone)
    }

    pub fn check_can_be_freed(&self) {
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
