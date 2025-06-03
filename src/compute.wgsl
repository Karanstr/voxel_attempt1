const WG_SIZE = 8;

@group(0) @binding(0)
var output_tex: texture_storage_2d<rgba8unorm, write>;

struct Data {
  render_root: vec2<u32>,
  padding1: vec2<u32>,
  camera_pos: vec3<f32>,
  // padding1: f32
  camera_dir: vec3<f32>,
  // padding2: f32
}
@group(0) @binding(1)
var<uniform> data: Data;

struct VoxelNode { children: array<u32, 8> }
@group(0) @binding(2)
var<storage> voxels: array<VoxelNode>;


@compute @workgroup_size(WG_SIZE, WG_SIZE)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let resolution = vec2<u32>(textureDimensions(output_tex));
  if (gid.x >= resolution.x || gid.y >= resolution.y) { return; }


  let color = vec4<f32>(
    f32(gid.x) / f32(resolution.x),
    f32(gid.y) / f32(resolution.y),
    0.5,
    1.0,
  );
  textureStore(output_tex, vec2<i32>(gid.xy), color);
}
