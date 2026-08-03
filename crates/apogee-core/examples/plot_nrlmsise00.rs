//! Example: tabulate NRLMSISE-00 density and temperature vs. altitude.
//!
//! Run with:
//!   cargo run --example plot_nrlmsise00 -p apogee-core -- [alt_min_km] [alt_max_km] [step_km]
//!
//! The program writes CSV to stdout with columns:
//!   altitude_km, density_kg_m3, temperature_exo_k, temperature_alt_k,
//!   he_m3, o_m3, n2_m3, o2_m3, ar_m3, h_m3, n_m3, anomalous_o_m3
//!
//! Pipe or redirect the output to a file and plot with your preferred tool, or
//! use `scripts/plot_nrlmsise00.py` for a ready-made matplotlib figure.

use apogee_common::units::Meters;
use apogee_core::aero::model::AtmosphereInput;
use apogee_core::aero::nrlmsise00::Nrlmsise00;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let alt_min_km = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(100.0);
    let alt_max_km = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(600.0);
    let step_km = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10.0);

    println!(
        "altitude_km,density_kg_m3,temperature_exo_k,temperature_alt_k,\
         he_m3,o_m3,n2_m3,o2_m3,ar_m3,h_m3,n_m3,anomalous_o_m3"
    );

    let mut alt_km = alt_min_km;
    while alt_km <= alt_max_km {
        let input = AtmosphereInput {
            altitude_m: Meters::new(alt_km * 1000.0),
            latitude_rad: 0.0,
            longitude_rad: 0.0,
            day_of_year: 80,
            seconds_utc: 12.0 * 3600.0,
            f107: 150.0,
            f107a: 150.0,
            ap: 4.0,
        };
        let out = Nrlmsise00::evaluate_simple(&input);
        println!(
            "{:.2},{:.6e},{:.2},{:.2},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e}",
            alt_km,
            out.density.into_value(),
            out.temperature.into_value(),
            out.temperature_alt.into_value(),
            out.number_densities.he,
            out.number_densities.o,
            out.number_densities.n2,
            out.number_densities.o2,
            out.number_densities.ar,
            out.number_densities.h,
            out.number_densities.n,
            out.number_densities.anomalous_o,
        );
        alt_km += step_km;
    }
}
