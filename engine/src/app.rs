use crate::wgpu_ctx::WgpuCtx;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};
use glam::{Vec2, Vec3};
use std::cell::OnceCell;
use crate::objects::GameData;


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
  fps_update_timer: f32, // We want to print fps once per second
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
        let new_window = Arc::new(
          event_loop.create_window(Window::default_attributes()).unwrap()
        );
        self.window.set(new_window.clone()).unwrap();
        new_window.request_redraw();
        let new_ctx = WgpuCtx::new(new_window);
        new_ctx.update_voxels(&self.game_data.sdg);
        self.wgpu_ctx.set(new_ctx).unwrap_or_else(|_| panic!("I'm not gonna let this fail quietly and I'm not implementing debug on WgpuCtx, that's way too much work"));
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

  fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
    match event {
      WindowEvent::CloseRequested => event_loop.exit(),
      WindowEvent::Resized(new_size) => {
        self.game_data.camera.aspect_ratio = new_size.width as f32 / new_size.height as f32;
        self.wgpu_ctx.get_mut().unwrap().resize(new_size);
      },
      WindowEvent::RedrawRequested => self.redraw(),
      WindowEvent::KeyboardInput { event, .. } => {
        if let PhysicalKey::Code(key_code) = event.physical_key {
          match event.state {
            ElementState::Pressed => if !self.keys_pressed.contains(&key_code) { self.keys_pressed.push(key_code); },
            ElementState::Released => self.keys_pressed.retain(|&k| k != key_code),
          }
        }
      },
      WindowEvent::MouseInput { state, button, .. } => {
        match state {
          ElementState::Pressed => if !self.mouse_buttons_pressed.contains(&button) { self.mouse_buttons_pressed.push(button) },
          ElementState::Released => self.mouse_buttons_pressed.retain(|&b| b != button)
        }
      },
      _ => (),
    }
  }
}

impl<'window> App<'window> {
  fn redraw(&mut self) {
    let before = Instant::now();
    self.tick_world();

    self.wgpu_ctx.get_mut().unwrap().draw(&self.game_data);
    self.window.get().unwrap().request_redraw();
    if self.fps_update_timer > 1.0 {
      println!("FPS: {:.1}", 1.0 / Instant::now().duration_since(before).as_secs_f32());
      self.fps_update_timer = 0.0;
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

  fn tick_world(&mut self) {
    let now = Instant::now();
    let dt = now.duration_since(self.last_update).as_secs_f32();
    self.last_update = now;
    if dt > 1.0 { return }
    self.fps_update_timer += dt;
    self.handle_inputs(dt);
  }

  fn handle_inputs(&mut self, delta_time: f32) {
    if self.keys_pressed.contains(&KeyCode::Escape)
    || (self.mouse_buttons_pressed.contains(&MouseButton::Left) && !self.mouse_captured) {
      self.toggle_mouse_capture()
    }
    if !self.mouse_captured { return }
    
    if self.mouse_delta != Vec2::ZERO {
      self.game_data.camera.rotate(self.mouse_delta, 0.002);
      self.mouse_delta = Vec2::ZERO;
    }

    let mut displacement = Vec3::ZERO; // Replace with impulse
    let camera_speed = self.game_data.camera.speed * delta_time;
    let (right, _, mut forward) = self.game_data.camera.basis().into();
    forward = forward.with_y(0.0).normalize();
    for key in &self.keys_pressed {
      match key {
        KeyCode::Escape => {}
        KeyCode::KeyW => { displacement += forward }
        KeyCode::KeyS => { displacement -= forward }
        KeyCode::KeyD => { displacement += right }
        KeyCode::KeyA => { displacement -= right }
        KeyCode::Space => { displacement += Vec3::Y }
        KeyCode::ShiftLeft => { displacement -= Vec3::Y }
        KeyCode::Equal => { self.game_data.camera.speed *= 1.003 }
        KeyCode::Minus => { self.game_data.camera.speed /= 1.003 }
        _ => ()
      }
    }
    self.game_data.camera.position += displacement.normalize_or_zero() * camera_speed;

  }

}

