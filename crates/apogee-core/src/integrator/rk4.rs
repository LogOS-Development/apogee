//! Fixed-step Runge-Kutta 4 integrator.

use super::{IntegrationResult, Integrator, StateDerivative, StateVector};

/// Classic fixed-step RK4 integrator.
#[derive(Debug, Clone, Copy)]
pub struct Rk4 {
    /// Step size (s). Must be positive.
    pub step: f64,
}

impl Rk4 {
    /// Create a new RK4 integrator with the given fixed step.
    pub fn new(step: f64) -> Self {
        assert!(step > 0.0, "RK4 step must be positive");
        Self { step }
    }
}

impl Integrator for Rk4 {
    fn step(
        &mut self,
        state: &mut StateVector,
        derivative_fn: &dyn Fn(&StateVector) -> StateDerivative,
        dt: f64,
    ) -> IntegrationResult {
        let sign = if dt >= 0.0 { 1.0 } else { -1.0 };
        let h = self.step * sign;
        let n = (dt.abs() / self.step).ceil() as usize;
        if n == 0 {
            return IntegrationResult {
                accepted: true,
                error_estimate: 0.0,
                step_taken: 0.0,
            };
        }
        let mut s = state.clone();
        let mut total = 0.0_f64;
        for _ in 0..n {
            let remaining = dt - total;
            let step = if remaining.abs() < self.step {
                remaining
            } else {
                h
            };
            rk4_stage(&mut s, derivative_fn, step);
            total += step;
        }
        *state = s;
        IntegrationResult {
            accepted: true,
            error_estimate: 0.0,
            step_taken: total,
        }
    }
}

fn rk4_stage(
    state: &mut StateVector,
    derivative_fn: &dyn Fn(&StateVector) -> StateDerivative,
    h: f64,
) {
    use crate::control::integrate_attitude;

    let k1 = derivative_fn(state);
    let mut s2 = state.clone();
    s2.position += k1.velocity * (0.5 * h);
    s2.velocity += k1.acceleration * (0.5 * h);
    s2.attitude = integrate_attitude(&s2.attitude, &s2.angular_velocity, 0.5 * h);
    let k2 = derivative_fn(&s2);

    let mut s3 = state.clone();
    s3.position += k2.velocity * (0.5 * h);
    s3.velocity += k2.acceleration * (0.5 * h);
    s3.attitude = integrate_attitude(&s3.attitude, &s3.angular_velocity, 0.5 * h);
    let k3 = derivative_fn(&s3);

    let mut s4 = state.clone();
    s4.position += k3.velocity * h;
    s4.velocity += k3.acceleration * h;
    s4.attitude = integrate_attitude(&s4.attitude, &s4.angular_velocity, h);
    let k4 = derivative_fn(&s4);

    state.position +=
        (k1.velocity + 2.0 * k2.velocity + 2.0 * k3.velocity + k4.velocity) * (h / 6.0);
    state.velocity +=
        (k1.acceleration + 2.0 * k2.acceleration + 2.0 * k3.acceleration + k4.acceleration)
            * (h / 6.0);
    state.attitude = integrate_attitude(&state.attitude, &state.angular_velocity, h);
    state.angular_velocity += (k1.angular_acceleration
        + 2.0 * k2.angular_acceleration
        + 2.0 * k3.angular_acceleration
        + k4.angular_acceleration)
        * (h / 6.0);
}

fn _integrate_attitude_quat(
    q: &nalgebra::Quaternion<f64>,
    omega: &nalgebra::Vector3<f64>,
    dt: f64,
) -> nalgebra::Quaternion<f64> {
    crate::control::integrate_attitude(q, omega, dt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_rk4_harmonic_oscillator() {
        // x'' = -x, analytic solution x(t) = cos(t), v(t) = -sin(t).
        let mut state = StateVector {
            position: apogee_common::Position::new(1.0, 0.0, 0.0),
            velocity: apogee_common::Velocity::new(0.0, 0.0, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };
        let mut integrator = Rk4::new(0.01);
        integrator.step(
            &mut state,
            &|s: &StateVector| {
                let acc = apogee_common::Position::new(-s.position.x, 0.0, 0.0);
                StateDerivative {
                    velocity: s.velocity,
                    acceleration: acc,
                    attitude_derivative: nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
                    angular_acceleration: nalgebra::Vector3::zeros(),
                }
            },
            2.0 * std::f64::consts::PI,
        );

        assert_relative_eq!(state.position.x, 1.0, epsilon = 1e-4);
        assert_relative_eq!(state.velocity.x, 0.0, epsilon = 1e-4);
    }
}
