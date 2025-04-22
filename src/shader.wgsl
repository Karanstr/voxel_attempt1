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

fn raymarch_voxels(camera_pos: vec3<f32>, direction: vec3<f32>, bounds: vec3<f32>, min_cell: f32) -> RayHit {
    let epsilon = 0.00001;
    let inv_direction = 1.0 / direction;

    let tMin = select(0.0 - camera_pos, bounds - camera_pos, direction < vec3(0.0)) * inv_direction;
    let tEntry = max(max(tMin.x, tMin.y), tMin.z);
    let initial_t = max(0.0, tEntry + epsilon);
    var pos = camera_pos + direction * initial_t;
    let stepdir = sign(direction);
    
    var normal = select(vec3(0.0), -stepdir, tMin == vec3<f32>(tEntry));
    var cur_t = initial_t;
    let max_steps = 100;
    for (var i = 0; i < max_steps; i++) {
        if (any(clamp(pos, vec3(0.0), bounds) != pos)) { break; }
        let cell = vec3<u32>(floor(pos / min_cell));
        let voxel_value = get_vox_data(data.render_root, cell);
        if (voxel_value != 0u) {
            var result = RayHit();
            result.hit = true;
            result.voxel_value = voxel_value;
            result.normal = normal;
            result.t = cur_t;
            return result;
        }
        let next_pos = select(
            select(ceil(pos), floor(pos), stepdir < vec3(0.0)),
            pos,
            stepdir == vec3(0.0)
        );
        let times = (next_pos - pos) * inv_direction;
        let t = min(times.x, min(times.y, times.z));
        normal = select(vec3(0.0), -stepdir, times == vec3<f32>(t));
        
        pos += direction * t + epsilon * stepdir;
        cur_t += t;
    }

    var result = RayHit();
    result.hit = false;
    return result;
}

// Camera is passed in with it's origin on the center of the voxel grid
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let cells = 1u << data.render_root[1];
    // Lowest voxel size
    let min_cell = 1.0;
    let bounds = vec3(f32(cells) * min_cell);

    let aspect = data.resolution.x / data.resolution.y;
    // Transform + Scale from <0,1> to <-1, 1> then scale by aspect ratio
    let uv = 2.0 * (in.texcoord - 0.5) * vec2<f32>(aspect, 1.0);
    
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
        bounds,
        min_cell,
    );
    
    if (hit.hit) {
        let hit_pos = camera_pos + ray_dir * hit.t;
        let percent_of_block = fract(hit_pos / min_cell);
        // Base color from normal
        
        let per_color = mix(vec3(0.0), vec3(1.0), percent_of_block);
        
        // Slightly vary color based on normal to help distinguish faces
        let normal_influence = abs(hit.normal) * 0.3;
        let voxel_id_color = vec3<f32>(
            0.8 - normal_influence.z * 0.2, 
            0.5 + normal_influence.x * 0.2, 
            0.4 + normal_influence.y * 0.3
        );
        let color = mix(voxel_id_color, per_color, vec3(0.5));
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
