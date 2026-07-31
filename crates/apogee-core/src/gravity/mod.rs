//! Gravity models: point-mass N-body, spherical harmonics, gradient torque.

pub mod gradient_torque;
pub mod point_mass;
pub mod spherical_harmonics;

pub use gradient_torque::*;
pub use point_mass::*;
pub use spherical_harmonics::*;

/// Trait for gravity acceleration computation.
pub trait GravityModel: Send + Sync {
    /// Returns acceleration in inertial frame (m/s^2).
    fn acceleration(
        &self,
        position: &apogee_common::Position,
        celestial: &crate::ephemeris::SolarSystemState,
    ) -> nalgebra::Vector3<f64>;

    /// Returns gravity gradient torque in body frame (N m).
    fn gradient_torque(
        &self,
        position: &apogee_common::Position,
        inertia: &nalgebra::Matrix3<f64>,
        celestial: &crate::ephemeris::SolarSystemState,
    ) -> nalgebra::Vector3<f64>;
}
