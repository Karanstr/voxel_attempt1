use sdg::prelude::*;
use crate::wgpu_ctx::WgpuCtx;
use crate::camera::Camera;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};
use glam::{Quat, UVec3, Vec2, Vec3};
use std::cell::OnceCell;
use std::collections::VecDeque;
use fastnoise_lite::{FastNoiseLite, NoiseType};

pub struct ObjectData {
  pub head: Index,
  pub height: u32,
  pub bounds: u32,
  pub pos: Vec3,
  pub rot: Quat,
}
impl ObjectData {
  pub fn new_block(sdg: &mut SparseDirectedGraph<BasicNode3d>, pos: Vec3) -> Self {
    let head = sdg.get_root(1);
    let height = 3;
    Self {
      head, 
      height,
      bounds: 2u32.pow(height),
      pos,
      rot: Quat::IDENTITY,
    }
  }
  pub fn new_ground(sdg: &mut SparseDirectedGraph<BasicNode3d>) -> Self {
    let mut head = sdg.get_root(0);
    let height = 11;
    let size = 2u32.pow(height);
    let mut noise = FastNoiseLite::new();
    noise.set_seed(None);
    noise.set_noise_type(Some(NoiseType::OpenSimplex2S));
    noise.set_frequency(Some(0.0027));
    for x in 0 .. size {
      for z in 0 .. size {
        let sample = noise.get_noise_2d(x as f32, z as f32) * 0.05;
        if sample > 0.0 {
          let y = (sample * size as f32) as u32;
          let path = Zorder3d::path_from(UVec3::new(x, y, z), height);
          head = sdg.set_node(head, &path, 1);
        }
      }
    }

    ObjectData {
      head,
      height,
      bounds: size,
      pos: Vec3::ZERO,
      rot: Quat::IDENTITY,
    }
  }
}

pub struct GameData {
  pub camera: Camera,
  pub speed: f32,
  pub sdg: SparseDirectedGraph<BasicNode3d>,
  pub world_data: ObjectData,
  pub cube1: ObjectData,
  pub cube2: ObjectData,
  pub cube3: ObjectData,
}
impl Default for GameData {
  fn default() -> Self {
    let mut sdg = SparseDirectedGraph::new();
    let _empty = sdg.add_leaf();
    let _full = sdg.add_leaf();
    let world_data = ObjectData::new_ground(&mut sdg);
    let cube1 = ObjectData::new_block(&mut sdg, Vec3::splat(100.));
    let cube2 = ObjectData::new_block(&mut sdg, Vec3::new(110., 100., 100.));
    let cube3 = ObjectData::new_block(&mut sdg, Vec3::new(100., 110., 100.));
    Self {
      camera: Camera::default(),
      speed: 64.0,
      sdg,
      world_data,
      cube1,
      cube2,
      cube3,
    }
  }
}

pub struct App<'window> {
  // Windowing
  window: OnceCell<Arc<Window>>,
  wgpu_ctx: OnceCell<WgpuCtx<'window>>,

  game_data: GameData,

  // Input
  keys_pressed: Vec<KeyCode>,
  mouse_delta: Vec2,
  mouse_buttons_pressed: Vec<MouseButton>,
  mouse_captured: bool,

  // Frame Timing
  last_update: Instant,
  frame_times: VecDeque<f32>,
  fps_update_timer: f32,
}

impl<'window> Default for App<'window> {
  fn default() -> Self {
    Self {
      window: OnceCell::new(),
      wgpu_ctx: OnceCell::new(),
      game_data: GameData::default(),
      keys_pressed: Vec::new(),
      mouse_delta: Vec2::ZERO,
      mouse_buttons_pressed: Vec::new(),
      mouse_captured: false,
      last_update: Instant::now(),
      frame_times: VecDeque::with_capacity(100),
      fps_update_timer: 0.0,
    }
  }
}

