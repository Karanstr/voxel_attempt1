use crate::app::App;
use winit::event_loop::{ControlFlow, EventLoop};
// use sdg::prelude::*;
// use crate::app::ObjectData;
// use std::time::Instant;

mod app;
mod wgpu_ctx;
mod camera;
mod wgpu_buffers;

fn main() {
  // let mut sdg = SparseDirectedGraph::new();
  // let _empty = sdg.add_leaf();
  // let _dirt = sdg.add_leaf();
  // let _grass = sdg.add_leaf();
  // let time = Instant::now();
  // let obj_data = ObjectData::new(&mut sdg);
  // dbg!(time.elapsed());
  let event_loop = EventLoop::new().unwrap();
  event_loop.set_control_flow(ControlFlow::Poll);
  let mut app = App::default();
  event_loop.run_app(&mut app).expect("App crashed");
}
