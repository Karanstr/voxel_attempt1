use crate::app::App;
use crate::graph::prelude::*;
use winit::error::EventLoopError;
use winit::event_loop::{ControlFlow, EventLoop};
use glam::UVec3;

mod app;
mod wgpu_ctx;
mod camera;
mod graph;

fn main() -> Result<(), EventLoopError> {
    // let mut sdg: SparseDirectedGraph<BasicNode3d> = SparseDirectedGraph::new();
    // for _ in 0..16 { sdg.add_leaf(); }
    // let mut head = sdg.get_root(0);
    // let height = 6;
    // let size = 2u32.pow(height);
    // let mut count = 0u32;
    // for x in 0..size {
    //   for y in 0..size {
    //     for z in 0..size {
    //       let leaf_val = ((x + y * 16 + z * 256) % 16) as u32; // 16 leaf values: 0..15
    //       let path: Vec<Zorder3d> = BasicPath3d::from_cell(UVec3::new(x, y, z), 7).steps();
    //       head = sdg.set_node(head, &path, leaf_val);
    //       count += 1;
    //       println!("{count} / {}", size.pow(3));
    //     }
    //   }
    // }
  let event_loop = EventLoop::new().unwrap();
  event_loop.set_control_flow(ControlFlow::Poll);
  let mut app = App::default();
  event_loop.run_app(&mut app)
}
