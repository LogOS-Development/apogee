//! Spherical harmonics gravity integration tests.
//!
//! These tests validate the spherical harmonics gravity engine through the
//! full propagation pipeline. They exercise the force aggregator's SH code
//! path (with and without third-body perturbations) and verify that J2
//! perturbations produce the expected nodal regression.
//!
//! Unit tests for the SH acceleration math and file loading live in
//! `crates/apogee-core/src/gravity/spherical_harmonics.rs`.

use apogee_common::constants::{GM_EARTH, GM_MOON, R_EARTH_EQ};
use apogee_common::units::{GravitationalParameter, Kilograms, Seconds};
use hifitime::Epoch;
use nalgebra::Vector3;

use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::{RigidBody, SimulationConfig};
use crate::gravity::SphericalHarmonics;
use crate::orbit::{j2_nodal_regression_rate, raan};
use crate::systems::step::{propagate, SimContext};

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

/// Build a circular orbit state vector at a given altitude and inclination.
fn circular_orbit(altitude: f64, inclination_deg: f64) -> (Vector3<f64>, Vector3<f64>) {
    let a = R_EARTH_EQ + altitude;
    let v_circ = (GM_EARTH / a).sqrt();
    let inc = inclination_deg.to_radians();
    let pos = Vector3::new(a, 0.0, 0.0);
    let vel = Vector3::new(0.0, v_circ * inc.cos(), v_circ * inc.sin());
    (pos, vel)
}

/// Build a simple 1000 kg spacecraft rigid body for propagation.
fn test_rigid_body() -> RigidBody {
    RigidBody {
        mass: Kilograms::new(1_000.0),
        inertia: nalgebra::Matrix3::identity(),
        cg_offset: Vector3::zeros(),
    }
}

/// Build a SimContext with J2-only SH gravity.
///
/// The central body (Earth) is included as the first gravity source so that
/// third-body perturbations can be added as additional sources. The SH model
/// handles the central body's gravity; the force aggregator skips source[0]
/// when computing third-body perturbations.
fn j2_context(epoch: Epoch) -> SimContext {
    let mut gravity_sources = crate::gravity::GravitySources::new();
    gravity_sources.push(
        apogee_common::units::GravitationalParameter::new(GM_EARTH),
        Vector3::zeros(),
    );
    SimContext {
        sim_config: SimulationConfig::default(),
        gravity_sources,
        sun_position: Vector3::new(-apogee_common::constants::AU, 0.0, 0.0),
        epoch,
        gravity_model: Some(SphericalHarmonics::j2_only()),
    }
}

/// Build a SimContext with point-mass gravity only (no SH).
fn point_mass_context(epoch: Epoch) -> SimContext {
    SimContext::single_body(GravitationalParameter::new(GM_EARTH), epoch)
}

fn kinematics(pos: Vector3<f64>, vel: Vector3<f64>) -> Kinematics {
    Kinematics {
        position: pos,
        velocity: vel,
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: Vector3::zeros(),
    }
}

// -----------------------------------------------------------------------
// J2 nodal regression validation
// -----------------------------------------------------------------------

#[test]
fn test_j2_nodal_regression_matches_analytical() {
    // Propagate a LEO satellite with a J2-only spherical harmonics gravity
    // model and verify the nodal regression rate matches the analytical
    // formula.
    //
    // We use a simple circular orbit at 400 km altitude, 51.6 deg inclination
    // (ISS-like), propagated for 3 orbits (~5.5 hours). The RAAN drift over
    // this period should match the analytical J2 prediction within the
    // integrator's accuracy.

    let altitude = 400_000.0_f64;
    let a = R_EARTH_EQ + altitude;
    let e = 0.0_f64;
    let inclination = 51.6_f64.to_radians();
    let (pos0, vel0) = circular_orbit(altitude, 51.6);

    let rb = test_rigid_body();
    let mut ctx = j2_context(Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0));

    let mut kin = kinematics(pos0, vel0);
    let dt = Seconds::new(10.0);
    let duration = Seconds::new(16_500.0); // ~3 orbits

    let raan0 = raan(&kin.position, &kin.velocity);
    propagate(&mut kin, &rb, None, None, &mut ctx, dt, duration);
    let raan1 = raan(&kin.position, &kin.velocity);

    let raan_drift = raan1 - raan0;

    // Analytical J2 nodal regression rate (rad/s).
    // Unnormalized J2 = -sqrt(5) * C_2,0.
    let sh_model = SphericalHarmonics::j2_only();
    let j2 = -5.0_f64.sqrt() * sh_model.c[2][0];
    let omega_dot = j2_nodal_regression_rate(GM_EARTH, a, e, inclination, j2, R_EARTH_EQ);
    let expected_drift = omega_dot * duration.into_value();

    let rel_err = (raan_drift - expected_drift).abs() / expected_drift.abs();
    assert!(
        rel_err < 0.05,
        "J2 nodal regression mismatch: numerical={raan_drift:.6e} rad, \
         analytical={expected_drift:.6e} rad, rel_err={rel_err:.4}"
    );
}

