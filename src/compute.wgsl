const WG_SIZE = 8;
const FP_BUMP: f32 = 0.0001; 

@group(0) @binding(0)
var output_tex: texture_storage_2d<rgba8unorm, write>;

struct Data {
  // [Idx, Height]
  render_root: vec2<u32>,
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
  let cells = (1u << data.render_root[1]) - 1;
  // If height is 3: 0,0,0 -> 7,7,7
  let bounds = vec3<f32>(f32(cells));

  // Transform + Scale from <0,1> to <-1, 1> then scale by aspect ratio
  let uv = 2.0 * ((vec2<f32>(gid.xy) + 0.5) / vec2<f32>(resolution.xy) - 0.5) * vec2<f32>(data.aspect_ratio, 1.0);

  // We don't need to normalize this, all we care about is the ratio
  let ray_dir = data.cam_forward + data.tan_fov * (data.cam_right * uv.x + data.cam_up * uv.y);
  
  let hit = dda_vox(data.cam_pos, ray_dir, bounds);
    
  if (hit.hit) {
    let hit_pos = data.cam_pos + ray_dir * hit.t;
    let block_size = f32(1u << hit.voxel[1]);
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

fn dda_vox(camera_pos: vec3<f32>, dir: vec3<f32>, bounds: vec3<f32>) -> RayHit {
  var result = RayHit();
  result.hit = false;
  result.t = 0.0;

  // We only march through grids for now, not into them.
  if (any(clamp(camera_pos, vec3<f32>(0.0), bounds) != camera_pos)) { return result; }

  let step = vec3<i32>(sign(dir));
  let inv_dir = vec3<f32>(step) / max(abs(dir), vec3<f32>(FP_BUMP));
  let t_delta = abs(inv_dir);

  var cur_voxel = vec3<i32>(floor(camera_pos));
  // result.voxel = vox_read(data.render_root, vec3<u32>(cur_voxel));
  // let block_size = 1 << result.voxel[1];
  // let mask = vec3<i32>(block_size - 1);
  // let offset = cur_voxel & mask;
  // let blocks = vec3<f32>(select(offset, mask - offset, step > vec3<i32>(0)));

  // Distance to first boundary
  var t_max = select(
    select(ceil(camera_pos) + FP_BUMP, floor(camera_pos) - FP_BUMP, step < vec3<i32>(0)) - camera_pos,
    vec3<f32>(10000000.0),
    step == vec3<i32>(0)
  ) * inv_dir;// * (blocks);


  for (var i = 0u; i < 100u; i++) {
    // We can't sample if we're outside of the grid (for now)
    if (any(clamp(cur_voxel, vec3<i32>(0), vec3<i32>(bounds)) != cur_voxel)) { break; }

    result.voxel = vox_read(data.render_root, vec3<u32>(cur_voxel));
    if (result.voxel[0] != 0u && result.t != 0.0) {
      result.hit = true;
      return result;
    }

    let block_size = 1 << result.voxel[1];
    let mask = vec3<i32>(block_size - 1);
    let offset = cur_voxel & mask;
    let sparse_max = t_max + t_delta * vec3<f32>(select(offset, mask - offset, step > vec3<i32>(0)));
    let min_t = min(min(sparse_max.x, sparse_max.y), sparse_max.z);

    loop {
      if (result.t + FP_BUMP >= min_t) { break; }
      result.t = min(min(t_max.x, t_max.y), t_max.z);
      result.normal = select(vec3<f32>(0.0), vec3<f32>(1.0), t_max == vec3<f32>(result.t));
      cur_voxel += step * vec3<i32>(result.normal);
      t_max += t_delta * result.normal;
    }
  }

  // No hit found
  return result;
}

// Trusts that you submit a cell which fits within the root and that root.height != 0
fn vox_read(root: vec2<u32>, cell: vec3<u32>) -> vec2<u32> {
  var cur_voxel = root;
  loop {
    let shift = cur_voxel[1] - 1;
    let childx = (cell.x >> shift) & 1;
    let childy = (cell.y >> shift) & 1;
    let childz = (cell.z >> shift) & 1;
    let next_index = voxels[cur_voxel[0]].children[childz << 2 | childy << 1 | childx];
    if (next_index == cur_voxel[0]) { break; }
    cur_voxel[0] = next_index;
    cur_voxel[1] -= 1;
    if (cur_voxel[1] == 0) { break; }
  }
  return cur_voxel;
}
