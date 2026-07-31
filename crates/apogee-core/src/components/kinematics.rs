//! Kinematic state: position, velocity, attitude, angular velocity.

use apogee_common::{Position, Velocity};
use nalgebra::{Quaternion, Vector3};

/// Translational and rotational state of a body.
#[derive(Debug, Clone)]
pub struct Kinematics {
    /// Inertial position (m).
    pub position: Position,
    /// Inertial velocity (m/s).
    pub velocity: Velocity,
    /// Attitude quaternion (body-to-inertial).
    pub attitude: Quaternion<f64>,
    /// Angular velocity in body frame (rad/s).
    pub angular_velocity: Vector3<f64>,
}

impl Default for Kinematics {
    fn default() -> Self {
        Self {
            position: Position::zeros(),
            velocity: Velocity::zeros(),
            attitude: Quaternion::identity(),
            angular_velocity: Vector3::zeros(),
        }
    }
}
