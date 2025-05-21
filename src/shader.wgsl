struct Data {
    model: mat4x4<f32>,
    projection: mat4x4<f32>,
    resolution: vec2<f32>,
    render_root: vec2<u32>,
    camera_pos: vec3<f32>,
    padding1: f32,
    camera_dir: vec3<f32>,
    voxel_count: f32,
}

@group(0) @binding(0)
var<uniform> data: Data;

// Define the structure that matches BasicNode3d in Rust
struct VoxelNode {
    children: array<u32, 8>
}
@group(0) @binding(1)
var<storage> voxels: array<VoxelNode>;

const MIN_BLOCK_SIZE: f32 = 1.0;
const FP_BUMP: f32 = 0.0001; 

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
    out.position = vec4<f32>(in.position.x, in.position.y, 0.0, 1.0);
    out.texcoord = in.texcoord;
    return out;
}

// Ray hit information
struct RayHit {
    hit: bool,
    // position: vec3<f32>,
    normal: vec3<f32>,
    t: f32,
    voxel_value: u32,
}

fn get_vox_data(root: vec2<u32>, cell: vec3<u32>) -> u32 {
    if (any(clamp(cell, vec3(0u), vec3(1u << root[1]) - 1) != cell)) { return 0u; }
    var cur_index = root[0];
    for (var cur_height = root[1]; cur_height > 0; cur_height -= 1) {
        let shift = cur_height - 1;
        let childx = (cell.x >> shift) & 1;
        let childy = (cell.y >> shift) & 1;
        let childz = (cell.z >> shift) & 1;
        let next_index = voxels[cur_index].children[childz << 2 | childy << 1 | childx];
        if (next_index == cur_index) { break; } else { cur_index = next_index; }
    }
    return cur_index;
}

// Assumes camera_pos has been normalized by min_cell
fn dda_vox(camera_pos: vec3<f32>, dir: vec3<f32>, bounds: vec3<f32>) -> RayHit {
    // Initialize result
    var result = RayHit();
    result.hit = false;
    // We only march through grids for now, not into them.
    if (any(clamp(camera_pos, vec3<f32>(0.0), bounds) != camera_pos)) { return result; }
    let cell_bounds = vec3<i32>(bounds);
    // Current voxel cell
    var cur_voxel = vec3<i32>(floor(camera_pos));
    
    // Direction to step in the grid (either 1, 0, or -1 for each axis)
    let step = vec3<i32>(sign(dir));
    
    // Handle zero components in direction to avoid division by zero
    let safe_dir = select(dir, vec3<f32>(FP_BUMP), abs(dir) < vec3<f32>(FP_BUMP));
    
    // Calculate inverse of direction for faster calculations
    let inv_dir = 1.0 / safe_dir;
    

    var t_max = select(
        select(
            (ceil(camera_pos + FP_BUMP) - camera_pos) * inv_dir,
            (floor(camera_pos) - camera_pos) * inv_dir,
            step < vec3<i32>(0)
        ),
        vec3<f32>(1000000.0),
        step == vec3<i32>(0)
    );
    
    // Distance between voxel boundaries
    let t_delta = abs(inv_dir);
    
    // Current distance along ray
    var t = 0.0;
    
    // Face normal
    var normal = vec3<f32>(0.0);
    
    // Main DDA loop
    for (var i = 0u; i < 100u; i++) {
        // Check current voxel
        let cur_val = get_vox_data(data.render_root, vec3<u32>(cur_voxel));
        
        if (cur_val != 0u && t != 0.0) {
            // We hit a voxel
            result.hit = true;
            result.voxel_value = cur_val;
            result.normal = normal;
            result.t = t;
            return result;
        }
        
        let min_t = min(min(t_max.x, t_max.y), t_max.z);
        let mask = select(
            vec3<f32>(0.0),
            vec3<f32>(1.0),
            t_max == vec3<f32>(min_t)
        );
        cur_voxel += step * vec3<i32>(mask);
        t = min_t + 0.001; // ?????
        t_max += t_delta * mask;
        normal = mask;
        
        // Check if we've gone outside the bounds
        if (any(clamp(cur_voxel, vec3<i32>(0), cell_bounds) != cur_voxel)) {
            break;
        }
    }
    
    // No hit found
    return result;
}

// Camera is passed in with it's origin on the center of the voxel grid
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let cells = 1u << data.render_root[1];
    // Lowest voxel size
    // 0,0 Bottom Right ->  cells * cell_length Top Left
    let bounds = vec3<f32>(f32(cells));

    let aspect = data.resolution.x / data.resolution.y;
    // Transform + Scale from <0,1> to <-1, 1> then scale by aspect ratio
    let uv = 2.0 * (in.texcoord - 0.5) * vec2<f32>(aspect, 1.0);
    
    let camera_pos = data.camera_pos / MIN_BLOCK_SIZE;
    let forward = data.camera_dir;

    let up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(forward, up));
    let camera_up = normalize(cross(right, forward));
    
    let tan_fov = tan(1.0);

    // We don't need to normalize this, all we care about is the ratio
    let ray_dir = forward + right * uv.x * tan_fov + camera_up * uv.y * tan_fov;
    
    let hit = dda_vox(
        camera_pos, 
        ray_dir, 
        bounds
    );
    
    if (hit.hit) {
        let hit_pos = camera_pos + ray_dir * hit.t;
        let percent_of_block = fract(hit_pos);

        let near_edge = vec3<i32>((percent_of_block < vec3<f32>(0.01)) | (percent_of_block > vec3<f32>(0.99)));
        let edge_count = near_edge.x + near_edge.y + near_edge.z;
        if (edge_count >= 2) { return vec4<f32>(0.0); }

        // Base color from normal
        let per_color = mix(vec3(0.0), vec3(1.0), percent_of_block);
        
        // Vary color based on normal to help distinguish faces
        let normal_influence = abs(hit.normal) * 0.3;
        let voxel_id_color = vec3<f32>(
            0.8 - normal_influence.z * 0.2, 
            0.5 + normal_influence.x * 0.2, 
            0.4 + normal_influence.y * 0.3
        );
        var color = mix(voxel_id_color, per_color, vec3(0.5));
        return vec4<f32>(color, 1.0);
    } else {
        // Grid visualization in the background
        // Project ray to the grid plane (y = 0)
        let grid_t = -camera_pos.y / ray_dir.y;
        
        if (grid_t > 0.0) {
            let hit_pos = camera_pos + ray_dir * grid_t;
            
            if (clamp(hit_pos.x, 0.0, bounds.x) == hit_pos.x
             && clamp(hit_pos.z, 0.0, bounds.z) == hit_pos.z) {
                
                // Draw grid lines
                let cell_x = fract(hit_pos.x);
                let cell_z = fract(hit_pos.z);
                
                let line_width = 0.05;
                if (cell_x < line_width || cell_x > 1.0 - line_width || 
                    cell_z < line_width || cell_z > 1.0 - line_width) {
                    return vec4<f32>(0.3, 0.3, 0.3, 1.0); // Grid lines
                }
                
                return vec4<f32>(0.1, 0.1, 0.1, 1.0); // Grid cells
            }
        }
        
        return vec4<f32>(0.0);
    }
}
