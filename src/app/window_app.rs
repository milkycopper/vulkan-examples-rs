use std::{cell::RefCell, rc::Rc, time::SystemTime};

use ash::vk::{self, DescriptorSetLayoutBinding};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    platform::run_return::EventLoopExtRunReturn,
    window::{Window, WindowBuilder},
};

use super::FixedVulkanStuff;
use crate::{
    camera::Camera,
    error::RenderResult,
    transforms::Direction,
    vulkan_objects::{Device, Instance, VulkanApiVersion},
};

#[derive(Clone, Copy)]
pub struct ClearValue {
    pub color: vk::ClearValue,
    pub depth_stencil: vk::ClearValue,
}

impl ClearValue {
    pub fn to_array(&self) -> [vk::ClearValue; 2] {
        [self.color, self.depth_stencil]
    }
}

pub trait WindowApp {
    fn new(event_loop: &EventLoop<()>) -> Self;
    fn draw_frame(&mut self);

    fn on_window_resized(&mut self, size: PhysicalSize<u32>);
    fn window_title() -> String;
    fn window(&self) -> &Window;

    fn last_frame_time_stamp(&self) -> SystemTime;
    fn camera(&mut self) -> &mut Camera;

    fn descriptor_pool_sizes() -> Vec<vk::DescriptorPoolSize>;
    fn descriptor_set_layout_bindings() -> Vec<DescriptorSetLayoutBinding>;

    fn window_size(&self) -> PhysicalSize<u32> {
        self.window().inner_size()
    }

    fn build_window(event_loop: &EventLoop<()>) -> Window {
        WindowBuilder::new()
            .with_title(Self::window_title())
            .with_inner_size(PhysicalSize::new(1800, 1200))
            .build(event_loop)
            .expect("Fail to build a window")
    }

    fn render_loop(&mut self, event_loop: &RefCell<EventLoop<()>>) {
        event_loop
            .borrow_mut()
            .run_return(|event, _, control_flow| {
                control_flow.set_poll();
                match event {
                    Event::WindowEvent {
                        event:
                            WindowEvent::CloseRequested
                            | WindowEvent::KeyboardInput {
                                input:
                                    KeyboardInput {
                                        state: ElementState::Pressed,
                                        virtual_keycode: Some(VirtualKeyCode::Escape),
                                        ..
                                    },
                                ..
                            },
                        ..
                    } => control_flow.set_exit(),

                    Event::WindowEvent {
                        event: WindowEvent::Resized(size),
                        ..
                    } => self.on_window_resized(size),

                    Event::WindowEvent {
                        event:
                            WindowEvent::KeyboardInput {
                                input:
                                    KeyboardInput {
                                        state: ElementState::Pressed,
                                        virtual_keycode: Some(key_code),
                                        ..
                                    },
                                ..
                            },
                        ..
                    } => self.on_keyboard_input(key_code),

                    Event::MainEventsCleared => {
                        let size = self.window_size();
                        if size.width > 0 && size.height > 0 {
                            self.draw_frame();
                        }
                    }
                    _ => (),
                }
            });
    }

    fn run(&mut self, event_loop: &mut RefCell<EventLoop<()>>) {
        self.render_loop(event_loop);
    }

    fn clear_value() -> ClearValue {
        ClearValue {
            color: vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0., 0., 0., 1.],
                },
            },
            depth_stencil: vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.,
                    stencil: 0,
                },
            },
        }
    }

    fn on_keyboard_input(&mut self, key_code: VirtualKeyCode) {
        let duration = SystemTime::now()
            .duration_since(self.last_frame_time_stamp())
            .unwrap()
            .as_secs_f32();
        match key_code {
            VirtualKeyCode::W => self.camera().translate_in_time(Direction::Up, duration),
            VirtualKeyCode::S => self.camera().translate_in_time(Direction::Down, duration),
            VirtualKeyCode::A => self.camera().translate_in_time(Direction::Left, duration),
            VirtualKeyCode::D => self.camera().translate_in_time(Direction::Right, duration),
            VirtualKeyCode::Q => self.camera().translate_in_time(Direction::Front, duration),
            VirtualKeyCode::E => self.camera().translate_in_time(Direction::Back, duration),
            VirtualKeyCode::I => self.camera().rotate_in_time(Direction::Up, duration),
            VirtualKeyCode::K => self.camera().rotate_in_time(Direction::Down, duration),
            VirtualKeyCode::J => self.camera().rotate_in_time(Direction::Left, duration),
            VirtualKeyCode::L => self.camera().rotate_in_time(Direction::Right, duration),
            VirtualKeyCode::U => self.camera().rotate_in_time(Direction::Front, duration),
            VirtualKeyCode::O => self.camera().rotate_in_time(Direction::Back, duration),
            _ => {}
        }
    }

    fn create_fixed_vulkan_stuff(window: &Window) -> RenderResult<FixedVulkanStuff> {
        let instance = Rc::new(
            Instance::builder()
                .window(window)
                .app_name_and_version(Self::window_title().as_str(), 0)
                .engine_name_and_version("No Engine", 0)
                .vulkan_api_version(VulkanApiVersion::V1_0)
                .enable_validation_layer(true)
                .build()?,
        );
        FixedVulkanStuff::new(window, instance)
    }

    fn create_descriptor_pool(device: &Device) -> RenderResult<vk::DescriptorPool> {
        let create_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&Self::descriptor_pool_sizes())
            .max_sets(FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT as u32)
            .build();
        Ok(unsafe { device.create_descriptor_pool(&create_info, None)? })
    }

    fn create_descriptor_set_layout(device: &Device) -> RenderResult<vk::DescriptorSetLayout> {
        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&Self::descriptor_set_layout_bindings())
            .build();
        Ok(unsafe {
            device.create_descriptor_set_layout(&descriptor_set_layout_create_info, None)?
        })
    }

    fn create_descriptor_sets(
        pool: vk::DescriptorPool,
        descriptor_set_layout: vk::DescriptorSetLayout,
        device: &Device,
    ) -> RenderResult<[vk::DescriptorSet; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT]> {
        unsafe {
            let allocate_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(pool)
                .set_layouts(&[descriptor_set_layout; FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT])
                .build();
            Ok(device
                .allocate_descriptor_sets(&allocate_info)?
                .try_into()
                .unwrap())
        }
    }
}

#[macro_export]
macro_rules! impl_window_fns {
    ($app_ty: ty) => {
        fn on_window_resized(&mut self, _size: PhysicalSize<u32>) {
            self.window_resized = true;
        }

        fn window_title() -> String {
            stringify!($app_ty).to_string()
        }

        fn window(&self) -> &Window {
            &self.window
        }

        fn last_frame_time_stamp(&self) -> SystemTime {
            self.last_frame_time_stamp
        }

        fn camera(&mut self) -> &mut Camera {
            &mut self.camera
        }
    };
}

#[macro_export]
macro_rules! impl_drop_trait {
    ($app_ty: ty) => {
        impl Drop for $app_ty {
            fn drop(&mut self) {
                unsafe {
                    self.fixed_vulkan_stuff.device.device_wait_idle().unwrap();
                    self.fixed_vulkan_stuff
                        .device
                        .destroy_pipeline(self.pipeline, None);
                    self.fixed_vulkan_stuff
                        .device
                        .destroy_pipeline_layout(self.pipeline_layout, None);
                    self.fixed_vulkan_stuff
                        .device
                        .destroy_descriptor_pool(self.descriptor_pool, None);
                    self.fixed_vulkan_stuff
                        .device
                        .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
                }
            }
        }
    };
}

pub use impl_drop_trait;
pub use impl_window_fns;
