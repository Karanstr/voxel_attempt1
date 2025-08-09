const WG_SIZE = 8;
const SENTINEL = -314159.0;
const OBJECTS = 4;

@group(0) @binding(0)
var output_tex: texture_storage_2d<rgba8unorm, write>;

struct VoxelObject {
  pos: vec3<f32>,
  inv_rot: mat3x3<f32>,
  head: u32,
  height: u32,
  extent: u32, // Eventually replace with vec3<u32>
}

struct Position {
  cell: vec3<i32>,
  offset: vec3<f32>,
}
fn update_pos(pos: ptr<function, Position>, delta: vec3<f32>, bump: vec3<f32>) {
  (*pos).cell += vec3<i32>(floor(delta));
  (*pos).offset += fract(delta) + bump;
  (*pos).cell += vec3<i32>(floor((*pos).offset));
  (*pos).offset = fract((*pos).offset);
}
fn to_pos(pos: vec3<f32>) -> Position { return Position( vec3<i32>(floor(pos)), fract(pos)); }

struct Camera {
  pos: vec3<f32>,
  rot: mat3x3<f32>,

  aspect_ratio: f32,
  tan_fov: f32,
}
@group(0) @binding(1)
var<uniform> cam: Camera;

struct VoxelNode { children: array<u32, 8> }
@group(0) @binding(2)
var<storage, read> voxels: array<VoxelNode>;

@group(0) @binding(3)
var<storage, read> objects: array<VoxelObject, OBJECTS>;

@compute @workgroup_size(WG_SIZE, WG_SIZE)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let resolution = vec2<u32>(textureDimensions(output_tex));
  // We do a little padding so we can fit into the workgroups correctly
  if gid.x >= resolution.x || gid.y >= resolution.y { return; }
  // Transform from <0,1> to <-1, 1>, then scale by aspect_ratio for proper dimensioning
  let uv = ((vec2<f32>(gid.xy) + 0.5) / vec2<f32>(resolution.xy) - 0.5) * 2 * vec2(cam.aspect_ratio, 1.0);
  textureStore(output_tex, vec2<i32>(gid.xy), march_init(uv));
}

// Ray hit information
struct RayHit {
  axis: vec3<bool>,
  voxel: vec2<u32>,
  steps: u32,
  t: f32,
}

// Naively attempt to trace each object
fn march_init(uv: vec2<f32>) -> vec4<f32> {
  let world_dir = cam.rot * vec3(uv * vec2(cam.tan_fov), 1.0);
  var hit: RayHit = RayHit();
  let stupid = 1.0; hit.t = stupid / 0.0;
  for (var idx = 0u; idx < OBJECTS; idx++) {
    let cur_hit = march_object(world_dir, idx);
    if cur_hit.voxel[0] != 0 && cur_hit.t < hit.t { hit = cur_hit; }
  }
  if hit.voxel[0] == 0 { return vec4(0.0); }
  let step_color = 1.0 / vec3<f32>(hit.steps);
  let normal_color = 1.0 + vec3<f32>(hit.axis) * vec3(-0.2, 0.3, 0.4);
  return vec4(step_color * normal_color, 1);
}

fn march_object(world_dir: vec3<f32>, idx: u32) -> RayHit {
  let ray_dir = objects[idx].inv_rot * world_dir;
  let inv_dir = 1.0 / ray_dir;
  let bump = sign(ray_dir) * 0.0001;
  let center = f32(objects[idx].extent >> 1);
  let ray_f32 = objects[idx].inv_rot * (cam.pos - objects[idx].pos - center) + center;
  var ray_pos = to_pos(ray_f32);
  var t_start = 0.0;
  if any(bitcast<vec3<u32>>(ray_pos.cell) >= vec3(objects[idx].extent)) {
    t_start = aabb_intersect(ray_f32, inv_dir, objects[idx].extent);
    if t_start == SENTINEL { return RayHit(); }
    t_start = max(0.0, t_start);
  }
  update_pos(&ray_pos, ray_dir * t_start, bump);
  var hit = dda_vox_v4(ray_pos, ray_dir, inv_dir, bump, idx);
  hit.t += t_start;
  return hit;
}

fn dda_vox_v4(initial_pos: Position, ray_dir: vec3<f32>, inv_dir: vec3<f32>, bump:vec3<f32>, idx: u32) -> RayHit {
  let dir_neg = bump < vec3(0.0);
  var result = RayHit();
  var pos = initial_pos;
  result.voxel = vox_read(objects[idx].head, objects[idx].height, pos.cell);
  while result.voxel[0] == 0 {
    result.steps += 1;
    // Sparse marching
    let neg_wall = pos.cell & vec3(~0i << result.voxel[1]);
    let pos_wall = neg_wall + (1i << result.voxel[1]);
    let next_wall = select(pos_wall, neg_wall, dir_neg);
    // Next position
    let t_wall = (vec3<f32>(next_wall - pos.cell) - pos.offset) * inv_dir;
    let t_next = min(min(t_wall.x, t_wall.y), t_wall.z);
    result.t += t_next;
    update_pos(&pos, t_next * ray_dir, bump);
    result.axis = t_wall == vec3(t_next);
    // Sample
    // We bitcast pos.cell to u32s to avoid the < 0 branching via underflow
    if any(bitcast<vec3<u32>>(pos.cell) >= vec3(objects[idx].extent)) { break; }
    result.voxel = vox_read(objects[idx].head, objects[idx].height, pos.cell);
  }
  return result;
}

fn vox_read(head: u32, height: u32, cell: vec3<i32>) -> vec2<u32> {
  var cur_idx = head;
  var cur_height = height;
  while cur_height != 0 {
    cur_height -= 1;
    let child = cell >> vec3<u32>(cur_height) & vec3<i32>(1);
    let next_idx = voxels[cur_idx].children[child.z << 2 | child.y << 1 | child.x];
    if next_idx == cur_idx { return vec2(cur_idx, cur_height + 1); }
    cur_idx = next_idx;
  }
  return vec2<u32>(cur_idx, cur_height);
}

fn aabb_intersect(ray_origin: vec3<f32>, inv_dir: vec3<f32>, extent: u32) -> f32 {
  let t1 = (vec3(0.0) - ray_origin) * inv_dir;
  let t2 = (vec3<f32>(extent) - ray_origin) * inv_dir;
  let min_t = min(t1, t2);
  let max_t = max(t1, t2);
  let t_entry = max(max(min_t.x, min_t.y), min_t.z);
  let t_exit = min(min(max_t.x, max_t.y), max_t.z);

  if t_entry > t_exit || t_exit < 0.0 { return SENTINEL; }
  return t_entry;
}

