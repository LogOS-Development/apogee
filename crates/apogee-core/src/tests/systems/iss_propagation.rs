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

use crate::components::drag_surfaces::{DragSurface, DragSurfaces};
use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::{RigidBody, SimulationConfig};
use crate::components::srp_surfaces::{SrpSurface, SrpSurfaces};
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
    SimContext::single_body(GM_EARTH, epoch)
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
    vel.norm_squared() / 2.0 - GM_EARTH / pos.norm()
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

// ------------------------------------------------------------------
// J2 nodal regression validation (issue #138)
// ------------------------------------------------------------------

/// Analytical J2 nodal regression rate (rad/s).
///
/// Omega_dot = -3/2 * n * J2 * (R_eq/a)^2 * cos(i) / (1-e^2)^2
///
/// where n = sqrt(GM/a^3) is the mean motion, J2 is the unnormalized
/// zonal harmonic, a is the semi-major axis, and i is the inclination.
fn j2_nodal_regression_rate(gm: f64, a: f64, e: f64, inclination: f64, j2: f64, r_eq: f64) -> f64 {
    let n = (gm / a.powi(3)).sqrt();
    let cos_i = inclination.cos();
    let e2 = e * e;
    -1.5 * n * j2 * (r_eq / a).powi(2) * cos_i / (1.0 - e2).powi(2)
}

/// Extract the right ascension of the ascending node (RAAN) from a state vector.
fn raan(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
    let h = pos.cross(vel);
    let n = Vector3::new(0.0, 0.0, 1.0).cross(&h);
    if n.norm() < 1e-10 {
        return 0.0;
    }
    n.y.atan2(n.x)
}

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
    let v_circ = (GM_EARTH / a).sqrt();

    // Set up initial state: position at ascending node, velocity in the
    // orbital plane with the given inclination.
    let pos0 = Vector3::new(a, 0.0, 0.0);
    let vel0 = Vector3::new(0.0, v_circ * inclination.cos(), v_circ * inclination.sin());

    let kinematics = Kinematics {
        position: pos0,
        velocity: vel0,
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: Vector3::zeros(),
    };
    let rigid_body = RigidBody {
        mass: Kilograms::new(1_000.0),
        inertia: nalgebra::Matrix3::identity(),
        cg_offset: Vector3::zeros(),
    };

    // Build a J2-only spherical harmonics model.
    let mut sh_model = crate::gravity::SphericalHarmonics::new(2, 0);
    // EGM2008 tide-free fully normalized C_2,0.
    sh_model.c[2][0] = -0.484165143790815e-03;

    let mut ctx = SimContext {
        sim_config: SimulationConfig::default(),
        gravity_sources: crate::gravity::GravitySources::new(),
        sun_position: Vector3::new(-apogee_common::constants::AU, 0.0, 0.0),
        epoch: Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0),
        gravity_model: Some(sh_model.clone()),
    };

    let raan0 = raan(&kinematics.position, &kinematics.velocity);

    let mut kin = kinematics;
    let dt = Seconds::new(10.0);
    let duration = Seconds::new(16_500.0); // ~3 orbits
    crate::systems::step::propagate(&mut kin, &rigid_body, None, None, &mut ctx, dt, duration);

    let raan1 = raan(&kin.position, &kin.velocity);

    // Total RAAN drift over the propagation.
    let raan_drift = raan1 - raan0;

    // Analytical J2 nodal regression rate (rad/s).
    // Unnormalized J2 = -sqrt(5) * C_2,0.
    let j2 = -5.0_f64.sqrt() * sh_model.c[2][0];
    let omega_dot = j2_nodal_regression_rate(GM_EARTH, a, e, inclination, j2, R_EARTH_EQ);
    let expected_drift = omega_dot * duration.into_value();

    // The numerical and analytical RAAN drift should agree to within ~5%
    // (the integrator introduces some error, and the analytical formula is
    // first-order in J2).
    let rel_err = (raan_drift - expected_drift).abs() / expected_drift.abs();
    assert!(
        rel_err < 0.05,
        "J2 nodal regression mismatch: numerical={raan_drift:.6e} rad, \
         analytical={expected_drift:.6e} rad, rel_err={rel_err:.4}"
    );
}

#[test]
fn test_sh_gravity_changes_orbit_vs_point_mass() {
    // Propagating with spherical harmonics gravity should produce a
    // different trajectory than point-mass gravity. The J2 perturbation
    // causes nodal regression that point-mass gravity does not.
    let altitude = 400_000.0_f64;
    let a = R_EARTH_EQ + altitude;
    let v_circ = (GM_EARTH / a).sqrt();
    let inclination = 51.6_f64.to_radians();

    let pos0 = Vector3::new(a, 0.0, 0.0);
    let vel0 = Vector3::new(0.0, v_circ * inclination.cos(), v_circ * inclination.sin());

    let rb = RigidBody {
        mass: Kilograms::new(1_000.0),
        inertia: nalgebra::Matrix3::identity(),
        cg_offset: Vector3::zeros(),
    };

    // Point-mass propagation.
    let mut kin_pm = Kinematics {
        position: pos0,
        velocity: vel0,
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: Vector3::zeros(),
    };
    let mut ctx_pm =
        SimContext::single_body(GM_EARTH, Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0));
    propagate(
        &mut kin_pm,
        &rb,
        None,
        None,
        &mut ctx_pm,
        Seconds::new(10.0),
        Seconds::new(16_500.0),
    );

    // SH (J2) propagation.
    let mut sh_model = crate::gravity::SphericalHarmonics::new(2, 0);
    sh_model.c[2][0] = -0.484165143790815e-03;

    let mut kin_sh = Kinematics {
        position: pos0,
        velocity: vel0,
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: Vector3::zeros(),
    };
    let mut ctx_sh = SimContext {
        sim_config: SimulationConfig::default(),
        gravity_sources: crate::gravity::GravitySources::new(),
        sun_position: Vector3::new(-apogee_common::constants::AU, 0.0, 0.0),
        epoch: Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0),
        gravity_model: Some(sh_model),
    };
    propagate(
        &mut kin_sh,
        &rb,
        None,
        None,
        &mut ctx_sh,
        Seconds::new(10.0),
        Seconds::new(16_500.0),
    );

    // The RAAN should be different: J2 causes regression, point-mass doesn't.
    let raan_pm = raan(&kin_pm.position, &kin_pm.velocity);
    let raan_sh = raan(&kin_sh.position, &kin_sh.velocity);
    let raan_diff = (raan_sh - raan_pm).abs();
    assert!(
        raan_diff > 1e-4,
        "SH gravity should produce different RAAN than point-mass: diff={raan_diff:.6e} rad"
    );
}
