use std::collections::{HashMap, VecDeque};
use glam::UVec3;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use vec_mem_heap::prelude::*;
pub type Index = u32;

#[allow(dead_code)]
pub trait Path<T : Childs> {
  fn new() -> Self;
  fn steps(&self) -> Vec<T>;
  fn step_down(&self, child:T) -> Self;
  fn to_cell(&self) -> UVec3;
  fn from_cell(cell:UVec3, depth:u32) -> Self;
  fn depth(&self) -> u32;
}
pub trait Childs: std::fmt::Debug + Clone + Copy {
  fn all() -> impl Iterator<Item = Self>;
  const COUNT: usize;
  fn from(coord: UVec3) -> Self;
  fn to_coord(&self) -> UVec3;
}

// Nodes are anything with valid children access
pub trait Node : Clone + std::fmt::Debug {
  type Children : Childs;

  fn new(children:&[Index]) -> Self;
  fn get(&self, child: Self::Children) -> Index;
  fn set(&mut self, child: Self::Children, index:Index);
  fn with_child(&self, child: Self::Children, index:Index) -> Self;
}
// GraphNodes are nodes which can be hashed and copied, making them valid for SDG storage
pub trait GraphNode : Node + Copy + std::hash::Hash + Eq {}

impl<T> Node for Option<T> where T: Node {
  type Children = T::Children;
  // This feels like a bad approach, but idk what the better one would be
  fn new(_:&[Index]) -> Self { panic!("Don't do that!") }
  fn get(&self, child:Self::Children) -> Index {
    self.as_ref().unwrap().get(child)
  }
  fn set(&mut self, child:Self::Children, index:Index) {
    self.as_mut().unwrap().set(child, index)
  }
  fn with_child(&self, child: Self::Children, index:Index) -> Self {
    let mut new = self.clone().unwrap();
    new.set(child, index);
    Some(new)
  }
}

pub struct SparseDirectedGraph<T: GraphNode> {
  pub nodes : NodeField<T>,
  pub index_lookup : HashMap<T, Index>,
  pub leaves: Vec<Index>,
}
impl<T: GraphNode> SparseDirectedGraph<T> {
  // Utility
  pub fn new() -> Self {
    Self {
      nodes : NodeField::new(),
      index_lookup : HashMap::new(),
      leaves : Vec::new(),
    }
  }

  /// Returns a trail with length path.len() + 1.
  /// trail.first() is the head of the trail and trail.last() is the node the path leads to.
  fn get_trail(&self, head:Index, path:&[T::Children]) -> Vec<Index>  {
    let mut trail = Vec::with_capacity(path.len() + 1);
    trail.push(head);
    for step in 0 .. path.len() {
      trail.push( self.child(trail[step], path[step]).unwrap() );
    }
    trail 
  }

  fn is_leaf(&self, index:Index) -> bool { self.leaves.binary_search(&index).is_ok() }

  pub fn add_leaf(&mut self) -> Index {
    let new_leaf = self.nodes.push(T::new(&vec![0; T::Children::COUNT])) as Index;
    self.nodes.replace(new_leaf as usize, T::new(&vec![new_leaf; T::Children::COUNT][..])).unwrap();
    for i in 0 .. self.leaves.len() {
      if new_leaf < self.leaves[i] {
        self.leaves.insert(i, new_leaf);
        return new_leaf
      }
    }
    self.leaves.push(new_leaf);
    new_leaf
  }

  // remove_leaf?

  // Private functions used for writing
  fn find_index(&self, node:&T) -> Option<Index> { self.index_lookup.get(node).copied() }

  fn add_node(&mut self, node:T) -> Index {
    let index = self.nodes.push(node.clone()) as Index;
    self.index_lookup.insert(node, index);
    index
  }

  /// Returns (Head of deleted tree, Head of new tree, Option<If any node along trail has only one reference, the node we should replace that node with>)
  fn propagate_change(&mut self, path: &[T::Children], trail: &[Index], mut new_child: Index,) -> (Index, Index, Option<T>) {
    for cur_depth in (0 .. path.len()).rev() {
      let cur_idx = trail[cur_depth];
      let new_node = self.node(cur_idx).unwrap().with_child(path[cur_depth], new_child);
      new_child = if let Some(idx) = self.find_index(&new_node) { 
        idx 
      } else if self.nodes.status(cur_idx as usize).unwrap() == 2 && !self.is_leaf(cur_idx) {
        return (cur_idx, cur_idx, Some(new_node))
      } else {
        self.add_node(new_node)
      };
    };
    (trail[0], new_child, None)
  }

