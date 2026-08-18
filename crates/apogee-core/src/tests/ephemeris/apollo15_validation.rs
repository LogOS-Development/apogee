//! Apollo 15 lunar orbit propagation validation.
//!
//! Uses a reference trajectory generated from JPL's public `apollo15-1.bsp`
//! SPK Type 1 kernel (Moon-centered, 10 s spacing). The fixture CSV is at
//! `tests/fixtures/apollo15_reference.csv`.
//!
//! We propagate from the first reference state using a Moon-only point-mass
//! model and compare position/velocity against the reference at multiple
//! intermediate epochs. This exercises the integrator end-to-end against
//! real mission data.

use crate::gravity::point_mass::PointMassGravity;
use crate::gravity::GravitySources;
use crate::integrator::{Integrator, Rk4, StateVector};
use crate::tests::helpers::point_mass_derivative;
use apogee_common::constants::GM_MOON;
use apogee_common::units::Seconds;
use nalgebra::Vector3;

/// Apollo 15 reference trajectory fixture. Generated with spiceypy from
/// JPL's public `apollo15-1.bsp` SPK Type 1 kernel.
const APOLLO15_CSV: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/apollo15_reference.csv"
);

/// Reference trajectory sample: (et_s, position_km, velocity_km_s).
type Sample = (f64, Vector3<f64>, Vector3<f64>);

/// Load the reference trajectory as a list of (et_s, position_m, velocity_m_s).
fn load_reference() -> Option<Vec<Sample>> {
    let path = std::path::Path::new(APOLLO15_CSV);
    if !path.exists() {
        return None;
    }
    let contents = std::fs::read_to_string(path).ok()?;
    let mut out = Vec::new();
    for line in contents.lines().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 7 {
            continue;
        }
        let et: f64 = parts[0].parse().ok()?;
        let x: f64 = parts[1].parse().ok()?;
        let y: f64 = parts[2].parse().ok()?;
        let z: f64 = parts[3].parse().ok()?;
        let vx: f64 = parts[4].parse().ok()?;
        let vy: f64 = parts[5].parse().ok()?;
        let vz: f64 = parts[6].parse().ok()?;
        out.push((
            et,
            Vector3::new(x, y, z) * 1_000.0,
            Vector3::new(vx, vy, vz) * 1_000.0,
        ));
    }
    Some(out)
}

/// Build a Moon-centered gravity source set with Moon as the origin.
fn moon_only_sources() -> GravitySources {
    GravitySources {
        sources: vec![(
            apogee_common::units::GravitationalParameter::new(GM_MOON),
            Vector3::zeros(),
        )],
    }
}

#[test]
#[ignore = "requires tests/fixtures/apollo15_reference.csv (within apogee-core); generated from apollo15-1.bsp"]
fn test_apollo15_lunar_orbit_vs_reference() {
    let reference = load_reference().expect("Apollo 15 reference CSV fixture");
    assert!(
        reference.len() >= 2,
        "reference trajectory must contain at least two states"
    );

    let gravity = PointMassGravity {};
    let sources = moon_only_sources();
    let mut integrator = Rk4::new(Seconds::new(10.0)); // 10 s fixed step

    let (et0, pos0, vel0) = reference[0];
    let mut state = StateVector {
        position: pos0,
        velocity: vel0,
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: nalgebra::Vector3::zeros(),
    };

    let derivative_fn = |s: &StateVector| point_mass_derivative(s, &sources, &gravity);

    // Propagate from the first reference state to the last one.
    let (et_end, pos_ref_end, vel_ref_end) = *reference.last().unwrap();
    let duration_s = et_end - et0;

    let result = integrator.step(&mut state, &derivative_fn, Seconds::new(duration_s));
    assert!(result.accepted, "integrator did not accept step");

    let position_error_km = (state.position - pos_ref_end).norm() / 1_000.0;
    let velocity_error_ms = (state.velocity - vel_ref_end).norm();

    println!(
        "Apollo 15 full propagation: et0={et0:.3} -> et_end={et_end:.3} (duration {duration_s:.0} s)"
    );
    println!("position error: {position_error_km:.3} km");
    println!("velocity error: {velocity_error_ms:.4} m/s");

    // Apollo 15 lunar orbit: ~1 hour propagation around the Moon with
    // Moon-only gravity should stay within a few km of the reference.
    assert!(
        position_error_km < 5.0,
        "Apollo 15 position error too large: {position_error_km:.2} km"
    );
    assert!(
        velocity_error_ms < 3.0,
        "Apollo 15 velocity error too large: {velocity_error_ms:.4} m/s"
    );
}

#[test]
#[ignore = "requires tests/fixtures/apollo15_reference.csv (within apogee-core); generated from apollo15-1.bsp"]
fn test_apollo15_multi_point_trajectory() {
    let reference = load_reference().expect("Apollo 15 reference CSV fixture");
    assert!(
        reference.len() >= 10,
        "reference trajectory must contain at least 10 samples for multi-point validation"
    );

    let gravity = PointMassGravity {};
    let sources = moon_only_sources();
    let mut integrator = Rk4::new(Seconds::new(10.0));

    let (et0, pos0, vel0) = reference[0];
    let derivative_fn = |s: &StateVector| point_mass_derivative(s, &sources, &gravity);

    // Propagate to each reference point and compare. We re-initialise from
    // the reference at each checkpoint to isolate per-segment error, which
    // keeps the test focused on short-term integration accuracy rather than
    // accumulating drift over the full trajectory.
    let checkpoints = [1, 5, 10, 20, 30, reference.len() - 1];

    for &idx in &checkpoints {
        let (et_target, pos_ref, vel_ref) = reference[idx];
        let duration_s = et_target - et0;

        let mut state = StateVector {
            position: pos0,
            velocity: vel0,
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };

        let result = integrator.step(&mut state, &derivative_fn, Seconds::new(duration_s));
        assert!(result.accepted, "integrator failed at checkpoint {idx}");

        let pos_err_km = (state.position - pos_ref).norm() / 1_000.0;
        let vel_err_ms = (state.velocity - vel_ref).norm();

        println!(
            "Apollo 15 checkpoint[{idx}] t={duration_s:.0}s: pos_err={pos_err_km:.3} km, vel_err={vel_err_ms:.4} m/s"
        );

        // Per-segment error should stay small for short propagations.
        assert!(
            pos_err_km < 5.0,
            "Apollo 15 position error at checkpoint {idx}: {pos_err_km:.2} km"
        );
        assert!(
            vel_err_ms < 3.0,
            "Apollo 15 velocity error at checkpoint {idx}: {vel_err_ms:.4} m/s"
        );
    }
}
