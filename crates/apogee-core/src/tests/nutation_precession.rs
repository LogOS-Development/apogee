//! NutationPrecessionModel tests — IAU 2000/2006.
//!
//! Tests the frame bias, precession (IAU 2006), and nutation (IAU 2000B)
//! matrices, and their composition into the full GCRF-to-true-of-date
//! transformation.
//!
//! # Test Sources
//!
//! IAU 2000 nutation model:
//! - Mathews, P.M., Herring, T.A., Buffett, B.A. (2002), "Modeling of
//!   nutation and precession: New nutation series for nonrigid Earth and
//!   insights into Earth's interior", J. Geophys. Res. 107(B4)
//!   https://ui.adsabs.harvard.edu/abs/2002JGRB..107.2068M
//! - IERS Conventions (2010), Ch. 5, §5.6: Nutation
//!   https://iers-conventions.obspm.fr/content/tn36.pdf
//!
//! IAU 2006 precession:
//! - Capitaine, N., Wallace, P.T., Chapront, J. (2003), "Expressions for
//!   IAU 2000 precession quantities", Astron. Astrophys. 412, 567-586
//!   https://ui.adsabs.harvard.edu/abs/2003A%26A...412..567C
//! - IERS Conventions (2010), Ch. 5, §5.5: Precession
//!   https://iers-conventions.obspm.fr/content/tn36.pdf
//!
//! Frame bias:
//! - IERS Conventions (2010), §5.2.4: Frame bias
//!   https://iers-conventions.obspm.fr/content/tn36.pdf
//!
//! Validation reference values:
//! - SOFA/ERFA library test cases (iauBp06, iauNut00b, iauP06e)
//!   http://iausofa.org/2024_0529_C/sofa/manual.pdf
//! - USNO/UKNO Explanatory Supplement to the Astronomical Almanac (2013), Ch. 3
//!   https://aa.usno.navy.mil/publications/exp_supp
//!
//! Fundamental arguments (Delauany variables) at J2000:
//! - Simon, J.L., Bretagnon, P., Chapront, J., et al. (1994),
//!   "Numerical Expressions for Precession Formulae and Mean Elements
//!   for the Moon and Planets", Astron. Astrophys. 282, 663-683
//!   https://ui.adsabs.harvard.edu/abs/1994A%26A...282..663S

use crate::frames::nutation_precession::NutationPrecessionModel;
use hifitime::{Epoch, TimeScale};
use nalgebra::{Matrix3, Vector3};

/// Tolerance for arcsecond-level comparisons (1 arcsec in radians).
const ARCSEC: f64 = 4.84813681109536e-6;

/// Tolerance for milliarcsecond comparisons (1 mas in radians).
const MAS: f64 = 4.84813681109536e-9;

/// J2000 epoch: 2000-01-01 12:00:00 TDB
fn j2000() -> Epoch {
    Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, TimeScale::TDB)
}

/// Epoch 2025-01-01 00:00:00 TDB (for non-trivial precession/nutation)
fn epoch_2025() -> Epoch {
    Epoch::from_gregorian(2025, 1, 1, 0, 0, 0, 0, TimeScale::TDB)
}

// ═══════════════════════════════════════════════
// FRAME BIAS TESTS
// ═══════════════════════════════════════════════

#[test]
fn test_frame_bias_is_small_rotation() {
    // The frame bias between GCRF and J2000 mean equator is sub-arcsecond.
    // The bias matrix should be close to identity.
    let model = NutationPrecessionModel::new();
    let bias = model.frame_bias_matrix();

    let identity = Matrix3::identity();
    let diff = (bias - identity).norm();

    // Frame bias is ~0.02 mas = ~1e-10 rad. Check it's small.
    assert!(
        diff < 1e-6,
        "Frame bias should be close to identity, diff = {diff:e}"
    );
}

#[test]
fn test_frame_bias_orthogonal() {
    // The bias matrix must be orthogonal: B * B^T = I
    let model = NutationPrecessionModel::new();
    let bias = model.frame_bias_matrix();
    let product = bias * bias.transpose();
    let identity = Matrix3::identity();
    let diff = (product - identity).norm();
    assert!(
        diff < 1e-10,
        "Frame bias must be orthogonal, B*B^T - I norm = {diff:e}"
    );
}

