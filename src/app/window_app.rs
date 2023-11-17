use std::{cell::RefCell, rc::Rc, time::SystemTime};

use ash::vk;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    platform::run_return::EventLoopExtRunReturn,
    window::Window,
};

use super::FixedVulkanStuff;
use crate::{
    camera::Camera,
    error::RenderResult,
    transforms::Direction,
    vulkan_objects::{InstanceBuilder, VulkanApiVersion},
};

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

    fn window_size(&self) -> PhysicalSize<u32> {
        self.window().inner_size()
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
            InstanceBuilder::default()
                .with_window(window)
                .with_app_name_and_version(Self::window_title().as_str(), 0)
                .with_engine_name_and_version("No Engine", 0)
                .with_vulkan_api_version(VulkanApiVersion::V1_0)
                .enable_validation_layer()
                .build()
                .unwrap(),
        );
        FixedVulkanStuff::new(window, instance)
    }
}

#[macro_export]
macro_rules! window_fns {
    ($app_ty: ty) => {
        fn on_window_resized(&mut self, _size: PhysicalSize<u32>) {
            self.window_resized = true;
        }

        fn window_title() -> String {
            stringify!(DrawTriangleApp).to_string()
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

pub use window_fns;
