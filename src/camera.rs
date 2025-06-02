use glam::{Mat4, Vec2, Vec3};

/// Camera struct for handling camera position, rotation, and movement
pub struct Camera {
  // Position and orientation
  position: Vec3,
  yaw: f32,   // Horizontal rotation in radians
  pitch: f32, // Vertical rotation in radians

  // Camera properties
  fov: f32,       // Field of view in degrees
  aspect_ratio: f32,
  near_plane: f32,
  far_plane: f32,
}

impl Default for Camera {
  fn default() -> Self {
    Self {
      position: Vec3::new(1.0, 3.0,2.0),
      yaw: 0.0,
      pitch: 0.0,
      fov: 60.0,
      aspect_ratio: 1.0,
      near_plane: 0.1,
      far_plane: 100.0,
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
    self.pitch = self.pitch.clamp(-89.9, 89.9);
    self.yaw = self.yaw % 360.0;
  }

  /// Returns the projection matrix for this camera
  pub fn projection_matrix(&self) -> Mat4 {
    Mat4::perspective_rh_gl(
      self.fov.to_radians(),
      self.aspect_ratio,
      self.near_plane,
      self.far_plane
    )
  }

  /// Sets the aspect ratio (typically when window is resized)
  pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
    self.aspect_ratio = aspect_ratio;
  }

  pub fn position(&self) -> Vec3 { self.position }

  pub fn set_position(&mut self, position: Vec3) { self.position = position; }

  pub fn forward(&self) -> Vec3 {
    let pitch_rad = self.pitch.to_radians();
    let yaw_rad = self.yaw.to_radians();

    Vec3::new(
      yaw_rad.cos() * pitch_rad.cos(),
      pitch_rad.sin(),
      yaw_rad.sin() * pitch_rad.cos()
    ).normalize()
  }

}
