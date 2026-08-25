//! ISS single-spacecraft propagation validation (Phase 1.6).
//!
//! These tests exercise the end-to-end 6DOF propagation pipeline on a real ISS
//! TLE. They are sanity checks that verify the propagation pipeline produces
//! physically reasonable trajectories with both point-mass and spherical
//! harmonics gravity models.
//!
//! Spherical harmonics gravity validation tests (J2 nodal regression,
//! SH-vs-point-mass comparison, acceleration direction) live in
//! `tests/gravity/spherical_harmonics.rs`.

use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
use apogee_common::units::{Area, GravitationalParameter, Kilograms, Seconds};
use hifitime::Epoch;
use nalgebra::Vector3;

use crate::components::drag_surfaces::{DragSurface, DragSurfaces};
use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::{RigidBody, SimulationConfig};
use crate::components::srp_surfaces::{SrpSurface, SrpSurfaces};
use crate::gravity::SphericalHarmonics;
use crate::orbit::specific_energy_earth;
use crate::systems::step::{propagate, step_and_advance, SimContext};
use crate::tle::Tle;
use crate::world::World;

/// ISS TLE snapshot from Celestrak (2026-07-31). Used as a fixed fixture so the
/// test is deterministic. Replace with a historical fixture once J2/EOP are in.
const ISS_TLE: &str = "ISS (ZARYA)             \r\n\
1 25544U 98067A   26212.89378683  .00008757  00000+0  16519-3 0  9996\r\n\
2 25544  51.6315  78.8506  0007211 358.5886   1.5081 15.49290909578688";

/// Epoch for the TLE epoch day 26212.89378683 (year 2026, day 213).
fn iss_epoch() -> Epoch {
    // Day 213 of 2026 = 2026-08-01. TLE epoch fractional .89378683 day ~ 21:27:04.
    Epoch::from_gregorian_utc(2026, 8, 1, 21, 27, 4, 0)
}

fn iss_components() -> (
    Tle,
    Kinematics,
    RigidBody,
    DragSurfaces,
    SrpSurfaces,
    SimulationConfig,
) {
    let tle = Tle::parse(ISS_TLE).expect("embedded ISS TLE should parse");
    let (pos, vel) = tle.to_state_vector();
    let kinematics = Kinematics {
        position: pos,
        velocity: vel,
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: Vector3::zeros(),
    };
    let rigid_body = RigidBody {
        mass: Kilograms::new(420_000.0),
        inertia: nalgebra::Matrix3::identity() * 1e7,
        cg_offset: Vector3::zeros(),
    };
    // ISS ballistic coefficient ~1e-4 m^2/kg, mass 420000 kg
    // -> Cd*A = 42 m^2. Use Cd=2.2, A~19 m^2.
    let drag_surfaces = DragSurfaces::from_surfaces(vec![DragSurface::new(Area::new(19.0), 2.2)]);
    let srp_surfaces = SrpSurfaces::from_surfaces(vec![SrpSurface::new(Area::new(2_500.0), 1.2)]);
    (
        tle,
        kinematics,
        rigid_body,
        drag_surfaces,
        srp_surfaces,
        SimulationConfig::default(),
    )
}

/// Build a SimContext with Earth at the origin (single gravity source).
fn earth_only_ctx(epoch: Epoch) -> SimContext {
    SimContext::single_body(GravitationalParameter::new(GM_EARTH), epoch)
}

#[test]
fn test_iss_one_orbit_energy_conservation() {
    let (_tle, mut kin, rb, drag, srp, sim_config) = iss_components();
    let mut ctx = SimContext {
        sim_config,
        ..earth_only_ctx(iss_epoch())
    };

    let e0 = specific_energy(&kin.position, &kin.velocity);
    propagate(
        &mut kin,
        &rb,
        Some(&drag),
        Some(&srp),
        &mut ctx,
        Seconds::new(30.0),
        Seconds::new(5_500.0),
    );
    let e1 = specific_energy(&kin.position, &kin.velocity);

    let rel_err = (e1 - e0).abs() / e0.abs();
    assert!(
        rel_err < 1e-6,
        "one-orbit energy drift too large: {:.6e}",
        rel_err
    );

    let altitude = kin.position.norm() - R_EARTH_EQ;
    assert!(
        altitude > 350_000.0 && altitude < 500_000.0,
        "altitude out of ISS range: {:.0} m",
        altitude
    );
}

