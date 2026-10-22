//! Apollo 15 lunar orbit propagation validation.
//!
//! Uses a reference trajectory generated from JPL's public `apollo15-1.bsp`
//! SPK Type 1 kernel (Moon-centered, 10 s spacing). The fixture CSV is at
//! `tests/fixtures/apollo15_reference.csv`.
//!
//! The full-arc test propagates one 3600 s pass with a point-mass Moon +
//! fixed-position Earth model and accepts a model-limited position
//! tolerance (the real lunar field's mascons, J2, and tides are
//! unmodelled). The segment test re-initialises from the reference at
//! every 60 s sample, so its tolerances stay tight enough to catch unit,
//! frame, and GM blunders. Both exercise the integrator end-to-end
//! against real mission data.

use crate::gravity::point_mass::PointMassGravity;
use crate::gravity::{GravitySourceEntry, GravitySources};
use crate::integrator::{Integrator, Rk4, StateVector};
use crate::tests::helpers::point_mass_derivative;
use apogee_common::constants::GM_MOON;
use apogee_common::units::{GravitationalParameter, Seconds};
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
///
/// The CSV is a committed fixture, so a missing file or malformed line is a
/// repository defect, not a runtime condition — fail loudly.
fn load_reference() -> Vec<Sample> {
    let path = std::path::Path::new(APOLLO15_CSV);
    assert!(
        path.exists(),
        "committed fixture missing: {}",
        path.display()
    );
    let contents = std::fs::read_to_string(path).expect("read apollo15_reference.csv");
    let mut out = Vec::new();
    for (i, line) in contents.lines().enumerate().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        assert_eq!(parts.len(), 7, "malformed CSV line {i}: {line:?}");
        let field = |j: usize| {
            parts[j]
                .parse::<f64>()
                .unwrap_or_else(|e| panic!("CSV line {i}, field {j}: {e}"))
        };
        out.push((
            field(0),
            Vector3::new(field(1), field(2), field(3)) * 1_000.0,
            Vector3::new(field(4), field(5), field(6)) * 1_000.0,
        ));
    }
    out
}

/// Build a Moon-centered gravity source set with Moon at origin and Earth
/// as a third-body perturbation.
fn moon_earth_sources() -> GravitySources {
    GravitySources {
        sources: vec![
            GravitySourceEntry {
                gm: GravitationalParameter::new(GM_MOON),
                position: Vector3::zeros(),
                spherical_harmonics: None,
            },
            GravitySourceEntry {
                gm: GravitationalParameter::new(apogee_common::constants::GM_EARTH),
                position: Vector3::new(384_400_000.0, 0.0, 0.0),
                spherical_harmonics: None,
            },
        ],
    }
}

#[test]
fn test_apollo15_lunar_orbit_vs_reference() {
    let reference = load_reference();
    assert!(
        reference.len() >= 2,
        "reference trajectory must contain at least two states"
    );

    let gravity = PointMassGravity {};
    let sources = moon_earth_sources();
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

    // Model-limited tolerance: the reference is the real Apollo 15
    // trajectory (mascons, J2, solar tides included); the test model is
    // point-mass Moon + fixed-position Earth. The measured unmodelled
    // acceleration of ~1.4e-3 m/s^2 accumulates ~14.7 km over this
    // 3600 s arc. 25 km keeps ~1.7x headroom over measured model error
    // while still failing loudly on unit/GM/frame blunders, which
    // produce hundreds of km. Tightening requires a lunar
    // spherical-harmonics model.
    assert!(
        position_error_km < 25.0,
        "Apollo 15 position error too large: {position_error_km:.2} km"
    );
    assert!(
        velocity_error_ms < 3.0,
        "Apollo 15 velocity error too large: {velocity_error_ms:.4} m/s"
    );
}

#[test]
fn test_apollo15_multi_point_trajectory() {
    let reference = load_reference();
    assert!(
        reference.len() >= 10,
        "reference trajectory must contain at least 10 samples for multi-point validation"
    );

    let gravity = PointMassGravity {};
    let sources = moon_earth_sources();
    let mut integrator = Rk4::new(Seconds::new(10.0));

    let derivative_fn = |s: &StateVector| point_mass_derivative(s, &sources, &gravity);

    // Re-initialise from the previous reference state for every
    // consecutive 60 s arc. Segment-level error sees only integration
    // error plus the unmodelled lunar field over one short arc, so the
    // tolerances stay tight: any unit (km/m), frame, or GM blunder
    // produces orders of magnitude more than these bounds.
    let mut max_pos_err_km = 0.0_f64;
    let mut max_vel_err_ms = 0.0_f64;
    for i in 1..reference.len() {
        let (et_prev, pos_prev, vel_prev) = reference[i - 1];
        let (et_i, pos_ref, vel_ref) = reference[i];
        let duration_s = et_i - et_prev;

        let mut state = StateVector {
            position: pos_prev,
            velocity: vel_prev,
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };

        let result = integrator.step(&mut state, &derivative_fn, Seconds::new(duration_s));
        assert!(result.accepted, "integrator failed at segment {i}");

        max_pos_err_km = max_pos_err_km.max((state.position - pos_ref).norm() / 1_000.0);
        max_vel_err_ms = max_vel_err_ms.max((state.velocity - vel_ref).norm());
    }

    println!(
        "Apollo 15 per-segment maxima over {} arcs: pos_err={max_pos_err_km:.4} km, vel_err={max_vel_err_ms:.4} m/s",
        reference.len() - 1
    );

    assert!(
        max_pos_err_km < 0.05,
        "Apollo 15 per-segment position error too large: {max_pos_err_km:.4} km"
    );
    assert!(
        max_vel_err_ms < 0.5,
        "Apollo 15 per-segment velocity error too large: {max_vel_err_ms:.4} m/s"
    );
}
