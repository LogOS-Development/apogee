use apogee_core::aero::hwm::{Hwm14, WindInput};
use apogee_core::aero::HorizontalWindModel;

/// Emit HWM14 wind components for a height profile.
fn main() {
    let model = Hwm14;

    println!("altitude_km,east_mps,north_mps,up_mps");

    for altitude_km in [100, 150, 200, 250, 300, 350, 400, 450, 500] {
        let input = WindInput {
            altitude_m: altitude_km as f64 * 1000.0,
            latitude_rad: (-11.95_f64).to_radians(),
            longitude_rad: (-76.77_f64).to_radians(),
            local_solar_time_hours: 12.0,
            day_of_year: 323,
            f107: -1.0,
            ap: 35.0,
        };

        let out = model.evaluate(&input);
        println!(
            "{},{:.3},{:.3},{:.3}",
            altitude_km, out.east_mps, out.north_mps, out.up_mps
        );
    }
}
