use std::collections::{HashMap, VecDeque};
use glam::UVec3;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use vec_mem_heap::prelude::*;

const CHILD_COUNT : usize = 8;
type Index = u32;

#[derive(Debug, Copy, Clone, Serialize, Deserialize, derive_new::new)]
pub struct Pointer {
    pub idx : Index,
    pub height : u32
}

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
    const COUNT: usize = CHILD_COUNT;
    fn from(coord: UVec3) -> Self;
    fn to_coord(&self) -> UVec3;
}

// Nodes are anything with valid children access
pub trait Node : Clone + std::fmt::Debug {
    type Children : Childs;
    
    fn new(children:&[Index]) -> Self;
    fn get(&self, child: Self::Children) -> Index;
    fn set(&mut self, child: Self::Children, index:Index);
}
// GraphNodes are nodes which can be hashed and copied, making them valid for SDG storage
pub trait GraphNode : Node + Copy + std::hash::Hash + Eq {}

impl<T> Node for Option<T> where T: Node {
    type Children = T::Children;
    fn new(_:&[Index]) -> Self { panic!("Don't do that!") }
    fn get(&self, child:Self::Children) -> Index {
        self.as_ref().unwrap().get(child)
    }
    fn set(&mut self, child:Self::Children, index:Index) {
        self.as_mut().unwrap().set(child, index)
    }
}

pub struct SparseDirectedGraph<T: GraphNode> {
    pub nodes : NodeField<T>,
    pub index_lookup : HashMap<T, Index>,
    leaf_count : u8,
}
impl<T: GraphNode> SparseDirectedGraph<T> {
    // Utility
    pub fn new(leaf_count:u8) -> Self {
        let mut instance = Self {
            nodes : NodeField::new(),
            index_lookup : HashMap::new(),
            leaf_count,
        };
        for i in 0 .. leaf_count {
            let leaf = [i as Index; CHILD_COUNT];
            instance.add_node(T::new(&leaf));
        }
        instance
    }
    
    pub fn is_leaf(&self, index:Index) -> bool {
        index < self.leaf_count as Index
    }

    fn get_trail(&self, start:Index, path:&[T::Children]) -> Vec<Index>  {
        let mut trail = vec![start];
        for step in 0 .. path.len() {
            let parent = trail[step];
            match self.child(parent, path[step]) {
                Ok( child ) if child != parent => trail.push(child),
                Ok(_) => break,
                Err(error) => panic!("Trail encountered a fatal error, {error:?}")
            };
        }
        trail 
    }

    // Private functions used for writing
    fn find_index(&self, node:&T) -> Option<Index> {
        self.index_lookup.get(node).copied()
    }

    fn add_node(&mut self, node:T) -> Index {
        let index = self.nodes.push(node.clone()) as Index;
        self.index_lookup.insert(node, index);
        index
    }

    fn propagate_change(
        &mut self,
        path: &[T::Children],
        trail: &[Index],
        mut cur_pointer: Pointer,
        mut old_parent: Index,
    ) -> Result<(Index, Pointer, Option<T>), AccessError> {
        for cur_depth in (0 .. path.len()).rev() {
            // Trailing off early means we're at a leaf, we can just repeat that leaf and get sparsity by default
            old_parent = if cur_depth < trail.len() { trail[cur_depth] } else { *trail.last().unwrap() };
            let new_parent_node =  {
                let mut new_parent = self.node(old_parent)?.clone();
                new_parent.set(path[cur_depth], cur_pointer.idx);
                new_parent
            };
            cur_pointer.height += 1;
            cur_pointer.idx = match self.find_index(&new_parent_node) {
                Some(pointer) => pointer,
                None => {
                    if self.nodes.status(old_parent as usize).unwrap() == 2 && !self.is_leaf(old_parent) {
                        cur_pointer.idx = old_parent;
                        return Ok((old_parent, cur_pointer, Some(new_parent_node)))
                    } else { self.add_node(new_parent_node) }
                }
            };
        };
        Ok((old_parent, cur_pointer, None))
    }

    // Public functions used for writing
    pub fn set_node(&mut self, start:Pointer, path:&[T::Children], new_pointer:Index) -> Result<Pointer, AccessError> {
        if let Some(pointer) = self.descend(start, path) { 
            if pointer.idx == new_pointer { return Ok(start) }
        } else { panic!("Unspecified Path") }
        let trail = self.get_trail(start.idx, path);
        let (old_parent, cur_pointer, early_node) = self.propagate_change(
            path,
            &trail[..],
            Pointer::new(new_pointer, start.height - path.len() as u32),
            start.idx,
        )?;
        let last_leaf = self.leaf_count as usize - 1;
        let old_nodes = bfs_nodes(self.nodes.data(), old_parent, last_leaf); 
        let early_exit = match early_node { Some(node) => {
            self.index_lookup.remove(&self.nodes.replace(old_parent as usize, node.clone()).unwrap());
            self.index_lookup.insert(node, old_parent);
            true
        } None => { false }};
        for index in bfs_nodes(self.nodes.data(), cur_pointer.idx, last_leaf) {
            self.nodes.add_ref(index as usize).unwrap()
        }
        self.mass_remove(&old_nodes);
        // Returning start because the root node never changes
        if early_exit { Ok(start) } else { Ok(cur_pointer) }
    }

