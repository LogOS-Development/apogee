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
//! DE441 is a 3.3 GB kernel that is NOT committed to the repo. This whole
//! module compiles only behind the `de441` cargo feature (nightly does
//! not enable it), so CI's `--include-ignored` can never reach these
//! tests. Locally, with the kernel fetched (`FETCH_DE441=1
//! ./scripts/fetch_data.sh`), run:
//!
//! ```sh
//! cargo test -p apogee-core --features de441 --lib -- --ignored
//! ```
//! A missing kernel panics with fetch instructions rather than
//! silently passing.
//!
//! Bodies and frames: all seven bodies (Sun 10, Mercury barycenter 1,
//! Venus barycenter 2, Earth-Moon barycenter 3, Mars barycenter 4,
//! Jupiter barycenter 5, Saturn barycenter 6) have SSB-relative (center 0)
//! segments in DE441, so no frame or center composition is needed —
//! integrated SSB positions compare directly against kernel SSB positions.
//!
//! Expected accuracy: joint RK4 integration (all dynamic bodies in one
//! pass, forces recomputed at every substage — issue #189 fixed). At
//! 0.5-day steps over 1 year with all 11 bodies: max error 1,502 km
//! (Moon-dominated RK4 truncation on its 27-day orbit). Energy conserved
//! to 2.6e-10. Halving the step shrinks the Moon error ~26× (fourth-order
//! convergence, verified by `nbody_step_size_convergence`).

use super::nbody_bodies::{gm_of, BODIES};
use crate::ephemeris::EphemerisService;
use crate::systems::step::step_world;
use crate::world::World;
use apogee_common::units::Seconds;
use hifitime::Epoch;

/// Path to the local DE441 kernel.
///
/// Panics with fetch instructions when the kernel is absent — these tests
/// are only compiled behind the `de441` feature, so a run that reaches
/// this point has explicitly requested them.
fn de441_path() -> std::path::PathBuf {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ephemeris/de441.bsp");
    assert!(
        path.exists(),
        "DE441 kernel not found at {}. Fetch it (3.3 GB, not used by CI) with: \
         FETCH_DE441=1 ./scripts/fetch_data.sh — or download directly from \
         https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp/de441.bsp",
        path.display()
    );
    path
}