#[test]
fn test_iss_24h_propagation_stays_leo() {
    let (_tle, mut kin, rb, drag, srp, sim_config) = iss_components();
    let mut ctx = SimContext {
        sim_config,
        ..earth_only_ctx(iss_epoch())
    };

    propagate(
        &mut kin,
        &rb,
        Some(&drag),
        Some(&srp),
        &mut ctx,
        Seconds::new(60.0),
        Seconds::new(86_400.0),
    );

    let altitude = kin.position.norm() - R_EARTH_EQ;
    assert!(
        altitude > 300_000.0 && altitude < 500_000.0,
        "24h propagation produced non-LEO altitude: {:.0} m",
        altitude
    );

    let speed = kin.velocity.norm();
    assert!(
        speed > 7_000.0 && speed < 8_000.0,
        "24h propagation produced unrealistic speed: {:.0} m/s",
        speed
    );
}

#[test]
fn test_iss_one_orbit_via_step_world() {
    let (_tle, kin, rb, drag, srp, _sim_config) = iss_components();

    let mut world = World::with_config_and_epoch(SimulationConfig::default(), iss_epoch());
    // Spawn Earth as a kinematic celestial body at the origin.
    world.add_celestial_body(crate::components::celestial::CelestialBodySpec::kinematic(
        399,
        Vector3::zeros(),
        Vector3::zeros(),
    ));
    let _entity = world.spawn((kin, rb, drag, srp));

    let e0 = {
        let entity = world
            .ecs
            .query::<(&Kinematics, &DragSurfaces)>()
            .iter()
            .next()
            .unwrap()
            .0;
        let kin = world.get_component::<Kinematics>(entity).unwrap();
        specific_energy(&kin.position, &kin.velocity)
    };

    // 1 orbit ~ 5500 s, step at 30 s.
    for _ in 0..184 {
        step_and_advance(&mut world, Seconds::new(30.0));
    }

    let e1 = {
        let entity = world
            .ecs
            .query::<(&Kinematics, &DragSurfaces)>()
            .iter()
            .next()
            .unwrap()
            .0;
        let kin = world.get_component::<Kinematics>(entity).unwrap();
        specific_energy(&kin.position, &kin.velocity)
    };
    let rel_err = (e1 - e0).abs() / e0.abs();
    assert!(
        rel_err < 1e-6,
        "step_world one-orbit energy drift too large: {:.6e}",
        rel_err
    );
}

#[test]
fn test_iss_via_propagate() {
    let (_tle, mut kin, rb, drag, srp, sim_config) = iss_components();
    let mut ctx = SimContext {
        sim_config,
        ..earth_only_ctx(iss_epoch())
    };

    let e0 = specific_energy(&kin.position, &kin.velocity);
    propagate(
        &mut kin,
        &rb,
        Some(&drag),
        Some(&srp),
        &mut ctx,
        Seconds::new(30.0),
        Seconds::new(5_500.0),
    );
    let e1 = specific_energy(&kin.position, &kin.velocity);
    let rel_err = (e1 - e0).abs() / e0.abs();
    assert!(
        rel_err < 1e-6,
        "propagate one-orbit energy drift too large: {:.6e}",
        rel_err
    );
}

fn specific_energy(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
    specific_energy_earth(pos, vel)
}

#[test]
fn test_drag_srp_changes_orbital_energy() {
    // Verify that drag + SRP surfaces actually change orbital energy
    // compared to a gravity-only propagation. Drag is dissipative, so
    // the energy with surfaces should be lower than the energy without.
    let (_tle, mut kin_with, rb, drag, srp, sim_config) = iss_components();
    let mut kin_without = kin_with.clone();

    let mut ctx_with = SimContext {
        sim_config,
        ..earth_only_ctx(iss_epoch())
    };
    let mut ctx_without = SimContext {
        sim_config,
        ..earth_only_ctx(iss_epoch())
    };

    let e0 = specific_energy(&kin_with.position, &kin_with.velocity);

    // Propagate with drag + SRP for several orbits.
    propagate(
        &mut kin_with,
        &rb,
        Some(&drag),
        Some(&srp),
        &mut ctx_with,
        Seconds::new(10.0),
        Seconds::new(20_000.0), // ~3.6 orbits
    );

    // Propagate without drag + SRP for the same duration.
    propagate(
        &mut kin_without,
        &rb,
        None,
        None,
        &mut ctx_without,
        Seconds::new(10.0),
        Seconds::new(20_000.0),
    );

    let e_with = specific_energy(&kin_with.position, &kin_with.velocity);
    let e_without = specific_energy(&kin_without.position, &kin_without.velocity);

    // The gravity-only propagation should conserve energy closely.
    let drift_without = (e_without - e0).abs() / e0.abs();
    assert!(
        drift_without < 1e-6,
        "gravity-only energy drift too large: {drift_without:.6e}"
    );

    // The propagation with drag + SRP should show a measurably different
    // energy. Drag is dissipative, so energy should decrease (become more
    // negative), meaning |e_with| > |e0| and e_with < e0.
    let energy_change_with = e_with - e0;
    let energy_change_without = e_without - e0;
    assert!(
        energy_change_with.abs() > energy_change_without.abs() * 10.0,
        "drag+SRP should produce a much larger energy change than numerical drift: \
         with={energy_change_with:.6e}, without={energy_change_without:.6e}"
    );

    // Drag is dissipative: the energy should decrease (orbit decays).
    assert!(
        energy_change_with < 0.0,
        "drag should decrease orbital energy (dissipative), got change={energy_change_with:.6e}"
    );
}

