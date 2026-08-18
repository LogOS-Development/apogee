//! Example: emit a 3D latitude/longitude/altitude grid of atmosphere and wind
//! data for external visualization.
//!
//! Run with:
//!   cargo run --example atmosphere_3d_grid -p apogee-core -- [alt_min_km] [alt_max_km]
//!
//! Output CSV columns:
//!   lat_deg, lon_deg, alt_km, density_kg_m3, temperature_k,
//!   wind_east_mps, wind_north_mps, wind_up_mps, model
//!
//! The grid covers the whole globe at fixed space-weather indices. Enable the
//! `hwm14` feature for real HWM14 wind vectors; otherwise the wind column is
//! zero (placeholder).

use apogee_common::units::{Meters, Radians};
use apogee_core::aero::{
    jacchia_bowman::JacchiaBowman, model::AtmosphereInput, nrlmsise00::Nrlmsise00,
};

const LAT_STEPS: usize = 19;
const LON_STEPS: usize = 36;
const ALT_STEPS: usize = 10;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let alt_min_km = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(100.0);
    let alt_max_km = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(500.0);

    println!(
        "lat_deg,lon_deg,alt_km,density_kg_m3,temperature_k,wind_east_mps,wind_north_mps,wind_up_mps,model"
    );

    for model in ["nrlmsise00", "jacchia_bowman"] {
        for i_lat in 0..LAT_STEPS {
            let lat_deg = -90.0 + i_lat as f64 * (180.0 / (LAT_STEPS - 1) as f64);
            let lat_rad = lat_deg.to_radians();
            for i_lon in 0..LON_STEPS {
                let lon_deg = -180.0 + i_lon as f64 * (360.0 / (LON_STEPS - 1) as f64);
                let lon_rad = lon_deg.to_radians();
                for i_alt in 0..ALT_STEPS {
                    let t_alt = i_alt as f64 / (ALT_STEPS - 1) as f64;
                    let alt_km = alt_min_km + t_alt * (alt_max_km - alt_min_km);

                    let input = AtmosphereInput {
                        altitude_m: Meters::new(alt_km * 1000.0),
                        latitude_rad: Radians::new(lat_rad),
                        longitude_rad: Radians::new(lon_rad),
                        day_of_year: 80,
                        seconds_utc: 12.0 * 3600.0,
                        f107: 150.0,
                        f107a: 150.0,
                        ap: 4.0,
                    };

                    let (density, temperature) = match model {
                        "jacchia_bowman" => {
                            let out = JacchiaBowman::evaluate_approx(&input);
                            (out.density.into_value(), out.temperature.into_value())
                        }
                        _ => {
                            let out = Nrlmsise00::evaluate_simple(&input);
                            (out.density.into_value(), out.temperature.into_value())
                        }
                    };

                    #[cfg(feature = "hwm14")]
                    let (east, north, up) = {
                        use apogee_core::aero::hwm::Hwm14;
                        let wind_input = apogee_core::aero::WindInput {
                            altitude_m: alt_km * 1000.0,
                            latitude_rad: lat_rad,
                            longitude_rad: lon_rad,
                            local_solar_time_hours: 12.0,
                            day_of_year: 80,
                            f107: 150.0,
                            ap: 4.0,
                        };
                        let w = Hwm14::evaluate(&wind_input);
                        (w.east_mps, w.north_mps, w.up_mps)
                    };

                    #[cfg(not(feature = "hwm14"))]
                    let (east, north, up) = (0.0, 0.0, 0.0);

                    println!(
                        "{:.2},{:.2},{:.2},{:.6e},{:.2},{:.3},{:.3},{:.3},{}",
                        lat_deg, lon_deg, alt_km, density, temperature, east, north, up, model
                    );
                }
            }
        }
    }
}
