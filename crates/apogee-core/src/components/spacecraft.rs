//! Spacecraft bundle: groups kinematics + dynamics + vehicle config.

/// ECS bundle representing a complete spacecraft entity.
#[derive(Debug, Clone, Default)]
pub struct SpacecraftBundle {
    pub kinematics: super::kinematics::Kinematics,
    pub dynamics: super::dynamics::Dynamics,
}
