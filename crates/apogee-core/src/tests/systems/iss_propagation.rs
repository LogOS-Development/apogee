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

use apogee_common::constants::R_EARTH_EQ;
use apogee_common::units::{Area, Kilograms, Seconds};
use nalgebra::Vector3;

use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::{RigidBody, SimulationConfig, SpacecraftConfig};
use crate::components::spacecraft::SpacecraftBundle;
use crate::ephemeris::kernel::{BodyState, SolarSystemState};
use crate::systems::step::{propagate, propagate_single, step_world};
use crate::tle::Tle;
use crate::world::World;

/// ISS TLE snapshot from Celestrak (2026-07-31). Used as a fixed fixture so the
/// test is deterministic. Replace with a historical fixture once J2/EOP are in.
const ISS_TLE: &str = "ISS (ZARYA)             \r\n\
1 25544U 98067A   26212.89378683  .00008757  00000+0  16519-3 0  9996\r\n\
2 25544  51.6315  78.8506 0007211 358.5886   1.5081 15.49290909578688";

fn iss_bundle() -> (Tle, SpacecraftBundle, SimulationConfig) {
    let tle = Tle::parse(ISS_TLE).expect("embedded ISS TLE should parse");
    let (pos, vel) = tle.to_state_vector();
    let bundle = SpacecraftBundle {
        kinematics: Kinematics {
            position: pos,
            velocity: vel,
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: Vector3::zeros(),
        },
        rigid_body: RigidBody {
            mass: Kilograms::new(420_000.0),
            inertia: nalgebra::Matrix3::identity() * 1e7,
            cg_offset: Vector3::zeros(),
        },
        config: SpacecraftConfig {
            ballistic_coefficient: 1e-4,
            srp_area: Area::new(2_500.0),
            reflectivity: 1.2,
            reference_mass_kg: 420_000.0,
        },
    };
    (tle, bundle, SimulationConfig::default())
}

fn earth_only_celestial() -> SolarSystemState {
    SolarSystemState {
        states: vec![BodyState {
            naif_id: 399,
            position: Vector3::zeros(),
            velocity: Vector3::zeros(),
        }],
    }
}

#[test]
fn test_iss_one_orbit_energy_conservation() {
    let (_tle, bundle, sim_config) = iss_bundle();
    let celestial = earth_only_celestial();

    let e0 = specific_energy(&bundle.kinematics.position, &bundle.kinematics.velocity);
    let mut bundle = bundle;
    propagate(
        &mut bundle,
        &sim_config,
        &celestial,
        Seconds::new(30.0),
        Seconds::new(5_500.0),
        212,
        0.0,
    );
    let e1 = specific_energy(&bundle.kinematics.position, &bundle.kinematics.velocity);

    let rel_err = (e1 - e0).abs() / e0.abs();
    assert!(
        rel_err < 1e-6,
        "one-orbit energy drift too large: {:.6e}",
        rel_err
    );

    let altitude = bundle.kinematics.position.norm() - R_EARTH_EQ;
    assert!(
        altitude > 350_000.0 && altitude < 500_000.0,
        "altitude out of ISS range: {:.0} m",
        altitude
    );
}

#[test]
fn test_iss_24h_propagation_stays_leo() {
    let (_tle, bundle, sim_config) = iss_bundle();
    let celestial = earth_only_celestial();

    let mut bundle = bundle;
    propagate(
        &mut bundle,
        &sim_config,
        &celestial,
        Seconds::new(60.0),
        Seconds::new(86_400.0),
        212,
        0.0,
    );

    let altitude = bundle.kinematics.position.norm() - R_EARTH_EQ;
    assert!(
        altitude > 300_000.0 && altitude < 500_000.0,
        "24h propagation produced non-LEO altitude: {:.0} m",
        altitude
    );

    let speed = bundle.kinematics.velocity.norm();
    assert!(
        speed > 7_000.0 && speed < 8_000.0,
        "24h propagation produced unrealistic speed: {:.0} m/s",
        speed
    );
}

#[test]
fn test_iss_one_orbit_via_step_world() {
    let (_tle, bundle, sim_config) = iss_bundle();
    let celestial = earth_only_celestial();

    let mut world = World::with_config(sim_config, celestial);
    world.day_of_year = 212;
    world.seconds_utc = 0.0;
    let _entity = world.spawn(bundle);

    let e0 = {
        let b = world.iter().next().unwrap().1;
        specific_energy(&b.kinematics.position, &b.kinematics.velocity)
    };

    // 1 orbit ≈ 5500 s, step at 30 s.
    for _ in 0..184 {
        step_world(&mut world, Seconds::new(30.0));
    }

    let e1 = {
        let b = world.iter().next().unwrap().1;
        specific_energy(&b.kinematics.position, &b.kinematics.velocity)
    };
    let rel_err = (e1 - e0).abs() / e0.abs();
    assert!(
        rel_err < 1e-6,
        "step_world one-orbit energy drift too large: {:.6e}",
        rel_err
    );
}

#[test]
fn test_iss_via_propagate_single() {
    let (_tle, bundle, sim_config) = iss_bundle();
    let celestial = earth_only_celestial();

    let e0 = specific_energy(&bundle.kinematics.position, &bundle.kinematics.velocity);
    let result = propagate_single(
        bundle,
        sim_config,
        celestial,
        Seconds::new(30.0),
        Seconds::new(5_500.0),
        212,
        0.0,
    );
    let e1 = specific_energy(&result.kinematics.position, &result.kinematics.velocity);
    let rel_err = (e1 - e0).abs() / e0.abs();
    assert!(
        rel_err < 1e-6,
        "propagate_single one-orbit energy drift too large: {:.6e}",
        rel_err
    );
}

fn specific_energy(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
    use apogee_common::constants::GM_EARTH;
    vel.norm_squared() / 2.0 - GM_EARTH / pos.norm()
}