  // Public functions used for writing
  pub fn set_node(&mut self, head:Index, path:&[T::Children], new_idx:Index) -> Index {
    let trail = self.get_trail(head, path);
    if *trail.last().unwrap() == new_idx { return head }
    let (head_removed, head_added, replace) = self.propagate_change(path, &trail, new_idx);
    let culled_nodes = bfs_nodes(self.nodes.data(), head_removed, &self.leaves); 
    let edit_head = if let Some(new_node) = replace {
      let old_node = self.nodes.replace(head_removed as usize, new_node.clone()).unwrap();
      self.index_lookup.remove(&old_node);
      self.index_lookup.insert(new_node, head_added);
      head
    } else { head_added };
    for index in bfs_nodes(self.nodes.data(), head_added, &self.leaves) {
      self.nodes.add_ref(index as usize).unwrap()
    }
    for index in &culled_nodes {
      self.nodes.remove_ref(*index as usize).unwrap();
      if self.nodes.status(*index as usize).unwrap() == 1 && !self.is_leaf(*index) {
        self.index_lookup.remove(&self.nodes.remove_ref(*index as usize).unwrap().unwrap());
      }
    }
    edit_head
  }

  // Public functions used for reading
  pub fn node(&self, idx:Index) -> Result<&T, AccessError> {
    self.nodes.get(idx as usize)
  }

  pub fn child(&self, idx:Index, child:T::Children) -> Result<Index, AccessError> {
    Ok( self.node(idx)?.get(child) )
  }

  pub fn descend(&self, head:Index, path:&[T::Children]) -> Index {
    *self.get_trail(head, path).last().unwrap()
  }

  pub fn get_root(&mut self, index:Index) -> Index {
    self.nodes.add_ref(index as usize).unwrap();
    index
  }

}


// Changing this system'll take too long atm, I want to do other stuff maybe
// #[derive(Serialize, Deserialize)]
// struct TreeStorage<N : Node> {
//   head: Index,
//   memory: Vec<N>,
// }
//
// // Add metadata for all sorts of whatever I feel like
// /// Assumes constant leaf count
// #[allow(dead_code)]
// impl<T: GraphNode + Serialize + DeserializeOwned> SparseDirectedGraph<T> {
//   pub fn save_object_json(&self, head:Index) -> String {
//     let mut object_graph = Self::new(self.leaf_count);
//     let head_index = object_graph.clone_graph(self.nodes.data(), head);
//     let storage = TreeStorage {
//       head : head_index,
//       memory : object_graph.nodes.data().clone()
//     };
//     serde_json::to_string(&storage).unwrap()
//   }
//
//   // Currently requires the nodetype of both graph and data to be the same.
//   pub fn load_object_json(&mut self, json:String) -> Index {
//     let temp:TreeStorage<T> = serde_json::from_str(&json).unwrap();
//     self.clone_graph(&temp.memory, temp.head)
//   }
//
//   // Assumes equal leaf count (between the two graphs)
//   fn clone_graph<N : Node> (&mut self, from:&Vec<N>, head:Index) -> Index {
//     let mut remapped = HashMap::new();
//     for i in 0 .. self.leaf_count as Index { remapped.insert(i, i); }
//     for pointer in bfs_nodes(from, head, (self.leaf_count - 1) as usize).into_iter().rev() {
//       if !remapped.contains_key(&pointer) {
//         let mut new_kids = Vec::with_capacity(CHILD_COUNT);
//         for child in N::Children::all() {
//           new_kids.push(from[pointer as usize].get(child));
//         }
//         let new_node = T::new(&new_kids);
//         remapped.insert(pointer, self.add_node(new_node));
//       }
//       self.nodes.add_ref(*remapped.get(&pointer).unwrap() as usize).unwrap();
//     }
//     *remapped.get(&head).unwrap() as Index
//   }
//
// }

// Utility function
pub fn bfs_nodes<N: Node>(nodes:&Vec<N>, head:Index, leaves:&Vec<Index>) -> Vec<Index> {
  let mut queue = VecDeque::from([head]);
  let mut bfs_indexes = Vec::new();
  while let Some(index) = queue.pop_front() {
    bfs_indexes.push(index);
    if leaves.binary_search(&index).is_err() {
      let parent = &nodes[index as usize];
      for child in N::Children::all() {
        queue.push_back(parent.get(child))
      }
    }
  }
  bfs_indexes
}

// Yap yap I know this should be a library. I'll do that once it's less everchanging.
#[test]
fn merge_check() {
  let mut sdg: SparseDirectedGraph<super::prelude::BasicNode3d> = SparseDirectedGraph::new();
  let empty = sdg.add_leaf();
  let full = sdg.add_leaf();
  let mut head = sdg.get_root(empty);
  for x in 0 .. 4 {
    for y in 0 .. 4 {
      for z in 0 .. 4 {
        let path = super::prelude::BasicPath3d::from_cell(UVec3::new(x, y, z), 2).steps();
        head = sdg.set_node(head, &path, full);
      }
    }
  }
  let _ = sdg.nodes.trim();
  assert_eq!(sdg.nodes.data().len(), 2);
}