/// Build a World with the five bodies as dynamic celestials, initial
/// states from DE441 at `epoch`.
fn build_nbody_world(ephemeris: &mut EphemerisService, epoch: Epoch) -> World {
    let mut world = World::new();
    for &(naif_id, _) in BODIES {
        let state = ephemeris.state_at_ssb(naif_id, epoch).unwrap();
        let gm = gm_of(naif_id);
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
#[ignore = "requires de441 feature + local 3.3 GB kernel (data/ephemeris/de441.bsp)"]
fn inner_solar_system_nbody_vs_de441() {
    let path = de441_path();

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
                let truth = ephemeris.state_at_ssb(naif_id, epoch).unwrap();
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

    // Error budget: observed max 1,502.7 km (Moon truncation) at 0.5-day
    // steps. Systematic errors (wrong GM, unit/frame mismatches, missing
    // forces) appear at 1e5+ km — two orders above budget.
    assert!(
        results.iter().all(|&(_, _, e)| e < 10_000.0),
        "position errors exceeded 10,000 km budget"
    );
}

/// Energy conservation of the 5-body system over the same integration.
#[test]
#[ignore = "requires de441 feature + local 3.3 GB kernel (data/ephemeris/de441.bsp)"]
fn inner_solar_system_energy_conservation() {
    let path = de441_path();

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
            let gm = gm_of(naif_id);
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

    // Joint RK4: observed drift 2.6e-10 over a year of half-day steps.
    assert!(drift < 1e-8, "energy drift too large: {drift:.3e}");
}

/// Earth-Moon center-chain composition against known geometry.
///
/// Verifies `state_at_ssb` composes the DE441 chain (Earth/Moon → EMB →
/// SSB) correctly: the composed Earth and Moon SSB positions must satisfy
/// the mass-weighted barycenter relation
///   (GM_E·r_E + GM_M·r_M) / (GM_E + GM_M) = r_EMB
/// to within kernel interpolation accuracy.
#[test]
#[ignore = "requires de441 feature + local 3.3 GB kernel (data/ephemeris/de441.bsp)"]
fn earth_moon_ssb_composition_matches_barycenter() {
    let path = de441_path();
    let mut ephemeris = EphemerisService::load(path.to_str().unwrap(), 64).unwrap();
    let epoch = Epoch::from_gregorian(2025, 6, 1, 0, 0, 0, 0, hifitime::TimeScale::TDB);

    let earth = ephemeris.state_at_ssb(399, epoch).unwrap();
    let moon = ephemeris.state_at_ssb(301, epoch).unwrap();
    let emb = ephemeris.state_at_ssb(3, epoch).unwrap();

    let gm_e = gm_of(399);
    let gm_m = gm_of(301);
    let total = gm_e + gm_m;
    let barycenter = (earth.position * gm_e + moon.position * gm_m) / total;

    let err_km = (barycenter - emb.position).norm() / 1_000.0;
    eprintln!("EMB composition error: {err_km:.6} km");
    // Earth-Moon separation is 384,400 km; the barycenter sits ~4,670 km
    // from Earth's center. Kernel interpolation error is sub-meter, so
    // the composition must agree to millimeters — allow generous margin.
    assert!(
        err_km < 0.001,
        "EMB composition error {err_km} km exceeds 1 m"
    );

    // Sanity: the Moon's SSB position is ~1 AU from the Sun, not 384,400 km.
    let moon_dist_au = moon.position.norm() / 1.495978707e11;
    assert!(
        (moon_dist_au - 1.0).abs() < 0.02,
        "Moon SSB distance {moon_dist_au} AU"
    );
}

/// Step-size convergence test: with joint integration (#189 fixed), the
/// error must converge at fourth order or better — halving the step
/// should shrink the Moon's 30-day error by ≥16×. (Measured: 26× — the
/// residual is clean RK4 truncation, not coupling error.)
#[test]
#[ignore = "requires de441 feature + local 3.3 GB kernel (data/ephemeris/de441.bsp)"]
fn nbody_step_size_convergence() {
    let path = de441_path();
    let mut ephemeris = EphemerisService::load(path.to_str().unwrap(), 64).unwrap();
    let epoch0 = Epoch::from_gregorian(2025, 1, 1, 0, 0, 0, 0, hifitime::TimeScale::TDB);
    let day = 86_400.0;

    // Moon position error at 30 days for two step sizes — the Moon is the
    // most coupling-sensitive body (27-day binary with Earth).
    let mut moon_err = |step: f64| -> f64 {
        let mut world = build_nbody_world(&mut ephemeris, epoch0);
        for _ in 0..(30.0 * day / step) as usize {
            step_world(&mut world, Seconds::new(step));
        }
        let epoch = epoch0 + 30.0 * day * hifitime::Unit::Second;
        let truth = ephemeris.state_at_ssb(301, epoch).unwrap();
        let entity = world.find_celestial(301).unwrap();
        let kin = world
            .get_component::<crate::components::kinematics::Kinematics>(entity)
            .unwrap();
        (kin.position - truth.position).norm() / 1_000.0
    };

    let e_coarse = moon_err(day);
    let e_fine = moon_err(day / 2.0);
    let ratio = e_coarse / e_fine;
    eprintln!("30-day Moon error: {e_coarse:.1} km -> {e_fine:.1} km (ratio {ratio:.2})");

    // Fourth-order convergence or better: ratio ≥ 16. A ratio near 2 means
    // the inter-body coupling regressed to first-order (frozen snapshot).
    assert!(
        ratio >= 16.0,
        "convergence ratio {ratio:.2} < 16 — inter-body coupling regressed to first-order"
    );
}
