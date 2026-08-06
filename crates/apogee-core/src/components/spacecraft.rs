//! Spacecraft bundle: groups kinematics + rigid body + vehicle config.

use super::kinematics::Kinematics;
use super::rigid_body::{RigidBody, SpacecraftConfig};

/// ECS bundle representing a complete spacecraft entity.
#[derive(Debug, Clone, Default)]
pub struct SpacecraftBundle {
    pub kinematics: Kinematics,
    pub rigid_body: RigidBody,
    pub config: SpacecraftConfig,
}