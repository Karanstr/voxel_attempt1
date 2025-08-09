use crate::camera::Camera;
use glam::Mat3;
use crate::app::ObjectData;


#[repr(C, align(16))]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ObjData {
  pos: [f32; 3],
  padding2: f32,

  inv_right: [f32; 3],
  padding3: f32,
  inv_up: [f32; 3],
  padding4: f32,
  inv_forward: [f32; 3],
  padding5: f32,
  
  head: u32,
  height: u32,
  extent: u32,
  padding: f32,
}
impl ObjData {
  pub fn new(data: &ObjectData) -> Self {
    let inv_mat = Mat3::from_quat(data.rot.inverse());
    Self {
      pos: data.pos.into(),
      padding2: 0.0,

      inv_right: inv_mat.col(0).into(),
      padding3: 0.0,
      inv_up: inv_mat.col(1).into(),
      padding4: 0.0,
      inv_forward: inv_mat.col(2).into(),
      padding5: 0.0,

      head: data.head,
      height: data.height,
      extent: data.bounds,
      padding: 0.0,
    }
  }
}

// Remember that vec3's are extended to 16 bytes
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CamData {
  pos: [f32; 3],
  padding2: f32,

  right: [f32; 3],
  padding3: f32,
  up: [f32; 3],
  padding4: f32,
  forward: [f32; 3],
  padding5: f32,

  aspect_ratio: f32,
  tan_fov: f32,
  padding1: [f32; 2],
}
impl CamData {
  pub fn new(camera: &Camera) -> Self {
    Self {
      pos: camera.position.into(),
      padding2: 0.0,

      right: camera.basis()[0].into(),
      padding3: 0.,
      up: camera.basis()[1].into(),
      padding4: 0.,
      forward: camera.basis()[2].into(),
      padding5: 0.,

      aspect_ratio: camera.aspect_ratio,
      tan_fov: (camera.fov / 2.).tan(),
      padding1: [0.0; 2],
   }
  } 
}
