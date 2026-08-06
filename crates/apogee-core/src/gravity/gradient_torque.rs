//! Gravity gradient torque.
//!
//! Computes the torque on a spacecraft due to the gravity gradient of a
//! nearby massive body. The formula used is the standard second-order
//! approximation:
//!
//!   tau = 3 * GM / R^5 * (R x (I * R))
//!
//! where `R` is the body-relative position vector expressed in the spacecraft
//! body frame, `I` is the inertia tensor in the body frame, and `x` is the
//! cross product. The result is in N m in the body frame, exposed as a
//! [`TorqueVector`] so the unit tag is visible at the public API surface.
//!
//! This is O(1) in the size of the inertia matrix.

use apogee_common::units::TorqueVector;
use nalgebra::{Matrix3, Vector3};

/// Compute gravity-gradient torque.
///
/// # Arguments
/// * `position` — body-relative position of the spacecraft in the body frame (m).
/// * `inertia` — inertia tensor in the body frame (kg m^2).
/// * `gm` — gravitational parameter of the attracting body (m^3/s^2).
///
/// # Returns
/// Torque vector in the body frame, as a [`TorqueVector`] (N·m).
pub fn gradient_torque(
    position: &Vector3<f64>,
    inertia: &Matrix3<f64>,
    gm: f64,
) -> Result<TorqueVector, String> {
    let r2 = position.norm_squared();
    if r2 == 0.0 {
        return Err("singularity: zero position vector in gravity gradient torque".into());
    }
    let r5 = r2 * r2 * r2.sqrt();
    let i_r = inertia * position;
    let cross = position.cross(&i_r);
    Ok(TorqueVector::new(3.0 * gm / r5 * cross))
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn diag_inertia(ixx: f64, iyy: f64, izz: f64) -> Matrix3<f64> {
        Matrix3::from_diagonal(&Vector3::new(ixx, iyy, izz))
    }

    #[test]
    fn test_zero_for_spherical_inertia() {
        // For I = identity * k, I*R is parallel to R, so R x I*R = 0.
        let inertia = Matrix3::identity() * 100.0;
        let position = Vector3::new(7_000_000.0, 0.0, 0.0);
        let gm = 3.986004415e14; // Earth GM

        let torque = gradient_torque(&position, &inertia, gm).unwrap();
        assert_relative_eq!(torque.raw().norm(), 0.0, epsilon = 1e-15);
    }

    #[test]
    fn test_sign_for_long_axis_along_position() {
        // Inertia larger about x (long axis) than y and z. Spacecraft on +x
        // axis, so R = (R, 0, 0). I*R = (Ixx*R, 0, 0), parallel to R -> no
        // torque about x or z. Wait, this is also zero. Use a position with
        // a z component so the misalignment with y inertia creates torque.
        let inertia = diag_inertia(1000.0, 500.0, 500.0);
        let position = Vector3::new(7_000_000.0, 0.0, 1_000_000.0);
        let gm = 3.986004415e14;

        let torque = gradient_torque(&position, &inertia, gm).unwrap();

        // I*R = (Ixx*Rx, Iyy*Ry, Izz*Rz) = (7e9, 0, 5e8)
        // R x I*R = (Ry*Iz*Rz - Rz*Iy*Ry, Rz*Ix*Rx - Rx*Iz*Rz, Rx*Iy*Ry - Ry*Ix*Rx)
        // = (0, 1e6*1000*7e6 - 7e6*500*1e6, 0)
        // = (0, 7e15 - 3.5e15, 0) = (0, 3.5e15, 0)
        // torque = 3*GM/R^5 * (0, 3.5e15, 0), positive y.
        assert!(
            torque.raw().y > 0.0,
            "expected positive torque about y, got {}",
            torque.raw().y
        );
        assert_relative_eq!(torque.raw().x, 0.0, epsilon = 1e-10);
        assert_relative_eq!(torque.raw().z, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn test_magnitude_scales_with_inverse_r5() {
        let inertia = diag_inertia(1000.0, 500.0, 500.0);
        let pos_near = Vector3::new(7_000_000.0, 0.0, 1_000_000.0);
        let pos_far = Vector3::new(14_000_000.0, 0.0, 2_000_000.0);
        let gm = 3.986004415e14;

        let tau_near = gradient_torque(&pos_near, &inertia, gm)
            .unwrap()
            .raw()
            .norm();
        let tau_far = gradient_torque(&pos_far, &inertia, gm)
            .unwrap()
            .raw()
            .norm();

        // Scaling: the cross product R x I*R scales as R^2 when R doubles,
        // while the denominator R^5 scales as 32, so the torque ratio is
        // 4 / 32 = 1/8.
        let ratio = tau_far / tau_near;
        assert_relative_eq!(ratio, 1.0 / 8.0, epsilon = 1e-6);
    }

    #[test]
    fn test_singularity_at_origin() {
        let inertia = diag_inertia(1.0, 2.0, 3.0);
        assert!(gradient_torque(&Vector3::zeros(), &inertia, 1.0).is_err());
    }

    #[test]
    fn test_principal_axis_aligned_with_radius_zero_torque() {
        // A principal-axis-aligned spacecraft (diagonal inertia, R along a
        // principal axis) feels zero gravity-gradient torque.
        let inertia = diag_inertia(800.0, 600.0, 400.0);
        let pos = Vector3::new(0.0, 7_000_000.0, 0.0);
        let gm = 3.986004415e14;
        let torque = gradient_torque(&pos, &inertia, gm).unwrap();
        assert_relative_eq!(torque.raw().norm(), 0.0, epsilon = 1e-9);
    }

    #[test]
    fn test_non_diagonal_inertia_produces_cross_terms() {
        // A non-diagonal inertia tensor couples radius vector components.
        let inertia =
            nalgebra::Matrix3::new(1000.0, 100.0, 50.0, 100.0, 800.0, 30.0, 50.0, 30.0, 600.0);
        let pos = Vector3::new(7_000_000.0, 1_000_000.0, 500_000.0);
        let gm = 3.986004415e14;
        let torque = gradient_torque(&pos, &inertia, gm).unwrap();

        // With a diagonal inertia this position gives only y torque; the
        // off-diagonal terms create x and z components as well.
        assert!(
            torque.raw().x.abs() > 1e-9,
            "expected x torque from I_xy/I_xz"
        );
        assert!(torque.raw().y.abs() > 1e-9, "expected y torque");
        assert!(
            torque.raw().z.abs() > 1e-9,
            "expected z torque from I_xz/I_yz"
        );
    }
}
