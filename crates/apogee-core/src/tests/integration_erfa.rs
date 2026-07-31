//! Integration tests — validation against ERFA (Essential Routines for
//! Fundamental Astronomy) reference values.
//!
//! These tests compare our NutationPrecessionModel and FrameService outputs
//! against reference values computed with the ERFA C library (v2.0), which
//! implements the same IAU 2000/2006 models. ERFA is the open-source fork of
//! the IAU SOFA library and is the de facto reference implementation.
//!
//! Reference values were generated at 5 epochs spanning 2000–2030 using
//! the following ERFA functions:
//! - eraNut00b: IAU 2000B nutation angles (Δψ, Δε)
//! - eraObl06: IAU 2006 mean obliquity
//! - eraEe00b: Equation of equinoxes (IAU 2000B)
//! - eraNum00b: Full GCRF-to-TOD rotation matrix (BPN)
//! - eraBp06: Frame bias (B) and precession (P) matrices
//!
//! # References
//!
//! ERFA library:
//! - ERFA: https://github.com/liberfa/erfa
//! - IAU SOFA: http://iausofa.org/
//!
//! IAU 2000B nutation model:
//! - McCarthy, D.D. & Luzum, B.J. (2003), "An Abridged Model of the IAU 2000
//!   Nutation Model", IERS Technical Note 32
//!   https://ui.adsabs.harvard.edu/abs/2004ITN....32.....M
//!
//! IAU 2006 precession:
//! - Capitaine, N., Wallace, P.T., Chapront, J. (2003), Astron. Astrophys.
//!   412, 567-586
//!   https://ui.adsabs.harvard.edu/abs/2003A%26A...412..567C
//!
//! IERS Conventions (2010), Ch. 5:
//! - https://iers-conventions.obspm.fr/content/tn36.pdf

use crate::frames::nutation_precession::NutationPrecessionModel;
use hifitime::{Epoch, TimeScale};
use nalgebra::{Matrix3, Vector3};

/// 1 arcsecond in radians.
const ARCSEC: f64 = 4.84813681109536e-6;

/// 1 milliarcsecond in radians.
const MAS: f64 = 4.84813681109536e-9;

// ═══════════════════════════════════════════════
// REFERENCE DATA (from ERFA v2.0)
// ═══════════════════════════════════════════════

/// J2000 epoch: 2000-01-01 12:00:00 TDB (JD 2451545.0)
fn j2000() -> Epoch {
    Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, TimeScale::TDB)
}

/// 2010-07-01 00:00:00 TDB (JD 2455378.5)
fn epoch_2010() -> Epoch {
    Epoch::from_gregorian(2010, 7, 1, 0, 0, 0, 0, TimeScale::TDB)
}

/// 2020-01-01 00:00:00 TDB (JD 2458849.5)
fn epoch_2020() -> Epoch {
    Epoch::from_gregorian(2020, 1, 1, 0, 0, 0, 0, TimeScale::TDB)
}

/// 2025-01-01 00:00:00 TDB (JD 2460676.5)
fn epoch_2025() -> Epoch {
    Epoch::from_gregorian(2025, 1, 1, 0, 0, 0, 0, TimeScale::TDB)
}

/// 2030-06-15 12:00:00 TDB (JD 2462668.0)
fn epoch_2030() -> Epoch {
    Epoch::from_gregorian(2030, 6, 15, 12, 0, 0, 0, TimeScale::TDB)
}

/// Angular difference between two rotation matrices in radians.
/// Computes the rotation angle of R1^T * R2, which is the magnitude of
/// the residual rotation between the two matrices.
fn matrix_angular_diff(r1: &Matrix3<f64>, r2: &Matrix3<f64>) -> f64 {
    let residual = r1.transpose() * r2;
    // The rotation angle θ satisfies: trace(R) = 1 + 2*cos(θ)
    let trace = residual.trace();
    // Clamp to [-1, 3] to avoid NaN from floating-point errors
    let cos_theta = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0);
    cos_theta.acos()
}

