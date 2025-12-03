@group(0) @binding(0)
var input_tex: texture_2d<f32>;      // RGBA: R,G=oct normal, B=Z, Block ID

@group(0) @binding(1)
var output_tex: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id : vec3<u32>) {
  let size = textureDimensions(output_tex);
  if (id.x >= size.x || id.y >= size.y) { return; }

  let uv = vec2<f32>(id.xy) / vec2<f32>(size);

  // ---- Read center pixel ----
  let center = textureLoad(input_tex, id.xy, 0);

  let voxel_hit = u32(center.a);
  if voxel_hit == 0 {
    textureStore(output_tex, id.xy, vec4<f32>(0.5, 0.5, 0.5, 1.0));
    return;
  }

  let normal_center = oct_decode(center.rg);
  let depth_center = center.b;

  // ---- Kernel settings ----
  let radius: i32 = 1;
  let invRes = 1.0 / vec2<f32>(size);  // pixel → UV

  var occ = 0.0;
  var count = 0.0;

  // ---- Adjacent-pixel SSAO ----
  for (var dx = -radius; dx <= radius; dx = dx + 1) {
    for (var dy = -radius; dy <= radius; dy = dy + 1) {
      if (dx == 0 && dy == 0) { continue; }

      let pos = vec2<i32>(i32(id.x) + dx, i32(id.y) + dy);

      // clamp to screen
      if (pos.x < 0 || pos.y < 0 || pos.x >= i32(size.x) || pos.y >= i32(size.y)) {
        continue;
      }

      let sample = textureLoad(input_tex, vec2<u32>(pos), 0);
      let depth_s = sample.b;

      // Only consider if sample pixel is closer (occluding)
      let depthDiff = depth_center - depth_s;
      if (depthDiff > 0.0) {
        // Decode neighbor normal for angular falloff
        let normal_s = oct_decode(sample.gb);

        // AO weight = how aligned normals are
        let ndot = clamp(dot(normal_center, normal_s), 0.0, 1.0);

        // scaled with depth difference
        let w = clamp(depthDiff * 4.0, 0.0, 1.0) * (1.0 - ndot * 0.7);

        occ = occ + w;
      }

      count = count + 1.0;
    }
  }

  let ao = 1.0 - occ / max(count, 1.0);
  
  let color = vec4(0.7, 0.3, .3, 1.0);

  textureStore(output_tex, id.xy, color * ao);
}

fn oct_decode(f: vec2<f32>) -> vec3<f32> {
  // back to [-1,1]
  let p = f * 2.0 - 1.0;
  var n = vec3<f32>(p.x, p.y, 1.0 - abs(p.x) - abs(p.y));

  if (n.z < 0.0) {
    let old = n;
    n.x = (1.0 - abs(old.y)) * select(-1.0, 1.0, old.x >= 0.0);
    n.y = (1.0 - abs(old.x)) * select(-1.0, 1.0, old.y >= 0.0);
  }

  return normalize(n);
}

