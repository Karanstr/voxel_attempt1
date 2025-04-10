use crate::wgpu_ctx::WgpuCtx;
use crate::wgpu_ctx::VoxelWorld;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};
use glam::Vec2;

pub struct App<'window> {
    window: Option<Arc<Window>>,
    wgpu_ctx: Option<WgpuCtx<'window>>,
    
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
            window: None,
            wgpu_ctx: None,
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
        if self.window.is_none() {
            let win_attr = Window::default_attributes().with_title("Voxel");
            let window = Arc::new(
                event_loop
                    .create_window(win_attr)
                    .expect("Failed to construct the window (app.rs)"),
            );
            self.window = Some(window.clone());
            let wgpu_ctx = WgpuCtx::new(window.clone(), VoxelWorld::default());
            self.wgpu_ctx = Some(wgpu_ctx);
            
            // Request initial redraw to start the continuous rendering loop
            window.request_redraw();
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: winit::event::DeviceId, event: winit::event::DeviceEvent) {
        if !self.mouse_captured {
            return;
        }
        
        match event {
            DeviceEvent::MouseMotion { delta } => {
                // Accumulate mouse movement
                self.mouse_delta.x += delta.0 as f32;
                self.mouse_delta.y += delta.1 as f32;
            },
            _ => {},
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                if let Some(wgpu_ctx) = self.wgpu_ctx.as_mut() {
                    wgpu_ctx.resize((new_size.width, new_size.height));
                }
            },
            WindowEvent::RedrawRequested => {
                // Update camera based on input
                self.update_camera();
                
                if let Some(wgpu_ctx) = self.wgpu_ctx.as_mut() {
                    wgpu_ctx.draw();
                }
                
                // Request another frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
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
                        
                        // Capture mouse on left click
                        if button == MouseButton::Left && !self.mouse_captured {
                            self.toggle_mouse_capture();
                        }
                    },
                    ElementState::Released => {
                        self.mouse_buttons_pressed.retain(|&b| b != button);
                    },
                }
            },
            _ => (),
        }
    }
}

impl<'window> App<'window> {
    fn toggle_mouse_capture(&mut self) {
        if let Some(window) = &self.window {
            self.mouse_captured = !self.mouse_captured;
            
            let grab_mode = if self.mouse_captured {
                CursorGrabMode::Confined
            } else {
                CursorGrabMode::None
            };
            
            // Attempt to set cursor grab mode
            if window.set_cursor_grab(grab_mode).is_ok() {
                window.set_cursor_visible(!self.mouse_captured);
            } else {
                self.mouse_captured = !self.mouse_captured; // Revert if failed
            }
        }
    }
    
    fn update_camera(&mut self) {
        // Get time delta
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;
        
        // Skip if no context or too large time step (e.g. after pause)
        if self.wgpu_ctx.is_none() || dt > 0.1 {
            return;
        }
        
        let wgpu_ctx = self.wgpu_ctx.as_mut().unwrap();
        let camera = wgpu_ctx.camera_mut();
        
        // Update camera rotation based on mouse movement
        if self.mouse_captured && self.mouse_delta != Vec2::ZERO {
            // Mouse sensitivity
            let sensitivity = 0.1;
            
            // Update camera angles
            camera.rotate(self.mouse_delta.x * sensitivity, -self.mouse_delta.y * sensitivity);
            
            // Reset mouse delta
            self.mouse_delta = Vec2::ZERO;
        }
        
        // Process keyboard input for camera movement
        if !self.keys_pressed.is_empty() {
            // Camera movement speed
            let camera_speed = 5.0 * dt;
            
            // Process keyboard input
            camera.process_keyboard(
                self.keys_pressed.contains(&KeyCode::KeyW),
                self.keys_pressed.contains(&KeyCode::KeyS),
                self.keys_pressed.contains(&KeyCode::KeyA),
                self.keys_pressed.contains(&KeyCode::KeyD),
                self.keys_pressed.contains(&KeyCode::Space),
                self.keys_pressed.contains(&KeyCode::ShiftLeft),
                camera_speed
            );
        }
    }
}
