//! Multi-rate integrator: RK8(9) outer, RK4(5) inner.

pub mod rk4;
pub mod rk45;
pub mod rk89;

pub use rk4::*;
pub use rk45::*;
pub use rk89::*;

/// Integrator trait.
pub trait Integrator: Send + std::fmt::Debug {
    /// Advance state by `dt`, calling `derivative_fn` to compute derivatives.
    fn step(
        &mut self,
        state: &mut StateVector,
        derivative_fn: &dyn Fn(&StateVector) -> StateDerivative,
        dt: f64,
    ) -> IntegrationResult;
}

/// Compact state vector for integration.
#[derive(Debug, Clone, Default)]
pub struct StateVector {
    pub position: apogee_common::Position,
    pub velocity: apogee_common::Velocity,
    /// Attitude quaternion (body-to-inertial).
    pub attitude: nalgebra::Quaternion<f64>,
    /// Angular velocity in body frame (rad/s).
    pub angular_velocity: nalgebra::Vector3<f64>,
}

impl StateVector {
    pub fn from_kinematics(k: &crate::components::kinematics::Kinematics) -> Self {
        Self {
            position: k.position,
            velocity: k.velocity,
            attitude: k.attitude,
            angular_velocity: k.angular_velocity,
        }
    }

    pub fn write_to_kinematics(&self, k: &mut crate::components::kinematics::Kinematics) {
        k.position = self.position;
        k.velocity = self.velocity;
        k.attitude = self.attitude;
        k.angular_velocity = self.angular_velocity;
    }
}

/// Time derivative of state.
#[derive(Debug, Clone, Default)]
pub struct StateDerivative {
    pub velocity: apogee_common::Velocity,
    pub acceleration: apogee_common::Position,
    /// Attitude derivative (quaternion time derivative).
    pub attitude_derivative: nalgebra::Quaternion<f64>,
    /// Angular acceleration in body frame (rad/s^2).
    pub angular_acceleration: nalgebra::Vector3<f64>,
}

/// Result of an integration step.
#[derive(Debug, Clone, Default)]
pub struct IntegrationResult {
    pub accepted: bool,
    pub error_estimate: f64,
    pub step_taken: f64,
}

/// Multi-rate integrator: separate rates for translation, attitude, modal.
#[derive(Debug)]
pub struct MultiRateIntegrator {
    pub outer: Box<dyn Integrator>,
    pub inner: Box<dyn Integrator>,
    pub flexible: Box<dyn Integrator>,
}
