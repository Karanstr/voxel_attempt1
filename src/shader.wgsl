struct Data {
    model: mat4x4<f32>,
    projection: mat4x4<f32>,
    resolution: vec2<f32>,
    padding1: vec2<f32>,
    camera_pos: vec3<f32>,
    padding2: f32,
    camera_dir: vec3<f32>,
    padding3: f32,
    // Each element in the array needs to be aligned to 16 bytes
    // We'll use array<vec4<u32>> instead of array<u32> to ensure proper alignment
    voxels: array<vec4<u32>, (8 * 8 * 8) / 4>,
}

@group(0) @binding(0)
var<uniform> data: Data;

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

// Ray hit information
struct RayHit {
    hit: bool,
    position: vec3<f32>,
    normal: vec3<f32>,
    t: f32,
    voxel_value: u32,
}

// Get voxel value at given position
fn get_voxel(pos: vec3<i32>) -> u32 {
    // Bounds check
    if (pos.x < 0 || pos.y < 0 || pos.z < 0 || pos.x >= 8 || pos.y >= 8 || pos.z >= 8) {
        return 0u;
    }
    
    // Calculate index into voxel array
    let flat_index = pos.x * 8 * 8 + pos.y * 8 + pos.z;
    let vec_index = flat_index / 4;
    let component_index = flat_index % 4;
    
    // Extract the correct component from the vec4
    let vec4_value = data.voxels[vec_index];
    
    if (component_index == 0) {
        return vec4_value.x;
    } else if (component_index == 1) {
        return vec4_value.y;
    } else if (component_index == 2) {
        return vec4_value.z;
    } else {
        return vec4_value.w;
    }
}

