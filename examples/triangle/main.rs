use std::cell::RefCell;

use winit::{
    dpi::PhysicalSize,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use vulkan_example_rs::app::WindowApp;
use vulkan_example_rs::window_fns;

struct DrawTriangleApp {
    window: Window,
    window_resized: bool,
}

impl WindowApp for DrawTriangleApp {
    window_fns!(DrawTriangleApp);

    fn new(event_loop: &winit::event_loop::EventLoop<()>) -> Self {
        let window = WindowBuilder::new()
            .with_title(Self::window_title())
            .with_inner_size(PhysicalSize::new(1800, 1200))
            .build(event_loop)
            .unwrap();
        DrawTriangleApp {
            window,
            window_resized: false,
        }
    }

    fn draw_frame(&mut self) {
        println!("Draw a frame")
    }

    fn on_keyboard_input(&mut self, _key_code: winit::event::VirtualKeyCode) {}
}

fn main() {
    let mut event_loop = RefCell::new(EventLoop::new());
    let mut app = DrawTriangleApp::new(&event_loop.borrow());
    app.run(&mut event_loop);
}
