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

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn kinematics_default() {
        let k = Kinematics::default();
        assert_relative_eq!(k.position.norm(), 0.0);
        assert_relative_eq!(k.velocity.norm(), 0.0);
        assert_relative_eq!(k.attitude.w, 1.0);
        assert_relative_eq!(k.angular_velocity.norm(), 0.0);
    }
}
