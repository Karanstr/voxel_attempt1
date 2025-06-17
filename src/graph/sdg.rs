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
// Consider writing a raw_pointers method or somthing so we can access all children without
// iterating? Would return a slice bc we don't know how compression would work.
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
  fn get(&self, child:Self::Children) -> Index { self.as_ref().unwrap().get(child) }
  fn set(&mut self, child:Self::Children, index:Index) { self.as_mut().unwrap().set(child, index) }
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
  pub fn new() -> Self {
    Self {
      nodes : NodeField::new(),
      index_lookup : HashMap::new(),
      leaves : Vec::new(),
    }
  }

  /// Returns a trail with length path.len() + 1. trail.first() is the head of the trail and trail.last() is the node the path leads to.
  fn get_trail(&self, head:Index, path:&[T::Children]) -> Vec<Index>  {
    let mut trail = Vec::with_capacity(path.len() + 1);
    trail.push(head);
    for step in 0 .. path.len() { trail.push(self.child(trail[step], path[step])) }
    trail 
  }

  pub fn add_leaf(&mut self) -> Index {
    let idx = self.nodes.push(T::new(&vec![0; T::Children::COUNT])) as Index;
    let leaf = T::new(&vec![idx; T::Children::COUNT][..]);
    self.nodes.replace(idx as usize, leaf.clone()).unwrap();
    self.leaves.insert(self.leaves.iter().position(|leaf| idx < *leaf ).unwrap_or(self.leaves.len()), idx);
    self.index_lookup.insert(leaf, idx);
    idx
  }

  pub fn remove_leaf(&mut self, leaf:Index) {
    let leaf_list_idx = self.leaves.binary_search(&leaf).expect(format!("Leaf {leaf} isn't a leaf!!").as_str());
    if self.nodes.status(leaf as usize).unwrap() != 1 { panic!("The graph still needs leaf {leaf}") } else {
      let leaf_node = self.nodes.remove_ref(leaf as usize).unwrap().unwrap();
      self.index_lookup.remove(&leaf_node);
      self.leaves.remove(leaf_list_idx);
    }
  }

  fn add_node(&mut self, node:T) -> Index {
    let index = self.nodes.push(node.clone()) as Index;
    self.index_lookup.insert(node.clone(), index);
    // This node keeps all immediate children alive + 1
    for child in T::Children::all() { self.nodes.add_ref(node.get(child) as usize).unwrap(); }
    index
  }

  fn propagate_change(&mut self, path: &[T::Children], trail: &[Index], mut new_child: Index,) -> Index {
    for cur_depth in (0 .. path.len()).rev() {
      let new_node = self.node(trail[cur_depth]).with_child(path[cur_depth], new_child);
      new_child = if let Some(idx) = self.find_index(&new_node) { idx } else { self.add_node(new_node) };
    };
    new_child
  }

  pub fn set_node(&mut self, head:Index, path:&[T::Children], new_idx:Index) -> Index {
    let trail = self.get_trail(head, path);
    if *trail.last().unwrap() == new_idx { return head }
    let new_head = self.propagate_change(path, &trail, new_idx);
    self.nodes.add_ref(new_head as usize).unwrap();
    self.decrement_ref(head);
    new_head
  }

  fn decrement_ref(&mut self, idx:Index) {
    if self.nodes.status(idx as usize).unwrap() <= 2 && self.is_leaf(idx) { panic!("Decrement would remove index {idx}, a leaf. Please remove_leaf instead") } 
    let mut queue = VecDeque::from([idx as usize]);
    while let Some(cur_idx) = queue.pop_front() {
      self.nodes.remove_ref(cur_idx).unwrap();
      if self.nodes.status(cur_idx).unwrap() == 1 {
        let old_node = self.nodes.remove_ref(cur_idx).unwrap().unwrap();
        self.index_lookup.remove(&old_node);
        for child in T::Children::all() {
          let child_idx = old_node.get(child);
          if self.is_leaf(child_idx) { continue }
          queue.push_back(child_idx as usize)
        }
      }
    }
  }
  
  fn find_index(&self, node:&T) -> Option<Index> { self.index_lookup.get(node).copied() }
  
  fn is_leaf(&self, idx:Index) -> bool { self.leaves.binary_search(&idx).is_ok() }

  fn node(&self, idx:Index) -> &T { self.nodes.get(idx as usize).unwrap() }

  fn child(&self, idx:Index, child:T::Children) -> Index { self.node(idx).get(child) }

  pub fn descend(&self, head:Index, path:&[T::Children]) -> Index { *self.get_trail(head, path).last().unwrap() }

  pub fn get_root(&mut self, idx:Index) -> Index { self.nodes.add_ref(idx as usize).unwrap(); idx }

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

// Note to self, write some more tests!!
//
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
