const WG_SIZE = 8;
const FP_BUMP: f32 = 0.0001; 

@group(0) @binding(0)
var output_tex: texture_storage_2d<rgba8unorm, write>;

struct Data {
  obj_head: u32,
  _obj_bounds: f32,
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
  let resolution = vec2<u32>(textureDimensions(output_tex));
  // We do a little padding so we can fit our pixels into the workgroups
  if (gid.x >= resolution.x || gid.y >= resolution.y) { return; }
  // Transform from <0,1> to <-1, 1>, then scale by aspect ratio
  let uv = 2 * ((vec2<f32>(gid.xy) + 0.5) / vec2<f32>(resolution.xy) - 0.5) * vec2<f32>(data.aspect_ratio, 1.0);
  textureStore(output_tex, vec2<i32>(gid.xy), march_init(uv));
}

// Ray hit information
struct RayHit {
  axis: vec3<bool>,
  t: f32,
  voxel: vec2<u32>,
}

// Cam_pos is normalized to 0.0 - 1.0 being within the region NOT TRUE YET
fn march_init(uv: vec2<f32>) -> vec4<f32> {
  let ray_dir = data.cam_forward + data.tan_fov * (data.cam_right * uv.x + data.cam_up * uv.y);
  let inv_dir = sign(ray_dir) / max(abs(ray_dir), vec3(FP_BUMP));

  var ray_origin = data.cam_pos;
  if (any(clamp(data.cam_pos, vec3(0.0), vec3<f32>(ENTIRE_BIT)) != data.cam_pos)) { 
  // if (any(clamp(data.cam_pos, vec3(0.0), vec3(1.0)) != data.cam_pos)) { 
    let t_start = aabb_intersect(data.cam_pos, inv_dir);
    if t_start == -314159.0 { return vec4(0.0); }
    ray_origin += ray_dir * max(0, t_start);
  } 
  let hit = dda_vox(ray_origin, inv_dir);

  var base_color: vec3<f32>;
  switch hit.voxel[0] {
    case 0: { base_color = vec3(0.0); }
    case 1: { base_color = make_color(115, 20, 146); }
    case 2: { base_color = make_color(20, 100, 20); }
    default: { base_color = vec3(1.0); } // Unknown block
  }
  let normal_color = vec3(
    1.0 - f32(hit.axis.z) * 0.2,
    1.0 + f32(hit.axis.x) * 0.3,
    1.0 + f32(hit.axis.y) * 0.4,
  );
  return vec4(base_color * normal_color, 0);
}

fn make_color(r: u32, g: u32, b: u32) -> vec3<f32> { return vec3(f32(r), f32(g), f32(b)) / 255.0; }

// The 7th bit represents half the voxel volume.
const HALF_BIT:u32 = 1 << 7;
const ENTIRE_BIT:u32 = 1 << 8;

// ray_origin is currently guaranteed to start within bounds
fn dda_vox(ray_origin: vec3<f32>, inv_dir: vec3<f32>) -> RayHit {
  let step = vec3<i32>(sign(inv_dir));
  // Top new system, bottom old system (remember to change the ray_origin normalization in wgpu_ctx)
  // let rounded_pos = (ray_origin + vec3<f32>(step) * FP_BUMP) * f32(ENTIRE_BIT);
  // Ceil if moving positively, floor otherwise
  // let dir_rounded_pos = rounded_pos + select(vec3(1.0), vec3(0.0), step < vec3<i32>(0));
  // let normal_pos = (ray_origin) * f32(ENTIRE_BIT);
  // var t_next = select(rounded_pos - normal_pos, vec3(10000000.0), step == vec3(0)) * inv_dir;
  // var cur_voxel = vec3<i32>(rounded_pos);
  let rounded_pos = select(ceil(ray_origin + FP_BUMP), floor(ray_origin - FP_BUMP), step < vec3<i32>(0));
  var t_next = select(rounded_pos - ray_origin, vec3(10000000.0), step == vec3(0)) * inv_dir;
  var cur_voxel = vec3<i32>(floor(ray_origin + FP_BUMP * vec3<f32>(step)));
  
  var result = RayHit();
  for (var i = 0u; i < 300u; i++) {
    result.voxel = vox_read(data.obj_head, vec3<u32>(cur_voxel));
    if result.voxel[0] != 0u { break; }

    let mask = vec3<i32>((1 << result.voxel[1]) - 1);
    let offset = cur_voxel & mask;
    let additional_blocks = vec3<i32>(select(-offset, mask - offset, vec3<i32>(0) < step));
    let sparse_next = t_next + inv_dir * vec3<f32>(additional_blocks);
    result.t = min(min(sparse_next.x, sparse_next.y), sparse_next.z);
    result.axis = sparse_next == vec3<f32>(result.t);

    // Sparse skipping
    t_next = select(t_next, sparse_next, sparse_next == vec3(result.t));
    cur_voxel = select(cur_voxel, cur_voxel + additional_blocks, sparse_next == vec3(result.t));

    // Catchup loop
    loop {
      let t_cur = min(min(t_next.x, t_next.y), t_next.z);
      cur_voxel = select(cur_voxel, cur_voxel + step, t_next == vec3(t_cur));
      t_next = select(t_next, t_next + abs(inv_dir), t_next == vec3(t_cur));
      if (t_cur + FP_BUMP >= result.t) { break; }
    }
    if (any(clamp(cur_voxel, vec3<i32>(0), vec3<i32>(ENTIRE_BIT - 1)) != cur_voxel)) { break; }
  }
  return result;
}

/// Trusts that you submit a cell which fits within the root
fn vox_read(head: u32, cell: vec3<u32>) -> vec2<u32> {
  var cur_idx = head;
  var height = 8u;
  while height != 0 {
    let child = cell >> vec3<u32>(height - 1) & vec3<u32>(1);
    let next_idx = voxels[cur_idx].children[child.z << 2 | child.y << 1 | child.x];
    if (next_idx == cur_idx) { break; }
    cur_idx = next_idx;
    height -= 1;
  }
  return vec2<u32>(cur_idx, height);
}

fn aabb_intersect(ray_origin: vec3<f32>, inv_dir: vec3<f32>) -> f32 {
  let t1 = (vec3(0.0) - ray_origin) * inv_dir;
  let t2 = (vec3(1.0) - ray_origin) * inv_dir;
  let min_t = min(t1, t2);
  let max_t = max(t1, t2);
  let t_entry = max(max(min_t.x, min_t.y), min_t.z);
  let t_exit = min(min(max_t.x, max_t.y), max_t.z);

  if t_entry > t_exit || t_exit < 0.0 { return -314159.0; } // Sentinel value
  return t_entry;
}