// ═══════════════════════════════════════════════
// PRECESSION TESTS (IAU 2006)
// ═══════════════════════════════════════════════

#[test]
fn test_precession_at_j2000_is_identity() {
    // At J2000.0, precession should be identity (no precession from J2000 to J2000).
    let model = NutationPrecessionModel::new();
    let p = model.precession_matrix(j2000());
    let identity = Matrix3::identity();
    let diff = (p - identity).norm();
    assert!(
        diff < 1e-10,
        "Precession at J2000 should be identity, diff = {diff:e}"
    );
}

#[test]
fn test_precession_2025_z_rotation_dominant() {
    // Precession from J2000 to 2025 is primarily a z-axis rotation
    // (precession of equinox along the ecliptic).
    // The dominant effect is ~0.27 degrees over 25 years.
    let model = NutationPrecessionModel::new();
    let p = model.precession_matrix(epoch_2025());

    // Check that the matrix is orthogonal
    let product = p * p.transpose();
    let identity = Matrix3::identity();
    let diff = (product - identity).norm();
    assert!(
        diff < 1e-10,
        "Precession matrix must be orthogonal, diff = {diff:e}"
    );

    // The z-axis component [0,2] and [2,0] should be small
    // (precession is mostly in the xy-plane, with a small tilt)
    // Over 25 years, θ_A ≈ 530 arcsec ≈ 0.15°, so z-x coupling ~0.003
    assert!(
        p[(0, 2)].abs() < 1e-2,
        "Precession z-x coupling should be small, got {}",
        p[(0, 2)]
    );
    assert!(
        p[(2, 0)].abs() < 1e-2,
        "Precession z-x coupling should be small, got {}",
        p[(2, 0)]
    );
}

#[test]
fn test_precession_orthogonal() {
    let model = NutationPrecessionModel::new();
    let p = model.precession_matrix(epoch_2025());
    let product = p * p.transpose();
    let identity = Matrix3::identity();
    let diff = (product - identity).norm();
    assert!(
        diff < 1e-10,
        "Precession must be orthogonal, diff = {diff:e}"
    );
}

// ═══════════════════════════════════════════════
// NUTATION TESTS (IAU 2000B)
// ═══════════════════════════════════════════════

#[test]
fn test_nutation_at_j2000_near_identity() {
    // At J2000.0, nutation is NOT zero — the fundamental arguments
    // (especially Ω = 125.04°) are nonzero, so Δψ ≈ -14 arcsec, Δε ≈ -5 arcsec.
    // The nutation matrix is still a small rotation.
    let model = NutationPrecessionModel::new();
    let n = model.nutation_matrix(j2000());
    let identity = Matrix3::identity();
    let diff = (n - identity).norm();
    // Nutation at J2000 produces a small rotation (~14 arcsec in radians ≈ 7e-5)
    assert!(
        diff < 1e-3,
        "Nutation at J2000 should be a small rotation, diff = {diff:e}"
    );
}

#[test]
fn test_nutation_2025_is_small() {
    // Nutation angles are at most ~10 arcseconds.
    // The nutation matrix should be close to identity.
    let model = NutationPrecessionModel::new();
    let n = model.nutation_matrix(epoch_2025());
    let identity = Matrix3::identity();
    let diff = (n - identity).norm();

    // 10 arcsec in radians ≈ 5e-5. Matrix norm of difference should be
    // on that order (since nutation is a small rotation).
    assert!(
        diff < 1e-3,
        "Nutation should be a small rotation, diff = {diff:e}"
    );
}

#[test]
fn test_nutation_orthogonal() {
    let model = NutationPrecessionModel::new();
    let n = model.nutation_matrix(epoch_2025());
    let product = n * n.transpose();
    let identity = Matrix3::identity();
    let diff = (product - identity).norm();
    assert!(
        diff < 1e-10,
        "Nutation matrix must be orthogonal, diff = {diff:e}"
    );
}

// ═══════════════════════════════════════════════
// COMBINED TRANSFORMATION TESTS
// ═══════════════════════════════════════════════

