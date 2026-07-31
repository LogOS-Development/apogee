//! Multi-rate integrator: RK8(9) outer, RK4(5) inner.

pub mod rk45;
pub mod rk89;

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
}

/// Time derivative of state.
#[derive(Debug, Clone, Default)]
pub struct StateDerivative {
    pub velocity: apogee_common::Velocity,
    pub acceleration: apogee_common::Position,
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
