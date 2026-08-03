use std::io::{stdout, Write};

use apogee_core::aero::jacchia_bowman::{JacchiaBowman, JacchiaBowmanInput};

/// Emit JB2008 density/temperature samples for validation against pyatmos.
///
/// Output columns:
///   altitude_km, rho, texo, tloc
///
/// Fixed conditions: 2014-07-22 22:18:45 UTC, lat 25°, lon 102°, Sun RA/Dec = 0,
/// sampled space-weather indices from the JB2008 reference files.
fn main() {
    let mut stdout = stdout().lock();
    writeln!(stdout, "altitude_km,rho,texo,tloc").unwrap();

    let input_for = |altitude_km: f64| JacchiaBowmanInput {
        mjd: 56860.9296875,
        yday: 202.9296875,
        sun: (0.0, 0.0),
        sat: (0.0, 25.0_f64.to_radians(), altitude_km),
        f10: 90.1,
        f10b: 128.4,
        s10: 99.0,
        s10b: 134.2,
        m10: 91.4,
        m10b: 130.3,
        y10: 100.8,
        y10b: 121.9,
        dstdtc: 32.3125,
    };

    for altitude in [
        100, 150, 200, 250, 300, 350, 400, 450, 500, 600, 700, 800, 900, 1000,
    ] {
        let out = JacchiaBowman::evaluate(&input_for(altitude as f64));
        writeln!(
            stdout,
            "{},{:.6e},{:.2},{:.2}",
            altitude,
            out.density.into_value(),
            out.temperature.into_value(),
            out.temperature_alt.into_value()
        )
        .unwrap();
    }
}
