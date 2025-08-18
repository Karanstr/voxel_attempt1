const WG_SIZE = 8;
const SENTINEL = -314159.0;
const OBJECTS = 4;

@group(0) @binding(0)
var output_tex: texture_storage_2d<rgba8unorm, write>;

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

struct VoxelObject {
  pos: vec3<f32>,
  inv_rot: mat3x3<f32>,
  head: u32,
  height: u32,
  extent: u32, // Eventually replace with vec3<u32> for tight aabb
}
@group(0) @binding(3)
var<storage, read> objects: array<VoxelObject, OBJECTS>;

@compute @workgroup_size(WG_SIZE, WG_SIZE)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let resolution = vec2<u32>(textureDimensions(output_tex));
  // We do a little padding so we can fit into the workgroups correctly
  if gid.x >= resolution.x || gid.y >= resolution.y { return; }
  // Transform from <0,1> to <-1, 1>, then scale by aspect_ratio for proper dimensioning
  let uv = ((vec2<f32>(gid.xy) + 0.5) / vec2<f32>(resolution.xy) - 0.5) * 2 * vec2(cam.aspect_ratio, 1.0);

  let world_dir = cam.rot * vec3(uv * vec2(cam.tan_fov), 1.0);
  let ray = march_objects(world_dir);
  var result = vec4(0.0);
  if ray.voxel[0] == 0 { result = vec4(0.0); } else {
    let step_color = 1.0 / vec3<f32>(ray.steps);
    let normal_color = 0.5 + vec3<f32>(ray.normal) * vec3(-0.2, 0.3, 0.4);
    result = vec4(step_color * normal_color, 1);
  }

  textureStore(output_tex, vec2<i32>(gid.xy), result);
}

struct Position {
  cell: vec3<i32>,
  offset: vec3<f32>,
}
struct Ray {
  pos: Position,
  dir: vec3<f32>,
  inv_dir: vec3<f32>,
  normal: vec3<bool>,
  voxel: vec2<u32>,
  t: f32,
  steps: u32,
  alive: bool,
}
fn move_ray(ray: ptr<function, Ray>, timestep: f32) {
  let delta = (*ray).dir * timestep;
  (*ray).pos.cell += vec3<i32>(floor(delta));
  (*ray).pos.offset += fract(delta) + sign((*ray).dir) * 0.0001;
  (*ray).pos.cell += vec3<i32>(floor( (*ray).pos.offset ));
  (*ray).pos.offset = fract( (*ray).pos.offset );

  (*ray).t += timestep;
}

fn march_objects(world_dir: vec3<f32>) -> Ray {
  var rays: array<Ray, OBJECTS>;
  let ONE = 1.0; let INF = ONE / 0.0;
  var best_ray = Ray(); best_ray.t = INF;

  for (var idx = 0u; idx < OBJECTS; idx += 1) {
    var ray = new_ray(world_dir, idx);
    if !ray.alive { continue; }
    ray.voxel = vox_read(objects[idx].head, objects[idx].height, ray.pos.cell);
    while ray.voxel[0] == 0 {
      dda_step(&ray);
      // If we've stepped outside of the object bounds
      // We bitcast pos.cell to u32s to avoid the < 0 branching via underflow
      if !all(bitcast<vec3<u32>>(ray.pos.cell) < vec3(objects[idx].extent)) { break; }
      // Sample current position
      ray.voxel = vox_read( objects[idx].head, objects[idx].height, ray.pos.cell);
    }
    if ray.t < best_ray.t { best_ray = ray; } 
  }
  return best_ray;
}

fn new_ray(world_dir: vec3<f32>, obj: u32) -> Ray {
  var ray: Ray = Ray();
  ray.dir = objects[obj].inv_rot * world_dir;
  ray.inv_dir = 1.0 / ray.dir;
  // We want rotation to be applied around the center of the object
  let center = f32(objects[obj].extent >> 1);
  let ray_f32 = objects[obj].inv_rot * (cam.pos - objects[obj].pos - center) + center;
  ray.pos = Position( vec3<i32>(floor(ray_f32)), fract(ray_f32));
  let t_start = aabb_intersect(ray_f32, ray.inv_dir, objects[obj].extent);
  ray.alive = t_start != SENTINEL;
  move_ray(&ray, max(0.0, t_start));
  return ray;
}

fn dda_step(ray: ptr<function, Ray>) {
  (*ray).steps += 1;
  // Sparse marching
  let neg_wall = (*ray).pos.cell & vec3(~0i << (*ray).voxel[1] );
  let pos_wall = neg_wall + (1i << (*ray).voxel[1] );
  let next_wall = select(pos_wall, neg_wall, sign((*ray).dir) * 0.0001 < vec3(0.0));
  // Next position
  let t_wall = ( vec3<f32>( next_wall - (*ray).pos.cell ) - (*ray).pos.offset ) * (*ray).inv_dir;
  let t_step = min(min(t_wall.x, t_wall.y), t_wall.z);
  move_ray(ray, t_step);
  (*ray).normal = t_wall == vec3(t_step);
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

  // Entry must be before exit and exit must be in the future
  if t_exit < t_entry | t_exit < 0.0 { return SENTINEL; }
  return t_entry;
}

