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

fn within3f(pos: vec3<f32>, grid_size: f32) -> bool {
    return all(pos >= vec3<f32>(0.0)) && all(pos <= vec3<f32>(grid_size));
}

fn within3u(pos: vec3<u32>, grid_size: u32) -> bool {
    return all(pos >= vec3<u32>(0)) && all(pos <= vec3<u32>(grid_size));
}

fn within3i(pos: vec3<i32>, grid_size: i32) -> bool {
    return all(pos >= vec3<i32>(0)) && all(pos <= vec3<i32>(grid_size));
}

fn withinf(pos: f32, grid_size: f32) -> bool {
    return pos >= 0.0 && pos <= grid_size;
}

// Get voxel value at given position
fn get_voxel(root: vec2<u32>, pos: vec3<f32>, voxel_size: f32) -> u32 {
    let grid_size = 1u << root[1];
    let cell = vec3<u32>(floor(2.0 * pos / voxel_size));
    var cur_index = root[0];
    for (var i = root[1]; i > 0; i--) {
        let childx = (cell.x >> i) & 1;
        let childy = (cell.y >> i) & 1;
        let childz = (cell.z >> i) & 1;
        let next_index = voxels[cur_index].children[childz << 2 | childy << 1 | childx];
        if (next_index == cur_index) { break; } else { cur_index = next_index; }
    }
    return cur_index;
}

fn raymarch_voxels(camera_pos: vec3<f32>, direction: vec3<f32>, grid_size: f32, grid_spacing: f32) -> RayHit {
    var result: RayHit;
    result.hit = false;
    result.normal = vec3<f32>(0.0, 0.0, 0.0);
    result.voxel_value = 0u;
    let epsilon = 0.00001;
    let zeros = vec3<f32>(0.0);

    let tMin = select(0.0 - camera_pos, grid_size - camera_pos, direction < zeros) / direction;

    if (!within3f(camera_pos, grid_size)
        && (((is_inf(tMin.x) || tMin.x < 0.0) && !withinf(camera_pos.x, grid_size))
        || ((is_inf(tMin.y) || tMin.y < 0.0) && !withinf(camera_pos.y, grid_size))
        || ((is_inf(tMin.z) || tMin.z < 0.0) && !withinf(camera_pos.z, grid_size)))
    ) { return result; }

    let tEntry = max(max(tMin.x, tMin.y), tMin.z);
    var t = max(0.0, tEntry) + epsilon;
    var pos = camera_pos + direction * t;
    
    let stepdir = sign(direction);
    result.normal = select(zeros, -stepdir, tMin == vec3<f32>(tEntry));
    let max_steps = 20;
    for (var i = 0; i < max_steps; i++) {
        if (!within3f(pos, grid_size)) { break; }
        result.voxel_value = get_voxel(data.render_root, pos, grid_spacing);
        if (result.voxel_value > 0u) {
            result.hit = true;
            return result;
        }

        let next_pos = select(
            select(ceil(pos), floor(pos), stepdir < zeros),
            pos,
            stepdir == zeros
        );
        let times = (next_pos - pos) / direction;
        let t = min(times.x, min(times.y, times.z));
        result.normal = select(zeros, -stepdir, times == vec3<f32>(t));
        
        pos += direction * t + epsilon * stepdir;
    }

    return result;
}

// Camera is passed in with it's origin on the center of the voxel grid
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let grid_size = f32(1u << data.render_root[1]);
    // Lowest voxel size
    let grid_spacing = 1.0;

    let aspect = data.resolution.x / data.resolution.y;
    // Transform from <0,1> to <-1, 1>
    let uv = vec2<f32>(in.texcoord.x * 2.0 - 1.0, in.texcoord.y * 2.0 - 1.0) * vec2<f32>(aspect, 1.0);
    
    let camera_pos = data.camera_pos;
    let forward = data.camera_dir;

    let up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(forward, up));
    let camera_up = normalize(cross(right, forward));
    
    let fov_rad = 1.;
    let ray_dir = normalize(forward + right * uv.x * tan(fov_rad) + camera_up * uv.y * tan(fov_rad));
    
    let hit = raymarch_voxels(
        camera_pos, 
        ray_dir, 
        grid_size,
        grid_spacing,
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
            
            if (withinf(hit_pos.x, grid_size) && withinf(hit_pos.z, grid_size)) {
                
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
        
        return vec4<f32>(0.0);
    }
}
