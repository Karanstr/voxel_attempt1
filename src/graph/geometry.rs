use glam::{Vec3, UVec3};
use crate::graph::sdg::{Pointer, Childs};
use crate::graph::basic_node3d::Zorder3d;

const MIN_BLOCK_SIZE: Vec3 = Vec3::ONE;
pub const LIM_OFFSET: f32 = 1. / 0xFFFF as f32;

pub fn cell_length(height:u32) -> Vec3 {
    MIN_BLOCK_SIZE * 2_f32.powi(height as i32)
}

pub fn point_to_cells(tl_point:Vec3, start:Pointer, target_height:u32, point:Vec3) -> [Option<UVec3>; 8]{
    let mut surrounding = [None; 8];
    let grid_length = cell_length(start.height);
    let cell_length = cell_length(target_height);
    let origin_position = point - (tl_point - grid_length / 2.);
    for (i, child) in Zorder3d::all().enumerate() {
        let direction = (2 * child.to_coord()).as_vec3() - 1.;
        let cur_point = origin_position + direction * LIM_OFFSET;
        if cur_point.clamp(Vec3::ZERO, grid_length) == cur_point {
            surrounding[i] = Some( (cur_point / cell_length).floor().as_uvec3() )
        }
    }
    surrounding
}