// ═══════════════════════════════════════════════
// MEAN OBLIQUITY TESTS (IAU 2006)
// ═══════════════════════════════════════════════

#[test]
fn test_obliquity_vs_erfa_j2000() {
    // ERFA eraObl06 at JD 2451545.0:
    // eps = 4.090926006005829e-01 rad = 23.439279444 deg
    let model = NutationPrecessionModel::new();
    let eps = model.mean_obliquity(j2000());
    let expected = 4.090926006005829e-01;
    assert!(
        (eps - expected).abs() < MAS,
        "Obliquity at J2000: diff = {} mas (expected < 1 mas)",
        (eps - expected) / MAS
    );
}

#[test]
fn test_obliquity_vs_erfa_2025() {
    // ERFA eraObl06 at JD 2460676.5:
    // eps = 4.090358313766700e-01 rad
    let model = NutationPrecessionModel::new();
    let eps = model.mean_obliquity(epoch_2025());
    let expected = 4.090358313766700e-01;
    assert!(
        (eps - expected).abs() < MAS,
        "Obliquity at 2025: diff = {} mas (expected < 1 mas)",
        (eps - expected) / MAS
    );
}

// ═══════════════════════════════════════════════
// NUTATION ANGLES TESTS (IAU 2000B)
// ═══════════════════════════════════════════════

#[test]
fn test_nutation_angles_vs_erfa_j2000() {
    // ERFA eraNut00b at JD 2451545.0:
    // dpsi = -6.754261253992235e-05 rad (-13.931664 arcsec)
    // deps = -2.797092331098565e-05 rad (-5.769417 arcsec)
    //
    // Our 20-term model should match within ~1 arcsec (truncation error
    // of the 80-term IAU 2000B model).
    let model = NutationPrecessionModel::new();
    let (dpsi, deps) = model.nutation_angles(j2000());
    let ref_dpsi = -6.754261253992235e-05;
    let ref_deps = -2.797092331098565e-05;

    let dpsi_err = (dpsi - ref_dpsi).abs() / ARCSEC;
    let deps_err = (deps - ref_deps).abs() / ARCSEC;
    assert!(
        dpsi_err < 1.0,
        "Δψ at J2000: {} arcsec error (expected < 1 arcsec)",
        dpsi_err
    );
    assert!(
        deps_err < 1.0,
        "Δε at J2000: {} arcsec error (expected < 1 arcsec)",
        deps_err
    );
}

#[test]
fn test_nutation_angles_vs_erfa_2025() {
    // ERFA eraNut00b at JD 2460676.5:
    // dpsi = 9.595592955062696e-07 rad (0.197923 arcsec)
    // deps = 4.122824101085561e-05 rad (8.503935 arcsec)
    let model = NutationPrecessionModel::new();
    let (dpsi, deps) = model.nutation_angles(epoch_2025());
    let ref_dpsi = 9.595592955062696e-07;
    let ref_deps = 4.122824101085561e-05;

    let dpsi_err = (dpsi - ref_dpsi).abs() / ARCSEC;
    let deps_err = (deps - ref_deps).abs() / ARCSEC;
    assert!(
        dpsi_err < 1.0,
        "Δψ at 2025: {} arcsec error (expected < 1 arcsec)",
        dpsi_err
    );
    assert!(
        deps_err < 1.0,
        "Δε at 2025: {} arcsec error (expected < 1 arcsec)",
        deps_err
    );
}

#[test]
fn test_nutation_angles_vs_erfa_2010() {
    // ERFA eraNut00b at JD 2455378.5:
    // dpsi = 8.390244203717109e-05 rad
    // deps = 7.390493140063497e-06 rad
    let model = NutationPrecessionModel::new();
    let (dpsi, deps) = model.nutation_angles(epoch_2010());
    let ref_dpsi = 8.390244203717109e-05;
    let ref_deps = 7.390493140063497e-06;

    let dpsi_err = (dpsi - ref_dpsi).abs() / ARCSEC;
    let deps_err = (deps - ref_deps).abs() / ARCSEC;
    assert!(
        dpsi_err < 1.0,
        "Δψ at 2010: {} arcsec error (expected < 1 arcsec)",
        dpsi_err
    );
    assert!(
        deps_err < 1.0,
        "Δε at 2010: {} arcsec error (expected < 1 arcsec)",
        deps_err
    );
}

