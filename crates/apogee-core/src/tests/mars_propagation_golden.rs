//! Golden-snapshot regression test for Mars propagation plots.
//!
//! This test reproduces the four Mars propagation plots using the DE441
//! ephemeris and compares the rendered PNGs against golden snapshots stored at
//! `tests/golden/ephemeris/mars_propagation/`. If the snapshots differ by more
//! than the tolerance, the test fails and writes the regenerated images next
//! to the golden files for inspection.

use crate::ephemeris::Kernel;
use hifitime::Epoch;

#[derive(Debug)]
struct PropagationSample {
    et: f64,
    heliocentric_position: nalgebra::Vector3<f64>,
    velocity: nalgebra::Vector3<f64>,
}

/// Propagate Mars barycenter (NAIF 4) and Sun (NAIF 10) from the DE441 kernel
/// over 2025-01-01 to 2026-07-01 TDB with daily samples, returning
/// heliocentric Mars position and SSB-relative velocity samples.
fn propagate_mars() -> Vec<PropagationSample> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let kernel_path = format!("{manifest_dir}/../../data/ephemeris/de441.bsp");
    let kernel = Kernel::load(&kernel_path).expect("DE441 kernel should load");

    let start = Epoch::from_gregorian(2025, 1, 1, 12, 0, 0, 0, hifitime::TimeScale::TDB);
    let end = Epoch::from_gregorian(2026, 7, 1, 12, 0, 0, 0, hifitime::TimeScale::TDB);
    let start_et = start.to_tdb_seconds();
    let end_et = end.to_tdb_seconds();
    let day = 86400.0;
    let n = ((end_et - start_et) / day).floor() as i32 + 1;

    (0..n)
        .map(|i| {
            let et = start_et + i as f64 * day;
            let mars = kernel
                .state_at(4, et)
                .expect("Mars barycenter state should be available");
            let sun = kernel
                .state_at(10, et)
                .expect("Sun state should be available");
            PropagationSample {
                et,
                heliocentric_position: mars.position - sun.position,
                velocity: mars.velocity,
            }
        })
        .collect()
}

#[test]
#[ignore = "requires DE441 kernel and golden snapshots"]
fn mars_propagation_golden_snapshots_match() {
    let samples = propagate_mars();
    assert!(
        !samples.is_empty(),
        "propagation should produce at least one sample"
    );

    // The actual rendering and image comparison is delegated to a Python
    // script because the Rust ecosystem for PNG generation is heavier than
    // matplotlib for this regression purpose.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let golden_dir = std::path::PathBuf::from(format!(
        "{manifest_dir}/tests/golden/ephemeris/mars_propagation"
    ));
    assert!(
        golden_dir.exists(),
        "golden snapshot directory should exist: {}",
        golden_dir.display()
    );

    // Serialize samples to a temporary CSV for the Python script.
    let out_dir = std::env::temp_dir().join("apogee_mars_propagation_test");
    std::fs::create_dir_all(&out_dir).expect("should create temp output directory");
    let csv_path = out_dir.join("samples.csv");
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&csv_path).expect("should create CSV");
        writeln!(file, "et,px,py,pz,vx,vy,vz").unwrap();
        for s in &samples {
            writeln!(
                file,
                "{},{},{},{},{},{},{}",
                s.et,
                s.heliocentric_position.x,
                s.heliocentric_position.y,
                s.heliocentric_position.z,
                s.velocity.x,
                s.velocity.y,
                s.velocity.z,
            )
            .unwrap();
        }
    }

    let status = std::process::Command::new("python3")
        .arg(format!(
            "{manifest_dir}/tests/scripts/compare_mars_plots.py"
        ))
        .arg(&csv_path)
        .arg(&golden_dir)
        .arg(&out_dir)
        .status()
        .expect("should run plot comparison script");

    assert!(
        status.success(),
        "Mars propagation plots differ from golden snapshots; regenerated images are in {}",
        out_dir.display()
    );
}
