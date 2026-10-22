//! 11-body n-body pipeline validation against committed DE441 truth.
//!
//! Truth states were extracted once from the local DE441 kernel with
//! `cargo run --example extract_de441_fixture --features de441` and
//! committed as `tests/fixtures/de441_ssb_states.csv` (~10 KB). This lets
//! every PR CI run execute a real 11-body end-to-end validation — ECS
//! spawn → force aggregation → RK4 → position comparison — without the
//! 3.3 GB kernel. The full-kernel suite (`nbody_validation`, behind the
//! `de441` feature) remains the deeper local check.
//!
//! Arc: 2025-01-01 TDB + 365 days, RK4 at 0.5-day steps, all 11 major
//! bodies dynamic, SSB-frame checkpoints every 42 days. Error growth is
//! clean RK4 truncation, Moon-dominated (27-day orbit at 0.5-day steps).

use super::nbody_bodies::{gm_of, BODIES};
use crate::systems::step::step_world;
use crate::world::World;
use apogee_common::units::Seconds;
use nalgebra::Vector3;

const FIXTURE_CSV: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/de441_ssb_states.csv"
);

const DAY: f64 = 86_400.0;

/// Fixture row: (days since 2025-01-01 TDB, NAIF ID, position km, velocity km/s).
type Row = (f64, i32, Vector3<f64>, Vector3<f64>);

/// Load the committed fixture. The CSV is a repository artifact, so a
/// missing file or malformed line is a repo defect — fail loudly.
fn load_fixture() -> Vec<Row> {
    let path = std::path::Path::new(FIXTURE_CSV);
    assert!(
        path.exists(),
        "committed fixture missing: {}",
        path.display()
    );
    let contents = std::fs::read_to_string(path).expect("read de441_ssb_states.csv");
    let mut out = Vec::new();
    for (i, line) in contents.lines().enumerate().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        assert_eq!(parts.len(), 8, "malformed CSV line {i}: {line:?}");
        let field = |j: usize| {
            parts[j]
                .parse::<f64>()
                .unwrap_or_else(|e| panic!("CSV line {i}, field {j}: {e}"))
        };
        let body_id = parts[1]
            .trim()
            .parse::<i32>()
            .unwrap_or_else(|e| panic!("CSV line {i}, body id: {e}"));
        out.push((
            field(0),
            body_id,
            Vector3::new(field(2), field(3), field(4)),
            Vector3::new(field(5), field(6), field(7)),
        ));
    }
    out
}

/// Per-body position-error budgets (km), maximum over all checkpoints.
///
/// Calibrated from measured maxima (day 365 of the arc, errors grow
/// ~quadratically with time; Moon-dominated RK4 truncation at 0.5-day
/// steps on its 27-day orbit): Sun 2.4, Mercury 361, Venus 95, Mars 30,
/// Jupiter 1.5, Saturn 0.05, Uranus 0.66, Neptune 0.45, Pluto 0.14,
/// Earth 49, Moon 1503. Budgets carry ~2x headroom over measurement;
/// a transcription blunder (wrong GM, wrong sign, wrong frame) produces
/// errors orders of magnitude beyond these.
const BUDGET_KM: &[(i32, f64)] = &[
    (10, 5.0),
    (1, 800.0),
    (2, 200.0),
    (4, 60.0),
    (5, 5.0),
    (6, 1.0),
    (7, 2.0),
    (8, 2.0),
    (9, 1.0),
    (399, 100.0),
    (301, 3_000.0),
];

fn budget_km(naif_id: i32) -> f64 {
    BUDGET_KM
        .iter()
        .find(|(id, _)| *id == naif_id)
        .map(|(_, km)| *km)
        .unwrap_or_else(|| panic!("no budget for NAIF ID {naif_id}"))
}

#[test]
fn solar_system_nbody_vs_de441_fixture() {
    let rows = load_fixture();

    // Checkpoint epochs (days) present in the fixture, excluding the
    // epoch-0 states used for spawning.
    let mut checkpoints: Vec<f64> = rows.iter().map(|(d, ..)| *d).collect();
    checkpoints.sort_by(|a, b| a.partial_cmp(b).unwrap());
    checkpoints.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    checkpoints.retain(|d| *d > 0.0);
    assert!(!checkpoints.is_empty(), "fixture has no comparison epochs");

    // Spawn the 11 dynamic bodies from the epoch-0 states.
    let mut world = World::new();
    let mut spawned = 0;
    for (days, naif_id, pos_km, vel_km_s) in &rows {
        if *days != 0.0 {
            continue;
        }
        let gm = gm_of(*naif_id);
        world.add_celestial_body(crate::components::celestial::CelestialBodySpec {
            naif_id: *naif_id,
            kind: crate::components::celestial::CelestialKind::Dynamic,
            position: pos_km * 1_000.0,
            velocity: vel_km_s * 1_000.0,
            gm: Some(apogee_common::units::GravitationalParameter::new(gm)),
            mass: Some(apogee_common::units::Kilograms::new(
                gm / apogee_common::constants::G.into_value(),
            )),
            spherical_harmonics: None,
        });
        spawned += 1;
    }
    assert_eq!(
        spawned,
        BODIES.len(),
        "fixture must have all 11 epoch-0 states"
    );

    // Index truth positions by (days, NAIF ID).
    let truth: std::collections::HashMap<(u64, i32), Vector3<f64>> = rows
        .iter()
        .filter(|(d, ..)| *d > 0.0)
        .map(|(d, id, pos, _)| (((*d * 100.0) as u64, *id), *pos))
        .collect();

    let step = DAY / 2.0;
    let total = *checkpoints.last().unwrap() * DAY;
    let mut elapsed = 0.0;
    let mut next = 0;
    let mut max_err: std::collections::HashMap<i32, f64> = std::collections::HashMap::new();

    while elapsed < total {
        step_world(&mut world, Seconds::new(step));
        elapsed += step;

        if next < checkpoints.len() && elapsed >= checkpoints[next] * DAY {
            let key = (checkpoints[next] * 100.0) as u64;
            for &(naif_id, _name) in BODIES {
                let expected = truth.get(&(key, naif_id)).unwrap_or_else(|| {
                    panic!(
                        "no truth row for body {naif_id} at day {}",
                        checkpoints[next]
                    )
                });
                let entity = world.find_celestial(naif_id).unwrap();
                let kin = world
                    .get_component::<crate::components::kinematics::Kinematics>(entity)
                    .unwrap();
                let err_km = (kin.position / 1_000.0 - expected).norm();
                eprintln!(
                    "  day {:>3}: {:>20} error {:9.3} km",
                    checkpoints[next], _name, err_km
                );
                let e = max_err.entry(naif_id).or_insert(0.0);
                *e = e.max(err_km);
            }
            next += 1;
        }
    }
    assert_eq!(
        next,
        checkpoints.len(),
        "integration did not reach all checkpoints"
    );

    for &(naif_id, _name) in BODIES {
        let measured = max_err[&naif_id];
        let budget = budget_km(naif_id);
        assert!(
            measured <= budget,
            "{_name}: position error {measured:.1} km exceeds budget {budget} km"
        );
    }
}

// Keep the epoch constant honest: the extractor starts from the same day.
#[test]
fn fixture_epoch_zero_matches_documented_arc_start() {
    let rows = load_fixture();
    assert!(
        rows.iter().any(|(d, ..)| *d == 0.0),
        "fixture must contain epoch-0 states (arc start 2025-01-01 TDB)"
    );
}
