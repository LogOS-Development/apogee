//! RK8(9) adaptive integrator (Dormand-Prince or Verner) — stub.

use apogee_common::units::Seconds;

use super::{IntegrationResult, Integrator, StateDerivative, StateVector};

/// RK8(9) adaptive-step integrator.
#[derive(Debug)]
pub struct Rk89 {
    pub tolerance: f64,
    pub min_step: f64,
    pub max_step: f64,
}

impl Default for Rk89 {
    fn default() -> Self {
        Self {
            tolerance: 1e-12,
            min_step: 1e-10,
            max_step: 1e3,
        }
    }
}

impl Integrator for Rk89 {
    fn step(
        &mut self,
        _state: &mut StateVector,
        _derivative_fn: &dyn Fn(&StateVector) -> StateDerivative,
        _dt: Seconds<f64>,
    ) -> IntegrationResult {
        // TODO: implement 8th-order stages with 9th-order error estimate
        IntegrationResult::default()
    }
}
