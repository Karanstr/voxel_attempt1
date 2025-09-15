use crate::camera::Camera;
use glam::{Vec3, UVec3, Quat};
use sdg::prelude::*;
use lilypads::Pond;
use fastnoise_lite::FastNoiseLite;
use fastnoise_lite::NoiseType;

// struct ObjectManager {
//   objects: Pond<VoxelObject>
// }
// impl ObjectManager {
// }

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DagRef {
  head: u32,
  height: u32,
}
impl DagRef { fn new(head: u32, height: u32) -> Self { Self { head, height} } }

// Impement parry3d::shape

pub struct VoxelObject {
  pub dag_ref: DagRef,
  // An aabb in local grid_space
  pub min_cell: UVec3,
  pub max_cell: UVec3,

  // The worldspace position of the min corner of the grid (NOT THE OBJECT)
  pub pos: Vec3,
  // The offset of the object's pivot point in local space,
  // (0,0,0) representing the bottom left back corner (pos)
  pub pivot_offset: Vec3,
  pub rot: Quat,
}
impl VoxelObject {
  pub fn is_point_solid(pos: Vec3) -> bool { todo!() } 

  pub fn floor(sdg: &mut SparseDirectedGraph<BasicNode3d>, pos: Vec3) -> Self {
    let mut head = sdg.get_root(0);
    let height = 4;
    let size = 2u32.pow(height);
    for x in 0 .. size {
      for z in 0 .. size {
        let path = Zorder3d::path_from(UVec3::new(x, 0, z), height);
        head = sdg.set_node(head, &path, 1);
      }
    }
    for y in 1 ..= 2 {
      for x in 3 ..= 4 {
        for z in 3 ..= 4 {
          let path = Zorder3d::path_from(UVec3::new(x, y, z), height);
          head = sdg.set_node(head, &path, 1);
        }
      }
    } 
 
    Self {
      dag_ref: DagRef::new(head, height),
      min_cell: UVec3::ZERO,
      max_cell: UVec3::splat(size - 1).with_y(3),
      pos,
      pivot_offset: Vec3::splat(size as f32) / 2.0,
      rot: Quat::IDENTITY,
    }
  }
}



// Remove these things?
// We may want to extract this all into the app facilitator instead
pub struct GameData {
  pub camera: Camera,
  pub sdg: SparseDirectedGraph<BasicNode3d>,
  pub objects: Vec<VoxelObject>,
}
impl Default for GameData {
  fn default() -> Self {
    let mut sdg = SparseDirectedGraph::new();
    let _empty = sdg.add_leaf();
    let _full = sdg.add_leaf();
    let floor = VoxelObject::floor(&mut sdg, Vec3::ZERO);
    Self {
      camera: Camera::default(),
      sdg,
      objects: Vec::from([floor]),
    }
  }
}
