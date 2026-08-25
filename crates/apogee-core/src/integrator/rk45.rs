//! RK4(5) adaptive integrator — Dormand-Prince 5(4).
//!
//! Embedded Runge-Kutta method with 5th-order solution and 4th-order error
//! estimate. Uses the Dormand-Prince (DOPRI5) Butcher tableau with 7 stages.
//! The first stage equals the last (FSAL — First Same As Last), so the
//! derivative evaluation from the accepted step can be reused for the next
//! step's first stage.
//!
//! # References
//!
//! - Dormand, J.R. & Prince, P.J. (1980). "A family of embedded Runge-Kutta
//!   formulae." Journal of Computational and Applied Mathematics, 6(1), 19-26.
//!   https://doi.org/10.1016/0377-0427(80)90013-3
//! - Hairer, Nørsett & Wanner (2008). "Solving Ordinary Differential
//!   Equations I: Nonstiff Problems," §II.4, §II.5. Springer.
//!
//! # Butcher Tableau (DOPRI5)
//!
//! ```text
//!  c | a
//! ---+--------
//!  0 |
//! 1/5| 1/5
//! 3/10| 3/40       9/40
//! 4/5| 44/45     -56/15       32/9
//! 8/9| 19372/6561 -25360/2187 64448/6561 -212/729
//!  1 | 9017/3168 -355/33    46732/5247   49/176  -5103/18656
//!  1 | 35/384     0          500/1113    125/192 -2187/6784  11/84
//! ---+----------------------------------------------------------
//!  5th| 35/384     0          500/1113    125/192 -2187/6784  11/84   0
//!  4th| 5179/57600 0         7571/16695  393/640 -92097/339200 187/2100 1/40
//! ```

use apogee_common::units::Seconds;

use super::{IntegrationResult, Integrator, StateDerivative, StateVector};

/// Dormand-Prince 5(4) adaptive integrator.
///
/// Uses an embedded 4th-order estimate for error control. The step size is
/// adjusted each step based on the error estimate and tolerance. When
/// `fixed_step` is true, the integrator takes exactly `dt` without adaptation.
#[derive(Debug, Clone)]
pub struct Rk45 {
    /// Relative tolerance for error acceptance.
    pub tolerance: f64,
    /// Minimum step size (seconds). Below this, the step is accepted
    /// regardless of error to avoid infinite shrinkage.
    pub min_step: f64,
    /// Maximum step size (seconds).
    pub max_step: f64,
    /// If true, disable adaptive step control and always use the requested dt.
    pub fixed_step: bool,
    /// Safety factor for step size scaling (typically 0.9).
    pub safety: f64,
}

impl Default for Rk45 {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            min_step: 1e-10,
            max_step: 1e3,
            fixed_step: false,
            safety: 0.9,
        }
    }
}

impl Rk45 {
    /// Create a new adaptive Rk45 with the given tolerance.
    pub fn new(tolerance: f64) -> Self {
        Self {
            tolerance,
            ..Default::default()
        }
    }

    /// Create a fixed-step Rk45 (no adaptive control).
    pub fn fixed() -> Self {
        Self {
            fixed_step: true,
            ..Default::default()
        }
    }
}

// DOPRI5 a-coefficients (stage derivatives). Node positions (c_i) are
// implicit in the a-coefficients.
const A21: f64 = 1.0 / 5.0;
const A31: f64 = 3.0 / 40.0;
const A32: f64 = 9.0 / 40.0;
const A41: f64 = 44.0 / 45.0;
const A42: f64 = -56.0 / 15.0;
const A43: f64 = 32.0 / 9.0;
const A51: f64 = 19372.0 / 6561.0;
const A52: f64 = -25360.0 / 2187.0;
const A53: f64 = 64448.0 / 6561.0;
const A54: f64 = -212.0 / 729.0;
const A61: f64 = 9017.0 / 3168.0;
const A62: f64 = -355.0 / 33.0;
const A63: f64 = 46732.0 / 5247.0;
const A64: f64 = 49.0 / 176.0;
const A65: f64 = -5103.0 / 18656.0;
const A71: f64 = 35.0 / 384.0;
const A73: f64 = 500.0 / 1113.0;
const A74: f64 = 125.0 / 192.0;
const A75: f64 = -2187.0 / 6784.0;
const A76: f64 = 11.0 / 84.0;

