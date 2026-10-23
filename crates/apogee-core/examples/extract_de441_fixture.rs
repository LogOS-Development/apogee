//! Extract SSB truth states from the local DE441 kernel into the
//! committed fixture used by `de441_fixture_validation`.
//!
//! Run once (requires the 3.3 GB kernel and the `de441` feature):
//!
//! ```sh
//! FETCH_DE441=1 ./scripts/fetch_data.sh   # if the kernel is absent
//! cargo run --example extract_de441_fixture --features de441
//! ```
//!
//! Output: `crates/apogee-core/tests/fixtures/de441_ssb_states.csv`.
//! Arc: 2025-01-01 TDB + 365 days, checkpoint every 42 days, all 11
//! major bodies, positions km / velocities km/s.

use apogee_core::ephemeris::EphemerisService;
use hifitime::Epoch;

/// Bodies dumped: (NAIF ID, display name) — mirrors `nbody_bodies::BODIES`.
const BODIES: &[i32] = &[10, 1, 2, 4, 5, 6, 7, 8, 9, 399, 301];

/// Checkpoint offsets from the arc start, in days.
const CHECKPOINT_DAYS: &[f64] = &[0.0, 42.0, 84.0, 126.0, 168.0, 210.0, 252.0, 294.0, 365.0];

fn main() {
    let kernel =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ephemeris/de441.bsp");
    assert!(
        kernel.exists(),
        "DE441 kernel not found at {} — fetch with FETCH_DE441=1 ./scripts/fetch_data.sh",
        kernel.display()
    );

    let mut ephemeris = EphemerisService::load(kernel.to_str().unwrap(), 64).unwrap();
    let epoch0 = Epoch::from_gregorian(2025, 1, 1, 0, 0, 0, 0, hifitime::TimeScale::TDB);
    let day = 86_400.0;

    let out_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/de441_ssb_states.csv");
    let mut csv = String::from("days_elapsed,body_id,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s\n");

    for &days in CHECKPOINT_DAYS {
        let epoch = epoch0 + days * day * hifitime::Unit::Second;
        for &naif_id in BODIES {
            let state = ephemeris.state_at_ssb(naif_id, epoch).unwrap();
            let pos_km = state.position / 1_000.0;
            let vel_km_s = state.velocity / 1_000.0;
            csv.push_str(&format!(
                "{days},{naif_id},{:.16e},{:.16e},{:.16e},{:.16e},{:.16e},{:.16e}\n",
                pos_km.x, pos_km.y, pos_km.z, vel_km_s.x, vel_km_s.y, vel_km_s.z
            ));
        }
    }

    std::fs::write(&out_path, csv).expect("write fixture CSV");
    println!(
        "wrote {} rows to {}",
        CHECKPOINT_DAYS.len() * BODIES.len(),
        out_path.display()
    );
}