#[test]
fn test_combined_gcrf_to_tod_at_j2000_near_identity() {
    // At J2000, P = I (no precession), so BPN = N * B.
    // Nutation at J2000 is ~14 arcsec, so the combined matrix is NOT
    // near identity — it includes the nutation rotation.
    // But it should be a small rotation (< 1e-3 rad).
    let model = NutationPrecessionModel::new();
    let combined = model.gcrf_to_tod_matrix(j2000());
    let identity = Matrix3::identity();
    let diff = (combined - identity).norm();
    // Nutation at J2000 is ~14 arcsec ≈ 7e-5 rad
    assert!(
        diff < 1e-3,
        "Combined transform at J2000 should be a small rotation, diff = {diff:e}"
    );
}

#[test]
fn test_combined_gcrf_to_tod_2025_orthogonal() {
    let model = NutationPrecessionModel::new();
    let combined = model.gcrf_to_tod_matrix(epoch_2025());
    let product = combined * combined.transpose();
    let identity = Matrix3::identity();
    let diff = (product - identity).norm();
    assert!(
        diff < 1e-10,
        "Combined transform must be orthogonal, diff = {diff:e}"
    );
}

#[test]
fn test_combined_includes_precession_dominant() {
    // The combined BPN matrix at 2025 should be dominated by precession.
    // Compare the combined matrix to just the precession matrix —
    // the difference should be small (nutation + bias are small).
    let model = NutationPrecessionModel::new();
    let combined = model.gcrf_to_tod_matrix(epoch_2025());
    let precession = model.precession_matrix(epoch_2025());
    let diff = (combined - precession).norm();

    // Nutation is ~10 arcsec and bias is ~0.02 mas.
    // The difference should be on the order of 10 arcsec in radians ≈ 5e-5.
    assert!(
        diff < 1e-3,
        "Combined - precession should be small (nutation+bias), diff = {diff:e}"
    );
}

// ═══════════════════════════════════════════════
// NUTATION ANGLES TESTS
// ═══════════════════════════════════════════════

#[test]
fn test_nutation_in_obliquity_at_j2000_reasonable() {
    // At J2000, nutation angles are NOT zero because the fundamental
    // arguments (especially Ω) are nonzero at J2000.
    // Expected: Δψ ≈ -14 arcsec, Δε ≈ -5 arcsec (from dominant Ω term).
    let model = NutationPrecessionModel::new();
    let (dpsi, deps) = model.nutation_angles(j2000());

    // At J2000, |Δψ| should be < 17.2 arcsec and |Δε| < 9.2 arcsec
    let max_dpsi = 17.2 * ARCSEC;
    let max_deps = 9.2 * ARCSEC;

    assert!(
        dpsi.abs() < max_dpsi,
        "Δψ at J2000 should be < 17.2 arcsec, got {} arcsec",
        dpsi / ARCSEC
    );
    assert!(
        deps.abs() < max_deps,
        "Δε at J2000 should be < 9.2 arcsec, got {} arcsec",
        deps / ARCSEC
    );
}

#[test]
fn test_nutation_angles_2025_reasonable() {
    // Nutation angles at 2025 should be within known bounds:
    // Δψ: up to ~17.2 arcsec, Δε: up to ~9.2 arcsec
    let model = NutationPrecessionModel::new();
    let (dpsi, deps) = model.nutation_angles(epoch_2025());

    // 17.2 arcsec in radians
    let max_dpsi = 17.2 * ARCSEC;
    // 9.2 arcsec in radians
    let max_deps = 9.2 * ARCSEC;

    assert!(
        dpsi.abs() < max_dpsi,
        "Δψ should be < 17.2 arcsec, got {} arcsec",
        dpsi / ARCSEC
    );
    assert!(
        deps.abs() < max_deps,
        "Δε should be < 9.2 arcsec, got {} arcsec",
        deps / ARCSEC
    );
}

// ═══════════════════════════════════════════════
// EQUATION OF EQUINOXES TEST
// ═══════════════════════════════════════════════

#[test]
fn test_equation_of_equinoxes_at_j2000_reasonable() {
    // Equation of equinoxes = Δψ * cos(ε_A)
    // At J2000, Δψ ≈ -14 arcsec, so EE ≈ -14 * cos(23.4°) ≈ -12.8 arcsec.
    let model = NutationPrecessionModel::new();
    let eq_eq = model.equation_of_equinoxes(j2000());

    // Should be within the nutation in longitude bound (~17.2 arcsec)
    let max_eq = 17.2 * ARCSEC;
    assert!(
        eq_eq.abs() < max_eq,
        "Equation of equinoxes at J2000 should be < 17.2 arcsec, got {} arcsec",
        eq_eq / ARCSEC
    );
}

