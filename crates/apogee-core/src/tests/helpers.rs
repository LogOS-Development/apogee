//! Shared test helpers.

use crate::ephemeris::kernel::SolarSystemState;
use crate::gravity::point_mass::PointMassGravity;
use crate::integrator::{StateDerivative, StateVector};

/// Acceleration function for the RK4 integrator using point-mass gravity.
///
/// Returns a [`StateDerivative`] whose `acceleration` field is the raw m/s²
/// vector extracted from the unit-aware [`AccelerationVec`].
pub fn point_mass_derivative(
    state: &StateVector,
    celestial: &SolarSystemState,
    gravity: &PointMassGravity,
) -> StateDerivative {
    let acceleration = gravity
        .acceleration(&state.position, celestial)
        .expect("valid point-mass acceleration");
    StateDerivative {
        velocity: state.velocity,
        acceleration: *acceleration.raw(),
        attitude_derivative: nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
        angular_acceleration: nalgebra::Vector3::zeros(),
    }
}
