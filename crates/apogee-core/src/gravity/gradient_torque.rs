//! Gravity gradient torque: R-hat cross I R-hat formulation — stub.

use nalgebra::{Matrix3, Vector3};

/// Compute gravity gradient torque.
///
/// tau = 3 * GM / R^3 * (R_hat x (I * R_hat))
pub fn gradient_torque(
    _position: &apogee_common::Position,
    _inertia: &Matrix3<f64>,
    _gm: f64,
) -> Vector3<f64> {
    // TODO: implement
    Vector3::zeros()
}
