//! Chebyshev polynomial evaluator for ephemeris interpolation — stub.

/// Evaluate Chebyshev polynomials of the first kind at time `t` (normalized to [-1, 1]).
///
/// Returns coefficients for degree 0..N.
pub fn chebyshev_eval(_t: f64, _degree: usize) -> Vec<f64> {
    // TODO: Clenshaw recurrence
    Vec::new()
}
