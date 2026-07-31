//! FrameService tests — reference frame transformations.
//!
//! Tests rotation matrices between ICRF, ECI, ECEF, ECLIPJ2000.
//! Uses nalgebra Vector3 and Matrix3 for position vectors and rotations.
//!
//! # Test Sources
//!
//! Obliquity of the ecliptic at J2000:
//! - Lieske, J.H., et al. (1977), Astron. Astrophys. 58, 1-16
//!   https://ui.adsabs.harvard.edu/abs/1977A%26A....58....1L/abstract
//! - Value verified against IAU 1976 constant: epsilon_0 = 23d26'21.448"
//!
//! Earth Rotation Angle:
//! - IERS Conventions (2010), Eq. (5.15)
//!   https://iers-conventions.obspm.fr/content/tn36.pdf
//!
//! ICRF2 realization (sub-arcsecond frame bias):
//! - Fey, A.L., et al. (2009), IERS Technical Note 35
//!   https://ui.adsabs.harvard.edu/abs/2009ITN....35.....F
//! - ICRF2 realization: ~0.02 mas frame bias
//!
//! Rotation matrix orthogonality (R * R^T = I):
//! - Goldstein, H., Poole, C.P. & Safko, J.L. (2002),
//!   "Classical Mechanics", 3rd ed., §4.2: Orthogonal Transformations

use crate::frames::frame_service::FrameService;
use crate::frames::Frame;
use hifitime::{Epoch, TimeScale};
use nalgebra::{Matrix3, Vector3};

fn j2000() -> Epoch {
    Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, TimeScale::TDB)
}

fn approx_eq_matrix(a: &Matrix3<f64>, b: &Matrix3<f64>, tol: f64) -> bool {
    (a - b).abs().max() < tol
}

fn approx_eq_vec(a: &Vector3<f64>, b: &Vector3<f64>, tol: f64) -> bool {
    (a - b).abs().max() < tol
}

#[test]
fn test_eci_to_ecliptic_obliquity() {
    let svc = FrameService::new();
    let pos_eci = Vector3::new(1.0, 0.0, 0.0);
    let pos_ecl = svc.transform_position(&pos_eci, Frame::Eci, Frame::EclipticJ2000, j2000());
    assert!(approx_eq_vec(&pos_ecl, &pos_eci, 1e-10));
}

#[test]
fn test_eci_to_ecliptic_z_axis() {
    let svc = FrameService::new();
    let pos_eci = Vector3::new(0.0, 0.0, 1.0);
    let pos_ecl = svc.transform_position(&pos_eci, Frame::Eci, Frame::EclipticJ2000, j2000());
    let obliquity = 23.4392911_f64.to_radians();
    let expected = Vector3::new(0.0, -obliquity.sin(), obliquity.cos());
    assert!(approx_eq_vec(&pos_ecl, &expected, 1e-6));
}

#[test]
fn test_ecliptic_to_eci_inverse() {
    let svc = FrameService::new();
    let pos = Vector3::new(0.5, 0.3, 0.8);
    let pos_ecl = svc.transform_position(&pos, Frame::Eci, Frame::EclipticJ2000, j2000());
    let pos_back = svc.transform_position(&pos_ecl, Frame::EclipticJ2000, Frame::Eci, j2000());
    assert!(approx_eq_vec(&pos_back, &pos, 1e-10));
}

#[test]
fn test_icrf_to_eci_near_identity_at_j2000() {
    let svc = FrameService::new();
    let pos = Vector3::new(1.0e8, 2.0e8, 3.0e8);
    let pos_icrf = svc.transform_position(&pos, Frame::Eci, Frame::Icrf, j2000());
    let diff = (pos_icrf - pos).norm();
    assert!(
        diff / pos.norm() < 1e-6,
        "ICRF-ECI diff ratio: {}",
        diff / pos.norm()
    );
}

#[test]
fn test_eci_to_ecef_applies_earth_rotation_angle() {
    // ECI x-axis at J2000 should rotate by -ERA around the common z-axis
    // when transformed to ECEF.
    let svc = FrameService::new();
    let pos_eci = Vector3::new(1.0, 0.0, 0.0);
    let pos_ecef = svc.transform_position(&pos_eci, Frame::Eci, Frame::Ecef, j2000());

    // ERA at J2000 from IERS Conventions (2010), Eq. (5.15).
    let era = 280.4606183744_f64.to_radians();
    let expected = Vector3::new(era.cos(), -era.sin(), 0.0);

    assert!(
        approx_eq_vec(&pos_ecef, &expected, 1e-4),
        "ECI x-axis at J2000 should map to ECEF at -ERA, expected {} got {}",
        expected,
        pos_ecef
    );
}

#[test]
fn test_ecef_to_eci_roundtrip() {
    let svc = FrameService::new();
    let pos = Vector3::new(7000.0, 0.0, 0.0);
    let pos_ecef = svc.transform_position(&pos, Frame::Eci, Frame::Ecef, j2000());
    let pos_back = svc.transform_position(&pos_ecef, Frame::Ecef, Frame::Eci, j2000());
    assert!(approx_eq_vec(&pos_back, &pos, 1e-6));
}

#[test]
fn test_rotation_matrix_orthogonal() {
    let svc = FrameService::new();
    let r = svc.rotation_matrix(Frame::Eci, Frame::Ecef, j2000());
    let product = r * r.transpose();
    let identity = Matrix3::identity();
    assert!(approx_eq_matrix(&product, &identity, 1e-10));
}

#[test]
fn test_ecliptic_rotation_orthogonal() {
    let svc = FrameService::new();
    let r = svc.rotation_matrix(Frame::Eci, Frame::EclipticJ2000, j2000());
    let product = r * r.transpose();
    let identity = Matrix3::identity();
    assert!(approx_eq_matrix(&product, &identity, 1e-10));
}

#[test]
fn test_transform_velocity_preserves_norm() {
    let svc = FrameService::new();
    let vel = Vector3::new(7.5, 0.0, 0.0);
    let vel_ecl = svc.transform_velocity(&vel, Frame::Eci, Frame::EclipticJ2000, j2000());
    assert!((vel_ecl.norm() - vel.norm()).abs() < 1e-10);
}

#[test]
fn test_earth_rotation_angle_at_j2000() {
    let svc = FrameService::new();
    let era = svc.earth_rotation_angle(j2000());
    let expected = 280.4606183744_f64.to_radians();
    let diff = (era - expected).abs();
    let wrapped = diff.min(std::f64::consts::TAU - diff);
    assert!(wrapped < 1e-6, "ERA at J2000 = {} rad", era);
}

#[test]
fn test_earth_rotation_angle_at_2025() {
    let svc = FrameService::new();
    let epoch = Epoch::from_gregorian(2025, 1, 1, 12, 0, 0, 0, TimeScale::TDB);
    let era = svc.earth_rotation_angle(epoch);
    let expected = 281.07203336_f64.to_radians();
    let diff = (era - expected).abs();
    let wrapped = diff.min(std::f64::consts::TAU - diff);
    assert!(wrapped < 1e-6, "ERA at 2025 = {} rad", era);
}

#[test]
fn test_icrf_to_ecef_at_j2000_orthogonal() {
    let svc = FrameService::new();
    let r = svc.rotation_matrix(Frame::Icrf, Frame::Ecef, j2000());
    let product = r * r.transpose();
    assert!(approx_eq_matrix(&product, &Matrix3::identity(), 1e-10));
}
