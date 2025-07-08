const WG_SIZE = 8;
const TREE_HEIGHT = 12u;
const SENTINEL = -314159.0; 

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
  if gid.x >= resolution.x || gid.y >= resolution.y { return; }
  // Transform from <0,1> to <-1, 1>, then scale by aspect ratio
  let uv = 2 * ((vec2<f32>(gid.xy) + 0.5) / vec2<f32>(resolution.xy) - 0.5) * vec2<f32>(data.aspect_ratio, 1.0);
  textureStore(output_tex, vec2<i32>(gid.xy), march_init(uv));
}

// Ray hit information
struct RayHit {
  axis: vec3<bool>,
  voxel: vec2<u32>,
  steps: u32,
}

fn march_init(uv: vec2<f32>) -> vec4<f32> {
  let ray_dir = data.cam_forward + data.tan_fov * (data.cam_right * uv.x + data.cam_up * uv.y);
  let inv_dir = sign(ray_dir) / abs(ray_dir);
  var ray_origin = data.cam_pos;
  if any(ray_origin < vec3(0.0)) || any(ray_origin > vec3(data._obj_bounds)) {
    let t_start = aabb_intersect(ray_origin, inv_dir);
    if t_start == SENTINEL { return vec4(0.0); }
    ray_origin += ray_dir * max(0, t_start);
  } 
  let hit = dda_vox_v4(ray_origin, ray_dir, inv_dir);
  let step_color = 1.0 / vec3<f32>(hit.steps);
  let normal_color = 1.0 + vec3<f32>(hit.axis) * vec3(-0.2, 0.3, 0.4);
  return vec4(step_color * normal_color, 1);
}

fn dda_vox_v4(ray_origin: vec3<f32>, ray_dir: vec3<f32>, inv_dir: vec3<f32>) -> RayHit {
  var result = RayHit();
  let step_f32 = vec3(sign(inv_dir));
  let step = vec3<i32>(step_f32);
  let dir_neg = step < vec3(0);
  var t = 0.0;

  while (result.steps < 500u) {
    result.steps += 1;
    let cur_pos = ray_origin + ray_dir * t;
    // Sample current position
    if any(cur_pos + step_f32 * 0.001 < vec3(0.0)) || any(cur_pos >= vec3(data._obj_bounds)) { break; }
    let cur_voxel = vec3<u32>(cur_pos + step_f32 * 0.001);
    result.voxel = vox_read(data.obj_head, cur_voxel);
    if result.voxel[0] != 0u { break; }
    // Sparse marching nonsense
    let neg_wall = cur_voxel & vec3(~0u << result.voxel[1]);
    let pos_wall = neg_wall + (1u << result.voxel[1]);
    let next_wall = select(pos_wall, neg_wall, dir_neg);
    
    // let distance = vec3<f32>(next_wall - vec3<u32>(cur_pos)) - fract(cur_pos);
    let distance = vec3<f32>(next_wall) - cur_pos;
    let t_wall = distance * inv_dir;
    let t_next = min(min(t_wall.x, t_wall.y), t_wall.z);
    result.axis = t_wall == vec3<f32>(t_next);
    t += t_next;
  }
  return result;
}

/// Trusts that you submit a valid cell
fn vox_read(head: u32, cell: vec3<u32>) -> vec2<u32> {
  var cur_idx = head;
  var height = TREE_HEIGHT;
  while height != 0 {
    let child = cell >> vec3<u32>(height - 1) & vec3<u32>(1);
    let next_idx = voxels[cur_idx].children[child.z << 2 | child.y << 1 | child.x];
    if next_idx == cur_idx { break; }
    cur_idx = next_idx;
    height -= 1;
  }
  return vec2<u32>(cur_idx, height);
}

fn aabb_intersect(ray_origin: vec3<f32>, inv_dir: vec3<f32>) -> f32 {
  let just_before = bitcast<vec3<f32>>(bitcast<vec3<u32>>(data._obj_bounds) - 1);
  let t1 = (vec3(0.0) - ray_origin) * inv_dir;
  let t2 = (just_before - ray_origin) * inv_dir;
  let min_t = min(t1, t2);
  let max_t = max(t1, t2);
  let t_entry = max(max(min_t.x, min_t.y), min_t.z);
  let t_exit = min(min(max_t.x, max_t.y), max_t.z);

  if t_entry > t_exit || t_exit < 0.0 { return SENTINEL; }
  return t_entry;
}