#[test]
fn test_nutation_angles_vs_erfa_2020() {
    // ERFA eraNut00b at JD 2458849.5:
    // dpsi = -7.996414118312039e-05 rad
    // deps = -8.249857851285796e-06 rad
    let model = NutationPrecessionModel::new();
    let (dpsi, deps) = model.nutation_angles(epoch_2020());
    let ref_dpsi = -7.996414118312039e-05;
    let ref_deps = -8.249857851285796e-06;

    let dpsi_err = (dpsi - ref_dpsi).abs() / ARCSEC;
    let deps_err = (deps - ref_deps).abs() / ARCSEC;
    assert!(
        dpsi_err < 1.0,
        "Δψ at 2020: {} arcsec error (expected < 1 arcsec)",
        dpsi_err
    );
    assert!(
        deps_err < 1.0,
        "Δε at 2020: {} arcsec error (expected < 1 arcsec)",
        deps_err
    );
}

#[test]
fn test_nutation_angles_vs_erfa_2030() {
    // ERFA eraNut00b at JD 2462668.0:
    // dpsi = 8.024389619812660e-05 rad
    // deps = -1.360009203634451e-05 rad
    let model = NutationPrecessionModel::new();
    let (dpsi, deps) = model.nutation_angles(epoch_2030());
    let ref_dpsi = 8.024389619812660e-05;
    let ref_deps = -1.360009203634451e-05;

    let dpsi_err = (dpsi - ref_dpsi).abs() / ARCSEC;
    let deps_err = (deps - ref_deps).abs() / ARCSEC;
    assert!(
        dpsi_err < 1.0,
        "Δψ at 2030: {} arcsec error (expected < 1 arcsec)",
        dpsi_err
    );
    assert!(
        deps_err < 1.0,
        "Δε at 2030: {} arcsec error (expected < 1 arcsec)",
        deps_err
    );
}

// ═══════════════════════════════════════════════
// EQUATION OF EQUINOXES TESTS
// ═══════════════════════════════════════════════

#[test]
fn test_equation_of_equinoxes_vs_erfa_j2000() {
    // ERFA eraEe00b at JD 2451545.0:
    // EE = -6.195892212970470e-05 rad
    let model = NutationPrecessionModel::new();
    let ee = model.equation_of_equinoxes(j2000());
    let expected = -6.195892212970470e-05;
    assert!(
        (ee - expected).abs() < ARCSEC,
        "EE at J2000: {} arcsec error (expected < 1 arcsec)",
        (ee - expected) / ARCSEC
    );
}

#[test]
fn test_equation_of_equinoxes_vs_erfa_2025() {
    // ERFA eraEe00b at JD 2460676.5:
    // EE = 8.807052943824431e-07 rad
    let model = NutationPrecessionModel::new();
    let ee = model.equation_of_equinoxes(epoch_2025());
    let expected = 8.807052943824431e-07;
    assert!(
        (ee - expected).abs() < ARCSEC,
        "EE at 2025: {} arcsec error (expected < 1 arcsec)",
        (ee - expected) / ARCSEC
    );
}

// ═══════════════════════════════════════════════
// BPN (GCRF-to-TOD) MATRIX TESTS
// ═══════════════════════════════════════════════

