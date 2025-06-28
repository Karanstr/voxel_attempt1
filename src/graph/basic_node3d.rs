use super::sdg::{ Node, GraphNode, Childs, Path, Index, };
use glam::UVec3;


// These names might be backwards, translating from top-left origin to bottom left origin
// Front-Back Z _00
// Top-Bottom Y 0_0
// Left-Right X 00_
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Zorder3d {
  FrontTopLeft,     // 000
  FrontTopRight,    // 001
  FrontBottomLeft,  // 010
  FrontBottomRight, // 011
  BackTopLeft,      // 100
  BackTopRight,     // 101
  BackBottomLeft,   // 110
  BackBottomRight   // 111
}
impl Path<Zorder3d> for Zorder3d {
  fn to_cell(path: Vec<Zorder3d>) -> UVec3 {
    let mut cell = UVec3::ZERO;
    for layer in 0 .. path.len() as u32 {
      cell = cell | (path[layer as usize].to_coord() << layer);
    }
    cell
  }

  fn path_from(mut cell:UVec3, depth:u32) -> Vec<Self> {
    if cell.max_element() >= 1 << depth { panic!("Cell is too large for depth {depth}") }
    let mut path = Vec::with_capacity(depth as usize);
    for _ in 0 .. depth {
      path.push(Self::new(cell & 0b1));
      cell = cell >> 1;
    }
    path.reverse();
    path
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
  fn new(quadrant: UVec3) -> Self {
    match quadrant {
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
    let bits = *self as u32;
    UVec3::new(bits & 1, bits & 0b10, bits & 0b100)
  }
}

pub type BasicNode3d = [Index; 8];
impl Node for BasicNode3d {
  type Children = Zorder3d;
  fn new(children:&[u32]) -> Self {  children.try_into().unwrap() }
  fn get(&self, child:Self::Children) -> Index { self[child as usize] }
  fn set(&mut self, child:Self::Children, index:Index) { self[child as usize] = index }
  fn with_child(&self, child: Self::Children, index:Index) -> Self {
    let mut new = *self;
    new.set(child, index);
    new
  }
}
impl GraphNode for BasicNode3d {}