// -----------------------------------------------------------------------
// ISS propagation with spherical harmonics gravity
// -----------------------------------------------------------------------

/// Build a SimContext with J2 spherical harmonics gravity for Earth.
fn sh_ctx(epoch: Epoch) -> SimContext {
    SimContext {
        gravity_model: Some(SphericalHarmonics::j2_only()),
        ..earth_only_ctx(epoch)
    }
}

#[test]
fn test_iss_one_orbit_with_j2_stays_leo() {
    // Propagate the ISS TLE for one orbit with J2 gravity. The orbit
    // should remain in LEO and the altitude should be in a reasonable
    // range. J2 causes nodal regression but does not significantly change
    // the semi-major axis over one orbit.
    let (_tle, mut kin, rb, drag, srp, sim_config) = iss_components();
    let mut ctx = SimContext {
        sim_config,
        ..sh_ctx(iss_epoch())
    };

    propagate(
        &mut kin,
        &rb,
        Some(&drag),
        Some(&srp),
        &mut ctx,
        Seconds::new(30.0),
        Seconds::new(5_500.0),
    );

    let altitude = kin.position.norm() - R_EARTH_EQ;
    assert!(
        altitude > 350_000.0 && altitude < 500_000.0,
        "altitude out of ISS range with J2: {:.0} m",
        altitude
    );
}

#[test]
fn test_iss_24h_with_j2_stays_leo() {
    // 24-hour propagation with J2 gravity. J2 causes nodal regression
    // (~5 deg/day for ISS), but the orbit should remain bounded in LEO.
    let (_tle, mut kin, rb, drag, srp, sim_config) = iss_components();
    let mut ctx = SimContext {
        sim_config,
        ..sh_ctx(iss_epoch())
    };

    propagate(
        &mut kin,
        &rb,
        Some(&drag),
        Some(&srp),
        &mut ctx,
        Seconds::new(60.0),
        Seconds::new(86_400.0),
    );

    let altitude = kin.position.norm() - R_EARTH_EQ;
    assert!(
        altitude > 300_000.0 && altitude < 500_000.0,
        "24h J2 propagation produced non-LEO altitude: {:.0} m",
        altitude
    );

    let speed = kin.velocity.norm();
    assert!(
        speed > 7_000.0 && speed < 8_000.0,
        "24h J2 propagation produced unrealistic speed: {:.0} m/s",
        speed
    );
}

#[test]
fn test_j2_produces_different_trajectory_than_point_mass() {
    // Propagating the ISS TLE with J2 gravity should produce a measurably
    // different trajectory than point-mass gravity over 24 hours. The J2
    // perturbation causes nodal regression and apsidal rotation that
    // point-mass gravity does not.
    let (_tle, kin, rb, drag, srp, sim_config) = iss_components();

    // Point-mass propagation.
    let mut kin_pm = kin.clone();
    let mut ctx_pm = SimContext {
        sim_config,
        ..earth_only_ctx(iss_epoch())
    };
    propagate(
        &mut kin_pm,
        &rb,
        Some(&drag),
        Some(&srp),
        &mut ctx_pm,
        Seconds::new(60.0),
        Seconds::new(86_400.0),
    );

    // J2 propagation.
    let mut kin_j2 = kin.clone();
    let mut ctx_j2 = SimContext {
        sim_config,
        ..sh_ctx(iss_epoch())
    };
    propagate(
        &mut kin_j2,
        &rb,
        Some(&drag),
        Some(&srp),
        &mut ctx_j2,
        Seconds::new(60.0),
        Seconds::new(86_400.0),
    );

    // After 24 hours, the positions should differ by more than 10 km
    // (J2 nodal regression is ~5 deg/day for ISS, which at 400 km altitude
    // corresponds to a cross-track difference of tens of km).
    let diff = (kin_pm.position - kin_j2.position).norm();
    assert!(
        diff > 10_000.0,
        "J2 should produce a measurably different trajectory after 24h: diff={diff:.0} m"
    );
}
