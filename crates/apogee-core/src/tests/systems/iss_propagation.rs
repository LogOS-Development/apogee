//! ISS single-spacecraft propagation validation (Phase 1.6).
//!
//! These tests exercise the end-to-end 6DOF propagation pipeline on a real ISS
//! TLE. They are intentionally sanity checks rather than strict 1 km vs
//! next-day TLE validation, because the latter requires:
//!   - J2 / spherical-harmonic gravity (Phase 1.3 is still point-mass only)
//!   - TEME-to-ICRF frame alignment and EOP
//!   - Historical next-day TLE fixture
//!
//! Those improvements are tracked in follow-up issues.

use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
use apogee_common::units::{Area, Kilograms, Seconds};
use hifitime::Epoch;
use nalgebra::Vector3;

use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::{RigidBody, SimulationConfig, SpacecraftConfig};
use crate::systems::step::{propagate, step_world, SimContext};
use crate::tle::Tle;
use crate::world::World;

/// ISS TLE snapshot from Celestrak (2026-07-31). Used as a fixed fixture so the
/// test is deterministic. Replace with a historical fixture once J2/EOP are in.
const ISS_TLE: &str = "ISS (ZARYA)             \r\n\
1 25544U 98067A   26212.89378683  .00008757  00000+0  16519-3 0  9996\r\n\
2 25544  51.6315  78.8506  0007211 358.5886   1.5081 15.49290909578688";

/// Epoch for the TLE epoch day 26212.89378683 (year 2026, day 213).
fn iss_epoch() -> Epoch {
    // Day 213 of 2026 = 2026-08-01. TLE epoch fractional .89378683 day ≈ 21:27:04.
    Epoch::from_gregorian_utc(2026, 8, 1, 21, 27, 4, 0)
}

fn iss_components() -> (
    Tle,
    Kinematics,
    RigidBody,
    SpacecraftConfig,
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
    let config = SpacecraftConfig {
        ballistic_coefficient: 1e-4,
        srp_area: Area::new(2_500.0),
        reflectivity: 1.2,
        reference_mass_kg: 420_000.0,
    };
    (
        tle,
        kinematics,
        rigid_body,
        config,
        SimulationConfig::default(),
    )
}

/// Build a SimContext with Earth at the origin (single gravity source).
fn earth_only_ctx(epoch: Epoch) -> SimContext {
    SimContext::single_body(GM_EARTH, epoch)
}

#[test]
fn test_iss_one_orbit_energy_conservation() {
    let (_tle, mut kin, rb, cfg, sim_config) = iss_components();
    let mut ctx = SimContext {
        sim_config,
        ..earth_only_ctx(iss_epoch())
    };

    let e0 = specific_energy(&kin.position, &kin.velocity);
    propagate(
        &mut kin,
        &rb,
        &cfg,
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
    let (_tle, mut kin, rb, cfg, sim_config) = iss_components();
    let mut ctx = SimContext {
        sim_config,
        ..earth_only_ctx(iss_epoch())
    };

    propagate(
        &mut kin,
        &rb,
        &cfg,
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
    let (_tle, kin, rb, cfg, _sim_config) = iss_components();

    let mut world = World::with_config_and_epoch(SimulationConfig::default(), iss_epoch());
    // Spawn Earth as a kinematic celestial body at the origin.
    world.add_celestial_body(crate::components::celestial::CelestialBodySpec::kinematic(
        399,
        Vector3::zeros(),
        Vector3::zeros(),
    ));
    let _entity = world.spawn((kin, rb, cfg));

    let e0 = {
        let entity = world
            .ecs
            .query::<(&Kinematics, &SpacecraftConfig)>()
            .iter()
            .next()
            .unwrap()
            .0;
        let kin = world.get_component::<Kinematics>(entity).unwrap();
        specific_energy(&kin.position, &kin.velocity)
    };

    // 1 orbit ≈ 5500 s, step at 30 s.
    for _ in 0..184 {
        step_world(&mut world, Seconds::new(30.0));
    }

    let e1 = {
        let entity = world
            .ecs
            .query::<(&Kinematics, &SpacecraftConfig)>()
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
    let (_tle, mut kin, rb, cfg, sim_config) = iss_components();
    let mut ctx = SimContext {
        sim_config,
        ..earth_only_ctx(iss_epoch())
    };

    let e0 = specific_energy(&kin.position, &kin.velocity);
    propagate(
        &mut kin,
        &rb,
        &cfg,
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
    vel.norm_squared() / 2.0 - GM_EARTH / pos.norm()
}
