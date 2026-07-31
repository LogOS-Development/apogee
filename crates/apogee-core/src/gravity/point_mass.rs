//! Point-mass N-body gravity — stub.

use crate::ephemeris::SolarSystemState;

/// Point-mass gravity model.
#[derive(Debug, Default)]
pub struct PointMassGravity;

impl PointMassGravity {
    /// Compute gravitational acceleration from all celestial bodies.
    pub fn acceleration(
        &self,
        position: &apogee_common::Position,
        celestial: &SolarSystemState,
    ) -> nalgebra::Vector3<f64> {
        // TODO: sum GM_i * (r_i - r) / |r_i - r|^3
        let _ = (position, celestial);
        nalgebra::Vector3::zeros()
    }
}
