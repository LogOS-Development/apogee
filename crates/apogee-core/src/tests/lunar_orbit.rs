//! Lunar orbit propagation integration test.
//!
//! Propagates a trajectory around the Moon using a simple Earth-Moon point-mass
//! N-body model and a fixed-step RK4 integrator. The initial conditions are
//! loosely inspired by the Apollo 11 lunar orbit: ~110 km altitude, circular
//! orbit period ~2 hours.

use crate::ephemeris::kernel::{BodyState, SolarSystemState};
use crate::gravity::point_mass::PointMassGravity;
use crate::integrator::{Integrator, Rk4, StateDerivative, StateVector};
use apogee_common::constants::GM_MOON;
use approx::assert_relative_eq;
use nalgebra::Vector3;

/// Radius of the Moon (m), mean value.
const R_MOON: f64 = 1_737_400.0;

/// Build a single-body Moon-centered model for testing the integrator.
fn moon_system() -> SolarSystemState {
    SolarSystemState {
        states: vec![BodyState {
            naif_id: 301,
            position: Vector3::zeros(),
            velocity: Vector3::zeros(),
        }],
    }
}

/// Acceleration function for the RK4 integrator.
fn point_mass_derivative(
    state: &StateVector,
    celestial: &SolarSystemState,
    gravity: &PointMassGravity,
) -> StateDerivative {
    let acc = gravity
        .acceleration(&state.position, celestial)
        .expect("valid point-mass acceleration");
    StateDerivative {
        velocity: state.velocity,
        acceleration: acc,
    }
}

#[test]
fn test_apollo_11_style_lunar_orbit() {
    let gravity = PointMassGravity::default();

    // Apollo 11 lunar orbit: ~110 km altitude, nearly circular.
    let altitude = 110_000.0;
    let orbit_radius = R_MOON + altitude;
    let orbital_speed = (GM_MOON / orbit_radius).sqrt();
    let period_expected = 2.0 * std::f64::consts::PI * orbit_radius / orbital_speed;

    // Propagate in the Moon-centered inertial frame. For this first test we
    // ignore Earth/Sun perturbations; a full Apollo trajectory would require
    // moving ephemerides and frame transformations.
    let moon_position = Vector3::zeros();
    let spacecraft_position = Vector3::new(0.0, orbit_radius, 0.0);
    let spacecraft_velocity = Vector3::new(-orbital_speed, 0.0, 0.0);

    let mut state = StateVector {
        position: spacecraft_position,
        velocity: spacecraft_velocity,
    };

    let celestial = moon_system();
    let mut integrator = Rk4::new(10.0); // 10 s fixed step

    let derivative_fn = |s: &StateVector| point_mass_derivative(s, &celestial, &gravity);

    // Propagate for one nominal lunar orbit period.
    let result = integrator.step(&mut state, &derivative_fn, period_expected);
    assert!(result.accepted, "integrator did not accept step");

    // After one period the spacecraft should be back near its starting
    // position relative to the Moon.
    let final_relative = state.position - moon_position;
    let initial_relative = Vector3::new(0.0, orbit_radius, 0.0);

    let position_error = (final_relative - initial_relative).norm();
    assert!(
        position_error < 0.01 * orbit_radius,
        "lunar orbit did not close: position error = {position_error} m after {} s",
        period_expected
    );

    let speed_final = state.velocity.norm();
    assert_relative_eq!(speed_final, orbital_speed, epsilon = 1e-3);

    // Final velocity should point roughly opposite to the initial -x direction.
    assert!(
        state.velocity.x < 0.0,
        "expected final velocity to remain -x"
    );
    assert!(
        state.velocity.y.abs() < 0.1 * speed_final,
        "expected small y velocity component"
    );
}
