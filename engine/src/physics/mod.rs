use rapier3d::prelude::*;
use nalgebra::Vector3;

mod voxel_obj_shape;


pub struct PhysicsManager {
  pipeline: PhysicsPipeline,
  gravity: Vector3<f32>,
  int_params: IntegrationParameters,
  islands: IslandManager,
  broad_phase: BroadPhaseBvh,
  narrow_phase: NarrowPhase,
  rigid_bodes: RigidBodySet,
  colliders: ColliderSet,
  impluse_joints: ImpulseJointSet,
  multibody_joints: MultibodyJointSet,
  ccd_solver: CCDSolver,
}
impl PhysicsManager {
  pub fn step(&mut self) {
    self.pipeline.step(
      &self.gravity,
      &self.int_params,
      &mut self.islands,
      &mut self.broad_phase,
      &mut self.narrow_phase,
      &mut self.rigid_bodes,
      &mut self.colliders,
      &mut self.impluse_joints,
      &mut self.multibody_joints,
      &mut self.ccd_solver,
      &(),
      &(),
    )
  }

}

// https://docs.rs/parry3d/0.23.0/parry3d/query/trait.QueryDispatcher.html
// We need to write a custom parry3d::query::QueryDispatcher to handle our custom VoxelObject shape implementation
