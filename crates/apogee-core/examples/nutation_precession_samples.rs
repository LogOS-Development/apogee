//! Example: sample nutation/precession quantities for validation plotting.
//!
//! Run via:
//!     cargo run --example nutation_precession_samples -p apogee-core --release
//!
//! Prints a CSV with yearly samples from 2000 through 2030:
//!   year, tdb_seconds, dpsi_arcsec, deps_arcsec, obliquity_deg,
//!   bpn_11, bpn_12, ..., bpn_33

use std::io::Write;

use apogee_core::frames::NutationPrecessionModel;
use hifitime::{Epoch, TimeScale, Unit};

fn main() {
    let model = NutationPrecessionModel::new();
    let start = Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, TimeScale::TDB);

    let mut stdout = std::io::stdout().lock();
    writeln!(
        stdout,
        "year,tdb_seconds,dpsi_arcsec,deps_arcsec,obliquity_deg,bpn_11,bpn_12,bpn_13,bpn_21,bpn_22,bpn_23,bpn_31,bpn_32,bpn_33"
    )
    .unwrap();

    let arcsec = 180.0 * 3600.0 / std::f64::consts::PI;

    for year in 2000..=2030 {
        let years = (year - 2000) as f64;
        let epoch = start + years * 365.25 * Unit::Day;
        let tdb_seconds = epoch.to_tdb_seconds();

        let (dpsi, deps) = model.nutation_angles(epoch);
        let obliquity = model.mean_obliquity(epoch);
        let bpn = model.gcrf_to_tod_matrix(epoch);

        writeln!(
            stdout,
            "{year},{tdb},{dpsi:.12},{deps:.12},{obl:.12},{m00:.15},{m01:.15},{m02:.15},{m10:.15},{m11:.15},{m12:.15},{m20:.15},{m21:.15},{m22:.15}",
            year = year,
            tdb = tdb_seconds,
            dpsi = dpsi * arcsec,
            deps = deps * arcsec,
            obl = obliquity.to_degrees(),
            m00 = bpn[(0, 0)],
            m01 = bpn[(0, 1)],
            m02 = bpn[(0, 2)],
            m10 = bpn[(1, 0)],
            m11 = bpn[(1, 1)],
            m12 = bpn[(1, 2)],
            m20 = bpn[(2, 0)],
            m21 = bpn[(2, 1)],
            m22 = bpn[(2, 2)],
        )
        .unwrap();
    }
}
