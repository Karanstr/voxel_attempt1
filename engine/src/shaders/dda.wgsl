const WG_SIZE = 8;
const TREE_HEIGHT = 11u;
const SENTINEL = -314159.0;

@group(0) @binding(0)
var output_tex: texture_storage_2d<rgba8unorm, write>;

struct VoxelObject {
  // transform: vec3<f32>, // Position?
  // 4byte padding
  head: u32,
  extent: u32, // Eventually replace with vec3<u32>
}

struct Position {
  cell: vec3<i32>,
  // 4byte padding
  offset: vec3<f32>,
  // 4byte padding
}
fn update_pos(pos: ptr<function, Position>, delta: vec3<f32>, bump: vec3<f32>) {
  (*pos).cell += vec3<i32>(floor(delta));
  (*pos).offset += fract(delta) + bump;
  (*pos).cell += vec3<i32>(floor((*pos).offset));
  (*pos).offset = fract((*pos).offset);
}

// Replace with matrix?
struct Camera {
  aspect_ratio: f32,
  tan_fov: f32,

  pos: Position,
  
  forward: vec3<f32>,
  // padding: f32
  right: vec3<f32>,
  // padding: f32
  up: vec3<f32>,
  // padding: f32
}
@group(0) @binding(1)
var<uniform> cam: Camera;

struct VoxelNode { children: array<u32, 8> }
@group(0) @binding(2)
var<storage> voxels: array<VoxelNode>;

@group(0) @binding(3)
var<uniform> obj: VoxelObject;

@compute @workgroup_size(WG_SIZE, WG_SIZE)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let resolution = vec2<u32>(textureDimensions(output_tex));
  // We do a little padding so we can fit into the workgroups correctly
  if gid.x >= resolution.x || gid.y >= resolution.y { return; }
  // Transform from <0,1> to <-1, 1>, then scale by aspect ratio
  let uv = ((vec2<f32>(gid.xy) + 0.5) / vec2<f32>(resolution.xy) - 0.5) * 2 * vec2(cam.aspect_ratio, 1.0);
  textureStore(output_tex, vec2<i32>(gid.xy), march_init(uv));
}

// Ray hit information
struct RayHit {
  axis: vec3<bool>,
  voxel: vec2<u32>,
  pos: Position,
  steps: u32,
}

fn march_init(uv: vec2<f32>) -> vec4<f32> {
  let ray_dir = cam.forward + cam.tan_fov * (cam.right * uv.x + cam.up * uv.y);
  let inv_dir = sign(ray_dir) / abs(ray_dir); // 1.0 / ray_dir;
  let bump = sign(ray_dir) * 0.001;
  var delta = vec3(0.0);
  // Perform translation up here
  if any(bitcast<vec3<u32>>(cam.pos.cell) >= vec3(obj.extent)) {
    let t_start = aabb_intersect(vec3<f32>(cam.pos.cell) + cam.pos.offset, inv_dir);
    if t_start == SENTINEL { return vec4(0.0); }
    delta = ray_dir * max(0, t_start);
  }
  var pos = cam.pos;
  update_pos(&pos, delta, bump);
  let hit = dda_vox_v4(pos, ray_dir, inv_dir, bump);
  if hit.voxel[0] == 0 { return vec4(0.0); }
  let step_color = 1.0 / vec3<f32>(hit.steps);
  let normal_color = 1.0 + vec3<f32>(hit.axis) * vec3(-0.2, 0.3, 0.4);
  return vec4(step_color * normal_color, 1);
}

fn dda_vox_v4(initial_pos: Position, ray_dir: vec3<f32>, inv_dir: vec3<f32>, bump:vec3<f32>) -> RayHit {
  let dir_neg = bump < vec3(0.0);
  var result = RayHit();
  var pos = initial_pos;

  result.voxel = vox_read(obj.head, pos.cell);
  while result.voxel[0] == 0 {
    result.steps += 1;
    // Sparse marching
    let neg_wall = pos.cell & vec3(~0i << result.voxel[1]);
    let pos_wall = neg_wall + (1i << result.voxel[1]);
    let next_wall = select(pos_wall, neg_wall, dir_neg);
    // Next position
    let t_wall = (vec3<f32>(next_wall - pos.cell) - pos.offset) * inv_dir;
    let t_next = min(min(t_wall.x, t_wall.y), t_wall.z);
    update_pos(&pos, t_next * ray_dir, bump);
    result.axis = t_wall == vec3(t_next);
    // Sample
    // We bitcast pos.cell to u32s to avoid the < 0 branching via underflow
    if any(bitcast<vec3<u32>>(pos.cell) >= vec3(obj.extent)) { break; }
    result.voxel = vox_read(obj.head, pos.cell);
  }
  result.pos = pos;
  return result;
}

fn vox_read(head: u32, cell: vec3<i32>) -> vec2<u32> {
  var cur_idx = head;
  var height = TREE_HEIGHT;
  var depth = 0;
  while height != 0 {
    height -= 1;
    let child = cell >> vec3<u32>(height) & vec3<i32>(1);
    let next_idx = voxels[cur_idx].children[child.z << 2 | child.y << 1 | child.x];
    if next_idx == cur_idx { return vec2(cur_idx, height + 1); }
    cur_idx = next_idx;
  }
  return vec2<u32>(cur_idx, height);
}

fn aabb_intersect(ray_origin: vec3<f32>, inv_dir: vec3<f32>) -> f32 {
  let t1 = (vec3(0.0) - ray_origin) * inv_dir;
  let t2 = (vec3<f32>(obj.extent) - ray_origin) * inv_dir;
  let min_t = min(t1, t2);
  let max_t = max(t1, t2);
  let t_entry = max(max(min_t.x, min_t.y), min_t.z);
  let t_exit = min(min(max_t.x, max_t.y), max_t.z);

  if t_entry > t_exit || t_exit < 0.0 { return SENTINEL; }
  return t_entry;
}

