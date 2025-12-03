const WG_SIZE = 8;
const SENTINEL = -314159.0;
const OBJECTS = 1;

// [OctNorm1, OctNorm2, Z, bitcasted BlockType] 
@group(0) @binding(0)
var output_tex: texture_storage_2d<rgba16float, write>;

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

// I only need linear transform, just store that 3x3
struct VoxelObject {
  pos: vec3<f32>,
  min_cell: vec3<u32>,
  extent: vec3<u32>,
  transform: mat4x4<f32>,
  inv_transform: mat4x4<f32>,
  head: u32,
  height: u32,
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

  let cam_dir = vec3(uv * vec2(cam.tan_fov), 1.0);
  let world_dir = cam.rot * cam_dir;
  let ray = march_objects(world_dir);

  let oct_normal = oct_encode(ray.global_normal);
  let result = vec4(oct_normal.x, oct_normal.y, (ray.t * cam_dir).z, f32(ray.voxel[0]));

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
  local_normal: vec3<bool>,
  global_normal: vec3<f32>,
  voxel: vec2<u32>,
  t: f32,
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
  var ray_obj_idx = 0u;

  for (var idx = 0u; idx < OBJECTS; idx += 1) {
    var ray = new_ray(world_dir, idx);
    if !ray.alive { continue; }
    ray_obj_idx = idx;
    ray.voxel = vox_read(objects[idx].head, objects[idx].height, ray.pos.cell);
    while ray.voxel[0] == 0 {
      dda_step(&ray);
      // If we've stepped outside of the object bounds
      // We bitcast pos.cell to u32s to avoid < 0 branching via underflow
      if !all(bitcast<vec3<u32>>(ray.pos.cell) - objects[idx].min_cell < objects[idx].extent) { break; }
      ray.voxel = vox_read(objects[idx].head, objects[idx].height, ray.pos.cell); // Sample current position
    }
    if ray.t < best_ray.t && ray.voxel[0] != 0 { best_ray = ray; } 
  }

  let linear = mat3x3<f32>(objects[ray_obj_idx].transform[0].xyz,
                           objects[ray_obj_idx].transform[1].xyz,
                           objects[ray_obj_idx].transform[2].xyz);
  let local_float_normal = vec3<f32>(best_ray.local_normal) * sign(best_ray.inv_dir) * vec3(1.0, -1.0, 1.0);
  best_ray.global_normal = normalize(linear * local_float_normal);
  return best_ray;
}

fn new_ray(world_dir: vec3<f32>, obj: u32) -> Ray {
  var ray = Ray();
  let pos_f32 = (objects[obj].inv_transform * vec4(cam.pos, 1.0)).xyz;
  ray.pos = Position( vec3<i32>(floor(pos_f32)), fract(pos_f32));
  ray.dir = (objects[obj].inv_transform * vec4(world_dir, 0.0)).xyz;
  ray.inv_dir = 1.0 / ray.dir;
  let intersection = aabb_intersect(pos_f32, ray.inv_dir, objects[obj].min_cell, objects[obj].extent);
  ray.alive = intersection.t != SENTINEL;
  ray.local_normal = intersection.normal;
  move_ray(&ray, max(0.0, intersection.t));
  return ray;
}

fn dda_step(ray: ptr<function, Ray>) {
  // Sparse marching
  let neg_wall = (*ray).pos.cell & vec3(~0i << (*ray).voxel[1] );
  let pos_wall = neg_wall + (1i << (*ray).voxel[1] );
  let next_wall = select(pos_wall, neg_wall, (*ray).dir < vec3(0.0));
  // Next position
  let t_wall = ( vec3<f32>( next_wall - (*ray).pos.cell ) - (*ray).pos.offset ) * (*ray).inv_dir;
  let t_step = min(min(t_wall.x, t_wall.y), t_wall.z);
  move_ray(ray, t_step);
  (*ray).local_normal = t_wall == vec3(t_step);
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


struct Intersection {
  t: f32,
  normal: vec3<bool>
}

fn aabb_intersect(ray_origin: vec3<f32>, inv_dir: vec3<f32>, min_cell: vec3<u32>, extent: vec3<u32>) -> Intersection {
  var intersection = Intersection(SENTINEL, vec3(false));
  let t1 = (vec3<f32>(min_cell) - ray_origin) * inv_dir;
  let t2 = t1 + vec3<f32>(extent) * inv_dir;
  let min_t = min(t1, t2);
  let max_t = max(t1, t2);
  let t_entry = max(max(min_t.x, min_t.y), min_t.z);
  let t_exit = min(min(max_t.x, max_t.y), max_t.z);
  // Entry must be before exit and exit must be forward
  if t_exit < t_entry | t_exit < 0.0 { return intersection; }
  intersection.t = t_entry;
  intersection.normal = vec3(t_entry) == min_t;
  return intersection;
}


fn oct_wrap(n: vec2<f32>) -> vec2<f32> {
  // Fold the lower hemisphere
  return (1.0 - abs(n.yx)) * vec2<f32>(select(vec2(-1.0), vec2(1.0), n >= vec2(0.0)));
}

fn oct_encode(n: vec3<f32>) -> vec2<f32> {
  // Project to octahedron
  let p = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
  // Fold negative z
  let p2 = select(p, oct_wrap(p), n.z >= 0.0);
  // Return into [0,1]
  return p2 * 0.5 + 0.5;
}
