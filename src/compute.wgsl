const WG_SIZE = 8;
const FP_BUMP: f32 = 0.0001; 

@group(0) @binding(0)
var output_tex: texture_storage_2d<rgba8unorm, write>;

struct Data {
  obj_head: u32,
  obj_bounds: f32,
  aspect_ratio: f32,
  tan_fov: f32,
  cam_pos: vec3<f32>,
  // padding: f32
  cam_forward: vec3<f32>,
  // padding: f32
  cam_right: vec3<f32>,
  // padding: f32
  cam_up: vec3<f32>,
  // padding: f32
}
@group(0) @binding(1)
var<uniform> data: Data;

struct VoxelNode { children: array<u32, 8> }
@group(0) @binding(2)
var<storage> voxels: array<VoxelNode>;

@compute @workgroup_size(WG_SIZE, WG_SIZE)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  // We do a little padding so we can fit our pixels into the workgroups
  // This prevents any unwanted computation on the edges
  let resolution = vec2<u32>(textureDimensions(output_tex));
  if (gid.x >= resolution.x || gid.y >= resolution.y) { return; }
  let color = march_init(gid, resolution);
  textureStore(output_tex, vec2<i32>(gid.xy), color);
}

// Ray hit information
struct RayHit {
  hit: bool,
  normal: vec3<f32>,
  t: f32,
  voxel: vec2<u32>,
}

fn march_init(gid: vec3<u32>, resolution: vec2<u32>) -> vec4<f32> {
  // For now we can't march into a grid, just through it.
  if (any(clamp(data.cam_pos, vec3<f32>(0.0), vec3<f32>(data.obj_bounds)) != data.cam_pos)) { return vec4<f32>(0.0); }

  // Transform + Scale from <0,1> to <-1, 1>, then scale by aspect ratio
  let uv = 2 * ((vec2<f32>(gid.xy) + 0.5) / vec2<f32>(resolution.xy) - 0.5) * vec2<f32>(data.aspect_ratio, 1.0);
  let ray_dir = data.cam_forward + data.tan_fov * (data.cam_right * uv.x + data.cam_up * uv.y);

  let hit = dda_vox(data.cam_pos, ray_dir, data.obj_bounds);
  if (hit.hit) {
    let hit_pos = data.cam_pos + ray_dir * (hit.t + FP_BUMP);
    // let block_size = data.obj_bounds / f32(1 << hit.voxel[1]);
    let block_size = f32(1 << hit.voxel[1]);
    let percent_of_block = fract(hit_pos / block_size);

    let near_edge = vec3<i32>((percent_of_block < vec3<f32>(0.01)) | (percent_of_block > vec3<f32>(1.0 - 0.01 / block_size)));
    let edge_count = near_edge.x + near_edge.y + near_edge.z;
    // Outline each cube
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
    let color = mix(voxel_id_color, per_color, vec3(0.5));
    return vec4<f32>(color, 1.0);
  } else {
    // This is where we would handle a skybox
    return vec4<f32>(0.0);
  }
}

// A minimum size dda step moves a distance of 1 unit right now.
fn dda_vox(camera_pos: vec3<f32>, dir: vec3<f32>, bounds: f32) -> RayHit {
  var result = RayHit();
  result.hit = false;

  // inv_dir represents the ratio of movement
  // practically, the smallest step we're allowed to take: 1 / (1 << 31). Maybe 1 << 30? I think we might need a bit to track overflow?

  // Order of operations should be as follows:
  // Setup:
  // - Sample current depth
  // - Determine when next wall intersections occur at current depth
  // Loop:
  // - Determine closest wall intersections
  // - Take the step
  // - Sample voxel, return if condition is met

  let step = vec3<i32>(sign(dir));
  var cur_voxel = vec3<i32>(floor(camera_pos + FP_BUMP * vec3<f32>(step)));
  result.voxel = vox_read(data.obj_head, vec3<u32>(cur_voxel));
  


  let inv_dir = vec3<f32>(step) / max(abs(dir), vec3<f32>(FP_BUMP));
  let t_step = abs(inv_dir);
  let rounded_pos = select(ceil(camera_pos + FP_BUMP), floor(camera_pos - FP_BUMP), step < vec3<i32>(0));
  var t_next = select(rounded_pos - camera_pos, vec3<f32>(10000000.0), step == vec3<i32>(0)) * inv_dir;
  
  for (var i = 0u; i < 100u; i++) {
    let block_size = 1 << result.voxel[1];
    let mask = vec3<i32>(block_size - 1);
    let offset = cur_voxel & mask;
    let additional_blocks = vec3<i32>(select(-offset, mask - offset, vec3<i32>(0) < step));
    let sparse_next = t_next + inv_dir * vec3<f32>(additional_blocks);
    result.t = min(min(sparse_next.x, sparse_next.y), sparse_next.z);
    result.normal = select(vec3<f32>(0.0), vec3<f32>(1.0), sparse_next == vec3<f32>(result.t));
    
    // Sparse skipping
    t_next = select(t_next, sparse_next, sparse_next == vec3<f32>(result.t));
    cur_voxel = select(cur_voxel, cur_voxel + additional_blocks, sparse_next == vec3<f32>(result.t));
    
    // Catchup loop
    loop {
      let t_cur = min(min(t_next.x, t_next.y), t_next.z);
      cur_voxel = select(cur_voxel, cur_voxel + step, t_next == vec3<f32>(t_cur));
      t_next = select(t_next, t_next + t_step, t_next == vec3<f32>(t_cur));
      if (t_cur + FP_BUMP >= result.t) { break; }
    }

    if (any(clamp(cur_voxel, vec3<i32>(0), vec3<i32>(bounds) - 1) != cur_voxel)) { break; }
    result.voxel = vox_read(data.obj_head, vec3<u32>(cur_voxel));
    if (result.voxel[0] != 0u) {
      result.hit = true;
      return result;
    }
  }
  // No hit found
  return result;
}

/// Trusts that you submit a cell which fits within the root
fn vox_read(head: u32, cell: vec3<u32>) -> vec2<u32> {
  var cur_idx = head;
  var height = 5u;
  while height != 0 {
    let child = cell >> vec3<u32>(height - 1) & vec3<u32>(1);
    let next_idx = voxels[cur_idx].children[child.z << 2 | child.y << 1 | child.x];
    if (next_idx == cur_idx) { break; }
    cur_idx = next_idx;
    height -= 1;
  }
  return vec2<u32>(cur_idx, height);
}

