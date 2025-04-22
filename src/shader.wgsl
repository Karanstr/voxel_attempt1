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

fn is_inf(val: f32) -> bool {
    let zero = f32(0.0);
    return abs(val) == f32(1.0) / zero;
}

// Ray hit information
struct RayHit {
    hit: bool,
    // position: vec3<f32>,
    normal: vec3<f32>,
    // t: f32,
    voxel_value: u32,
}

fn get_vox_data(root: vec2<u32>, cell: vec3<u32>) -> u32 {
    if (any(clamp(cell, vec3(0u), vec3(1u << root[1]) - 1) != cell)) { return 0u; }
    var cur_index = root[0];
    for (var cur_height = root[1]; cur_height >= 0; cur_height -= 1) {
        let childx = (cell.x >> cur_height) & 1;
        let childy = (cell.y >> cur_height) & 1;
        let childz = (cell.z >> cur_height) & 1;
        let next_index = voxels[cur_index].children[childz << 2 | childy << 1 | childx];
        if (next_index == cur_index) { break; } else { cur_index = next_index; }
    }
    return cur_index;
}

fn raymarch_voxels(camera_pos: vec3<f32>, direction: vec3<f32>, cells: u32, min_cell: f32) -> RayHit {
    var result: RayHit;
    result.hit = false;
    let epsilon = 0.00001;
    let inv_direction = 1.0 / direction;
    let bounds = vec3(f32(cells) * min_cell);

    let tMin = select(0.0 - camera_pos, bounds - camera_pos, direction < vec3(0.0)) * inv_direction;
    let tEntry = max(max(tMin.x, tMin.y), tMin.z);
    let initial_t = max(0.0, tEntry + epsilon);
    var pos = camera_pos + direction * initial_t;
    let stepdir = sign(direction);
    
    result.normal =  select(vec3(0.0), -stepdir, tMin == vec3<f32>(tEntry));
    let max_steps = 100;
    for (var i = 0; i < max_steps; i++) {
        if (any(clamp(pos, vec3(0.0), bounds) != pos)) { break; }
        let cell = vec3<u32>(floor(pos / min_cell));
        result.voxel_value = get_vox_data(data.render_root, cell);
        if (result.voxel_value > 0u) {
            result.hit = true;
            return result;
        }
        let next_pos = select(
            select(ceil(pos), floor(pos), stepdir < vec3(0.0)),
            pos,
            stepdir == vec3(0.0)
        );
        let times = (next_pos - pos) * inv_direction;
        let t = min(times.x, min(times.y, times.z));
        result.normal = select(vec3(0.0), -stepdir, times == vec3<f32>(t));
        
        pos += direction * t + epsilon * stepdir;
    }

    return result;
}

// fn impr_march(camera_pos: vec3<f32>, direction: vec3<f32>, grid_size: f32, grid_spacing: f32) -> RayHit {
//     let step = vec3<i32>(sign(direction));
//     let epsilon = 0.00001;
//     let step_t = abs(grid_spacing / direction);
//     let tMin = select(0.0 - camera_pos, grid_size - camera_pos, direction < vec3(0.0)) / direction + epsilon * vec3<f32>(step);
//     let tEntry = max(max(tMin.x, tMin.y), tMin.z);
//     let entry_pos = camera_pos + direction * max(0.0, tEntry);

//     var cur_voxel = vec3<i32>(floor(entry_pos / grid_spacing));
//     let next_walls = vec3<f32>(cur_voxel + step) * grid_spacing;
//     var marched_t = (next_walls - entry_pos) / direction;

//     let max_steps = 100;
//     for (var i = 0; i < max_steps; i++) {
//         // if (!within3i(cur_voxel, i32(grid_size))) { break; }
//         let min_t = min(marched_t.x, min(marched_t.y, marched_t.z));
//         let mask = vec3<f32>(vec3<f32>(min_t) == marched_t);
//         cur_voxel += step * vec3<i32>(mask);
//         marched_t += step_t * mask;
//         let cur_voxel_value = get_vox_data(data.render_root, vec3<u32>(cur_voxel));
//         if (cur_voxel_value > 0u) {
//             var result: RayHit;
//             result.hit = true;
//             result.normal = vec3<f32>(-step) * mask;
//             result.voxel_value = cur_voxel_value;
//             return result;
//         }
//     }

//     var result: RayHit;
//     result.hit = false;
//     return result;
// }

// Camera is passed in with it's origin on the center of the voxel grid
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let cells = 1u << data.render_root[1];
    // Lowest voxel size
    let min_cell = 1.0;
    let bounds = vec3(f32(cells) * min_cell);

    let aspect = data.resolution.x / data.resolution.y;
    // Transform from <0,1> to <-0.5, 0.5> then scale by aspect ratio
    let uv = (in.texcoord - 0.5) * vec2<f32>(aspect, 1.0);
    
    let camera_pos = data.camera_pos;
    let forward = data.camera_dir;

    let up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(forward, up));
    let camera_up = normalize(cross(right, forward));
    
    let fov_rad = 1.;
    // We don't need to normalize this, all we care about is the ratio
    let ray_dir = forward + right * uv.x * tan(fov_rad) + camera_up * uv.y * tan(fov_rad);
    
    let hit = raymarch_voxels(
        camera_pos, 
        ray_dir, 
        cells,
        min_cell,
    );
    
    if (hit.hit) {
        // Base color from normal
        let normal_color = hit.normal * 0.5 + 0.5;
        
        // Slightly vary color based on normal to help distinguish faces
        let normal_influence = abs(hit.normal) * 0.3;
        let voxel_id_color = vec3<f32>(
            0.8 - normal_influence.z * 0.2, 
            0.5 + normal_influence.x * 0.2, 
            0.4 + normal_influence.y * 0.3
        );
        return vec4<f32>(voxel_id_color, 1.0);
    } else {
        // Grid visualization in the background
        // Project ray to the grid plane (y = 0)
        let grid_t = -camera_pos.y / ray_dir.y;
        
        if (grid_t > 0.0) {
            let hit_pos = camera_pos + ray_dir * grid_t;
            
            if (clamp(hit_pos.x, 0.0, bounds.x) == hit_pos.x
             && clamp(hit_pos.z, 0.0, bounds.z) == hit_pos.z) {
                
                // Draw grid lines
                let cell_x = fract(hit_pos.x / min_cell);
                let cell_z = fract(hit_pos.z / min_cell);
                
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