// -----------------------------------------------------------------------
// SH vs point-mass comparison
// -----------------------------------------------------------------------

#[test]
fn test_sh_gravity_changes_orbit_vs_point_mass() {
    // Propagating with spherical harmonics gravity should produce a
    // different trajectory than point-mass gravity. The J2 perturbation
    // causes nodal regression that point-mass gravity does not.
    let (pos0, vel0) = circular_orbit(400_000.0, 51.6);
    let rb = test_rigid_body();
    let epoch = Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0);
    let dt = Seconds::new(10.0);
    let duration = Seconds::new(16_500.0);

    // Point-mass propagation.
    let mut kin_pm = kinematics(pos0, vel0);
    let mut ctx_pm = point_mass_context(epoch);
    propagate(&mut kin_pm, &rb, None, None, &mut ctx_pm, dt, duration);

    // SH (J2) propagation.
    let mut kin_sh = kinematics(pos0, vel0);
    let mut ctx_sh = j2_context(epoch);
    propagate(&mut kin_sh, &rb, None, None, &mut ctx_sh, dt, duration);

    // The RAAN should be different: J2 causes regression, point-mass doesn't.
    let raan_pm = raan(&kin_pm.position, &kin_pm.velocity);
    let raan_sh = raan(&kin_sh.position, &kin_sh.velocity);
    let raan_diff = (raan_sh - raan_pm).abs();
    assert!(
        raan_diff > 1e-4,
        "SH gravity should produce different RAAN than point-mass: diff={raan_diff:.6e} rad"
    );
}

// -----------------------------------------------------------------------
// SH gravity acceleration direction
// -----------------------------------------------------------------------

#[test]
fn test_j2_acceleration_dominantly_radial_at_equator() {
    // At a point on the equator, the J2 acceleration should be dominantly
    // radial (pointing toward Earth's center) with a small non-radial
    // component due to the oblateness.
    let pos = Vector3::new(R_EARTH_EQ + 400_000.0, 0.0, 0.0);
    let model = SphericalHarmonics::j2_only();
    let accel = model.acceleration(&pos).unwrap();
    let radial = accel.raw().x;
    let non_radial = (accel.raw().y.powi(2) + accel.raw().z.powi(2)).sqrt();

    assert!(
        radial.abs() > 100.0 * non_radial.abs(),
        "radial component should dominate at equator: radial={radial:.6e}, \
         non_radial={non_radial:.6e}"
    );
    // Acceleration should be negative (toward center).
    assert!(
        radial < 0.0,
        "radial acceleration should be toward center (negative x): {radial:.6e}"
    );
}

#[test]
fn test_j2_acceleration_has_z_component_at_pole() {
    // At the pole, the J2 acceleration should be purely radial (along z).
    // The point-mass part is along -z, and the J2 correction at the pole
    // is also along z but with a different magnitude.
    let pos = Vector3::new(0.0, 0.0, R_EARTH_EQ + 400_000.0);
    let model = SphericalHarmonics::j2_only();
    let accel = model.acceleration(&pos).unwrap();

    // x and y components should be negligible.
    assert!(
        accel.raw().x.abs() < 1e-6,
        "x component should be ~0 at pole: {}",
        accel.raw().x
    );
    assert!(
        accel.raw().y.abs() < 1e-6,
        "y component should be ~0 at pole: {}",
        accel.raw().y
    );
    // z should be negative (toward center).
    assert!(
        accel.raw().z < 0.0,
        "z acceleration should be toward center at pole: {}",
        accel.raw().z
    );
}

// -----------------------------------------------------------------------
// Force aggregator with SH + third-body
// -----------------------------------------------------------------------