#[test]
fn test_bpn_matrix_vs_erfa_j2000() {
    // ERFA eraNum00b at JD 2451545.0:
    // BPN = [[ 1.00000000e+00,  6.1969e-05,  2.6867e-05],
    //         [-6.1970e-05,     1.00000000e+00,  2.7970e-05],
    //         [-2.6865e-05,    -2.7972e-05,     1.00000000e+00]]
    let model = NutationPrecessionModel::new();
    let bpn = model.gcrf_to_tod_matrix(j2000());
    let expected = Matrix3::new(
        9.999999977189977e-01,
        6.196913538355048e-05,
        2.686690829991369e-05,
        -6.196988685154057e-05,
        9.999999976887036e-01,
        2.797009083765900e-05,
        -2.686517495547050e-05,
        -2.797175571317423e-05,
        9.999999992479216e-01,
    );

    let ang_diff = matrix_angular_diff(&bpn, &expected);
    assert!(
        ang_diff < ARCSEC,
        "BPN at J2000: angular diff = {} arcsec (expected < 1 arcsec)",
        ang_diff / ARCSEC
    );
}

#[test]
fn test_bpn_matrix_vs_erfa_2025() {
    // Full GCRF-to-TOD matrix = N * P * B at JD 2460676.5:
    let model = NutationPrecessionModel::new();
    let bpn = model.gcrf_to_tod_matrix(epoch_2025());
    let expected = Matrix3::new(
        9.999814159792794e-01,
        -5.591587322489912e-03,
        -2.429371791578215e-03,
        5.591487315238603e-03,
        9.999843663627151e-01,
        -4.795590469135004e-05,
        2.429601961291844e-03,
        3.437121191551985e-05,
        9.999970479221112e-01,
    );

    let ang_diff = matrix_angular_diff(&bpn, &expected);
    assert!(
        ang_diff < ARCSEC,
        "BPN at 2025: angular diff = {} arcsec (expected < 1 arcsec)",
        ang_diff / ARCSEC
    );
}

#[test]
fn test_bpn_matrix_vs_erfa_2010() {
    // Full GCRF-to-TOD matrix = N * P * B at JD 2455378.5:
    let model = NutationPrecessionModel::new();
    let bpn = model.gcrf_to_tod_matrix(epoch_2010());
    let expected = Matrix3::new(
        9.999965076974531e-01,
        -2.423967598246843e-03,
        -1.053078340746152e-03,
        2.423959864057249e-03,
        9.999970621678187e-01,
        -8.620615499958598e-06,
        1.053096143073701e-03,
        6.067965756740585e-06,
        9.999994454756965e-01,
    );

    let ang_diff = matrix_angular_diff(&bpn, &expected);
    assert!(
        ang_diff < ARCSEC,
        "BPN at 2010: angular diff = {} arcsec (expected < 1 arcsec)",
        ang_diff / ARCSEC
    );
}

#[test]
fn test_bpn_matrix_vs_erfa_2020() {
    // Full GCRF-to-TOD matrix = N * P * B at JD 2458849.5:
    let model = NutationPrecessionModel::new();
    let bpn = model.gcrf_to_tod_matrix(epoch_2020());
    let expected = Matrix3::new(
        9.999884991693111e-01,
        -4.398727376893707e-03,
        -1.911210763861806e-03,
        4.398743255375687e-03,
        9.999903254736674e-01,
        4.104665923554371e-06,
        1.911174218498911e-03,
        -1.251154417922984e-05,
        9.999981736266199e-01,
    );

    let ang_diff = matrix_angular_diff(&bpn, &expected);
    assert!(
        ang_diff < ARCSEC,
        "BPN at 2020: angular diff = {} arcsec (expected < 1 arcsec)",
        ang_diff / ARCSEC
    );
}

#[test]
fn test_bpn_matrix_vs_erfa_2030() {
    // Full GCRF-to-TOD matrix = N * P * B at JD 2462668.0:
    let model = NutationPrecessionModel::new();
    let bpn = model.gcrf_to_tod_matrix(epoch_2030());
    let expected = Matrix3::new(
        9.999718351104827e-01,
        -6.883689718603802e-03,
        -2.990618939732927e-03,
        6.883730602086620e-03,
        9.999763068401158e-01,
        3.377337443596048e-06,
        2.990524833979536e-03,
        -2.396385744155388e-05,
        9.999955280834802e-01,
    );

    let ang_diff = matrix_angular_diff(&bpn, &expected);
    assert!(
        ang_diff < ARCSEC,
        "BPN at 2030: angular diff = {} arcsec (expected < 1 arcsec)",
        ang_diff / ARCSEC
    );
}