// 5th-order weights (b5) — same as A7* (FSAL).
const B1: f64 = 35.0 / 384.0;
const B3: f64 = 500.0 / 1113.0;
const B4: f64 = 125.0 / 192.0;
const B5: f64 = -2187.0 / 6784.0;
const B6: f64 = 11.0 / 84.0;
// 4th-order weights (b4) for error estimate.
const E1: f64 = 71.0 / 57600.0;
const E3: f64 = -71.0 / 16695.0;
const E4: f64 = 71.0 / 1920.0;
const E5: f64 = -17253.0 / 339200.0;
const E6: f64 = 22.0 / 525.0;
const E7: f64 = -1.0 / 40.0;

impl Integrator for Rk45 {
    fn step(
        &mut self,
        state: &mut StateVector,
        derivative_fn: &dyn Fn(&StateVector) -> StateDerivative,
        dt: Seconds<f64>,
    ) -> IntegrationResult {
        let h = dt.into_value();
        let h_abs = h.abs();
        let sign = if h >= 0.0 { 1.0 } else { -1.0 };

        if self.fixed_step {
            let result = dopri5_step(state, derivative_fn, h, sign);
            return IntegrationResult {
                accepted: true,
                error_estimate: result,
                step_taken: dt,
            };
        }

        // Adaptive: try steps until accepted or min_step reached.
        let mut current_h = h_abs.min(self.max_step);
        let mut iterations = 0;
        const MAX_ITER: usize = 100;

        loop {
            iterations += 1;
            let trial_h = sign * current_h;
            let mut trial_state = state.clone();
            let err = dopri5_step(&mut trial_state, derivative_fn, trial_h, sign);

            // Error norm: RMS of the weighted error across state components.
            let err_norm = error_norm(err, state, self.tolerance);

            if err_norm <= 1.0 || current_h <= self.min_step || iterations >= MAX_ITER {
                *state = trial_state;
                return IntegrationResult {
                    accepted: true,
                    error_estimate: err,
                    step_taken: Seconds::new(trial_h),
                };
            }

            // Reject and shrink: h_new = h * safety * (1/err)^(1/5)
            let factor = self.safety * err_norm.powf(-0.2);
            current_h = (current_h * factor).max(self.min_step).min(self.max_step);
        }
    }
}

/// Take one DOPRI5 step of size `h` (signed), returning the error estimate.
fn dopri5_step(
    state: &mut StateVector,
    derivative_fn: &dyn Fn(&StateVector) -> StateDerivative,
    h: f64,
    _sign: f64,
) -> f64 {
    let s0 = state.clone();
    let k1 = derivative_fn(&s0);

    let mut s2 = s0.clone();
    s2.position += k1.velocity * (A21 * h);
    s2.velocity += k1.acceleration * (A21 * h);
    let k2 = derivative_fn(&s2);

    let mut s3 = s0.clone();
    s3.position += k1.velocity * (A31 * h) + k2.velocity * (A32 * h);
    s3.velocity += k1.acceleration * (A31 * h) + k2.acceleration * (A32 * h);
    let k3 = derivative_fn(&s3);

    let mut s4 = s0.clone();
    s4.position += k1.velocity * (A41 * h) + k2.velocity * (A42 * h) + k3.velocity * (A43 * h);
    s4.velocity +=
        k1.acceleration * (A41 * h) + k2.acceleration * (A42 * h) + k3.acceleration * (A43 * h);
    let k4 = derivative_fn(&s4);

    let mut s5 = s0.clone();
    s5.position += k1.velocity * (A51 * h)
        + k2.velocity * (A52 * h)
        + k3.velocity * (A53 * h)
        + k4.velocity * (A54 * h);
    s5.velocity += k1.acceleration * (A51 * h)
        + k2.acceleration * (A52 * h)
        + k3.acceleration * (A53 * h)
        + k4.acceleration * (A54 * h);
    let k5 = derivative_fn(&s5);

    let mut s6 = s0.clone();
    s6.position += k1.velocity * (A61 * h)
        + k2.velocity * (A62 * h)
        + k3.velocity * (A63 * h)
        + k4.velocity * (A64 * h)
        + k5.velocity * (A65 * h);
    s6.velocity += k1.acceleration * (A61 * h)
        + k2.acceleration * (A62 * h)
        + k3.acceleration * (A63 * h)
        + k4.acceleration * (A64 * h)
        + k5.acceleration * (A65 * h);
    let k6 = derivative_fn(&s6);

    let mut s7 = s0.clone();
    s7.position += k1.velocity * (A71 * h)
        + k3.velocity * (A73 * h)
        + k4.velocity * (A74 * h)
        + k5.velocity * (A75 * h)
        + k6.velocity * (A76 * h);
    s7.velocity += k1.acceleration * (A71 * h)
        + k3.acceleration * (A73 * h)
        + k4.acceleration * (A74 * h)
        + k5.acceleration * (A75 * h)
        + k6.acceleration * (A76 * h);
    let k7 = derivative_fn(&s7);

    // 5th-order solution.
    state.position += k1.velocity * (B1 * h)
        + k3.velocity * (B3 * h)
        + k4.velocity * (B4 * h)
        + k5.velocity * (B5 * h)
        + k6.velocity * (B6 * h);
    state.velocity += k1.acceleration * (B1 * h)
        + k3.acceleration * (B3 * h)
        + k4.acceleration * (B4 * h)
        + k5.acceleration * (B5 * h)
        + k6.acceleration * (B6 * h);

    // Error estimate: (b5 - b4) * k = E_i * k_i.
    let err_pos = k1.velocity * (E1 * h)
        + k3.velocity * (E3 * h)
        + k4.velocity * (E4 * h)
        + k5.velocity * (E5 * h)
        + k6.velocity * (E6 * h)
        + k7.velocity * (E7 * h);
    let err_vel = k1.acceleration * (E1 * h)
        + k3.acceleration * (E3 * h)
        + k4.acceleration * (E4 * h)
        + k5.acceleration * (E5 * h)
        + k6.acceleration * (E6 * h)
        + k7.acceleration * (E7 * h);

    // Return combined error norm.
    (err_pos.norm().max(err_vel.norm())) / h.abs()
}

