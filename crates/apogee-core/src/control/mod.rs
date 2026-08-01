//! Guidance, navigation, and control (GNC) subsystem for spacecraft.
//!
//! This module provides attitude state estimation, attitude control, actuator
//! allocation, and a flight-mode state machine. It is intentionally separate
//! from the propagation layer: the GNC system computes commanded torques, and
//! the integrator in `systems::step` applies them to the rigid-body dynamics.

pub mod actuators;
pub mod controller;
pub mod estimator;
pub mod magnetorquers;
pub mod state_machine;

pub use actuators::*;
pub use controller::*;
pub use estimator::*;
pub use magnetorquers::*;
pub use state_machine::*;

use nalgebra::{Quaternion, UnitQuaternion, Vector3};

/// Combined commanded control output for one spacecraft.
#[derive(Debug, Clone, Default)]
pub struct ControlOutput {
    /// Net body-frame torque to apply (N m).
    pub torque_nm: Vector3<f64>,
    /// Net body-frame force to apply (N). Typically zero for attitude-only GNC.
    pub force_n: Vector3<f64>,
    /// Current flight mode selected by the state machine.
    pub mode: FlightMode,
}

/// Convenience: build a pure-torque control output.
pub fn torque_command(torque_nm: Vector3<f64>, mode: FlightMode) -> ControlOutput {
    ControlOutput {
        torque_nm,
        force_n: Vector3::zeros(),
        mode,
    }
}

/// Shortest-path quaternion error: `q_err = q_desired^* * q_actual`.
///
/// Returns the vector part (axis * sin(half-angle)) which is proportional to
/// the rotation error for small angles. The scalar part sign is flipped if
/// needed so the error takes the shortest path.
pub fn quaternion_error(
    actual: &UnitQuaternion<f64>,
    desired: &UnitQuaternion<f64>,
) -> (Vector3<f64>, f64) {
    let q_err = actual.inverse() * desired;
    let q = q_err.quaternion();
    // Ensure shortest path
    let sign = if q.w < 0.0 { -1.0 } else { 1.0 };
    (Vector3::new(sign * q.i, sign * q.j, sign * q.k), sign * q.w)
}

/// Integrate angular velocity into a quaternion over `dt` using first-order Lie
/// update (Euler-Rodrigues). Good enough for GNC loop rates; the propagation
/// integrator uses higher-order methods for the physics step.
pub fn integrate_attitude(q: &Quaternion<f64>, omega_rad_s: &Vector3<f64>, dt: f64) -> Quaternion<f64> {
    let half_dt = dt * 0.5;
    let delta = Quaternion::new(0.0, omega_rad_s.x * half_dt, omega_rad_s.y * half_dt, omega_rad_s.z * half_dt);
    let q_next = q + delta * q;
    q_next.normalize()
}
