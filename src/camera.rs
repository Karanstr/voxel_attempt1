use glam::{Vec2, Vec3};
use std::f32::consts::PI;
const QUARTER: f32 = PI / 2.;

/// Camera struct for handling camera position, rotation, and movement
pub struct Camera {
  // Position
  pub position: Vec3,
  yaw: f32,   // Horizontal rotation in radians
  pitch: f32, // Vertical rotation in radians

  // Camera properties
  pub aspect_ratio: f32,
  pub fov: f32,
}

impl Default for Camera {
  fn default() -> Self {
    Self {
      position: Vec3::new(1.0, 3.0,2.0),
      yaw: 0.0,
      pitch: 0.0,
      aspect_ratio: 1.0,
      fov: 1.5,
    }
  }
}

impl Camera {
  /// Rotates the camera by the specified yaw and pitch deltas
  pub fn rotate(&mut self, raw_delta: Vec2, sensitivity: f32) {
    self.yaw += raw_delta.x * sensitivity;
    self.pitch -= raw_delta.y * sensitivity;
    self.pitch = self.pitch.clamp(-QUARTER + 0.001, QUARTER - 0.001);
    self.yaw = self.yaw % (PI * 2.);
  }

  pub fn forward(&self) -> Vec3 {
    let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
    let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
    Vec3::new(
      yaw_cos * pitch_cos,
      pitch_sin,
      yaw_sin * pitch_cos,
    ).normalize()
  }
  
  /// [Right, Up, Forward]
  pub fn basis(&self) -> [Vec3; 3] {
    let forward = self.forward();
    let right = forward.cross(Vec3::Y).normalize();
    let up = right.cross(forward).normalize();
    [right, up, forward]
  }

}