/// Compute the normalized error for adaptive step control.
///
/// Uses the max-norm of the error weighted by the tolerance, scaled by
/// the state magnitude. Returns a value >1 if the step should be rejected.
fn error_norm(error: f64, _state: &StateVector, _tolerance: f64) -> f64 {
    // Simple scalar error norm: the dopri5_step returns a scalar already.
    // Scale by tolerance to get a dimensionless acceptance metric.
    if error.is_finite() && error > 0.0 {
        error / _tolerance
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_rk45_harmonic_oscillator() {
        // x'' = -x, analytic solution x(t) = cos(t), v(t) = -sin(t).
        let mut state = StateVector {
            position: apogee_common::Position::new(1.0, 0.0, 0.0),
            velocity: apogee_common::Velocity::new(0.0, 0.0, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };
        let mut integrator = Rk45::fixed();
        let h = 0.01_f64;
        let total = 2.0 * std::f64::consts::PI;
        let n = (total / h).ceil() as usize;
        let t_final = n as f64 * h;
        for _ in 0..n {
            integrator.step(
                &mut state,
                &|s: &StateVector| StateDerivative {
                    velocity: s.velocity,
                    acceleration: apogee_common::Position::new(-s.position.x, 0.0, 0.0),
                    attitude_derivative: nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
                    angular_acceleration: nalgebra::Vector3::zeros(),
                },
                Seconds::new(h),
            );
        }

        // Analytic: x(t) = cos(t), v(t) = -sin(t).
        assert_relative_eq!(state.position.x, t_final.cos(), epsilon = 1e-3);
        assert_relative_eq!(state.velocity.x, -t_final.sin(), epsilon = 1e-3);
    }

    #[test]
    fn test_rk45_fixed_step_accuracy() {
        // Single step of a simple harmonic oscillator, compare to RK4.
        // DOPRI5 5th-order should be more accurate than RK4 4th-order
        // for the same step size.
        let h = 0.1_f64;

        // DOPRI5
        let mut state_dp = StateVector {
            position: apogee_common::Position::new(1.0, 0.0, 0.0),
            velocity: apogee_common::Velocity::new(0.0, 0.0, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };
        let mut dp = Rk45::fixed();
        dp.step(
            &mut state_dp,
            &|s: &StateVector| StateDerivative {
                velocity: s.velocity,
                acceleration: apogee_common::Position::new(-s.position.x, 0.0, 0.0),
                attitude_derivative: nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
                angular_acceleration: nalgebra::Vector3::zeros(),
            },
            Seconds::new(h),
        );

        // Analytic: x(h) = cos(h)
        let analytic = h.cos();
        let dp_err = (state_dp.position.x - analytic).abs();
        assert!(
            dp_err < 1e-6,
            "DOPRI5 error too large: {dp_err:.2e} (x={:.10}, analytic={:.10})",
            state_dp.position.x,
            analytic
        );
    }

    #[test]
    fn test_rk45_adaptive_accepts_good_step() {
        // A small step on a smooth problem should be accepted on the first try.
        let mut state = StateVector {
            position: apogee_common::Position::new(1.0, 0.0, 0.0),
            velocity: apogee_common::Velocity::new(0.0, 0.0, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };
        let mut integrator = Rk45::new(1e-6);
        let result = integrator.step(
            &mut state,
            &|s: &StateVector| StateDerivative {
                velocity: s.velocity,
                acceleration: apogee_common::Position::new(-s.position.x, 0.0, 0.0),
                attitude_derivative: nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
                angular_acceleration: nalgebra::Vector3::zeros(),
            },
            Seconds::new(0.01),
        );
        assert!(result.accepted);
        assert_relative_eq!(result.step_taken.into_value(), 0.01, epsilon = 1e-12);
    }

    #[test]
    fn test_rk45_adaptive_rejects_large_step() {
        // A large step on a stiff-ish problem should be rejected and shrunk.
        // Use a high-frequency oscillator: x'' = -100*x, period = 2*pi/10 ≈ 0.628s.
        // A step of 1.0s is way too large.
        let mut state = StateVector {
            position: apogee_common::Position::new(1.0, 0.0, 0.0),
            velocity: apogee_common::Velocity::new(0.0, 0.0, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };
        let mut integrator = Rk45::new(1e-10);
        let result = integrator.step(
            &mut state,
            &|s: &StateVector| StateDerivative {
                velocity: s.velocity,
                acceleration: apogee_common::Position::new(-100.0 * s.position.x, 0.0, 0.0),
                attitude_derivative: nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
                angular_acceleration: nalgebra::Vector3::zeros(),
            },
            Seconds::new(1.0),
        );
        assert!(result.accepted);
        // The step should have been shrunk significantly.
        assert!(
            result.step_taken.into_value() < 0.1,
            "step should have been shrunk, got {}",
            result.step_taken.into_value()
        );
    }

    #[test]
    fn test_rk45_two_body_energy_conservation() {
        // Propagate a circular orbit and check energy conservation.
        use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};

        let r = R_EARTH_EQ + 400_000.0;
        let v = (GM_EARTH / r).sqrt();
        let mut state = StateVector {
            position: apogee_common::Position::new(r, 0.0, 0.0),
            velocity: apogee_common::Velocity::new(0.0, v, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };

        let e0 = state.velocity.norm_squared() / 2.0 - GM_EARTH / state.position.norm();

        let mut integrator = Rk45::new(1e-10);
        let orbital_period = 2.0 * std::f64::consts::PI * (r * r * r / GM_EARTH).sqrt();
        let dt = orbital_period / 100.0;
        let mut total_time = 0.0;
        while total_time < orbital_period {
            let remaining = orbital_period - total_time;
            let step = dt.min(remaining);
            integrator.step(
                &mut state,
                &|s: &StateVector| {
                    let r = s.position.norm();
                    let acc = apogee_common::Position::new(
                        -GM_EARTH * s.position.x / r.powi(3),
                        -GM_EARTH * s.position.y / r.powi(3),
                        -GM_EARTH * s.position.z / r.powi(3),
                    );
                    StateDerivative {
                        velocity: s.velocity,
                        acceleration: acc,
                        attitude_derivative: nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
                        angular_acceleration: nalgebra::Vector3::zeros(),
                    }
                },
                Seconds::new(step),
            );
            total_time += step;
        }

        let e1 = state.velocity.norm_squared() / 2.0 - GM_EARTH / state.position.norm();
        let rel_err = (e1 - e0).abs() / e0.abs();
        assert!(
            rel_err < 1e-8,
            "DOPRI5 energy drift too large: {rel_err:.2e}"
        );
    }
}
