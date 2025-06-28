use std::collections::{HashMap, VecDeque};
use glam::UVec3;
// use serde::{de::DeserializeOwned, Deserialize, Serialize};
use lilypads::Pond;

pub type Index = u32;
#[allow(unused)]
pub trait Path<T : Childs> {
  fn to_cell(path: Vec<T>) -> UVec3;
  fn path_from(cell:UVec3, depth:u32) -> Vec<T>;
}
pub trait Childs: std::fmt::Debug + Clone + Copy {
  fn all() -> impl Iterator<Item = Self>;
  const COUNT: usize;
  fn new(quadrant: UVec3) -> Self;
  fn to_coord(&self) -> UVec3;
}

// Nodes are anything with valid children access
pub trait Node : Clone + Copy + std::fmt::Debug {
  type Children : Childs;
  fn new(children:&[u32]) -> Self;
  fn get(&self, child: Self::Children) -> Index;
  fn set(&mut self, child: Self::Children, index:Index);
  fn with_child(&self, child: Self::Children, index:Index) -> Self;
}
// GraphNodes are nodes which can be hashed, making them valid for SDG storage
pub trait GraphNode : Node + std::hash::Hash + Eq {}

pub struct SparseDirectedGraph<T: GraphNode> {
  pub nodes : Pond<T>,
  ref_count: Vec<u32>,
  index_lookup : HashMap<T, Index>,
  leaves: Vec<Index>,
}
impl<T: GraphNode> SparseDirectedGraph<T> {
  pub fn new() -> Self {
    Self {
      nodes : Pond::new(),
      ref_count : Vec::new(),
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

  fn add_ref(&mut self, idx: Index) {
    if idx as usize >= self.ref_count.len() { self.ref_count.resize(idx as usize + 1, 0) }
    self.ref_count[idx as usize] += 1;
  }

  fn get_ref(&self, idx: Index) -> u32 { self.ref_count[idx as usize] }
  
  pub fn add_leaf(&mut self) -> Index {
    let idx = self.nodes.next_allocated() as Index;
    let leaf = T::new(&vec![idx; T::Children::COUNT][..]);
    self.nodes.write(idx as usize, leaf);
    self.leaves.insert(
      self.leaves.iter().position(|leaf| idx < *leaf ).unwrap_or(self.leaves.len()),
      idx
    );
    self.index_lookup.insert(leaf, idx);
    idx
  }

  pub fn _remove_leaf(&mut self, leaf:Index) {
    let leaf_list_idx = self.leaves.binary_search(&leaf).expect(format!("Index {leaf} isn't a leaf!!").as_str());
    if self.get_ref(leaf) > 0 { panic!("The graph still needs leaf {leaf}") } else {
      let leaf_node = self.nodes.free(leaf as usize).unwrap();
      self.index_lookup.remove(&leaf_node);
      self.leaves.remove(leaf_list_idx);
    }
  }

  fn add_node(&mut self, node:T) -> Index {
    let idx = self.nodes.alloc(node.clone()) as Index;
    for child in T::Children::all() { self.add_ref(node.get(child)); }
    self.index_lookup.insert(node, idx);
    idx
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
    self.add_ref(new_head);
    self.decrement_ref(head);
    new_head
  }

  fn decrement_ref(&mut self, idx:Index) {
    let mut queue = vec![idx];
    while let Some(cur_idx) = queue.pop() {
      self.ref_count[cur_idx as usize] -= 1;
      if self.get_ref(cur_idx) == 0 && !self.is_leaf(cur_idx) {
        let old_node = self.nodes.free(cur_idx as usize).unwrap();
        self.index_lookup.remove(&old_node);
        for child in T::Children::all() {
          queue.push(old_node.get(child));
        }
      }
    }
  }
  
  pub fn get_root(&mut self, idx:Index) -> Index { self.add_ref(idx); idx }

  fn find_index(&self, node:&T) -> Option<Index> { self.index_lookup.get(node).copied() }
  
  fn is_leaf(&self, idx:Index) -> bool { self.leaves.binary_search(&idx).is_ok() }

  fn node(&self, idx:Index) -> &T { self.nodes.get(idx as usize).unwrap() }

  fn child(&self, idx:Index, child:T::Children) -> Index { self.node(idx).get(child) }

  pub fn descend(&self, head:Index, path:&[T::Children]) -> Index { *self.get_trail(head, path).last().unwrap() }


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

#[test]
fn merge_check() {
  let mut sdg: SparseDirectedGraph<super::prelude::BasicNode3d> = SparseDirectedGraph::new();
  let empty = sdg.add_leaf();
  let full = sdg.add_leaf();
  let mut head = sdg.get_root(empty);
  for x in 0 .. 4 {
    for y in 0 .. 4 {
      for z in 0 .. 4 {
        let path = super::prelude::Zorder3d::path_from(UVec3::new(x, y, z), 2);
        head = sdg.set_node(head, &path, full);
      }
    }
  }
  let _ = sdg.nodes.trim();
  assert_eq!(sdg.nodes.unsafe_data().len(), 2);
}
