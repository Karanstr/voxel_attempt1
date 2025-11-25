@group(0) @binding(0)
var input_tex: texture_2d<f32>;

@group(0) @binding(1)
var output_tex: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3<u32>) {
  let size = vec2<u32>(textureDimensions(output_tex));
  if (id.x >= size.x || id.y >= size.y) { return; }
  let color = textureLoad(input_tex, id.xy, 0) * vec4(1.0, 1.0, 0.0, 0.0);

  // Write to storage texture
  textureStore(output_tex, id.xy, color);
}
