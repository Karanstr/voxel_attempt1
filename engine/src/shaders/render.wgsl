const SCALE = 1.0 / 1.0;
const VERTICIES = array<vec2<f32>, 3>(
  vec2<f32>(-1.0, -3.0),
  vec2<f32>(3.0, 1.0),
  vec2<f32>(-1.0, 1.0)
);

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {
  return vec4<f32>(VERTICIES[idx], 0.0, 1.0);
}

@group(0) @binding(0)
var my_texture: texture_2d<f32>;
@group(0) @binding(1)
var my_sampler: sampler;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
  let uv = SCALE * frag_coord.xy / vec2<f32>(textureDimensions(my_texture));
  let flipped_uv = vec2<f32>(uv.x, 1.0 - uv.y);
  return textureSample(my_texture, my_sampler, flipped_uv);
}
