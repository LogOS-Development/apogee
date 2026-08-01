//! Attitude controller supporting PID and proportional-derivative quaternion
//! feedback.
//!
//! The controller outputs a commanded body-frame torque. Torque allocation
//! across reaction wheels and thrusters is delegated to the actuator module.

use nalgebra::{UnitQuaternion, Vector3};

use crate::control::{FlightMode, quaternion_error, torque_command, ControlOutput};

/// Attitude controller gains.
#[derive(Debug, Clone)]
pub struct AttitudeControllerGains {
    /// Proportional quaternion-error gain (N m / rad).
    pub kp: f64,
    /// Derivative angular-velocity gain (N m s / rad).
    pub kd: f64,
    /// Integral gain (N m / rad s).
    pub ki: f64,
}

impl AttitudeControllerGains {
    /// Reasonable starting point for a 1 kg m^2 inertia, 0.1 Hz bandwidth.
    pub fn nominal() -> Self {
        Self {
            kp: 0.1,
            kd: 0.2,
            ki: 0.0,
        }
    }
}

/// Controller setpoint and mode-specific behavior.
#[derive(Debug, Clone)]
pub struct AttitudeSetpoint {
    /// Desired body-to-inertial attitude.
    pub attitude: UnitQuaternion<f64>,
    /// Desired body-frame angular velocity (rad/s).
    pub angular_velocity: Vector3<f64>,
}

impl Default for AttitudeSetpoint {
    fn default() -> Self {
        Self {
            attitude: UnitQuaternion::identity(),
            angular_velocity: Vector3::zeros(),
        }
    }
}

/// Simple attitude controller with optional integral term.
#[derive(Debug, Clone)]
pub struct AttitudeController {
    gains: AttitudeControllerGains,
    integral: Vector3<f64>,
    /// Maximum commanded torque (N m) to keep actuators in bounds.
    pub max_torque_nm: f64,
}

impl AttitudeController {
    pub fn new(gains: AttitudeControllerGains, max_torque_nm: f64) -> Self {
        Self {
            gains,
            integral: Vector3::zeros(),
            max_torque_nm,
        }
    }

    /// Reset integral accumulator.
    pub fn reset(&mut self) {
        self.integral = Vector3::zeros();
    }

    /// Compute control torque given current estimated state and setpoint.
    pub fn compute(
        &mut self,
        estimated_attitude: &UnitQuaternion<f64>,
        estimated_rate: &Vector3<f64>,
        setpoint: &AttitudeSetpoint,
        mode: FlightMode,
        dt: f64,
    ) -> ControlOutput {
        match mode {
            FlightMode::Idle | FlightMode::Point | FlightMode::Maneuver => {
                let (err_vec, _err_scalar) = quaternion_error(estimated_attitude, &setpoint.attitude);
                // Quaternion vector part is roughly axis * sin(theta/2); double for small-angle axis.
                let angle_error = err_vec * 2.0;
                let rate_error = estimated_rate - setpoint.angular_velocity;

                self.integral += angle_error * dt;
                // Anti-windup: clamp integral contribution to max torque fraction.
                let max_integral = self.max_torque_nm * 0.3;
                self.integral = self.integral.cap_nans().cap_max_norm(max_integral);

                let mut torque = angle_error * self.gains.kp
                    + rate_error * self.gains.kd
                    + self.integral * self.gains.ki;

                // Saturation
                let norm = torque.norm();
                if norm > self.max_torque_nm {
                    torque *= self.max_torque_nm / norm;
                }

                torque_command(torque, mode)
            }
            FlightMode::Coast => {
                // Damping only: drive angular velocity to setpoint rate.
                let rate_error = estimated_rate - setpoint.angular_velocity;
                let mut torque = rate_error * self.gains.kd;
                let norm = torque.norm();
                if norm > self.max_torque_nm {
                    torque *= self.max_torque_nm / norm;
                }
                torque_command(torque, mode)
            }
            FlightMode::Safe => {
                // Safe mode: no active control torque. Sun-point or slow spin is
                // implemented by higher-level safe-mode logic, not here.
                torque_command(Vector3::zeros(), mode)
            }
        }
    }
}

trait ClampHelpers {
    fn cap_nans(self) -> Self;
    fn cap_max_norm(self, max_norm: f64) -> Self;
}

impl ClampHelpers for Vector3<f64> {
    fn cap_nans(self) -> Self {
        self.map(|v| if v.is_nan() { 0.0 } else { v })
    }

    fn cap_max_norm(self, max_norm: f64) -> Self {
        let n = self.norm();
        if n > max_norm && n > 0.0 {
            self * (max_norm / n)
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use nalgebra::{UnitQuaternion, Vector3};

    use super::*;

    #[test]
    fn test_pointing_error_drives_torque() {
        let mut ctrl = AttitudeController::new(AttitudeControllerGains::nominal(), 1.0);
        let q_est = UnitQuaternion::identity();
        let q_des = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), 0.1);
        let setpoint = AttitudeSetpoint {
            attitude: q_des,
            angular_velocity: Vector3::zeros(),
        };
        let out = ctrl.compute(&q_est, &Vector3::zeros(), &setpoint, FlightMode::Point, 0.1);
        // Torque should be roughly around -Z to correct +Z rotation.
        assert!(out.torque_nm.z > 0.0);
        assert!(out.torque_nm.norm() > 0.0);
        assert!(out.torque_nm.norm() <= 1.0 + 1e-9);
    }

    #[test]
    fn test_safe_mode_zero_torque() {
        let mut ctrl = AttitudeController::new(AttitudeControllerGains::nominal(), 1.0);
        let q_des = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 1.0);
        let setpoint = AttitudeSetpoint {
            attitude: q_des,
            angular_velocity: Vector3::zeros(),
        };
        let out = ctrl.compute(
            &UnitQuaternion::identity(),
            &Vector3::zeros(),
            &setpoint,
            FlightMode::Safe,
            0.1,
        );
        assert_relative_eq!(out.torque_nm, Vector3::zeros(), epsilon = 1e-9);
    }
}