// ═══════════════════════════════════════════════
// VECTOR TRANSFORMATION TESTS
// ═══════════════════════════════════════════════

#[test]
fn test_vector_transform_vs_erfa_j2000() {
    // ERFA: transform [7000, 1000, 500] through BPN at J2000:
    // [7000.075386623, 999.5801935262, 499.7839716436]
    let model = NutationPrecessionModel::new();
    let v = Vector3::new(7000.0, 1000.0, 500.0);
    let result = model.transform_gcrf_to_tod(&v, j2000());
    let expected = Vector3::new(7.000075386623e3, 9.995801935262e2, 4.997839716436e2);

    let diff = (result - expected).norm();
    // 1 arcsec angular error on a 7000 km vector ≈ 7000 * ARCSEC ≈ 0.034 km = 34 m
    let pos_tol = v.norm() * ARCSEC;
    assert!(
        diff < pos_tol,
        "Vector transform at J2000: diff = {} m (tol = {} m)",
        diff * 1000.0,
        pos_tol * 1000.0
    );
}

#[test]
fn test_vector_transform_vs_erfa_2025() {
    // Full GCRF-to-TOD transform at JD 2460676.5:
    // [6993.0636386367, 1039.1007996170, 517.0401089020]
    let model = NutationPrecessionModel::new();
    let v = Vector3::new(7000.0, 1000.0, 500.0);
    let result = model.transform_gcrf_to_tod(&v, epoch_2025());
    let expected = Vector3::new(6.993063638637e3, 1.039100799617e3, 5.170401089020e2);

    let diff = (result - expected).norm();
    let pos_tol = v.norm() * ARCSEC;
    assert!(
        diff < pos_tol,
        "Vector transform at 2025: diff = {} m (tol = {} m)",
        diff * 1000.0,
        pos_tol * 1000.0
    );
}

#[test]
fn test_vector_transform_vs_erfa_2010() {
    // Full GCRF-to-TOD transform at JD 2455378.5:
    // [6997.0250471125, 1016.9604709085, 507.3774637051]
    let model = NutationPrecessionModel::new();
    let v = Vector3::new(7000.0, 1000.0, 500.0);
    let result = model.transform_gcrf_to_tod(&v, epoch_2010());
    let expected = Vector3::new(6.997025047113e3, 1.016960470909e3, 5.073774637051e2);

    let diff = (result - expected).norm();
    let pos_tol = v.norm() * ARCSEC;
    assert!(
        diff < pos_tol,
        "Vector transform at 2010: diff = {} m (tol = {} m)",
        diff * 1000.0,
        pos_tol * 1000.0
    );
}

#[test]
fn test_vector_transform_vs_erfa_2020() {
    // Full GCRF-to-TOD transform at JD 2458849.5:
    // [6994.5651614264, 1030.7835805943, 513.3647947986]
    let model = NutationPrecessionModel::new();
    let v = Vector3::new(7000.0, 1000.0, 500.0);
    let result = model.transform_gcrf_to_tod(&v, epoch_2020());
    let expected = Vector3::new(6.994565161426e3, 1.030783580594e3, 5.133647947986e2);

    let diff = (result - expected).norm();
    let pos_tol = v.norm() * ARCSEC;
    assert!(
        diff < pos_tol,
        "Vector transform at 2020: diff = {} m (tol = {} m)",
        diff * 1000.0,
        pos_tol * 1000.0
    );
}

#[test]
fn test_vector_transform_vs_erfa_2030() {
    // Full GCRF-to-TOD transform at JD 2462668.0:
    // [6991.4238466263, 1048.1641096864, 520.9074740196]
    let model = NutationPrecessionModel::new();
    let v = Vector3::new(7000.0, 1000.0, 500.0);
    let result = model.transform_gcrf_to_tod(&v, epoch_2030());
    let expected = Vector3::new(6.991423846626e3, 1.048164109686e3, 5.209074740196e2);

    let diff = (result - expected).norm();
    let pos_tol = v.norm() * ARCSEC;
    assert!(
        diff < pos_tol,
        "Vector transform at 2030: diff = {} m (tol = {} m)",
        diff * 1000.0,
        pos_tol * 1000.0
    );
}

