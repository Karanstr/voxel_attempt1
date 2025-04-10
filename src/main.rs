use macroquad::prelude::*;
use macroquad::input::set_cursor_grab;
// Load shader source from external files
const VERTEX_SHADER: &str = include_str!("shader.vert");
const FRAGMENT_SHADER: &str = include_str!("shader.frag");

// Camera structure to track position and orientation
struct Camera {
    position: Vec3,
    pitch: f32,
    yaw: f32,
    speed: f32,
    sensitivity: f32,
    grabbed: bool,
}
impl Camera {
    fn new(position: Vec3, direction: Vec3) -> Self {
        // Calculate initial pitch and yaw from direction vector
        let dir = direction.normalize();
        let pitch = (-dir.y).asin();
        let yaw = dir.z.atan2(dir.x);
        
        Camera {
            position,
            pitch,
            yaw,
            speed: 0.05,
            sensitivity: 0.2, // Reduced sensitivity for radians
            grabbed: true,
        }
    }

    // Update camera position based on keyboard input
    fn update(&mut self) {
        let mut delta = Vec3::ZERO;
        
        // Calculate direction vector from pitch and yaw
        let direction = Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            -self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos()
        ).normalize();
        
        // Forward/backward movement
        if is_key_down(KeyCode::W) {
            delta += direction * self.speed;
        }
        if is_key_down(KeyCode::S) {
            delta -= direction * self.speed;
        }

        // Strafe left/right
        let right = direction.cross(Vec3::new(0.0, 1.0, 0.0)).normalize();
        if is_key_down(KeyCode::A) {
            delta -= right * self.speed;
        }
        if is_key_down(KeyCode::D) {
            delta += right * self.speed;
        }

        // Up/down movement
        if is_key_down(KeyCode::Space) {
            delta.y -= self.speed;
        }
        if is_key_down(KeyCode::LeftShift) {
            delta.y += self.speed;
        }
        self.position += delta;

        if is_mouse_button_pressed(MouseButton::Left) {
            set_cursor_grab(true);
            show_mouse(false);
            self.grabbed = true;
        }

        if self.grabbed {
            // Mouse look (with captured mouse)
            let delta = mouse_delta_position();
            
            // Update yaw (horizontal rotation)
            if delta.x != 0.0 {
                self.yaw -= delta.x * self.sensitivity;
            }
            
            // Update pitch (vertical rotation) with clamping to prevent flipping
            if delta.y != 0.0 {
                self.pitch += delta.y * self.sensitivity;
                // Clamp pitch to prevent camera flipping
                self.pitch = self.pitch.clamp(-std::f32::consts::PI / 2.0 + 0.01, std::f32::consts::PI / 2.0 - 0.01);
            }
        }
        
        // Toggle mouse capture with Escape key
        if is_key_pressed(KeyCode::Escape) {
            set_cursor_grab(false);
            show_mouse(true);
            self.grabbed = false;
        }
    }
}

#[macroquad::main("Voxel Game")]
async fn main() {
    // Capture the mouse cursor by default
    set_cursor_grab(true);
    show_mouse(false);
    
    // Create a camera
    let mut camera = Camera::new(
        Vec3::new(0.0, 0.0, 2.0),  // Initial position
        Vec3::new(0.0, 0.0, -1.0), // Looking forward
    );
    
    // Create a material with our GLSL shaders
    let material = load_material(
        ShaderSource::Glsl {
            vertex: VERTEX_SHADER,
            fragment: FRAGMENT_SHADER,
        },
        MaterialParams {
            uniforms: vec![
                UniformDesc::new("u_resolution", UniformType::Float2),
                UniformDesc::new("u_time", UniformType::Float1),
                UniformDesc::new("u_camera_position", UniformType::Float3),
                UniformDesc::new("u_camera_direction", UniformType::Float3),
            ],
            textures: vec![],
            ..Default::default()
        },
    )
    .expect("Failed to load material");

    loop {
        // Update camera based on input
        camera.update();
        
        // Use our shader material
        gl_use_material(&material);
        
        // Update shader uniforms
        material.set_uniform("u_resolution", vec2(screen_width(), screen_height()));
        material.set_uniform("u_time", get_time() as f32);
        material.set_uniform("u_camera_position", camera.position.to_array());
        // Calculate direction vector from pitch and yaw
        let direction = Vec3::new(
            camera.yaw.cos() * camera.pitch.cos(),
            -camera.pitch.sin(),
            camera.yaw.sin() * camera.pitch.cos()
        ).normalize();
        material.set_uniform("u_camera_direction", direction.to_array());
        
        // Draw full-screen rectangle with our shader
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), BLANK);
        
        // Return to default material
        gl_use_default_material();

        // Display info text
        draw_text(
            format!("Position: ({:.2}, {:.2}, {:.2})", 
                camera.position.x, camera.position.y, camera.position.z).as_str(), 
            20.0, 50.0, 20.0, WHITE
        );

        next_frame().await
    }
}