// DDA-based ray tracing through the voxel grid
fn raymarch_voxels(origin: vec3<f32>, direction: vec3<f32>) -> RayHit {
    var result: RayHit;
    result.hit = false;
    result.t = 1000.0;
    result.normal = vec3<f32>(0.0, 0.0, 0.0);
    result.position = vec3<f32>(0.0, 0.0, 0.0);
    result.voxel_value = 0u;
    
    // Grid dimensions
    let grid_size = vec3<f32>(8.0, 8.0, 8.0);
    
    // Find intersection with the grid bounds
    // Calculate entry and exit points for the ray with the grid bounds
    var tMin = vec3<f32>(-1000.0);
    var tMax = vec3<f32>(1000.0);
    
    // Handle X planes with improved numerical stability
    if (abs(direction.x) > 0.00001) {
        let tx1 = (0.0 - origin.x) / direction.x;
        let tx2 = (grid_size.x - origin.x) / direction.x;
        tMin.x = min(tx1, tx2);
        tMax.x = max(tx1, tx2);
    } else {
        // Ray is parallel to the X axis
        if (origin.x < 0.0 || origin.x > grid_size.x) {
            // Ray is outside grid bounds on X axis and won't intersect
            return result;
        }
    }
    
    // Handle Y planes with improved numerical stability
    if (abs(direction.y) > 0.00001) {
        let ty1 = (0.0 - origin.y) / direction.y;
        let ty2 = (grid_size.y - origin.y) / direction.y;
        tMin.y = min(ty1, ty2);
        tMax.y = max(ty1, ty2);
    } else {
        // Ray is parallel to the Y axis
        if (origin.y < 0.0 || origin.y > grid_size.y) {
            // Ray is outside grid bounds on Y axis and won't intersect
            return result;
        }
    }
    
    // Handle Z planes with improved numerical stability
    if (abs(direction.z) > 0.00001) {
        let tz1 = (0.0 - origin.z) / direction.z;
        let tz2 = (grid_size.z - origin.z) / direction.z;
        tMin.z = min(tz1, tz2);
        tMax.z = max(tz1, tz2);
    } else {
        // Ray is parallel to the Z axis
        if (origin.z < 0.0 || origin.z > grid_size.z) {
            // Ray is outside grid bounds on Z axis and won't intersect
            return result;
        }
    }
    
    // Find the maximum entry and minimum exit points
    let tEntry = max(max(tMin.x, tMin.y), tMin.z);
    let tExit = min(min(tMax.x, tMax.y), tMax.z);
    
    // If tEntry > tExit, the ray misses the grid
    // If tExit < 0, the grid is behind the ray
    if (tEntry > tExit || tExit < 0.0) {
        return result;
    }
    
    // Ensure we start at or after the ray origin
    // Add a small epsilon to avoid numerical precision issues at grid boundaries
    var t = max(0.0, tEntry) + 0.0001;
    
    // Calculate the starting position at the grid boundary
    var pos = origin + direction * t;
    
    // Initialize the current voxel position
    // Use a more robust method to ensure we're inside the grid
    var voxel_pos = vec3<i32>(clamp(floor(pos), vec3<f32>(0.0), vec3<f32>(grid_size - 1.0)));
    
    // Initialize hit_face based on which boundary we entered from
    var hit_face = vec3<f32>(0.0);
    
    // Determine which face we're entering from by checking which t value is largest
    if (tEntry == tMin.x) {
        hit_face = vec3<f32>(-sign(direction.x), 0.0, 0.0);
    } else if (tEntry == tMin.y) {
        hit_face = vec3<f32>(0.0, -sign(direction.y), 0.0);
    } else if (tEntry == tMin.z) {
        hit_face = vec3<f32>(0.0, 0.0, -sign(direction.z));
    }
    
    // Calculate the step direction for each axis
    let step = vec3<i32>(
        i32(sign(direction.x)),
        i32(sign(direction.y)),
        i32(sign(direction.z))
    );
    
    // Calculate the initial tDelta values (distance along the ray to the next voxel boundary)
    var tDelta = vec3<f32>(
        select(1000000.0, abs(1.0 / direction.x), step.x != 0),
        select(1000000.0, abs(1.0 / direction.y), step.y != 0),
        select(1000000.0, abs(1.0 / direction.z), step.z != 0)
    );
    
    // Calculate the initial tMax values (distance along the ray to the next voxel boundary)
    // First, calculate the fractional part of the position with proper handling
    let frac_pos = fract(pos);
    
    // Then calculate the distance to the next voxel boundary with improved robustness
    var tNext = vec3<f32>();
    
    // Handle X axis
    if (step.x > 0) {
        tNext.x = (1.0 - frac_pos.x) * tDelta.x;
    } else if (step.x < 0) {
        tNext.x = frac_pos.x * tDelta.x;
    } else {
        tNext.x = 1000000.0;
    }
    
    // Handle Y axis
    if (step.y > 0) {
        tNext.y = (1.0 - frac_pos.y) * tDelta.y;
    } else if (step.y < 0) {
        tNext.y = frac_pos.y * tDelta.y;
    } else {
        tNext.y = 1000000.0;
    }
    
    // Handle Z axis
    if (step.z > 0) {
        tNext.z = (1.0 - frac_pos.z) * tDelta.z;
    } else if (step.z < 0) {
        tNext.z = frac_pos.z * tDelta.z;
    } else {
        tNext.z = 1000000.0;
    }
    
    // Maximum number of steps to prevent infinite loops
    // Increased for better rendering quality when viewing from outside the grid
    let max_steps = 300;
    
    // DDA algorithm
    for (var i = 0; i < max_steps; i++) {
        // Check if current voxel is filled
        if (voxel_pos.x >= 0 && voxel_pos.x < 8 && 
            voxel_pos.y >= 0 && voxel_pos.y < 8 && 
            voxel_pos.z >= 0 && voxel_pos.z < 8) {
            
            let voxel_value = get_voxel(voxel_pos);
            if (voxel_value > 0u) {
                // We hit a voxel
                result.hit = true;
                result.position = pos;
                result.t = t;
                result.voxel_value = voxel_value;
                result.normal = hit_face; // Use the float-based normal directly
                
                return result;
            }
        } else {
            // We've left the grid bounds
            break;
        }
        
        // Find the minimum tNext value to determine which voxel boundary to cross next
        if (tNext.x < tNext.y && tNext.x < tNext.z) {
            // X axis
            t = tNext.x;
            tNext.x += tDelta.x;
            voxel_pos.x += step.x;
            hit_face = vec3<f32>(-sign(direction.x), 0.0, 0.0);
        } else if (tNext.y < tNext.z) {
            // Y axis
            t = tNext.y;
            tNext.y += tDelta.y;
            voxel_pos.y += step.y;
            hit_face = vec3<f32>(0.0, -sign(direction.y), 0.0);
        } else {
            // Z axis
            t = tNext.z;
            tNext.z += tDelta.z;
            voxel_pos.z += step.z;
            hit_face = vec3<f32>(0.0, 0.0, -sign(direction.z));
        }
        
        // Update position
        pos = origin + direction * t;
        
        // Check if we've exited the grid
        if (t > tExit) {
            break;
        }
    }
    
    return result;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert texture coordinates to ray direction in view space
    // Map from [0,1] to [-1,1] for x and y
    let aspect = data.resolution.x / data.resolution.y;
    
    // Properly handle aspect ratio for even ray distribution
    let uv = vec2<f32>(
        (in.texcoord.x * 2.0 - 1.0) * aspect,
        in.texcoord.y * 2.0 - 1.0
    );
    
    // Camera setup - position outside the grid looking at center
    // Position camera to see multiple faces of the voxel grid
    let camera_pos = data.camera_pos;
    let forward = normalize(data.camera_dir); // Use camera_dir directly as the forward vector
    let up = vec3<f32>(0.0, 1.0, 0.0);
    
    // Create camera basis vectors
    let right = normalize(cross(forward, up));
    let camera_up = normalize(cross(right, forward));
    
    // Create ray direction with proper perspective projection
    let fov_rad = 0.8; // Field of view in radians (approximately 45 degrees)
    
    // This creates a proper perspective projection without radial distortion
    // The ray direction is calculated as a linear combination of the basis vectors
    // scaled by the tangent of the field of view angle
    let ray_dir = normalize(forward + right * uv.x * tan(fov_rad) + camera_up * uv.y * tan(fov_rad));
    
    // Debug visualization - show grid lines
    // This helps visualize the voxel grid structure
    let grid_size = 8.0;
    let grid_spacing = 1.0;
    
    // Perform DDA-based ray tracing
    let hit = raymarch_voxels(camera_pos, ray_dir);
    
    // Color based on hit result
    if (hit.hit) {
        // Base color from normal
        let normal_color = hit.normal * 0.5 + 0.5;
        
        // Make voxel color more vibrant to stand out
        // Slightly vary color based on normal to help distinguish faces
        let normal_influence = abs(hit.normal) * 0.3;
        let voxel_id_color = vec3<f32>(
            1.0 - normal_influence.z * 0.2, 
            0.5 + normal_influence.x * 0.2, 
            0.2 + normal_influence.y * 0.3  // Vary color slightly based on face orientation
        );
        
        // Final color with lighting and distance adjustment
        return vec4<f32>(voxel_id_color, 1.0);
    } else {
        // Grid visualization in the background
        // Project ray to the grid plane (y=0)
        let grid_t = -camera_pos.y / ray_dir.y;
        
        if (grid_t > 0.0) {
            let hit_pos = camera_pos + ray_dir * grid_t;
            
            // Check if we're within grid bounds
            if (hit_pos.x >= 0.0 && hit_pos.x <= grid_size && 
                hit_pos.z >= 0.0 && hit_pos.z <= grid_size) {
                
                // Draw grid lines
                let cell_x = fract(hit_pos.x / grid_spacing);
                let cell_z = fract(hit_pos.z / grid_spacing);
                
                let line_width = 0.05;
                if (cell_x < line_width || cell_x > 1.0 - line_width || 
                    cell_z < line_width || cell_z > 1.0 - line_width) {
                    return vec4<f32>(0.3, 0.3, 0.3, 1.0); // Grid lines
                }
                
                return vec4<f32>(0.1, 0.1, 0.1, 1.0); // Grid cells
            }
        }
        
        // Sky gradient
        let sky_t = in.texcoord.y;
        return vec4<f32>(0.5 + sky_t * 0.2, 0.7 + sky_t * 0.1, 1.0, 1.0);
    }
}
