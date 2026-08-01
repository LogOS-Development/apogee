//! Spacecraft bundle: groups kinematics + dynamics + vehicle config.

use super::dynamics::{Dynamics, SpacecraftConfig};
use super::kinematics::Kinematics;

/// ECS bundle representing a complete spacecraft entity.
#[derive(Debug, Clone, Default)]
pub struct SpacecraftBundle {
    pub kinematics: Kinematics,
    pub dynamics: Dynamics,
    pub config: SpacecraftConfig,
}
