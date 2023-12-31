use std::{cell::RefCell, rc::Rc, time::SystemTime};

use ash::vk::{self, DescriptorSetLayoutBinding};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    platform::run_return::EventLoopExtRunReturn,
    window::{Window, WindowBuilder},
};

use super::{FixedVulkanStuff, UIOverlay};
use crate::{
    camera::{Camera, Direction},
    error::RenderResult,
    vulkan_wrappers::{Device, Instance, VulkanApiVersion, VulkanDebugInfoStrategy},
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

pub struct FrameCounter {
    pub double_buffer_frame: usize,
    pub frame_count: u64,
    pub last_fps_update_time_stamp: SystemTime,
    pub fps: f64,
    pub fps_update_delay: u64,
}

impl FrameCounter {
    pub fn new(fps_update_delay: usize) -> Self {
        assert!(fps_update_delay > 0);
        Self {
            double_buffer_frame: 0,
            frame_count: 0,
            last_fps_update_time_stamp: SystemTime::now(),
            fps: 0.,
            fps_update_delay: fps_update_delay as u64,
        }
    }

    pub fn update(&mut self) {
        self.frame_count += 1;
        self.double_buffer_frame =
            (self.double_buffer_frame + 1) % FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT;

        if self.count_since_last_update() == 0 {
            let now = SystemTime::now();
            let duration = now
                .duration_since(self.last_fps_update_time_stamp)
                .unwrap()
                .as_secs_f64();
            self.fps = self.fps_update_delay as f64 / duration;
            self.last_fps_update_time_stamp = now;
        }
    }

    pub fn count_since_last_update(&self) -> u64 {
        self.frame_count % self.fps_update_delay
    }
}

impl Default for FrameCounter {
    fn default() -> Self {
        Self::new(1000)
    }
}

pub trait WindowApp {
    fn new(event_loop: &EventLoop<()>) -> Self;
    fn draw_frame(&mut self);

    fn on_window_resized(&mut self, size: PhysicalSize<u32>);
    fn window_title() -> String;
    fn window(&self) -> &Window;

    fn frame_counter(&self) -> &FrameCounter;
    fn camera(&mut self) -> &mut Camera;
    fn ui(&mut self) -> &mut UIOverlay;

    fn descriptor_pool_sizes() -> Vec<vk::DescriptorPoolSize>;
    fn descriptor_set_layout_bindings() -> Vec<DescriptorSetLayoutBinding>;

    fn update_ui<T: AsRef<str>>(&mut self, infos: &[T]) {
        if self.frame_counter().frame_count < self.frame_counter().fps_update_delay
            || self.frame_counter().count_since_last_update()
                < FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT as u64
        {
            let fps = self.frame_counter().fps;
            let double_buffer_frame = self.frame_counter().double_buffer_frame;
            self.ui().imgui_context.io_mut().display_size = self.window_size().into();
            let ui = self.ui().imgui_context.new_frame();
            ui.window("Vulkan Examples").build(|| {
                ui.text(Self::window_title());
                infos.iter().for_each(|info| ui.text(info));
                ui.text(format!("fps: {fps:.2}"));
            });
            self.ui().update(double_buffer_frame).unwrap();
        }
    }

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
        let duration = self.frame_counter().fps.recip() as f32;
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
                .debug_strategy(VulkanDebugInfoStrategy::DEFAULT_PRINT_ALL)
                .build()?,
        );
        FixedVulkanStuff::new(window, instance)
    }

    fn create_descriptor_pool(device: &Device) -> RenderResult<vk::DescriptorPool> {
        let pool_sizes = Self::descriptor_pool_sizes();
        let create_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(FixedVulkanStuff::MAX_FRAMES_IN_FLIGHT as u32)
            .build();
        Ok(unsafe { device.create_descriptor_pool(&create_info, None)? })
    }

    fn create_descriptor_set_layout(device: &Device) -> RenderResult<vk::DescriptorSetLayout> {
        let bindings = Self::descriptor_set_layout_bindings();
        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
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

        fn frame_counter(&self) -> &FrameCounter {
            &self.frame_counter
        }

        fn camera(&mut self) -> &mut Camera {
            &mut self.camera
        }

        fn ui(&mut self) -> &mut UIOverlay {
            &mut self.ui_overlay
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