    pub fn mass_remove(&mut self, indices:&[Index]) {
        for index in indices {
            self.nodes.remove_ref(*index as usize).unwrap();
            if self.nodes.status(*index as usize).unwrap() == 1 && !self.is_leaf(*index) {
                self.index_lookup.remove(&self.nodes.remove_ref(*index as usize).unwrap().unwrap());
            }
        }
    }

    // Public functions used for reading
    pub fn node(&self, pointer:Index) -> Result<&T, AccessError> {
        self.nodes.get(pointer as usize)
    }

    pub fn child(&self, node:Index, child:T::Children) -> Result<Index, AccessError> {
        Ok( self.node(node)?.get(child) )
    }
    
    pub fn descend(&self, start:Pointer, path:&[T::Children]) -> Option<Pointer> {
        if start.height < path.len() as u32 { panic!("Path is longer than start allows.") }
        let trail = self.get_trail(start.idx, path);
        let node_pointer = trail.last()?;
        Some(Pointer::new(*node_pointer, start.height - (trail.len() as u32 - 1)))
    }

    pub fn get_root(&mut self, leaf:Index, height:u32) -> Pointer {
        self.nodes.add_ref(leaf as usize).unwrap();
        Pointer::new(leaf, height)
    }

}


#[derive(Serialize, Deserialize)]
struct TreeStorage<N : Node> {
    root: Pointer,
    memory: Vec<N>,
}
// Assumes constant leaf count. Eventually add more metadata
impl<T: GraphNode + Serialize + DeserializeOwned> SparseDirectedGraph<T> {
    pub fn save_object_json(&self, start:Pointer) -> String {
        let mut object_graph = Self::new(self.leaf_count);
        let root_index = object_graph.clone_graph(self.nodes.data(), start.idx);
        let storage = TreeStorage {
            root : Pointer::new(root_index, start.height),
            memory : self.nodes.data().iter().map(|node| {
                let Some(data) = node else { return None };
                Some(data.clone())
            }).collect(),
        };
        serde_json::to_string(&storage).unwrap()
    }
    
    // Currently requires the nodetype of both graph and data to be the same.
    pub fn load_object_json(&mut self, json:String) -> Pointer {
        let temp:TreeStorage<T> = serde_json::from_str(&json).unwrap();
        Pointer::new(self.clone_graph(&temp.memory, temp.root.idx), temp.root.height)
    }

    // Assumes equal leaf count (between the two graphs)
    fn clone_graph<N : Node> (&mut self, from:&Vec<N>, start:Index) -> Index {
        let mut remapped = HashMap::new();
        for i in 0 .. self.leaf_count as Index { remapped.insert(i, i); }
        for pointer in bfs_nodes(from, start, (self.leaf_count - 1) as usize).into_iter().rev() {
            if !remapped.contains_key(&pointer) {
                let mut new_kids = Vec::with_capacity(CHILD_COUNT);
                for child in N::Children::all() {
                    new_kids.push(from[pointer as usize].get(child));
                }
                let new_node = T::new(&new_kids);
                remapped.insert(pointer, self.add_node(new_node));
            }
            self.nodes.add_ref(*remapped.get(&pointer).unwrap() as usize).unwrap();
        }
        *remapped.get(&start).unwrap() as Index
    }

}

// Utility function
pub fn bfs_nodes<N: Node>(nodes:&Vec<N>, start:Index, last_leaf:usize) -> Vec<Index> {
    let mut queue = VecDeque::from([start]);
    let mut bfs_indexes = Vec::new();
    while let Some(index) = queue.pop_front() {
        bfs_indexes.push(index);
        if index > last_leaf as Index {
            for child in N::Children::all() {
                queue.push_back(nodes[index as usize].get(child))
            }
        }
    }
    bfs_indexes
}

// Move to a geometry area?
// impl<T, const D: usize> super::dag::SparseDirectedGraph<T, D> where T : super::dag::GraphNode<D> {
//     pub fn dfs_leaf_cells(&self, start:Pointer) -> Vec<CellData> {
//         let mut stack = Vec::from([(start.idx, ZorderPath::root())]);
//         let mut leaves = Vec::new();
//         while let Some((pointer, zorder)) = stack.pop() {
//             if self.is_leaf(pointer) {
//                 leaves.push(CellData::new(Pointer::new(pointer, start.height - zorder.depth), zorder.to_cell()));
//             } else { for i in 0 .. 4 {
//                     let children = self.node(pointer).unwrap().children();
//                     stack.push((children[i], zorder.step_down(i as u32)));
//                 }
//             }
//         }
//         leaves
//     }
// }