#[test]
fn test_equation_of_equinoxes_2025_reasonable() {
    // Equation of equinoxes is at most ~17.2 * cos(23.4°) ≈ 15.8 arcsec
    let model = NutationPrecessionModel::new();
    let eq_eq = model.equation_of_equinoxes(epoch_2025());

    let max_eq = 17.2 * ARCSEC;
    assert!(
        eq_eq.abs() < max_eq,
        "Equation of equinoxes should be < 17.2 arcsec, got {} arcsec",
        eq_eq / ARCSEC
    );
}

// ═══════════════════════════════════════════════
// MEAN OBLIQUITY TEST (IAU 2006)
// ═══════════════════════════════════════════════

#[test]
fn test_mean_obliquity_at_j2000() {
    // Mean obliquity at J2000.0 = 23°26'21.406" = 23.4392795°
    // (IAU 2006 value, slightly different from IAU 1976's 23.4392911°)
    let model = NutationPrecessionModel::new();
    let eps = model.mean_obliquity(j2000());

    let expected = 23.4392795_f64.to_radians();
    // Should match to < 1 mas
    assert!(
        (eps - expected).abs() < MAS,
        "Mean obliquity at J2000: expected {expected:e} rad, got {eps:e} rad, diff = {} mas",
        (eps - expected) / MAS
    );
}

#[test]
fn test_mean_obliquity_2025_decreased() {
    // Obliquity is decreasing over time (~47 arcsec/century)
    let model = NutationPrecessionModel::new();
    let eps_j2000 = model.mean_obliquity(j2000());
    let eps_2025 = model.mean_obliquity(epoch_2025());

    assert!(
        eps_2025 < eps_j2000,
        "Obliquity should decrease: J2000 = {} rad, 2025 = {} rad",
        eps_j2000,
        eps_2025
    );

    // Over 25 years, decrease should be ~12 arcsec
    let decrease = (eps_j2000 - eps_2025) / ARCSEC;
    assert!(
        (decrease - 12.0).abs() < 2.0,
        "Obliquity decrease over 25 years should be ~12 arcsec, got {decrease} arcsec"
    );
}

// ═══════════════════════════════════════════════
// VECTOR TRANSFORMATION TESTS
// ═══════════════════════════════════════════════

#[test]
fn test_transform_vector_at_j2000_small() {
    // Transforming a vector at J2000 changes it by the nutation rotation
    // (~14 arcsec). The change should be small but not zero.
    let model = NutationPrecessionModel::new();
    let v = Vector3::new(1.0, 0.0, 0.0);
    let transformed = model.transform_gcrf_to_tod(&v, j2000());

    let diff = (transformed - v).norm();
    // Nutation at J2000 is ~14 arcsec ≈ 7e-5 rad
    assert!(
        diff < 1e-3,
        "Vector transform at J2000 should be small, diff = {diff:e}"
    );
}

#[test]
fn test_transform_vector_preserves_length() {
    // Rotations preserve length.
    let model = NutationPrecessionModel::new();
    let v = Vector3::new(7000.0, 0.0, 0.0); // ~LEO altitude
    let transformed = model.transform_gcrf_to_tod(&v, epoch_2025());

    let orig_len = v.norm();
    let new_len = transformed.norm();
    assert!(
        (orig_len - new_len).abs() / orig_len < 1e-10,
        "Rotation should preserve length: orig = {orig_len}, new = {new_len}"
    );
}

#[test]
fn test_transform_vector_round_trip() {
    // Transforming forward then back should return the original.
    let model = NutationPrecessionModel::new();
    let v = Vector3::new(7000.0, 1000.0, 500.0);
    let forward = model.transform_gcrf_to_tod(&v, epoch_2025());
    let back = model.transform_tod_to_gcrf(&forward, epoch_2025());

    let diff = (back - v).norm();
    assert!(
        diff < 1e-6,
        "Round-trip transform should recover original, diff = {diff:e}"
    );
}
