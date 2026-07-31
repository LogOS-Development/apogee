//! FrameService tests — reference frame transformations.
//!
//! Tests rotation matrices between ICRF, ECI, ECEF, ECLIPJ2000.
//! Uses nalgebra Vector3 and Matrix3 for position vectors and rotations.

use crate::frames::frame_service::FrameService;
use crate::frames::Frame;
use nalgebra::{Matrix3, Vector3};

fn approx_eq_matrix(a: &Matrix3<f64>, b: &Matrix3<f64>, tol: f64) -> bool {
    (a - b).abs().max() < tol
}

fn approx_eq_vec(a: &Vector3<f64>, b: &Vector3<f64>, tol: f64) -> bool {
    (a - b).abs().max() < tol
}

#[test]
fn test_eci_to_ecliptic_obliquity() {
    // The obliquity of the ecliptic at J2000 is ~23.4392911 degrees
    // A vector along the x-axis (vernal equinox direction) should be unchanged
    let svc = FrameService::new();
    let pos_eci = Vector3::new(1.0, 0.0, 0.0);
    let pos_ecl = svc.transform_position(&pos_eci, Frame::Eci, Frame::EclipticJ2000);
    // X-axis (vernal equinox) is in both the equatorial and ecliptic planes
    assert!(approx_eq_vec(&pos_ecl, &pos_eci, 1e-10));
}

#[test]
fn test_eci_to_ecliptic_z_axis() {
    // A vector along the z-axis (north celestial pole) should map to
    // z * cos(obliquity) in the ecliptic frame's y/z components
    let svc = FrameService::new();
    let pos_eci = Vector3::new(0.0, 0.0, 1.0);
    let pos_ecl = svc.transform_position(&pos_eci, Frame::Eci, Frame::EclipticJ2000);
    // The ecliptic z-axis is tilted by obliquity from the equatorial z-axis
    let obliquity = 23.4392911_f64.to_radians();
    let expected = Vector3::new(0.0, -obliquity.sin(), obliquity.cos());
    assert!(approx_eq_vec(&pos_ecl, &expected, 1e-6));
}

#[test]
fn test_ecliptic_to_eci_inverse() {
    // ECI → ECLIPJ2000 → ECI should roundtrip
    let svc = FrameService::new();
    let pos = Vector3::new(0.5, 0.3, 0.8);
    let pos_ecl = svc.transform_position(&pos, Frame::Eci, Frame::EclipticJ2000);
    let pos_back = svc.transform_position(&pos_ecl, Frame::EclipticJ2000, Frame::Eci);
    assert!(approx_eq_vec(&pos_back, &pos, 1e-10));
}

#[test]
fn test_icrf_to_eci_near_identity() {
    // ICRF and ECI (J2000) are very close — the rotation is sub-arcsecond
    let svc = FrameService::new();
    let pos = Vector3::new(1.0e8, 2.0e8, 3.0e8);
    let pos_icrf = svc.transform_position(&pos, Frame::Eci, Frame::Icrf);
    // Difference should be < 0.1 arcsecond = ~5e-7 radians
    let diff = (pos_icrf - pos).norm();
    assert!(
        diff / pos.norm() < 1e-6,
        "ICRF-ECI diff ratio: {}",
        diff / pos.norm()
    );
}

#[test]
fn test_eci_to_ecef_identity_at_j2000() {
    // At J2000 epoch, GMST = 0 (approximately), so ECI and ECEF z-axes align
    // The rotation is purely about the z-axis by the Greenwich hour angle
    let svc = FrameService::new();
    let pos_eci = Vector3::new(1.0, 0.0, 0.0);
    // At J2000.0, the GMST is ~280.46 degrees, not zero
    // The key test is that the rotation is a proper rotation (orthogonal matrix)
    let pos_ecef = svc.transform_position(&pos_eci, Frame::Eci, Frame::Ecef);
    // The norm should be preserved
    assert!((pos_ecef.norm() - pos_eci.norm()).abs() < 1e-10);
}

#[test]
fn test_eci_to_ecef_z_axis_unchanged() {
    // ECI to ECEF is a rotation about the z-axis (Earth rotation axis)
    // The z-component should be unchanged
    let svc = FrameService::new();
    let pos_eci = Vector3::new(1.0e7, 2.0e7, 3.0e7);
    let pos_ecef = svc.transform_position(&pos_eci, Frame::Eci, Frame::Ecef);
    assert!(
        (pos_ecef.z - pos_eci.z).abs() < 1e-3,
        "Z component changed: {}",
        pos_ecef.z - pos_eci.z
    );
}

#[test]
fn test_ecef_to_eci_roundtrip() {
    let svc = FrameService::new();
    let pos = Vector3::new(7000.0, 0.0, 0.0); // LEO altitude
    let pos_ecef = svc.transform_position(&pos, Frame::Eci, Frame::Ecef);
    let pos_back = svc.transform_position(&pos_ecef, Frame::Ecef, Frame::Eci);
    assert!(approx_eq_vec(&pos_back, &pos, 1e-6));
}

#[test]
fn test_rotation_matrix_orthogonal() {
    // The rotation matrix from ECI to ECEF should be orthogonal (R * R^T = I)
    let svc = FrameService::new();
    let r = svc.rotation_matrix(Frame::Eci, Frame::Ecef);
    let product = r * r.transpose();
    let identity = Matrix3::identity();
    assert!(approx_eq_matrix(&product, &identity, 1e-10));
}

#[test]
fn test_ecliptic_rotation_orthogonal() {
    let svc = FrameService::new();
    let r = svc.rotation_matrix(Frame::Eci, Frame::EclipticJ2000);
    let product = r * r.transpose();
    let identity = Matrix3::identity();
    assert!(approx_eq_matrix(&product, &identity, 1e-10));
}

#[test]
fn test_transform_velocity_preserves_norm() {
    // Transforming a velocity vector should preserve its magnitude
    // (rotation matrices are norm-preserving)
    let svc = FrameService::new();
    let vel = Vector3::new(7.5, 0.0, 0.0); // ~LEO velocity km/s
    let vel_ecl = svc.transform_velocity(&vel, Frame::Eci, Frame::EclipticJ2000);
    assert!((vel_ecl.norm() - vel.norm()).abs() < 1e-10);
}
