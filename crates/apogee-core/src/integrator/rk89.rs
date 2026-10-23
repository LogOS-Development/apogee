//! RK8(5) adaptive integrator — Dormand-Prince 8(5,3).
//!
//! 12-stage embedded Runge-Kutta method with 8th-order solution and
//! 5th/3rd-order error estimates. Uses the DOP853 coefficients from Hairer,
//! Nørsett & Wanner. Designed for high-precision trajectory propagation where
//! energy conservation over long arcs is critical.
//!
//! # References
//!
//! - Hairer, Nørsett & Wanner (2008). "Solving Ordinary Differential
//!   Equations I: Nonstiff Problems," §II.5. Springer.
//!   Coefficients from dop853.f (Hairer's scientific computing repository).
//! - Prince, P.J. & Dormand, J.R. (1981). "High order embedded Runge-Kutta
//!   formulae." J. Comput. Appl. Math., 7(1), 67-75.
//!
//! # Energy Conservation
//!
//! For a circular two-body orbit, DOP853 with tight tolerance (1e-12) should
//! conserve energy to ΔE/E < 1e-10 per orbit — the target from issue #143.

// Coefficients are from the reference implementation with full precision.
use apogee_common::units::Seconds;

use super::{IntegrationResult, Integrator, StateDerivative, StateVector};

/// Dormand-Prince 8(5,3) adaptive integrator.
///
/// 12-stage method with 8th-order solution and embedded 5th/3rd-order
/// error estimates. Adaptive step size control with tolerance-based
/// acceptance/rejection.
#[derive(Debug, Clone)]
pub struct Rk89 {
    /// Relative tolerance for error acceptance.
    pub tolerance: f64,
    /// Minimum step size (seconds).
    pub min_step: f64,
    /// Maximum step size (seconds).
    pub max_step: f64,
    /// Safety factor for step size scaling.
    pub safety: f64,
    /// Fixed-step mode: take exactly `dt` without adaptive control.
    pub fixed_step: bool,
}

impl Default for Rk89 {
    fn default() -> Self {
        Self {
            tolerance: 1e-12,
            min_step: 1e-10,
            max_step: 1e3,
            safety: 0.9,
            fixed_step: false,
        }
    }
}

impl Rk89 {
    /// Create a new adaptive Rk89 with the given tolerance.
    pub fn new(tolerance: f64) -> Self {
        Self {
            tolerance,
            ..Default::default()
        }
    }

    /// Create a fixed-step Rk89 (no adaptive control).
    pub fn fixed() -> Self {
        Self {
            fixed_step: true,
            ..Default::default()
        }
    }
}

// DOP853 a-coefficients (stage derivatives). Node positions (c_i) are
// implicit in the a-coefficients and not stored separately.
const A2_1: f64 = 0.05260015195876773;
const A3_1: f64 = 0.0197250569845379;
const A3_2: f64 = 0.0591751709536137;
const A4_1: f64 = 0.02958758547680685;
const A4_3: f64 = 0.08876275643042054;
const A5_1: f64 = 0.2413651341592667;
const A5_3: f64 = -0.8845494793282861;
const A5_4: f64 = 0.924834003261792;
const A6_1: f64 = 0.037037037037037035;
const A6_4: f64 = 0.17082860872947386;
const A6_5: f64 = 0.12546768756682242;
const A7_1: f64 = 0.037109375;
const A7_4: f64 = 0.17025221101954405;
const A7_5: f64 = 0.06021653898045596;
const A7_6: f64 = -0.017578125;
const A8_1: f64 = 0.03709200011850479;
const A8_4: f64 = 0.17038392571223998;
const A8_5: f64 = 0.10726203044637328;
const A8_6: f64 = -0.015319437748624402;
const A8_7: f64 = 0.008273789163814023;
const A9_1: f64 = 0.6241109587160757;
const A9_4: f64 = -3.3608926294469414;
const A9_5: f64 = -0.868219346841726;
const A9_6: f64 = 27.59209969944671;
const A9_7: f64 = 20.154067550477894;
const A9_8: f64 = -43.48988418106996;
const A10_1: f64 = 0.47766253643826434;
const A10_4: f64 = -2.4881146199716677;
const A10_5: f64 = -0.590290826836843;
const A10_6: f64 = 21.230051448181193;
const A10_7: f64 = 15.279233632882423;
const A10_8: f64 = -33.28821096898486;
const A10_9: f64 = -0.020331201708508627;
const A11_1: f64 = -0.9371424300859873;
const A11_4: f64 = 5.186372428844064;
const A11_5: f64 = 1.0914373489967295;
const A11_6: f64 = -8.149787010746927;
const A11_7: f64 = -18.52006565999696;
const A11_8: f64 = 22.739487099350505;
const A11_9: f64 = 2.4936055526796523;
const A11_10: f64 = -3.0467644718982196;
const A12_1: f64 = 2.273310147516538;
const A12_4: f64 = -10.53449546673725;
const A12_5: f64 = -2.0008720582248625;
const A12_6: f64 = -17.9589318631188;
const A12_7: f64 = 27.94888452941996;
const A12_8: f64 = -2.8589982771350235;
const A12_9: f64 = -8.87285693353063;
const A12_10: f64 = 12.360567175794303;
const A12_11: f64 = 0.6433927460157636;

