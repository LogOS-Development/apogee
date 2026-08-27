//! N-body integrator validation against DE441 ephemeris kinematics (#157).
//!
//! Strategy: spawn the Sun and the four inner-planet barycenters as
//! **dynamic** bodies with initial states read from DE441 at a start epoch,
//! integrate forward under mutual point-mass gravity using `step_world`
//! (RK4 fixed-step), and compare the integrated positions against DE441
//! ground truth at checkpoints.
//!
//! This validates the whole critical path end-to-end:
//! kernel evaluation → initial states → ECS spawning → force aggregation →
//! RK4 integration → position comparison.
//!
//! DE441 is a 3.3 GB kernel that is NOT committed to the repo. Tests are
//! `#[ignore]`d by default and only run locally when the kernel exists:
//!
//! ```sh
//! cargo test -p apogee-core --lib tests::ephemeris::nbody_validation -- --ignored
//! ```
//!
//! Bodies and frames: all seven bodies (Sun 10, Mercury barycenter 1,
//! Venus barycenter 2, Earth-Moon barycenter 3, Mars barycenter 4,
//! Jupiter barycenter 5, Saturn barycenter 6) have SSB-relative (center 0)
//! segments in DE441, so no frame or center composition is needed —
//! integrated SSB positions compare directly against kernel SSB positions.
//!
//! Expected accuracy: RK4 fixed-step with first-order inter-body coupling
//! (each body integrates against a frozen snapshot of the others — see
//! issue for joint-integration refactor). Observed at 0.5-day steps over
//! 1 year: ~15,000 km max error (Mercury-dominated, growing ~linearly in
//! step size and ~quadratically in time). The budget below is set to
//! catch systematic errors — missing forces, wrong GM, unit or frame
//! mismatches — which manifest at 1e6+ km, three orders above budget.

use crate::ephemeris::EphemerisService;
use crate::systems::step::step_world;
use crate::world::World;
use apogee_common::units::Seconds;
use hifitime::Epoch;

/// Bodies under test: (NAIF ID, display name).
///
/// Includes the giant planets: Jupiter and Saturn perturb the inner system
/// at the ~1e5 km/year level; omitting them dominates the error budget.
const BODIES: &[(i32, &str)] = &[
    (10, "Sun"),
    (1, "Mercury barycenter"),
    (2, "Venus barycenter"),
    (3, "Earth-Moon barycenter"),
    (4, "Mars barycenter"),
    (5, "Jupiter barycenter"),
    (6, "Saturn barycenter"),
];

/// GM values for the bodies (m³/s²), DE441-consistent.
///
/// For barycenters the GM is the total of the system (e.g. EMB = Earth +
/// Moon), matching how DE441 integrates them.
#[allow(clippy::excessive_precision)] // GM values quoted from DE441 documentation
const GM: &[(i32, f64)] = &[
    (10, 1.32712440041279419e20),
    (1, 2.203186e13),
    (2, 3.2485859200000005e14),
    (3, 4.035032326940000e14), // Earth + Moon
    (4, 4.282837581600000e13),
    (5, 1.267127648000002e17), // Jupiter system
    (6, 3.793118700000000e16), // Saturn system
];

fn de441_path() -> Option<std::path::PathBuf> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ephemeris/de441.bsp");
    path.exists().then_some(path)
}

/// Build a World with the five bodies as dynamic celestials, initial
/// states from DE441 at `epoch`.
fn build_nbody_world(ephemeris: &mut EphemerisService, epoch: Epoch) -> World {
    let mut world = World::new();
    for &(naif_id, _) in BODIES {
        let state = ephemeris.state_at(naif_id, epoch).unwrap();
        let gm = GM
            .iter()
            .find(|(id, _)| *id == naif_id)
            .map(|(_, gm)| *gm)
            .unwrap();
        world.add_celestial_body(crate::components::celestial::CelestialBodySpec {
            naif_id,
            kind: crate::components::celestial::CelestialKind::Dynamic,
            position: state.position,
            velocity: state.velocity,
            gm: Some(apogee_common::units::GravitationalParameter::new(gm)),
            mass: Some(apogee_common::units::Kilograms::new(
                gm / apogee_common::constants::G.into_value(),
            )),
            spherical_harmonics: None,
        });
    }
    world
}

/// Checkpoint comparison: integrated position vs DE441 at each checkpoint.
#[test]
#[ignore = "requires local 3.3 GB DE441 kernel (data/ephemeris/de441.bsp)"]
fn inner_solar_system_nbody_vs_de441() {
    let Some(path) = de441_path() else {
        eprintln!("skipping: DE441 kernel not present");
        return;
    };

    let mut ephemeris = EphemerisService::load(path.to_str().unwrap(), 64).unwrap();

    // Start 2025-01-01, integrate 1 year.
    let epoch0 = Epoch::from_gregorian(2025, 1, 1, 0, 0, 0, 0, hifitime::TimeScale::TDB);
    let mut world = build_nbody_world(&mut ephemeris, epoch0);

    let day = 86_400.0;
    let step = day / 2.0; // 0.5-day RK4 steps
    let total = 365.0 * day;
    let checkpoints = [30.0, 90.0, 180.0, 365.0]; // days

    let mut elapsed = 0.0;
    let mut next_checkpoint = 0;
    let mut results = Vec::new();

    while elapsed < total {
        // Step the world with a fixed-step integrator.
        step_world(&mut world, Seconds::new(step));
        elapsed += step;

        if next_checkpoint < checkpoints.len() && elapsed >= checkpoints[next_checkpoint] * day {
            // Compare every body against DE441 at the current epoch.
            let epoch = epoch0 + elapsed * hifitime::Unit::Second;
            let mut max_err_km: f64 = 0.0;
            for &(naif_id, name) in BODIES {
                let truth = ephemeris.state_at(naif_id, epoch).unwrap();
                let entity = world.find_celestial(naif_id).unwrap();
                let kin = world
                    .get_component::<crate::components::kinematics::Kinematics>(entity)
                    .unwrap();
                let err_km = (kin.position - truth.position).norm() / 1_000.0;
                results.push((checkpoints[next_checkpoint], name, err_km));
                max_err_km = max_err_km.max(err_km);
            }
            eprintln!(
                "day {:>3}: max position error {:.1} km",
                checkpoints[next_checkpoint], max_err_km
            );
            next_checkpoint += 1;
        }
    }

    for &(days, name, err_km) in &results {
        eprintln!("  {name} @ {days} days: {err_km:.1} km");
    }

    // Error budget: catches systematic errors (wrong GM, unit/frame
    // mismatches, missing forces) which appear at 1e6+ km. Truncation +
    // first-order coupling residual at 0.5-day steps is ~1.5e4 km.
    assert!(
        results.iter().all(|&(_, _, e)| e < 100_000.0),
        "position errors exceeded 100,000 km budget"
    );
}

