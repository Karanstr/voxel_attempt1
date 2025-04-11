use serde::{Deserialize, Serialize};
use super::sdg::{
    Node, GraphNode,
    Childs, Path
};
use glam::UVec3;
type Index = u32;

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
            Self::FrontTopLeft => 0,
            Self::FrontTopRight => 1,
            Self::FrontBottomLeft => 2,
            Self::FrontBottomRight => 3,
            Self::BackTopLeft => 4,
            Self::BackTopRight => 5,
            Self::BackBottomLeft => 6,
            Self::BackBottomRight => 7
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


pub struct BasicPath3d<Zorder3d>(Vec<Zorder3d>);
impl<Zorder3d : Childs> Path<Zorder3d> for BasicPath3d<Zorder3d> {
    
    fn new() -> Self { Self(Vec::new()) }

    fn step_down(&self, child:Zorder3d) -> Self {
        let mut new = self.0.clone();
        new.push(child);
        Self(new)
    }

    fn to_cell(&self) -> UVec3 {
        let mut x = 0;
        let mut y = 0;
        for layer in 0 .. self.depth() {
            let coord = self.steps()[layer as usize].to_coord();
            x |= (coord.x as u32) << layer;
            y |= (coord.y as u32) << layer;
        }
        UVec3::new(x, y, 0)
    }

    fn from_cell(cell: UVec3, depth: u32) -> Self {
        let mut path = Vec::with_capacity(depth as usize);
        let mut x = cell.x;
        let mut y = cell.y;
        let mut z = cell.z;
        for _ in 0 .. depth {
            path.push(Zorder3d::from(UVec3::new(x & 0b1, y & 0b1, z & 0b1)));
            x >>= 1;
            y >>= 1;
            z >>= 1;
        }
        path.reverse();
        Self(path)
    }

    fn steps(&self) -> Vec<Zorder3d> { self.0.clone() }
    fn depth(&self) -> u32 { self.0.len() as u32 }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, 
    Serialize, Deserialize, 
    bytemuck::Pod, bytemuck::Zeroable
)]
pub struct BasicNode3d {
    children: [Index; 8]
}

impl Node for BasicNode3d {
    type Children = Zorder3d;

    fn new(children:&[Index]) -> Self {
        if children.len() != 8 { panic!("Invalid number of children"); }
        Self { 
            children: children.clone().try_into().unwrap()
        } 
    }
    fn get(&self, child:Self::Children) -> Index {
        self.children[child.to_index()]
    }
    fn set(&mut self, child:Self::Children, index:Index) {
        self.children[child.to_index()] = index
    }
}
impl GraphNode for BasicNode3d {}
