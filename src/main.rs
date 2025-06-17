use crate::app::App;
use winit::error::EventLoopError;
use winit::event_loop::{ControlFlow, EventLoop};
use crate::graph::prelude::*;
use crate::app::ObjectData;

mod app;
mod wgpu_ctx;
mod camera;
mod graph;

fn main() -> Result<(), EventLoopError> {
  // let mut sdg = SparseDirectedGraph::new();
  // let _empty = sdg.add_leaf();
  // let _dirt = sdg.add_leaf();
  // let _grass = sdg.add_leaf();
  // let obj_data = ObjectData::new(&mut sdg);
  let event_loop = EventLoop::new().unwrap();
  event_loop.set_control_flow(ControlFlow::Poll);
  let mut app = App::default();
  event_loop.run_app(&mut app)
}
