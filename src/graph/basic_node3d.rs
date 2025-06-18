use super::sdg::{
  Node, GraphNode,
  Childs, Path, Index
};
use glam::UVec3;
use vec_mem_heap::Nullable;

// Front-Back Z
// Top-Bottom Y
// Left-Right X
#[derive(Debug, Clone, Copy)]
pub enum Zorder3d {
  FrontTopLeft,
  FrontTopRight,
  FrontBottomLeft,
  FrontBottomRight,
  BackTopLeft,
  BackTopRight,
  BackBottomLeft,
  BackBottomRight
}
impl Zorder3d {
  fn to_index(&self) -> usize {
    match self {
      Self::FrontTopLeft => 0, // 000
      Self::FrontTopRight => 1, // 001
      Self::FrontBottomLeft => 2, // 010
      Self::FrontBottomRight => 3, // 011
      Self::BackTopLeft => 4, // 100
      Self::BackTopRight => 5, // 101
      Self::BackBottomLeft => 6, // 110
      Self::BackBottomRight => 7, // 111
    }
  }
}
impl Childs for Zorder3d {
  const COUNT: usize = 8;
  fn all() -> impl Iterator<Item = Self> {
    [
      Self::FrontTopLeft,
      Self::FrontTopRight,
      Self::FrontBottomLeft,
      Self::FrontBottomRight,
      Self::BackTopLeft,
      Self::BackTopRight,
      Self::BackBottomLeft,
      Self::BackBottomRight
    ].into_iter()
  }
  fn from(coord: UVec3) -> Self {
    match coord {
      UVec3 {x: 0, y: 0, z: 0} => Self::FrontTopLeft,
      UVec3 {x: 1, y: 0, z: 0} => Self::FrontTopRight,
      UVec3 {x: 0, y: 1, z: 0} => Self::FrontBottomLeft,
      UVec3 {x: 1, y: 1, z: 0} => Self::FrontBottomRight,
      UVec3 {x: 0, y: 0, z: 1} => Self::BackTopLeft,
      UVec3 {x: 1, y: 0, z: 1} => Self::BackTopRight,
      UVec3 {x: 0, y: 1, z: 1} => Self::BackBottomLeft,
      UVec3 {x: 1, y: 1, z: 1} => Self::BackBottomRight,
      _ => unimplemented!()
    }
  }
  fn to_coord(&self) -> UVec3 {
    match self {
      Self::FrontTopLeft => UVec3::new(0, 0, 0),
      Self::FrontTopRight => UVec3::new(1, 0, 0),
      Self::FrontBottomLeft => UVec3::new(0, 1, 0),
      Self::FrontBottomRight => UVec3::new(1, 1, 0),
      Self::BackTopLeft => UVec3::new(0, 0, 1),
      Self::BackTopRight => UVec3::new(1, 0, 1),
      Self::BackBottomLeft => UVec3::new(0, 1, 1),
      Self::BackBottomRight => UVec3::new(1, 1, 1)
    }
  }
}

// This needs revamping
pub type BasicPath3d = Vec<Zorder3d>;
impl<Zorder3d : Childs> Path<Zorder3d> for Vec<Zorder3d> {
  fn new() -> Self { Vec::new() }

  fn to_cell(&self) -> UVec3 {
    let mut x = 0;
    let mut y = 0;
    let mut z = 0;
    for layer in 0 .. self.depth() {
      let coord = self.step(layer as usize).to_coord();
      x |= (coord.x as u32) << layer;
      y |= (coord.y as u32) << layer;
      z |= (coord.z as u32) << layer;
    }
    UVec3::new(x, y, z)
  }

  fn from_cell(cell: UVec3, depth: u32) -> Self {
    if cell.max_element() > (1 << depth) - 1 { panic!("Cell is too large for depth {depth}") }
    let mut path = Vec::with_capacity(depth as usize);
    let mut x = cell.x;
    let mut y = cell.y;
    let mut z = cell.z;
    for _ in 0 .. depth {
      path.push(Zorder3d::from(UVec3::new(x & 0b1, y & 0b1, z & 0b1)));
      x >>= 1; y >>= 1; z >>= 1;
    }
    path.reverse();
    path
  }

  fn step(&self, idx:usize ) -> Zorder3d { self[idx] }
  fn depth(&self) -> u32 { self.len() as u32 }
}

#[repr(C)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, std::hash::Hash, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BasicNode3d([Index; 8]);
impl Nullable for BasicNode3d {
  const NULLVALUE:Self = BasicNode3d([Index::MAX; 8]);
}
impl Node for BasicNode3d {
  type Children = Zorder3d;
  type Naive = [Index; 8];
  fn new(children:&[u32]) -> Self { BasicNode3d( children.try_into().unwrap() ) }
  fn naive(&self) -> Self::Naive { self.0 }
  fn get(&self, child:Self::Children) -> Index { self.0[child.to_index()] }
  fn set(&mut self, child:Self::Children, index:Index) { self.0[child.to_index()] = index }
  fn with_child(&self, child: Self::Children, index:Index) -> Self {
    let mut new = *self;
    new.set(child, index);
    new
  }
}
impl GraphNode for BasicNode3d {}