impl<'window> ApplicationHandler for App<'window> {
  // Create window and wgpu_ctx
  // Requests redraw
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    match self.window.get() {
      Some(already) => already.request_redraw(),
      None => {
        let new_window = Arc::new(event_loop.create_window(
          Window::default_attributes().with_title("Voxel")
        ).unwrap());
        self.window.set(new_window.clone()).unwrap();
        new_window.request_redraw();
        let new_ctx = WgpuCtx::new(new_window);
        new_ctx.update_voxels(&self.game_data.sdg);
        self.wgpu_ctx.set(new_ctx).unwrap_or_else(|_| panic!("Should be impossible to get here, but I'm not gonna let this fail quietly somehow and I'm not implementing debug on WgpuCtx, that's way too much work"));
      }
    }
  }

  // Cursor is locked, so we need to acquire mouse motion directly
  fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: winit::event::DeviceId, event: winit::event::DeviceEvent) {
    // Don't trigger any device events  unless mouse is locked
    if !self.mouse_captured { return }
    if let DeviceEvent::MouseMotion { delta } = event {
      self.mouse_delta.x += delta.0 as f32;
      self.mouse_delta.y += delta.1 as f32;
    }
  }

  // This function shouldn't perform any advanced logic, simply act as a passthrough for data? 
  fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
    match event {
      WindowEvent::CloseRequested => event_loop.exit(),
      WindowEvent::Resized(new_size) => {
        self.game_data.camera.aspect_ratio = new_size.width as f32 / new_size.height as f32;
        self.wgpu_ctx.get_mut().unwrap().resize(new_size);
      },
      WindowEvent::RedrawRequested => {
        // Update camera based on input
        self.tick_camera();
        self.display_fps(1.);

        self.wgpu_ctx.get_mut().unwrap().draw(&self.game_data);
        self.window.get().unwrap().request_redraw();
      },
      WindowEvent::KeyboardInput { event, .. } => {
        if let PhysicalKey::Code(key_code) = event.physical_key {
          match event.state {
            ElementState::Pressed => {
              if !self.keys_pressed.contains(&key_code) { self.keys_pressed.push(key_code); }
              // Toggle mouse capture with Escape key
              // Not a huge fan of handling these key presses in two different places..
              if key_code == KeyCode::Escape { self.toggle_mouse_capture() }
            },
            ElementState::Released => self.keys_pressed.retain(|&k| k != key_code),
          }
        }
      },
      WindowEvent::MouseInput { state, button, .. } => {
        match state {
          ElementState::Pressed => {
            if !self.mouse_buttons_pressed.contains(&button) { self.mouse_buttons_pressed.push(button) }
            // Capture cursor on left click
            // Same as escape, I don't like the dual processing and plan to create specific functions to handle them.
            if button == MouseButton::Left && !self.mouse_captured { self.toggle_mouse_capture(); }
          },
          ElementState::Released => self.mouse_buttons_pressed.retain(|&b| b != button)
        }
      },
      _ => (),
    }
  }
}

impl<'window> App<'window> {    
  fn store_frame_time(&mut self, dt: f32) {
    if self.frame_times.len() == 100 { self.frame_times.pop_front(); }
    self.frame_times.push_back(dt);
    self.fps_update_timer += dt;
  }

  fn display_fps(&mut self, time_since_last: f32) {
    if self.fps_update_timer >= time_since_last {
      self.fps_update_timer = 0.0;
      println!("FPS: {:.1}", self.frame_times.len() as f32 / self.frame_times.iter().sum::<f32>());
    }
  }

  fn toggle_mouse_capture(&mut self) {
    let window = self.window.get().unwrap();
    let new_mode = if self.mouse_captured { CursorGrabMode::None } else { CursorGrabMode::Confined };
    if window.set_cursor_grab(new_mode).is_ok() {
      window.set_cursor_visible(self.mouse_captured);
      self.mouse_captured = !self.mouse_captured;
    }
  }

  fn tick_camera(&mut self) {
    let now = Instant::now();
    let dt = now.duration_since(self.last_update).as_secs_f32();
    self.last_update = now;
    if dt > 0.1 { return }

    self.store_frame_time(dt);
    self.game_data.cube1.rot *= Quat::from_rotation_y(0.001);
    if !self.mouse_captured { return } // Player controls should only work while mouse is captured
    if self.mouse_delta != Vec2::ZERO {
      self.game_data.camera.rotate(self.mouse_delta, 0.002);
      self.mouse_delta = Vec2::ZERO;
    }
    if !self.keys_pressed.is_empty() {
      let camera_speed = self.game_data.speed * dt;
      let (right, _, mut forward) = self.game_data.camera.basis().into();
      forward = forward.with_y(0.0).normalize();
      let mut displacement = Vec3::ZERO;
      // This feels like a really silly way to key lookups when a hashmap would prob be better..
      if self.keys_pressed.contains(&KeyCode::KeyW) {
        displacement += forward;
      }
      if self.keys_pressed.contains(&KeyCode::KeyS) {
        displacement -= forward;
      }
      if self.keys_pressed.contains(&KeyCode::KeyA) {
        displacement -= right;
      }
      if self.keys_pressed.contains(&KeyCode::KeyD) {
        displacement += right;
      }
      if self.keys_pressed.contains(&KeyCode::Space) {
        displacement += Vec3::Y;
      }
      if self.keys_pressed.contains(&KeyCode::ShiftLeft) {
        displacement -= Vec3::Y;
      }
      if self.keys_pressed.contains(&KeyCode::Equal) {
        self.game_data.speed *= 1.003;
      }
      if self.keys_pressed.contains(&KeyCode::Minus) {
        self.game_data.speed /= 1.003;
      }
      self.game_data.camera.position += displacement.normalize_or_zero() * camera_speed;
    }
  }

}