/// Energy conservation of the 5-body system over the same integration.
#[test]
#[ignore = "requires local 3.3 GB DE441 kernel (data/ephemeris/de441.bsp)"]
fn inner_solar_system_energy_conservation() {
    let Some(path) = de441_path() else {
        eprintln!("skipping: DE441 kernel not present");
        return;
    };

    let mut ephemeris = EphemerisService::load(path.to_str().unwrap(), 64).unwrap();
    let epoch0 = Epoch::from_gregorian(2025, 1, 1, 0, 0, 0, 0, hifitime::TimeScale::TDB);
    let mut world = build_nbody_world(&mut ephemeris, epoch0);

    let total_energy = |world: &World| -> f64 {
        // KE + PE over all pairs.
        let mut states = Vec::new();
        for &(naif_id, _) in BODIES {
            let entity = world.find_celestial(naif_id).unwrap();
            let kin = world
                .get_component::<crate::components::kinematics::Kinematics>(entity)
                .unwrap();
            let gm = GM
                .iter()
                .find(|(id, _)| *id == naif_id)
                .map(|(_, gm)| *gm)
                .unwrap();
            states.push((gm, kin.position, kin.velocity));
        }
        let mut energy = 0.0;
        for &(gm, _pos, vel) in &states {
            energy += 0.5 * gm * vel.norm_squared(); // KE per unit test mass
        }
        for i in 0..states.len() {
            for j in (i + 1)..states.len() {
                let (gm_i, pos_i, _) = states[i];
                let (gm_j, pos_j, _) = states[j];
                energy -= gm_i * gm_j / (pos_i - pos_j).norm(); // PE
            }
        }
        energy
    };

    let e0 = total_energy(&world);
    let day = 86_400.0;
    for _ in 0..730 {
        step_world(&mut world, Seconds::new(day / 2.0));
    }
    let e1 = total_energy(&world);
    let drift = ((e1 - e0) / e0).abs();
    eprintln!("energy drift over 1 year: {drift:.3e}");

    // RK4 is not symplectic; expect small but non-zero drift. 730 half-day
    // steps of the seven-body system gives ~5e-6 observed; budget 1e-5.
    assert!(drift < 1e-5, "energy drift too large: {drift:.3e}");
}

/// Step-size convergence canary: currently the inter-body coupling is
/// first-order (each body integrates against a frozen snapshot of the
/// others), so halving the step halves the error. When the integrator is
/// refactored to joint integration, this ratio should jump to ~16 (RK4
/// fourth order) — and this test's expectation should be tightened.
#[test]
#[ignore = "requires local 3.3 GB DE441 kernel (data/ephemeris/de441.bsp)"]
fn nbody_step_size_convergence() {
    let Some(path) = de441_path() else {
        eprintln!("skipping: DE441 kernel not present");
        return;
    };
    let mut ephemeris = EphemerisService::load(path.to_str().unwrap(), 64).unwrap();
    let epoch0 = Epoch::from_gregorian(2025, 1, 1, 0, 0, 0, 0, hifitime::TimeScale::TDB);
    let day = 86_400.0;

    // Mercury position error at 30 days for two step sizes.
    let mut mercury_err = |step: f64| -> f64 {
        let mut world = build_nbody_world(&mut ephemeris, epoch0);
        for _ in 0..(30.0 * day / step) as usize {
            step_world(&mut world, Seconds::new(step));
        }
        let epoch = epoch0 + 30.0 * day * hifitime::Unit::Second;
        let truth = ephemeris.state_at(1, epoch).unwrap();
        let entity = world.find_celestial(1).unwrap();
        let kin = world
            .get_component::<crate::components::kinematics::Kinematics>(entity)
            .unwrap();
        (kin.position - truth.position).norm() / 1_000.0
    };

    let e_coarse = mercury_err(day);
    let e_fine = mercury_err(day / 2.0);
    let ratio = e_coarse / e_fine;
    eprintln!("30-day Mercury error: {e_coarse:.1} km -> {e_fine:.1} km (ratio {ratio:.2})");

    // First-order coupling dominates: ratio ≈ 2. Accept 1.5–3.0; a jump
    // toward 16 means the coupling was fixed and this expectation (and
    // the error budgets above) should be tightened.
    assert!(
        (1.5..3.0).contains(&ratio),
        "convergence ratio changed to {ratio:.2} — if ~16, inter-body coupling was fixed; tighten budgets"
    );
}