// ═══════════════════════════════════════════════
// PRECESSION MATRIX TESTS
// ═══════════════════════════════════════════════

#[test]
fn test_precession_matrix_vs_erfa_j2000() {
    // ERFA eraBp06 precession matrix at JD 2451545.0:
    // P = identity (no precession at J2000)
    let model = NutationPrecessionModel::new();
    let p = model.precession_matrix(j2000());
    let identity = Matrix3::identity();
    let ang_diff = matrix_angular_diff(&p, &identity);
    assert!(
        ang_diff < 1e-10,
        "Precession at J2000 should be identity, ang_diff = {}",
        ang_diff
    );
}

#[test]
fn test_precession_matrix_vs_erfa_2025() {
    // ERFA eraBp06 precession matrix at JD 2460676.5:
    let model = NutationPrecessionModel::new();
    let p = model.precession_matrix(epoch_2025());
    let expected = Matrix3::new(
        9.999814220288324e-01,
        -5.590636237461227e-03,
        -2.429070533004524e-03,
        5.590636312593602e-03,
        9.999843722478536e-01,
        -6.759157532782652e-06,
        2.429070360083325e-03,
        -6.821017966337235e-06,
        9.999970497809779e-01,
    );

    // Debug: print our matrix
    eprintln!("Our P at 2025:");
    for i in 0..3 {
        eprintln!(
            "  [{:.15e}, {:.15e}, {:.15e}]",
            p[(i, 0)],
            p[(i, 1)],
            p[(i, 2)]
        );
    }
    eprintln!("ERFA P at 2025:");
    for i in 0..3 {
        eprintln!(
            "  [{:.15e}, {:.15e}, {:.15e}]",
            expected[(i, 0)],
            expected[(i, 1)],
            expected[(i, 2)]
        );
    }

    // Debug: print the centuries value by checking mean_obliquity
    let eps = model.mean_obliquity(epoch_2025());
    let eps_j2000 = model.mean_obliquity(j2000());
    eprintln!("Obliquity at J2000: {:.15e}", eps_j2000);
    eprintln!("Obliquity at 2025:  {:.15e}", eps);
    eprintln!("Obliquity diff:    {:.15e} rad", eps_j2000 - eps);
    // Expected decrease: ~12 arcsec over 25 years
    let expected_decrease = 12.0 * 4.84813681109536e-6;
    eprintln!("Expected decrease:  {:.15e} rad", expected_decrease);

    let ang_diff = matrix_angular_diff(&p, &expected);
    assert!(
        ang_diff < MAS,
        "Precession at 2025: angular diff = {} mas (expected < 1 mas)",
        ang_diff / MAS
    );
}

// ═══════════════════════════════════════════════
// FRAME BIAS MATRIX TESTS
// ═══════════════════════════════════════════════

#[test]
fn test_frame_bias_vs_erfa() {
    // ERFA eraBp06 frame bias matrix at J2000:
    // B = [[ 1.0,         -7.078e-08,   8.056e-08],
    //      [ 7.078e-08,    1.0,          3.306e-08],
    //      [-8.056e-08,   -3.306e-08,    1.0      ]]
    let model = NutationPrecessionModel::new();
    let b = model.frame_bias_matrix();
    let expected = Matrix3::new(
        9.999999999999941e-01,
        -7.078368960971556e-08,
        8.056213977613186e-08,
        7.078368694637676e-08,
        9.999999999999969e-01,
        3.305943735432137e-08,
        -8.056214211620057e-08,
        -3.305943169218395e-08,
        9.999999999999962e-01,
    );
    let ang_diff = matrix_angular_diff(&b, &expected);
    assert!(
        ang_diff < MAS,
        "Frame bias: angular diff = {} mas (expected < 1 mas)",
        ang_diff / MAS
    );
}
