use glam::{Vec3, Mat4};

/// Camera struct for handling camera position, rotation, and movement
pub struct Camera {
    // Position and orientation
    position: Vec3,
    yaw: f32,   // Horizontal rotation in degrees
    pitch: f32, // Vertical rotation in degrees
    
    // Derived vectors
    forward: Vec3,
    right: Vec3,
    up: Vec3,
    
    // Camera properties
    fov: f32,       // Field of view in degrees
    aspect_ratio: f32,
    near_plane: f32,
    far_plane: f32,
}

impl Default for Camera {
    fn default() -> Self {
        let mut camera = Self {
            position: Vec3::new(4.0, 4.0, 12.0),
            yaw: -90.0, // -90 degrees so we start looking along negative Z
            pitch: 0.0,
            forward: Vec3::new(0.0, 0.0, -1.0),
            right: Vec3::new(1.0, 0.0, 0.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            fov: 60.0,
            aspect_ratio: 1.0,
            near_plane: 0.1,
            far_plane: 100.0,
        };
        
        // Initialize derived vectors
        camera.update_vectors();
        
        camera
    }
}

impl Camera {
    /// Updates the camera's orientation vectors based on yaw and pitch
    pub fn update_vectors(&mut self) {
        // Calculate new forward vector from yaw and pitch
        let pitch_rad = self.pitch.to_radians();
        let yaw_rad = self.yaw.to_radians();
        
        self.forward = Vec3::new(
            yaw_rad.cos() * pitch_rad.cos(),
            pitch_rad.sin(),
            yaw_rad.sin() * pitch_rad.cos()
        ).normalize();
        
        // Recalculate right and up vectors
        let world_up = Vec3::new(0.0, 1.0, 0.0);
        self.right = self.forward.cross(world_up).normalize();
        self.up = self.right.cross(self.forward).normalize();
    }
    
    /// Moves the camera in the specified direction
    pub fn move_camera(&mut self, direction: Vec3, amount: f32) {
        self.position += direction * amount;
    }
    
    /// Rotates the camera by the specified yaw and pitch deltas
    pub fn rotate(&mut self, yaw_delta: f32, pitch_delta: f32) {
        self.yaw += yaw_delta;
        self.pitch += pitch_delta; // Inverted Y for intuitive mouse control
        
        // Clamp pitch to avoid gimbal lock
        self.pitch = self.pitch.clamp(-89.0, 89.0);
        
        // Update camera vectors
        self.update_vectors();
    }
    
    /// Returns the view matrix for this camera
    pub fn view_matrix(&self) -> Mat4 {
        // Look-at matrix from camera position to target position
        let target = self.position + self.forward;
        Mat4::look_at_rh(self.position, target, self.up)
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
    
    // Getters and setters
    
    pub fn position(&self) -> Vec3 {
        self.position
    }
    
    pub fn position_array(&self) -> [f32; 3] {
        [self.position.x, self.position.y, self.position.z]
    }
    
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }
    
    pub fn set_position_array(&mut self, position: [f32; 3]) {
        self.position = Vec3::from_array(position);
    }
    
    pub fn forward(&self) -> Vec3 {
        self.forward
    }
    
    pub fn forward_array(&self) -> [f32; 3] {
        [self.forward.x, self.forward.y, self.forward.z]
    }
    
    pub fn target_position(&self) -> Vec3 {
        self.position + self.forward
    }
    
    pub fn target_position_array(&self) -> [f32; 3] {
        let target = self.position + self.forward;
        [target.x, target.y, target.z]
    }
    
    pub fn right(&self) -> Vec3 {
        self.right
    }
    
    pub fn up(&self) -> Vec3 {
        self.up
    }
    
    pub fn yaw(&self) -> f32 {
        self.yaw
    }
    
    pub fn pitch(&self) -> f32 {
        self.pitch
    }
    
    /// Process keyboard input for camera movement
    pub fn process_keyboard(&mut self, forward: bool, backward: bool, left: bool, right: bool, 
                           up: bool, down: bool, speed: f32) {
        let mut movement = Vec3::ZERO;
        
        if forward {
            movement += self.forward;
        }
        if backward {
            movement -= self.forward;
        }
        if right {
            movement += self.right;
        }
        if left {
            movement -= self.right;
        }
        if up {
            movement += self.up;
        }
        if down {
            movement -= self.up;
        }
        
        // Normalize movement vector if it's not zero
        if movement != Vec3::ZERO {
            movement = movement.normalize();
            self.move_camera(movement, speed);
        }
    }
}
