struct Uniforms {
    model: mat4x4<f32>,
    projection: mat4x4<f32>,
    resolution: vec2<f32>,
    mouse_position: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Vertex shader
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) texcoord: vec2<f32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
};
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Pass through position as is (already in NDC space)
    out.position = vec4<f32>(in.position.x, in.position.y, 0.0, 1.0);
    // Pass texture coordinates to fragment shader
    out.texcoord = in.texcoord;
    return out;
}

// Calculate Manhattan distance between two points
fn manhattan_distance(p1: vec2<f32>, p2: vec2<f32>) -> f32 {
    return abs(p1.x - p2.x) + abs(p1.y - p2.y);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert texture coordinates to pixel coordinates
    var pixel_pos = in.texcoord * uniforms.resolution;
    // Flip y coordinate so y = 0 is top
    pixel_pos.y = uniforms.resolution.y - pixel_pos.y;
    
    // Define cell size (adjust as needed)
    let cell_size = 20.0;
    
    // Calculate cell coordinates
    let cell_x = floor(pixel_pos.x / cell_size);
    let cell_y = floor(pixel_pos.y / cell_size);
    let cell_pos = vec2<f32>(cell_x, cell_y);
    
    // Calculate mouse position in cell coordinates
    let mouse_cell_x = floor(uniforms.mouse_position.x * uniforms.resolution.x / cell_size);
    let mouse_cell_y = floor(uniforms.mouse_position.y * uniforms.resolution.y / cell_size);
    let mouse_cell_pos = vec2<f32>(mouse_cell_x, mouse_cell_y);
    
    // Calculate Manhattan distance from current cell to mouse cell
    let dist = manhattan_distance(cell_pos, mouse_cell_pos);
    
    // Create a color based on the Manhattan distance
    // Use modulo to create a repeating pattern
    let max_steps = 5.0;
    let normalized_dist = dist / max_steps;
    let step_value = dist % max_steps;
    
    // Create grid lines
    let cell_edge_x = fract(pixel_pos.x / cell_size) < 0.05 || fract(pixel_pos.x / cell_size) > 0.95;
    let cell_edge_y = fract(pixel_pos.y / cell_size) < 0.05 || fract(pixel_pos.y / cell_size) > 0.95;
    let is_grid = cell_edge_x || cell_edge_y;
    
    // Create color based on distance
    var color: vec3<f32>;
    if (dist <= max_steps) {
        // Rainbow effect based on distance
        if (step_value < 1.0) {
            color = vec3<f32>(1.0, 0.0, 0.0); // Red
        } else if (step_value < 2.0) {
            color = vec3<f32>(1.0, 0.5, 0.0); // Orange
        } else if (step_value < 3.0) {
            color = vec3<f32>(1.0, 1.0, 0.0); // Yellow
        } else if (step_value < 4.0) {
            color = vec3<f32>(0.0, 1.0, 0.0); // Green
        } else {
            color = vec3<f32>(0.0, 0.0, 1.0); // Blue
        }
    } else {
        // Default color for cells beyond max_steps
        color = vec3<f32>(0.1, 0.1, 0.1);
    }
    
    // Apply grid lines
    if (is_grid) { 
        color = mix(color, vec3<f32>(0.5, 0.5, 0.5), 0.5);
    }
    
    // Highlight the cell under the mouse
    if (cell_pos.x == mouse_cell_pos.x && cell_pos.y == mouse_cell_pos.y) {
        color = vec3<f32>(1.0, 1.0, 1.0);
    }
    
    return vec4<f32>(color, 1.0);
}