#[test]
fn test_sh_with_third_body_perturbation() {
    // When a SH gravity model is provided AND there are multiple gravity
    // sources, the force aggregator should compute SH for the central body
    // and add point-mass perturbations from other sources.
    //
    // We verify this by propagating with SH + a fake second body, and
    // checking that the trajectory differs from SH-only.
    let (pos0, vel0) = circular_orbit(400_000.0, 51.6);
    let rb = test_rigid_body();
    let epoch = Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0);
    let dt = Seconds::new(10.0);
    let duration = Seconds::new(5_500.0); // ~1 orbit

    // SH-only propagation.
    let mut kin_sh_only = kinematics(pos0, vel0);
    let mut ctx_sh_only = j2_context(epoch);
    propagate(
        &mut kin_sh_only,
        &rb,
        None,
        None,
        &mut ctx_sh_only,
        dt,
        duration,
    );

    // SH + third-body propagation: add a Moon-mass body far away.
    let mut kin_sh_3body = kinematics(pos0, vel0);
    let mut ctx_sh_3body = j2_context(epoch);
    ctx_sh_3body.gravity_sources.push(
        GravitationalParameter::new(GM_MOON),
        Vector3::new(384_400_000.0, 0.0, 0.0),
    );
    propagate(
        &mut kin_sh_3body,
        &rb,
        None,
        None,
        &mut ctx_sh_3body,
        dt,
        duration,
    );

    // The trajectories should differ due to the third-body perturbation.
    let diff = (kin_sh_only.position - kin_sh_3body.position).norm();
    assert!(
        diff > 1.0,
        "third-body perturbation should produce a measurable difference: {diff:.6e} m"
    );
}

// -----------------------------------------------------------------------
// Backward compatibility: no SH model = point-mass
// -----------------------------------------------------------------------

#[test]
fn test_no_gravity_model_falls_back_to_point_mass() {
    // When gravity_model is None, the force aggregator should use
    // point-mass gravity. This is the backward-compatible path.
    let (pos0, vel0) = circular_orbit(400_000.0, 0.0); // equatorial
    let rb = test_rigid_body();
    let epoch = Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0);
    let dt = Seconds::new(10.0);
    let duration = Seconds::new(5_500.0);

    // Both contexts use point-mass (no SH model).
    let mut kin1 = kinematics(pos0, vel0);
    let mut ctx1 = point_mass_context(epoch);
    propagate(&mut kin1, &rb, None, None, &mut ctx1, dt, duration);

    let mut kin2 = kinematics(pos0, vel0);
    let mut ctx2 = SimContext {
        gravity_model: None,
        ..point_mass_context(epoch)
    };
    propagate(&mut kin2, &rb, None, None, &mut ctx2, dt, duration);

    // Both should produce identical trajectories.
    let diff = (kin1.position - kin2.position).norm();
    assert!(
        diff < 1e-10,
        "None gravity_model should be identical to point-mass: diff={diff:.6e}"
    );
}

// -----------------------------------------------------------------------
// Energy conservation with SH gravity
// -----------------------------------------------------------------------

#[test]
fn test_j2_orbit_energy_perturbation_is_small() {
    // The J2 perturbation is conservative — it does not dissipate energy.
    // Over a few orbits, the total energy should not drift significantly.
    // The energy will oscillate (J2 exchanges between elements), but the
    // average should be stable.
    let (pos0, vel0) = circular_orbit(400_000.0, 51.6);
    let rb = test_rigid_body();
    let epoch = Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0);

    let mut kin = kinematics(pos0, vel0);
    let mut ctx = j2_context(epoch);

    let e0 = crate::orbit::specific_energy_earth(&kin.position, &kin.velocity);
    propagate(
        &mut kin,
        &rb,
        None,
        None,
        &mut ctx,
        Seconds::new(10.0),
        Seconds::new(16_500.0), // ~3 orbits
    );
    let e1 = crate::orbit::specific_energy_earth(&kin.position, &kin.velocity);

    // J2 is a conservative force. Energy should not drift more than ~0.1%
    // (allowing for integrator error + short-period J2 oscillation).
    let rel_drift = (e1 - e0).abs() / e0.abs();
    assert!(
        rel_drift < 1e-3,
        "J2 orbit energy drift too large: {rel_drift:.6e}"
    );
}
