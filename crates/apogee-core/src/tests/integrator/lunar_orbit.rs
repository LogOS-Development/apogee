//! Lunar orbit propagation integration test.
//!
//! Propagates a trajectory around the Moon using a simple Earth-Moon point-mass
//! N-body model and a fixed-step RK4 integrator. The initial conditions are
//! loosely inspired by the Apollo 11 lunar orbit: ~110 km altitude, circular
//! orbit period ~2 hours.

use crate::gravity::point_mass::PointMassGravity;
use crate::gravity::{GravitySourceEntry, GravitySources};
use crate::integrator::{Integrator, Rk4, StateVector};
use crate::tests::helpers::point_mass_derivative;
use apogee_common::constants::GM_MOON;
use apogee_common::gravitational_parameter;
use apogee_common::units::{GravitationalParameter, Seconds};
use approx::assert_relative_eq;
use nalgebra::Vector3;

/// Radius of the Moon (m), mean value.
const R_MOON: f64 = 1_737_400.0;

/// Build a single-body Moon-centered model for testing the integrator.
fn moon_system() -> GravitySources {
    let gm = gravitational_parameter(301)
        .map(GravitationalParameter::new)
        .unwrap_or_default();
    GravitySources {
        sources: vec![GravitySourceEntry {
            gm,
            position: Vector3::zeros(),
            spherical_harmonics: None,
        }],
    }
}

/// Query JPL HORIZONS for a geocentric state vector of the given NAIF body
/// at a single epoch. Returns position (m) and velocity (m/s) in the ecliptic
/// J2000 frame. This is intentionally minimal; failures return an error so the
/// test can skip when the network is unavailable.
fn horizons_geocentric_state(
    naif_id: i32,
    start_epoch: &str,
    stop_epoch: &str,
) -> Result<(Vector3<f64>, Vector3<f64>), String> {
    let start = urlencoding::encode(start_epoch);
    let stop = urlencoding::encode(stop_epoch);
    let url = format!(
        "https://ssd.jpl.nasa.gov/api/horizons.api?format=text&COMMAND={naif_id}&OBJ_DATA=NO&MAKE_EPHEM=YES&EPHEM_TYPE=VECTOR&CENTER=@399&START_TIME={start}&STOP_TIME={stop}&STEP_SIZE=1m&OUT_UNITS=KM-S&REF_PLANE=ECLIP&VEC_TABLE=2"
    );

    let response = reqwest::blocking::get(&url)
        .map_err(|e| format!("HORIZONS request failed: {e}"))?
        .text()
        .map_err(|e| format!("HORIZONS response read failed: {e}"))?;

    if response.contains("INPUT ERROR") || response.contains("No such record") {
        return Err(format!("HORIZONS API error in response:\n{response}"));
    }

    parse_horizons_vector_block(&response)
        .ok_or_else(|| "HORIZONS response did not contain a state vector".to_string())
}

/// Parse the first X/Y/Z/VX/VY/VZ block in a HORIZONS text response. Units are
/// assumed km and km/s; returned in m and m/s.
fn parse_horizons_vector_block(text: &str) -> Option<(Vector3<f64>, Vector3<f64>)> {
    let re = regex::Regex::new(
        r"X\s*=\s*([+\-0-9.Ee]+)\s*Y\s*=\s*([+\-0-9.Ee]+)\s*Z\s*=\s*([+\-0-9.Ee]+)\s*\n\s*VX\s*=\s*([+\-0-9.Ee]+)\s*VY\s*=\s*([+\-0-9.Ee]+)\s*VZ\s*=\s*([+\-0-9.Ee]+)",
    )
    .ok()?;
    let caps = re.captures(text)?;
    let pos = Vector3::new(
        caps[1].parse::<f64>().ok()? * 1_000.0,
        caps[2].parse::<f64>().ok()? * 1_000.0,
        caps[3].parse::<f64>().ok()? * 1_000.0,
    );
    let vel = Vector3::new(
        caps[4].parse::<f64>().ok()? * 1_000.0,
        caps[5].parse::<f64>().ok()? * 1_000.0,
        caps[6].parse::<f64>().ok()? * 1_000.0,
    );
    Some((pos, vel))
}

#[test]
fn test_apollo_11_style_lunar_orbit() {
    let gravity = PointMassGravity {};

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
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: nalgebra::Vector3::zeros(),
    };

    let sources = moon_system();
    let mut integrator = Rk4::new(Seconds::new(10.0)); // 10 s fixed step

    let derivative_fn = |s: &StateVector| point_mass_derivative(s, &sources, &gravity);

    // Propagate for one nominal lunar orbit period.
    let result = integrator.step(&mut state, &derivative_fn, Seconds::new(period_expected));
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

#[test]
#[ignore = "requires network access to JPL HORIZONS; set RUSTFLAGS or run with --ignored"]
fn test_moon_geocentric_orbit_vs_horizons_apollo_era() {
    // Apollo 11 lunar orbit insertion epoch, roughly 1969-07-19 21:22 UTC.
    let start_epoch = "1969-07-19T21:22:00";
    let duration_s = 2.0 * 3600.0; // 2 hours, about one lunar orbit

    let (moon_start_pos, moon_start_vel) =
        horizons_geocentric_state(301, start_epoch, "1969-07-19T21:23:00")
            .expect("HORIZONS query for Moon");
    let (moon_end_pos, moon_end_vel_expected) =
        horizons_geocentric_state(301, "1969-07-19T23:22:00", "1969-07-19T23:23:00")
            .expect("HORIZONS query for Moon");

    // Treat the Moon as the test particle and propagate it around the Earth
    // using a point-mass Earth+Moon model in the geocentric inertial frame.
    let gm_earth = gravitational_parameter(399)
        .map(GravitationalParameter::new)
        .unwrap_or_default();
    let sources = GravitySources {
        sources: vec![GravitySourceEntry {
            gm: gm_earth,
            position: Vector3::zeros(),
            spherical_harmonics: None,
        }],
    };

    let mut state = StateVector {
        position: moon_start_pos,
        velocity: moon_start_vel,
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: nalgebra::Vector3::zeros(),
    };

    let gravity = PointMassGravity {};
    let mut integrator = Rk4::new(Seconds::new(60.0)); // 1 minute step
    let derivative_fn = |s: &StateVector| point_mass_derivative(s, &sources, &gravity);

    let result = integrator.step(&mut state, &derivative_fn, Seconds::new(duration_s));
    assert!(result.accepted);

    let position_error_km = (state.position - moon_end_pos).norm() / 1_000.0;
    let velocity_error_ms = (state.velocity - moon_end_vel_expected).norm();

    assert!(
        position_error_km < 100.0,
        "Moon geocentric position error = {position_error_km:.2} km after 2 h"
    );
    assert!(
        velocity_error_ms < 1.0,
        "Moon geocentric velocity error = {velocity_error_ms:.4} m/s after 2 h"
    );
}