// DOP853 5th-order error estimate coefficients (E5).
const E5_0: f64 = 0.01312004499419488;
const E5_5: f64 = -1.2251564463762044;
const E5_6: f64 = -0.4957589496572502;
const E5_7: f64 = 1.6643771824549864;
const E5_8: f64 = -0.35032884874997366;
const E5_9: f64 = 0.3341791187130175;
const E5_10: f64 = 0.08192320648511571;
const E5_11: f64 = -0.022355307863886294;

// DOP853 8th-order solution weights (b); b2..b5 are zero.
// Source: sources/dop853.f parameter block (Hairer & Wanner).
const B_1: f64 = 0.054293734116568765;
const B_6: f64 = 4.450312892752409;
const B_7: f64 = 1.8915178993145003;
const B_8: f64 = -5.801203960010585;
const B_9: f64 = 0.3111643669578199;
const B_10: f64 = -0.1521609496625161;
const B_11: f64 = 0.20136540080403034;
const B_12: f64 = 0.04471061572777259;

impl Integrator for Rk89 {
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
            let err = dop853_step(state, derivative_fn, h);
            return IntegrationResult {
                accepted: true,
                error_estimate: err,
                step_taken: dt,
            };
        }

        let mut current_h = h_abs.min(self.max_step);
        let mut iterations = 0;
        const MAX_ITER: usize = 100;

        loop {
            iterations += 1;
            let trial_h = sign * current_h;
            let mut trial_state = state.clone();
            let err = dop853_step(&mut trial_state, derivative_fn, trial_h);

            let err_norm = if err.is_finite() && err > 0.0 {
                err / self.tolerance
            } else {
                0.0
            };

            if err_norm <= 1.0 || current_h <= self.min_step || iterations >= MAX_ITER {
                *state = trial_state;
                return IntegrationResult {
                    accepted: true,
                    error_estimate: err,
                    step_taken: Seconds::new(trial_h),
                };
            }

            // Reject and shrink: h_new = h * safety * (1/err)^(1/8)
            let factor = self.safety * err_norm.powf(-1.0 / 8.0);
            current_h = (current_h * factor).max(self.min_step).min(self.max_step);
        }
    }
}

