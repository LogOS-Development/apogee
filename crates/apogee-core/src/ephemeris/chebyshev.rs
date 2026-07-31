//! Chebyshev polynomial evaluator for ephemeris interpolation.
//!
//! SPICE Type 2/3 ephemeris segments store position (and optionally velocity)
//! as coefficients of Chebyshev polynomials of the first kind. This module
//! implements stable evaluation via Clenshaw recurrence and a companion
//! recurrence for the derivative.
//!
//! # Chebyshev polynomials of the first kind
//!
//! T_0(x) = 1
//! T_1(x) = x
//! T_{n+1}(x) = 2 x T_n(x) - T_{n-1}(x)
//!
//! A function represented by coefficients `c[0..N]` is evaluated as
//!
//!   f(x) = Σ c_n T_n(x)
//!
//! over the normalized domain x ∈ [-1, 1]. For ephemeris use, physical time
//! is mapped to x by
//!
//!   x = (t - mid) / radius
//!
//! where `mid` and `radius` define the segment interval
//! [mid - radius, mid + radius].
//!
//! # References
//!
//! - NAIF/SPICE SPK required reading, "SPK File Format"
//!   <https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/spk.html>
//! - Newhall, X X (1989), "The Numerical Representation of Planets and
//!   Satellites", JPL IOM 89-032
//! - Clenshaw, C.W. (1955), "A note on the summation of Chebyshev series",
//!   Math. Tables Aids Comput. 9(51), 118-120
//!   <https://doi.org/10.1090/S0025-5718-1955-0071856-0>
//! - Fox, L. & Parker, I.B. (1968), "Chebyshev Polynomials in Numerical
//!   Analysis", Oxford University Press, Ch. 4
//!
//! Clenshaw recurrence stability:
//! - Oliver, J. (1977), "An error analysis of the modified Clenshaw method
//!   for evaluating Chebyshev and Fourier series", IMA J. Appl. Math. 20(3)
//!   <https://doi.org/10.1093/imamat/20.3.379>

/// Evaluate a Chebyshev series at `x` using Clenshaw recurrence.
///
/// `coefficients` are ordered from degree 0 to degree N. `x` must be in
/// the normalized interval `[-1, 1]`.
///
/// # Algorithm
///
/// Clenshaw backward recurrence:
///
/// ```text
/// b_N+2 = b_N+1 = 0
/// b_k = c_k + 2 x b_{k+1} - b_{k+2}   for k = N .. 1
/// f(x) = c_0 + x b_1 - b_2
/// ```
///
/// # Complexity
///
/// Time: O(N), space: O(1).
///
/// # Panics
///
/// Panics if `coefficients` is empty (no series to evaluate).
pub fn chebyshev_eval(x: f64, coefficients: &[f64]) -> f64 {
    let n = coefficients.len();
    assert!(n > 0, "chebyshev_eval: coefficient slice must be non-empty");

    // Handle low-degree cases explicitly to avoid the recurrence overhead
    // and to make the math obvious.
    match n {
        1 => return coefficients[0],
        2 => return coefficients[0] + x * coefficients[1],
        _ => {}
    }

    let mut b_kp2 = 0.0; // b_{k+2}
    let mut b_kp1 = 0.0; // b_{k+1}

    for k in (1..n).rev() {
        let b_k = coefficients[k] + 2.0 * x * b_kp1 - b_kp2;
        b_kp2 = b_kp1;
        b_kp1 = b_k;
    }

    // Final step: f(x) = c_0 + x b_1 - b_2
    coefficients[0] + x * b_kp1 - b_kp2
}

/// Evaluate the derivative of a Chebyshev series at `x`.
///
/// The derivative coefficients `d_k` of a Chebyshev series with
/// coefficients `c_k` satisfy (from Fox & Parker, Ch. 4):
///
/// ```text
/// d_{N-1} = 2 N c_N
/// d_k = 2 (k + 1) c_{k+1} + (k + 1) / (k + 2) * d_{k+2}
///         for k = N-2 .. 1
/// d_0 = c_1 + d_2 / 2
/// ```
///
/// with `d_{N+1} = 0`. Then `f'(x)` is evaluated from `d_k` using the same
/// Clenshaw recurrence as [`chebyshev_eval`].
///
/// # Complexity
///
/// Time: O(N), space: O(N) for the temporary derivative-coefficient slice.
///
/// # Panics
///
/// Panics if `coefficients` is empty.
pub fn chebyshev_derivative(x: f64, coefficients: &[f64]) -> f64 {
    let n = coefficients.len();
    assert!(
        n > 0,
        "chebyshev_derivative: coefficient slice must be non-empty"
    );

    if n == 1 {
        return 0.0; // constant series
    }

    if n == 2 {
        // f(x) = c_0 + c_1 x  =>  f'(x) = c_1
        return coefficients[1];
    }

    // Compute derivative coefficients d_k for the no-half convention
    // f(x) = Σ c_k T_k(x). The recurrence is:
    //   d_N = d_{N+1} = 0
    //   d_{k-1} = 2 k c_k + d_{k+1}     for k = N .. 2
    //   d_0 = c_1 + d_2 / 2
    let mut d = vec![0.0; n + 1];

    for k in (2..=n - 1).rev() {
        d[k - 1] = 2.0 * k as f64 * coefficients[k] + d[k + 1];
    }

    d[0] = coefficients[1] + 0.5 * d[2];

    chebyshev_eval(x, &d[..n])
}

