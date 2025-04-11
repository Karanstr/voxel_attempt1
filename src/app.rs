use crate::wgpu_ctx::WgpuCtx;
use crate::wgpu_ctx::VoxelWorld;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};
use glam::{Vec2, Vec3};
use crate::camera::Camera;
use std::cell::OnceCell;

pub struct App<'window> {
    window: OnceCell<Arc<Window>>,
    wgpu_ctx: OnceCell<WgpuCtx<'window>>,
    camera: Camera,
    
    // Input state
    keys_pressed: Vec<KeyCode>,
    mouse_delta: Vec2,
    mouse_buttons_pressed: Vec<MouseButton>,
    mouse_captured: bool,
    
    // Timing
    last_update: Instant,
}

impl<'window> Default for App<'window> {
    fn default() -> Self {
        Self {
            window: OnceCell::new(),
            wgpu_ctx: OnceCell::new(),
            camera: Camera::default(),
            keys_pressed: Vec::new(),
            mouse_delta: Vec2::ZERO,
            mouse_buttons_pressed: Vec::new(),
            mouse_captured: false,
            last_update: Instant::now(),
        }
    }
}

impl<'window> ApplicationHandler for App<'window> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = self.window.get_or_init(|| {
            let win_attr = Window::default_attributes().with_title("Voxel");
            Arc::new(event_loop.create_window(win_attr)
                .expect("Failed to construct the window (app.rs)")
            )
        });
        self.wgpu_ctx.get_or_init(|| {
            WgpuCtx::new(window.clone(), VoxelWorld::default())
        });
        
        window.request_redraw();
    }

    // Cursor is locked, so we need to acquire mouse motion directly
    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: winit::event::DeviceId, event: winit::event::DeviceEvent) {
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
                self.camera.set_aspect_ratio(new_size.width as f32 / new_size.height as f32);
                self.wgpu_ctx.get_mut().unwrap().resize((new_size.width, new_size.height));
            },
            WindowEvent::RedrawRequested => {
                // Update camera based on input
                self.tick_camera();
                self.wgpu_ctx.get_mut().unwrap().draw(&self.camera);
                self.window.get().unwrap().request_redraw();
            },
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if !self.keys_pressed.contains(&key_code) {
                                self.keys_pressed.push(key_code);
                            }
                            
                            // Toggle mouse capture with Escape key
                            if key_code == KeyCode::Escape {
                                self.toggle_mouse_capture();
                            }
                        },
                        ElementState::Released => {
                            self.keys_pressed.retain(|&k| k != key_code);
                        },
                    }
                }
            },
            WindowEvent::MouseInput { state, button, .. } => {
                match state {
                    ElementState::Pressed => {
                        if !self.mouse_buttons_pressed.contains(&button) {
                            self.mouse_buttons_pressed.push(button);
                        }
                        // Capture cursor on left click
                        if button == MouseButton::Left && !self.mouse_captured {
                            self.toggle_mouse_capture();
                        }
                    },
                    ElementState::Released => self.mouse_buttons_pressed.retain(|&b| b != button)
                }
            },
            _ => (),
        }
    }
}

impl<'window> App<'window> {
    fn toggle_mouse_capture(&mut self) {
        let window = self.window.get().unwrap();
        let new_mode = if self.mouse_captured {
            CursorGrabMode::None
        } else { CursorGrabMode::Locked };
        if window.set_cursor_grab(new_mode).is_ok() {
            window.set_cursor_visible(self.mouse_captured);
            self.mouse_captured = !self.mouse_captured;
        }
    }
    
    fn tick_camera(&mut self) {
        // Get time delta
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;
        if dt > 0.1 { return }
        
        // Update camera rotation based on mouse
        if self.mouse_captured && self.mouse_delta != Vec2::ZERO {
            self.camera.rotate(self.mouse_delta, 0.1);
            self.mouse_delta = Vec2::ZERO;
        }
        
        // Update camera position based on keyboard
        if !self.keys_pressed.is_empty() {
            let camera_speed = 5.0 * dt;
            let forward = self.camera.forward();
            let right = forward.cross(Vec3::Y).normalize();
            let mut displacement = Vec3::ZERO;
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
            
            self.camera.translate(displacement * camera_speed);
        }
    }

}
