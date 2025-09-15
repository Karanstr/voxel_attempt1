use crate::objects::VoxelObject;
use nalgebra::Vector3;
use rapier3d::geometry::{Shape, PointQuery, RayCast};
use rapier3d::parry::shape::FeatureId;
use rapier3d::parry::bounding_volume::Aabb;
use rapier3d::parry::query::{Ray, RayIntersection};
use glam::{BVec3, IVec3, Vec3};

// This needs access to the actual dag to sample
// Returns [value, height]
struct Sample {
  value: u32,
  height: u32,
}
impl Sample { fn is_solid(&self) -> bool { self.value != 0} }
fn sample_cell(cell: IVec3) -> Sample { todo!() }

struct Position {
  cell: IVec3,
  offset: Vec3,
}
impl Position {
  fn new(pos: Vec3) -> Self {
    Self { cell: pos.floor().as_ivec3(), offset: pos.fract() }
  }
}
struct LargeRay {
  pos: Position,
  dir: Vec3,
  inv_dir: Vec3,
  normal: BVec3,
  t: f32
}
impl LargeRay {
  fn step(&mut self, dt: f32) {
    let delta = self.dir * dt;
    self.pos.cell += delta.floor().as_ivec3();
    // Small Epsilon bump
    self.pos.offset += delta.fract() + delta.signum() * 0.0001;
    self.pos.cell += self.pos.offset.floor().as_ivec3();
    self.pos.offset = self.pos.offset.fract();
    self.t += dt;
  }
}

impl RayCast for VoxelObject {
  // I imagine I don't care, but if these conversions to glam really cause problems I can learn
  /// If solid is false, the shape is hollow
  fn cast_local_ray_and_get_normal(&self, ext_ray: &Ray, max_toi: f32, solid: bool,) -> Option<RayIntersection> {
    // We first need to project the shape onto the local aabb
    let aabb = Aabb::new(self.min_cell.as_vec3().into(), self.max_cell.as_vec3().into());
    // We need to set our origin to be the negative corner of the shape
    // but rapier thinks our origin is at the pivot
    let translation = Vector3::new(self.pivot_offset.x, self.pivot_offset.y, self.pivot_offset.z);
    let shifted_ray = Ray::new(ext_ray.origin - translation, ext_ray.dir);
    let dir: Vec3 = shifted_ray.dir.into();
    let inv_dir = 1.0 / dir;
    let mut ray = LargeRay {
      pos: Position::new(shifted_ray.origin.into()),
      dir,
      inv_dir,
      normal: BVec3::FALSE,
      t: 0.0,
    };
    ray.step(aabb.cast_local_ray(&shifted_ray, max_toi, true)?);
    
    let mut sample = sample_cell(ray.pos.cell);
    // if we're inside the shape and it's not hollow, return an immediate intersection
    if sample.is_solid() && solid { return Some(RayIntersection::new(
      0.,
      Vector3::new(0., 0., 0.,),
      FeatureId::Unknown
    ))}
    // if we're outside the shape, return once we find something solid
    // if we're inside the shape, return once we find something non-solid
    // we can do this because above we already covered the inside & solid case
    let searching_for_entrance = sample.is_solid();
    let dir_negative = ray.dir.cmplt(Vec3::ZERO);
    loop {
      let neg_wall = ray.pos.cell & IVec3::splat(!0 << sample.height);
      let pos_wall = neg_wall + (1 << sample.height);
      let next_wall = IVec3::from_array([
        if dir_negative.x { neg_wall.x } else { pos_wall.x }, 
        if dir_negative.y { neg_wall.y } else { pos_wall.y }, 
        if dir_negative.z { neg_wall.z } else { pos_wall.z }, 
      ]);
      let t_wall = ((next_wall - ray.pos.cell).as_vec3() - ray.pos.offset) * ray.inv_dir;
      let t_step = t_wall.min_element();
      ray.step(t_step);
      ray.normal = t_wall.cmpeq(Vec3::splat(t_step));
      
      // Terminate if we surpass max testing range
      if ray.t > max_toi { return None }
      // Terminate if we're outside of bounds
      if ray.pos.cell.clamp(self.min_cell.as_ivec3(), self.max_cell.as_ivec3()).cmpne(ray.pos.cell).any() {
        return None
      }
      sample = sample_cell(ray.pos.cell);
      // Terminate if we found what we're looking for
      if searching_for_entrance == sample.is_solid() { break }
    }
    let faces_hit = [ray.normal.x, ray.normal.y, ray.normal.z];
    let mut normal = ray.dir.signum(); // All faces are axis aligned
    let mut hit_count = 0;
    for axis in 0 .. 3 {
      let did_hit = faces_hit[axis] as i32;
      hit_count += did_hit;
      normal[axis] *= did_hit as f32;
    }
    let feature = FeatureId::Face(1);
    Some(RayIntersection::new(ray.t, normal.into(), feature))
  }

}
// https://docs.rs/parry3d/0.23.0/parry3d/shape/trait.Shape.html
//
// This means we must implement PointQuery
// This is kinda tricky (we need to identify the closest point by using some kind of spiraling neighbor search)
// https://docs.rs/parry3d/0.23.0/parry3d/query/point/trait.PointQuery.html
//

