//! Dynamic properties: mass, inertia, ballistic coefficients.

use nalgebra::Matrix3;

/// Mass and inertia properties of a rigid body.
#[derive(Debug, Clone)]
pub struct Dynamics {
    /// Total mass (kg).
    pub mass: f64,
    /// Inertia tensor in body frame (kg m^2).
    pub inertia: Matrix3<f64>,
    /// Center of mass offset from reference point (m).
    pub cg_offset: nalgebra::Vector3<f64>,
}

impl Default for Dynamics {
    fn default() -> Self {
        Self {
            mass: 1.0,
            inertia: Matrix3::identity(),
            cg_offset: nalgebra::Vector3::zeros(),
        }
    }
}
