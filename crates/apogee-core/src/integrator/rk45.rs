//! RK4(5) fixed/adaptive integrator — stub.

use apogee_common::units::Seconds;

use super::{IntegrationResult, Integrator, StateDerivative, StateVector};

/// RK4(5) embedded integrator.
#[derive(Debug)]
pub struct Rk45 {
    pub tolerance: f64,
    pub fixed_step: bool,
}

impl Default for Rk45 {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            fixed_step: false,
        }
    }
}

impl Integrator for Rk45 {
    fn step(
        &mut self,
        _state: &mut StateVector,
        _derivative_fn: &dyn Fn(&StateVector) -> StateDerivative,
        _dt: Seconds<f64>,
    ) -> IntegrationResult {
        // TODO: implement RK4(5) stages
        IntegrationResult::default()
    }
}
