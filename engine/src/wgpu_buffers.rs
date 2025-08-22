use crate::camera::Camera;
use glam::Mat4;
use crate::app::ObjectData;


#[repr(C, align(16))]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ObjData {
  pos: [f32; 3],
  pad1: u32,
  min_cell: [u32; 3],
  pad2: u32,
  extent: [u32; 3],
  pad3: u32,

  inv_transform: [ [f32; 4]; 4],
  
  head: u32,
  height: u32,
  pad4: [u32; 2],
}
impl ObjData {
  pub fn new(data: &ObjectData) -> Self {
    // I don't really understand the matrix math yet, but it works.
    let inv_transform = 
      Mat4::from_translation(data.center_of_rot) *
      Mat4::from_quat(data.rot.inverse()) * 
      Mat4::from_translation(-data.pos - data.center_of_rot);
    Self {
      pos: data.pos.into(),
      pad1: 0,
      min_cell: data.min_cell.into(),
      pad2: 0,
      extent: data.extent.into(),
      pad3: 0,

      inv_transform: [
        inv_transform.col(0).into(),
        inv_transform.col(1).into(),
        inv_transform.col(2).into(),
        inv_transform.col(3).into(),
      ],

      head: data.head,
      height: data.height,
      pad4: [0; 2],
    }
  }
}

#[repr(C, align(16))]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CamData {
  pos: [f32; 3],
  pad1: f32,

  right: [f32; 3],
  pad2: f32,
  up: [f32; 3],
  pad3: f32,
  forward: [f32; 3],
  pad4: f32,

  aspect_ratio: f32,
  tan_fov: f32,
  pad5: [f32; 2],
}
impl CamData {
  pub fn new(camera: &Camera) -> Self {
    Self {
      pos: camera.position.into(),
      pad1: 0.0,

      right: camera.basis()[0].into(),
      pad2: 0.,
      up: camera.basis()[1].into(),
      pad3: 0.,
      forward: camera.basis()[2].into(),
      pad4: 0.,

      aspect_ratio: camera.aspect_ratio,
      tan_fov: (camera.fov / 2.).tan(),
      pad5: [0.0; 2],
   }
  } 
}
