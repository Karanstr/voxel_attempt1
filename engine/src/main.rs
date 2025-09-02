use crate::app::App;
use winit::event_loop::{ControlFlow, EventLoop};

mod app;
mod wgpu_ctx;
mod camera;
mod wgpu_buffers;
mod physics;
mod objects;

fn main() {
  let event_loop = EventLoop::new().unwrap();
  event_loop.set_control_flow(ControlFlow::Poll);
  let mut app = App::default();
  event_loop.run_app(&mut app).expect("App crashed");
}
