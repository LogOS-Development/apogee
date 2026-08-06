use crate::ephemeris::Kernel;
use hifitime::Epoch;

/// DE441 integration test: compare evaluated Mars barycenter state against a
/// JPL Horizons reference vector.
///
/// Horizons query parameters:
///   - COMMAND = 4   (Mars barycenter)
///   - CENTER  = @0  (solar system barycenter)
///   - TIME_TYPE = TDB
///   - OUT_UNITS = KM-S
///   - epoch = 2025-01-01 12:00:00 TDB  (et = 789004800 s)
#[test]
#[ignore = "loads the DE441 kernel (3.5 GB) and is slow; run with --ignored or in the nightly slow-test job"]
#[ntest::timeout(300_000)]
fn de441_mars_barycenter_vs_horizons() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/ephemeris/de441.bsp"
    );
    // Skip if the DE441 kernel has not been downloaded yet. CI runs with
    // fetched data; local unit-test runs can safely ignore this.
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: DE441 kernel not found at {path}");
        return;
    }
    let kernel = Kernel::load(path).unwrap();

    let epoch = Epoch::from_gregorian(2025, 1, 1, 12, 0, 0, 0, hifitime::TimeScale::TDB);
    let state = kernel.state_at(4, epoch.to_tdb_seconds()).unwrap();

    // Horizons reference vector (TDB, KM-S).
    let expected_pos = nalgebra::Vector3::new(
        -79_849_986.264_232_5,
        205_757_251.415_215_1,
        96_553_111.211_221_74,
    );
    let expected_vel = nalgebra::Vector3::new(
        -21.965_537_259_949_41,
        -5.560_580_018_886_087,
        -1.957_730_883_643_913,
    );

    let pos_err = (state.position - expected_pos).norm();
    let vel_err = (state.velocity - expected_vel).norm();

    assert!(pos_err < 1.0, "position error too large: {} km", pos_err);
    assert!(vel_err < 1e-3, "velocity error too large: {} km/s", vel_err);
}
