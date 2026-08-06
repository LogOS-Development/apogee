//! Gravity models: point-mass N-body, spherical harmonics, gradient torque.

pub mod gradient_torque;
pub mod point_mass;
pub mod spherical_harmonics;

pub use gradient_torque::*;
pub use point_mass::*;
pub use spherical_harmonics::*;

use apogee_common::units::{AccelerationVector, TorqueVector};

/// Trait for gravity acceleration computation.
pub trait GravityModel: Send + Sync {
    /// Returns acceleration in inertial frame, tagged as [`AccelerationVector`]
    /// (m/s²) at the public API surface.
    fn acceleration(
        &self,
        position: &apogee_common::Position,
        celestial: &crate::ephemeris::SolarSystemState,
    ) -> AccelerationVector;

    /// Returns gravity gradient torque in body frame as a [`TorqueVector`] (N·m).
    fn gradient_torque(
        &self,
        position: &apogee_common::Position,
        inertia: &nalgebra::Matrix3<f64>,
        celestial: &crate::ephemeris::SolarSystemState,
    ) -> TorqueVector;
}