/// Evaluate position and velocity from a Chebyshev coefficient set.
///
/// SPK Type 3 stores three sets of coefficients, one per coordinate
/// (x, y, z). `coefficients` must be a slice of three equal-length slices,
/// each normalized to the same time interval. The returned velocity is in
/// the same physical units as position per unit normalized time; multiply
/// by `2.0 / radius` to convert to physical units if `x` was obtained from
/// a physical time interval.
///
/// # Panics
///
/// Panics if the three coordinate coefficient slices do not have the same
/// length.
pub fn chebyshev_eval_3d(
    x: f64,
    coefficients: [&[f64]; 3],
) -> (nalgebra::Vector3<f64>, nalgebra::Vector3<f64>) {
    let len = coefficients[0].len();
    assert!(
        coefficients[1].len() == len && coefficients[2].len() == len,
        "chebyshev_eval_3d: all three coordinate coefficient slices must have the same length"
    );

    let px = chebyshev_eval(x, coefficients[0]);
    let py = chebyshev_eval(x, coefficients[1]);
    let pz = chebyshev_eval(x, coefficients[2]);

    let vx = chebyshev_derivative(x, coefficients[0]);
    let vy = chebyshev_derivative(x, coefficients[1]);
    let vz = chebyshev_derivative(x, coefficients[2]);

    (
        nalgebra::Vector3::new(px, py, pz),
        nalgebra::Vector3::new(vx, vy, vz),
    )
}

/// Map a physical time to the normalized Chebyshev domain `[-1, 1]`.
///
/// `mid` and `radius` define the interval `[mid - radius, mid + radius]`.
/// Values outside this interval are still mapped linearly; callers should
/// ensure the input belongs to the intended segment.
pub fn normalized_time(t: f64, mid: f64, radius: f64) -> f64 {
    assert!(radius > 0.0, "normalized_time: radius must be positive");
    (t - mid) / radius
}

