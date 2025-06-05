use glam::{Vec2, Vec3};
use std::f32::consts::PI;
const QUARTER: f32 = PI / 2.;

/// Camera struct for handling camera position, rotation, and movement
pub struct Camera {
  // Position
  position: Vec3,
  yaw: f32,   // Horizontal rotation in radians
  pitch: f32, // Vertical rotation in radians

  // Camera properties
  aspect_ratio: f32,
}

impl Default for Camera {
  fn default() -> Self {
    Self {
      position: Vec3::new(1.0, 3.0,2.0),
      yaw: 0.0,
      pitch: 0.0,
      aspect_ratio: 1.0,
    }
  }
}

impl Camera {

  /// Moves the camera in the specified direction
  pub fn translate(&mut self, offset: Vec3) {
    self.position += offset;
  }

  /// Rotates the camera by the specified yaw and pitch deltas
  pub fn rotate(&mut self, raw_delta: Vec2, sensitivity: f32) {
    self.yaw += raw_delta.x * sensitivity;
    self.pitch -= raw_delta.y * sensitivity;
    self.pitch = self.pitch.clamp(-QUARTER + 0.001, QUARTER - 0.001);
    self.yaw = self.yaw % (PI * 2.);
  }

  /// Sets the aspect ratio (typically when window is resized)
  pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
    self.aspect_ratio = aspect_ratio;
  }

  pub fn position(&self) -> Vec3 { self.position }

  pub fn _set_position(&mut self, position: Vec3) { self.position = position; }

  // No reason to normalize this I think, all we care about is the ratio
  pub fn forward(&self) -> Vec3 {
    let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
    let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
    Vec3::new(
      yaw_cos * pitch_cos,
      pitch_sin,
      yaw_sin * pitch_cos,
    )
  }

}
