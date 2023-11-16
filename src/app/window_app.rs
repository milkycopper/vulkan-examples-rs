use std::cell::RefCell;

use ash::vk;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    platform::run_return::EventLoopExtRunReturn,
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
    fn on_keyboard_input(&mut self, key_code: VirtualKeyCode);

    fn window_size(&self) -> PhysicalSize<u32>;

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
}