/// Compute the value of the Chebyshev polynomial T_n(x) directly.
///
/// This is primarily useful for tests and diagnostics; for series summation
/// prefer [`chebyshev_eval`]. Uses the trigonometric form
/// `T_n(x) = cos(n * arccos(x))` for `|x| ≤ 1` and the hyperbolic form
/// otherwise.
pub fn chebyshev_polynomial(n: usize, x: f64) -> f64 {
    if x.abs() <= 1.0 {
        (n as f64 * x.acos()).cos()
    } else {
        let sign: f64 = if x > 0.0 { 1.0 } else { -1.0 };
        let exp = (x.abs() + (x * x - 1.0).sqrt()).acosh();
        sign.powi(n as i32) * (n as f64 * exp).cosh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_t0_is_one() {
        assert_relative_eq!(chebyshev_polynomial(0, 0.5), 1.0, epsilon = 1e-12);
        assert_relative_eq!(chebyshev_polynomial(0, -0.5), 1.0, epsilon = 1e-12);
        assert_relative_eq!(chebyshev_polynomial(0, 2.0), 1.0, epsilon = 1e-12);
    }

    #[test]
    fn test_t1_is_x() {
        for x in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            assert_relative_eq!(chebyshev_polynomial(1, x), x, epsilon = 1e-12);
        }
    }

    #[test]
    fn test_t3_at_half() {
        // T_3(x) = 4x^3 - 3x. T_3(0.5) = 4*(1/8) - 3/2 = -1.0.
        assert_relative_eq!(chebyshev_polynomial(3, 0.5), -1.0, epsilon = 1e-12);
    }

    #[test]
    fn test_constant_series() {
        let c = [7.0];
        assert_relative_eq!(chebyshev_eval(0.5, &c), 7.0, epsilon = 1e-12);
    }

    #[test]
    fn test_linear_series() {
        // f(x) = 2 + 3 x
        let c = [2.0, 3.0];
        assert_relative_eq!(chebyshev_eval(0.5, &c), 3.5, epsilon = 1e-12);
        assert_relative_eq!(chebyshev_eval(-0.5, &c), 0.5, epsilon = 1e-12);
    }

    #[test]
    fn test_quadratic_series() {
        // f(x) = 1 + 2 T_1(x) + 3 T_2(x) = 1 + 2x + 3(2x^2-1)
        //      = -2 + 2x + 6x^2
        let c = [1.0, 2.0, 3.0];
        let x = 0.25;
        let expected = -2.0 + 2.0 * x + 6.0 * x * x;
        assert_relative_eq!(chebyshev_eval(x, &c), expected, epsilon = 1e-12);
    }

    #[test]
    fn test_cubic_series_against_polynomial() {
        // f(x) = 1 + 2x + 3(2x^2-1) + 4(4x^3-3x)
        //      = -2 - 10x + 6x^2 + 16x^3
        let c = [1.0, 2.0, 3.0, 4.0];
        let x = -0.6;
        let expected = -2.0 - 10.0 * x + 6.0 * x * x + 16.0 * x * x * x;
        assert_relative_eq!(chebyshev_eval(x, &c), expected, epsilon = 1e-12);
    }

    #[test]
    fn test_derivative_of_linear_series() {
        // f(x) = 2 + 3x -> f'(x) = 3
        let c = [2.0, 3.0];
        for x in [-1.0, 0.0, 1.0] {
            assert_relative_eq!(chebyshev_derivative(x, &c), 3.0, epsilon = 1e-12);
        }
    }

    #[test]
    fn test_derivative_of_quadratic_series() {
        // f(x) = 1 + 2x + 3(2x^2-1) = -2 + 2x + 6x^2
        // f'(x) = 2 + 12x
        let c = [1.0, 2.0, 3.0];
        let x = 0.25;
        assert_relative_eq!(chebyshev_derivative(x, &c), 2.0 + 12.0 * x, epsilon = 1e-12);
    }

    #[test]
    fn test_derivative_of_cubic_series() {
        // f(x) = -2 - 10x + 6x^2 + 16x^3
        // f'(x) = -10 + 12x + 48x^2
        let c = [1.0, 2.0, 3.0, 4.0];
        let x = -0.6;
        let expected = -10.0 + 12.0 * x + 48.0 * x * x;
        assert_relative_eq!(chebyshev_derivative(x, &c), expected, epsilon = 1e-12);
    }

    #[test]
    fn test_higher_degree_derivative() {
        // f(x) = T_5(x) = 16x^5 - 20x^3 + 5x
        // f'(x) = 80x^4 - 60x^2 + 5
        let c = [0.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let x: f64 = 0.7;
        let expected = 80.0 * x.powi(4) - 60.0 * x.powi(2) + 5.0;
        assert_relative_eq!(chebyshev_derivative(x, &c), expected, epsilon = 1e-12);
    }

    #[test]
    fn test_eval_3d_matches_individual_coordinates() {
        let cx = [1.0, 0.5, -0.25];
        let cy = [2.0, -1.0, 0.0];
        let cz = [0.0, 1.0, 1.0];
        let x = 0.3;

        let (pos, vel) = chebyshev_eval_3d(x, [&cx, &cy, &cz]);

        assert_relative_eq!(pos.x, chebyshev_eval(x, &cx), epsilon = 1e-12);
        assert_relative_eq!(pos.y, chebyshev_eval(x, &cy), epsilon = 1e-12);
        assert_relative_eq!(pos.z, chebyshev_eval(x, &cz), epsilon = 1e-12);

        assert_relative_eq!(vel.x, chebyshev_derivative(x, &cx), epsilon = 1e-12);
        assert_relative_eq!(vel.y, chebyshev_derivative(x, &cy), epsilon = 1e-12);
        assert_relative_eq!(vel.z, chebyshev_derivative(x, &cz), epsilon = 1e-12);
    }

    #[test]
    fn test_normalized_time_maps_interval_to_pm1() {
        let mid = 10.0;
        let radius = 2.0;
        assert_relative_eq!(normalized_time(8.0, mid, radius), -1.0, epsilon = 1e-12);
        assert_relative_eq!(normalized_time(10.0, mid, radius), 0.0, epsilon = 1e-12);
        assert_relative_eq!(normalized_time(12.0, mid, radius), 1.0, epsilon = 1e-12);
    }

    #[test]
    fn test_clenshaw_matches_direct_polynomials() {
        // For a range of degrees, build coefficients that represent a known
        // monomial expressed in Chebyshev basis and compare evaluations.
        for x in [-0.95, -0.5, 0.0, 0.33, 0.99] {
            // x^4 = (3 T_0 + 4 T_2 + T_4) / 8
            let c4 = [3.0 / 8.0, 0.0, 4.0 / 8.0, 0.0, 1.0 / 8.0];
            assert_relative_eq!(chebyshev_eval(x, &c4), x.powi(4), epsilon = 1e-12);

            // x^5 = (10 T_1 + 5 T_3 + T_5) / 16
            let c5 = [0.0, 10.0 / 16.0, 0.0, 5.0 / 16.0, 0.0, 1.0 / 16.0];
            assert_relative_eq!(chebyshev_eval(x, &c5), x.powi(5), epsilon = 1e-12);
        }
    }

    #[test]
    fn test_derivative_of_monomial_series() {
        // x^4 = (3 T_0 + 4 T_2 + T_4) / 8  =>  derivative = 4 x^3
        let c4 = [3.0 / 8.0, 0.0, 4.0 / 8.0, 0.0, 1.0 / 8.0];
        let x: f64 = -0.4;
        assert_relative_eq!(
            chebyshev_derivative(x, &c4),
            4.0 * x.powi(3),
            epsilon = 1e-12
        );
    }
}