/// Take one DOP853 step of size `h`, returning the scalar error estimate.
fn dop853_step(
    state: &mut StateVector,
    derivative_fn: &dyn Fn(&StateVector) -> StateDerivative,
    h: f64,
) -> f64 {
    let s0 = state.clone();
    let k1 = derivative_fn(&s0);

    // Stage 2
    let mut s2 = s0.clone();
    s2.position += k1.velocity * (A2_1 * h);
    s2.velocity += k1.acceleration * (A2_1 * h);
    let k2 = derivative_fn(&s2);

    // Stage 3
    let mut s3 = s0.clone();
    s3.position += k1.velocity * (A3_1 * h) + k2.velocity * (A3_2 * h);
    s3.velocity += k1.acceleration * (A3_1 * h) + k2.acceleration * (A3_2 * h);
    let k3 = derivative_fn(&s3);

    // Stage 4
    let mut s4 = s0.clone();
    s4.position += k1.velocity * (A4_1 * h) + k3.velocity * (A4_3 * h);
    s4.velocity += k1.acceleration * (A4_1 * h) + k3.acceleration * (A4_3 * h);
    let k4 = derivative_fn(&s4);

    // Stage 5
    let mut s5 = s0.clone();
    s5.position += k1.velocity * (A5_1 * h) + k3.velocity * (A5_3 * h) + k4.velocity * (A5_4 * h);
    s5.velocity +=
        k1.acceleration * (A5_1 * h) + k3.acceleration * (A5_3 * h) + k4.acceleration * (A5_4 * h);
    let k5 = derivative_fn(&s5);

    // Stage 6
    let mut s6 = s0.clone();
    s6.position += k1.velocity * (A6_1 * h) + k4.velocity * (A6_4 * h) + k5.velocity * (A6_5 * h);
    s6.velocity +=
        k1.acceleration * (A6_1 * h) + k4.acceleration * (A6_4 * h) + k5.acceleration * (A6_5 * h);
    let k6 = derivative_fn(&s6);

    // Stage 7
    let mut s7 = s0.clone();
    s7.position += k1.velocity * (A7_1 * h)
        + k4.velocity * (A7_4 * h)
        + k5.velocity * (A7_5 * h)
        + k6.velocity * (A7_6 * h);
    s7.velocity += k1.acceleration * (A7_1 * h)
        + k4.acceleration * (A7_4 * h)
        + k5.acceleration * (A7_5 * h)
        + k6.acceleration * (A7_6 * h);
    let k7 = derivative_fn(&s7);

    // Stage 8
    let mut s8 = s0.clone();
    s8.position += k1.velocity * (A8_1 * h)
        + k4.velocity * (A8_4 * h)
        + k5.velocity * (A8_5 * h)
        + k6.velocity * (A8_6 * h)
        + k7.velocity * (A8_7 * h);
    s8.velocity += k1.acceleration * (A8_1 * h)
        + k4.acceleration * (A8_4 * h)
        + k5.acceleration * (A8_5 * h)
        + k6.acceleration * (A8_6 * h)
        + k7.acceleration * (A8_7 * h);
    let k8 = derivative_fn(&s8);

    // Stage 9
    let mut s9 = s0.clone();
    s9.position += k1.velocity * (A9_1 * h)
        + k4.velocity * (A9_4 * h)
        + k5.velocity * (A9_5 * h)
        + k6.velocity * (A9_6 * h)
        + k7.velocity * (A9_7 * h)
        + k8.velocity * (A9_8 * h);
    s9.velocity += k1.acceleration * (A9_1 * h)
        + k4.acceleration * (A9_4 * h)
        + k5.acceleration * (A9_5 * h)
        + k6.acceleration * (A9_6 * h)
        + k7.acceleration * (A9_7 * h)
        + k8.acceleration * (A9_8 * h);
    let k9 = derivative_fn(&s9);

    // Stage 10
    let mut s10 = s0.clone();
    s10.position += k1.velocity * (A10_1 * h)
        + k4.velocity * (A10_4 * h)
        + k5.velocity * (A10_5 * h)
        + k6.velocity * (A10_6 * h)
        + k7.velocity * (A10_7 * h)
        + k8.velocity * (A10_8 * h)
        + k9.velocity * (A10_9 * h);
    s10.velocity += k1.acceleration * (A10_1 * h)
        + k4.acceleration * (A10_4 * h)
        + k5.acceleration * (A10_5 * h)
        + k6.acceleration * (A10_6 * h)
        + k7.acceleration * (A10_7 * h)
        + k8.acceleration * (A10_8 * h)
        + k9.acceleration * (A10_9 * h);
    let k10 = derivative_fn(&s10);

    // Stage 11
    let mut s11 = s0.clone();
    s11.position += k1.velocity * (A11_1 * h)
        + k4.velocity * (A11_4 * h)
        + k5.velocity * (A11_5 * h)
        + k6.velocity * (A11_6 * h)
        + k7.velocity * (A11_7 * h)
        + k8.velocity * (A11_8 * h)
        + k9.velocity * (A11_9 * h)
        + k10.velocity * (A11_10 * h);
    s11.velocity += k1.acceleration * (A11_1 * h)
        + k4.acceleration * (A11_4 * h)
        + k5.acceleration * (A11_5 * h)
        + k6.acceleration * (A11_6 * h)
        + k7.acceleration * (A11_7 * h)
        + k8.acceleration * (A11_8 * h)
        + k9.acceleration * (A11_9 * h)
        + k10.acceleration * (A11_10 * h);
    let k11 = derivative_fn(&s11);

    // Stage 12 — its derivative k12 feeds the error estimate below.
    let mut s12 = s0.clone();
    s12.position += k1.velocity * (A12_1 * h)
        + k4.velocity * (A12_4 * h)
        + k5.velocity * (A12_5 * h)
        + k6.velocity * (A12_6 * h)
        + k7.velocity * (A12_7 * h)
        + k8.velocity * (A12_8 * h)
        + k9.velocity * (A12_9 * h)
        + k10.velocity * (A12_10 * h)
        + k11.velocity * (A12_11 * h);
    s12.velocity += k1.acceleration * (A12_1 * h)
        + k4.acceleration * (A12_4 * h)
        + k5.acceleration * (A12_5 * h)
        + k6.acceleration * (A12_6 * h)
        + k7.acceleration * (A12_7 * h)
        + k8.acceleration * (A12_8 * h)
        + k9.acceleration * (A12_9 * h)
        + k10.acceleration * (A12_10 * h)
        + k11.acceleration * (A12_11 * h);
    let k12 = derivative_fn(&s12);

    // 8th-order solution: y_new = y0 + h * sum(b_i * k_i) with Hairer's b
    // weights (b2..b5 are zero). NOT the stage-12 argument: DOP853 is not
    // FSAL, the a12 row only places the k12 node (sources/dop853.f:649).
    state.position += k1.velocity * (B_1 * h)
        + k6.velocity * (B_6 * h)
        + k7.velocity * (B_7 * h)
        + k8.velocity * (B_8 * h)
        + k9.velocity * (B_9 * h)
        + k10.velocity * (B_10 * h)
        + k11.velocity * (B_11 * h)
        + k12.velocity * (B_12 * h);
    state.velocity += k1.acceleration * (B_1 * h)
        + k6.acceleration * (B_6 * h)
        + k7.acceleration * (B_7 * h)
        + k8.acceleration * (B_8 * h)
        + k9.acceleration * (B_9 * h)
        + k10.acceleration * (B_10 * h)
        + k11.acceleration * (B_11 * h)
        + k12.acceleration * (B_12 * h);

    // Error estimate: E5_i * k_i.
    let err_pos = k1.velocity * (E5_0 * h)
        + k6.velocity * (E5_5 * h)
        + k7.velocity * (E5_6 * h)
        + k8.velocity * (E5_7 * h)
        + k9.velocity * (E5_8 * h)
        + k10.velocity * (E5_9 * h)
        + k11.velocity * (E5_10 * h)
        + k12.velocity * (E5_11 * h);
    let err_vel = k1.acceleration * (E5_0 * h)
        + k6.acceleration * (E5_5 * h)
        + k7.acceleration * (E5_6 * h)
        + k8.acceleration * (E5_7 * h)
        + k9.acceleration * (E5_8 * h)
        + k10.acceleration * (E5_9 * h)
        + k11.acceleration * (E5_10 * h)
        + k12.acceleration * (E5_11 * h);

    err_pos.norm().max(err_vel.norm()) / h.abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrator::tableau_sources::{
        assert_convergence_order, fortran_params, harmonic_derivative,
    };
    use approx::assert_relative_eq;

    #[test]
    fn test_rk89_global_convergence_order_is_eight() {
        assert_convergence_order(|| Box::new(Rk89::fixed()), 0.4, 8, 6.0);
    }

    #[test]
    fn test_rk89_tableau_matches_dop853_source() {
        let f = fortran_params("dop853.f");
        let rust: &[(&str, f64)] = &[
            ("a21", A2_1),
            ("a31", A3_1),
            ("a32", A3_2),
            ("a41", A4_1),
            ("a43", A4_3),
            ("a51", A5_1),
            ("a53", A5_3),
            ("a54", A5_4),
            ("a61", A6_1),
            ("a64", A6_4),
            ("a65", A6_5),
            ("a71", A7_1),
            ("a74", A7_4),
            ("a75", A7_5),
            ("a76", A7_6),
            ("a81", A8_1),
            ("a84", A8_4),
            ("a85", A8_5),
            ("a86", A8_6),
            ("a87", A8_7),
            ("a91", A9_1),
            ("a94", A9_4),
            ("a95", A9_5),
            ("a96", A9_6),
            ("a97", A9_7),
            ("a98", A9_8),
            ("a101", A10_1),
            ("a104", A10_4),
            ("a105", A10_5),
            ("a106", A10_6),
            ("a107", A10_7),
            ("a108", A10_8),
            ("a109", A10_9),
            ("a111", A11_1),
            ("a114", A11_4),
            ("a115", A11_5),
            ("a116", A11_6),
            ("a117", A11_7),
            ("a118", A11_8),
            ("a119", A11_9),
            ("a1110", A11_10),
            ("a121", A12_1),
            ("a124", A12_4),
            ("a125", A12_5),
            ("a126", A12_6),
            ("a127", A12_7),
            ("a128", A12_8),
            ("a129", A12_9),
            ("a1210", A12_10),
            ("a1211", A12_11),
            ("b1", B_1),
            ("b6", B_6),
            ("b7", B_7),
            ("b8", B_8),
            ("b9", B_9),
            ("b10", B_10),
            ("b11", B_11),
            ("b12", B_12),
            ("er1", E5_0),
            ("er6", E5_5),
            ("er7", E5_6),
            ("er8", E5_7),
            ("er9", E5_8),
            ("er10", E5_9),
            ("er11", E5_10),
            ("er12", E5_11),
        ];
        for (name, value) in rust {
            let fort = f
                .get(*name)
                .unwrap_or_else(|| panic!("dop853.f lacks {name}"));
            // Same decimal literal on both sides: a correctly-rounded parse
            // of the source digits must equal the Rust constant bit-exactly.
            assert!(value == fort, "{name}: rust {value} vs dop853.f {fort}");
        }
    }

    #[test]
    fn test_rk89_tableau_consistency() {
        let f = fortran_params("dop853.f");
        // Each stage argument sits at its node: sum_j a_ij = c_i.
        let rows: &[(f64, &str)] = &[
            (A2_1, "c2"),
            (A3_1 + A3_2, "c3"),
            (A4_1 + A4_3, "c4"),
            (A5_1 + A5_3 + A5_4, "c5"),
            (A6_1 + A6_4 + A6_5, "c6"),
            (A7_1 + A7_4 + A7_5 + A7_6, "c7"),
            (A8_1 + A8_4 + A8_5 + A8_6 + A8_7, "c8"),
            (A9_1 + A9_4 + A9_5 + A9_6 + A9_7 + A9_8, "c9"),
            (A10_1 + A10_4 + A10_5 + A10_6 + A10_7 + A10_8 + A10_9, "c10"),
            (
                A11_1 + A11_4 + A11_5 + A11_6 + A11_7 + A11_8 + A11_9 + A11_10,
                "c11",
            ),
        ];
        for (sum, cname) in rows {
            let c = f
                .get(*cname)
                .unwrap_or_else(|| panic!("dop853.f lacks {cname}"));
            assert!((sum - c).abs() < 1e-12, "{cname}: row sum {sum}");
        }
        // Stage 12 sits at t + h (c12 = 1, implicit in the source).
        let s12_sum = A12_1 + A12_4 + A12_5 + A12_6 + A12_7 + A12_8 + A12_9 + A12_10 + A12_11;
        assert!((s12_sum - 1.0).abs() < 1e-12, "stage 12 node: {s12_sum}");
        // Quadrature condition: b weights sum to 1.
        let b_sum = B_1 + B_6 + B_7 + B_8 + B_9 + B_10 + B_11 + B_12;
        assert!((b_sum - 1.0).abs() < 1e-12, "b-row sum: {b_sum}");
        // Error row = b(5th) - b(3rd); both sum to 1, so er sums to 0.
        let er_sum = E5_0 + E5_5 + E5_6 + E5_7 + E5_8 + E5_9 + E5_10 + E5_11;
        assert!(er_sum.abs() < 1e-12, "er-row sum: {er_sum}");
    }

    #[test]
    fn test_rk89_fixed_mode_takes_exact_step() {
        let mut integ = Rk89::fixed();
        let mut state = StateVector {
            position: apogee_common::Position::new(1.0, 0.0, 0.0),
            velocity: apogee_common::Velocity::new(0.0, 0.0, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };
        let result = integ.step(&mut state, &harmonic_derivative, Seconds::new(0.4));
        assert!(result.accepted);
        assert!((result.step_taken.into_value() - 0.4).abs() < 1e-15);
    }

    #[test]
    fn test_rk89_harmonic_oscillator() {
        let mut state = StateVector {
            position: apogee_common::Position::new(1.0, 0.0, 0.0),
            velocity: apogee_common::Velocity::new(0.0, 0.0, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };
        let mut integrator = Rk89::new(1e-12);
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

        assert_relative_eq!(state.position.x, t_final.cos(), epsilon = 1e-8);
        assert_relative_eq!(state.velocity.x, -t_final.sin(), epsilon = 1e-8);
    }

    #[test]
    fn test_rk89_adaptive_accepts_good_step() {
        let mut state = StateVector {
            position: apogee_common::Position::new(1.0, 0.0, 0.0),
            velocity: apogee_common::Velocity::new(0.0, 0.0, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };
        let mut integrator = Rk89::new(1e-10);
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
    }

    #[test]
    fn test_rk89_two_body_energy_conservation() {
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

        let mut integrator = Rk89::new(1e-12);
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
            rel_err < 1e-10,
            "DOP853 energy drift too large: {rel_err:.2e}"
        );
    }
}
