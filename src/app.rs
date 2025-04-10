use crate::wgpu_ctx::WgpuCtx;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent, MouseButton};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};
use glam::Vec3;
use std::time::Instant;

pub struct App<'window> {
    window: Option<Arc<Window>>,
    wgpu_ctx: Option<WgpuCtx<'window>>,
    camera_position: Vec3,
    camera_direction: Vec3,
    last_frame_time: Instant,
    mouse_pressed: bool,
    last_mouse_position: Option<(f64, f64)>,
    current_mouse_position: Option<(f64, f64)>,
    keys_pressed: [bool; 6], // W, A, S, D, Space, Shift
    flag: bool,
    cell_steps: f32,
}

impl<'window> Default for App<'window> {
    fn default() -> Self {
        Self {
            window: None,
            wgpu_ctx: None,
            camera_position: Vec3::new(0.0, 0.0, -2.0),
            camera_direction: Vec3::new(0.0, 0.0, 1.0),
            last_frame_time: Instant::now(),
            mouse_pressed: false,
            last_mouse_position: None,
            current_mouse_position: None,
            keys_pressed: [false; 6],
            flag: false,
            cell_steps: 5.0, // Default number of steps from mouse position
        }
    }
}

impl<'window> ApplicationHandler for App<'window> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let win_attr = Window::default_attributes().with_title("wgpu winit example");
            // use Arc.
            let window = Arc::new(
                event_loop
                    .create_window(win_attr)
                    .expect("create window err."),
            );
            self.window = Some(window.clone());
            let wgpu_ctx = WgpuCtx::new(window.clone());
            self.wgpu_ctx = Some(wgpu_ctx);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                if let (Some(wgpu_ctx), Some(window)) =
                    (self.wgpu_ctx.as_mut(), self.window.as_ref())
                {
                    wgpu_ctx.resize((new_size.width, new_size.height));
                    window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        repeat: false,
                        state,
                        logical_key,
                        ..
                    },
                ..
            } => {
                let pressed = state == ElementState::Pressed;
                
                // Handle resize with Enter key
                if pressed && logical_key == Key::Named(NamedKey::Enter) {
                    if let (Some(wgpu_ctx), Some(window)) =
                        (self.wgpu_ctx.as_mut(), self.window.as_ref())
                    {
                        let size = window.inner_size();
                        let w = size.width.max(1);
                        let h = size.height.max(1);
                        if self.flag {
                            wgpu_ctx.resize((w, h));
                        } else {
                            wgpu_ctx.resize((w / 2, h / 2));
                        }
                        self.flag = !self.flag;
                        window.request_redraw();
                    }
                    return;
                }
                
                // Handle camera movement keys
                match logical_key {
                    Key::Character(c) if c == "w" || c == "W" => self.keys_pressed[0] = pressed,
                    Key::Character(c) if c == "a" || c == "A" => self.keys_pressed[1] = pressed,
                    Key::Character(c) if c == "s" || c == "S" => self.keys_pressed[2] = pressed,
                    Key::Character(c) if c == "d" || c == "D" => self.keys_pressed[3] = pressed,
                    Key::Named(NamedKey::Space) => self.keys_pressed[4] = pressed,
                    Key::Named(NamedKey::Shift) => self.keys_pressed[5] = pressed,
                    Key::Named(NamedKey::Escape) => {
                        if pressed {
                            event_loop.exit();
                        }
                    },
                    _ => {}
                }
                
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // Always store current mouse position for shader
                self.current_mouse_position = Some((position.x, position.y));
                
                // Update mouse position in shader
                if let (Some(window), Some(wgpu_ctx)) = (self.window.as_ref(), self.wgpu_ctx.as_mut()) {
                    let size = window.inner_size();
                    // Normalize mouse position to 0.0-1.0 range
                    let normalized_x = position.x as f32 / size.width as f32;
                    let normalized_y = position.y as f32 / size.height as f32;
                    wgpu_ctx.update_mouse_position([normalized_x, normalized_y]);
                    window.request_redraw();
                }
                
                // Handle camera rotation when mouse is pressed
                if self.mouse_pressed {
                    if let Some((last_x, last_y)) = self.last_mouse_position {
                        let dx = position.x - last_x;
                        let dy = position.y - last_y;
                        
                        // Rotate camera based on mouse movement
                        let sensitivity = 0.003;
                        
                        // Yaw rotation (around y-axis)
                        let right = Vec3::new(1.0, 0.0, 0.0);
                        let up = Vec3::new(0.0, 1.0, 0.0);
                        
                        // Rotate around y-axis (left/right)
                        let yaw_rotation = glam::Quat::from_axis_angle(
                            up,
                            -dx as f32 * sensitivity
                        );
                        
                        // Rotate around x-axis (up/down)
                        let pitch_rotation = glam::Quat::from_axis_angle(
                            right,
                            -dy as f32 * sensitivity
                        );
                        
                        // Apply rotations
                        self.camera_direction = yaw_rotation.mul_vec3(self.camera_direction);
                        self.camera_direction = pitch_rotation.mul_vec3(self.camera_direction);
                        self.camera_direction = self.camera_direction.normalize();
                    }
                    
                    self.last_mouse_position = Some((position.x, position.y));
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.mouse_pressed = state == ElementState::Pressed;
                if !self.mouse_pressed {
                    self.last_mouse_position = None;
                }
            }
            WindowEvent::RedrawRequested => {
                // Update camera position based on keys pressed
                let now = Instant::now();
                let dt = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;
                
                let speed = 2.0 * dt; // Movement speed
                
                // Calculate forward and right vectors
                let forward = self.camera_direction.normalize();
                let right = forward.cross(Vec3::new(0.0, 1.0, 0.0)).normalize();
                let up = Vec3::new(0.0, 1.0, 0.0);
                
                // Apply movement based on keys pressed
                if self.keys_pressed[0] { // W - Forward
                    self.camera_position += forward * speed;
                }
                if self.keys_pressed[1] { // A - Left
                    self.camera_position -= right * speed;
                }
                if self.keys_pressed[2] { // S - Backward
                    self.camera_position -= forward * speed;
                }
                if self.keys_pressed[3] { // D - Right
                    self.camera_position += right * speed;
                }
                if self.keys_pressed[4] { // Space - Up
                    self.camera_position += up * speed;
                }
                if self.keys_pressed[5] { // Shift - Down
                    self.camera_position -= up * speed;
                }
                
                if let Some(wgpu_ctx) = self.wgpu_ctx.as_mut() {
                    // Update cell steps in wgpu context
                    wgpu_ctx.update_cell_steps(self.cell_steps);
                    wgpu_ctx.draw();
                }
                
                // Request another frame if any movement keys are pressed
                if self.keys_pressed.iter().any(|&pressed| pressed) || self.mouse_pressed {
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                }
            }
            _ => (),
        }
    }
}