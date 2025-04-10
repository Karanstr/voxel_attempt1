use crate::wgpu_ctx::WgpuCtx;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

pub struct App<'window> {
    window: Option<Arc<Window>>,
    wgpu_ctx: Option<WgpuCtx<'window>>,
    current_mouse_position: Option<(f64, f64)>,
}

impl<'window> Default for App<'window> {
    fn default() -> Self {
        Self {
            window: None,
            wgpu_ctx: None,
            current_mouse_position: None,
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
            let wgpu_ctx = WgpuCtx::new(window.clone());
            self.wgpu_ctx = Some(wgpu_ctx);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
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
            }
            WindowEvent::RedrawRequested => {
                if let Some(wgpu_ctx) = self.wgpu_ctx.as_mut() {
                    wgpu_ctx.draw();
                }
            }
            _ => (),
        }
    }
}