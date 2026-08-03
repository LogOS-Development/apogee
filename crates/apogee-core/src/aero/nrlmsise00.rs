//! NRLMSISE-00 atmosphere model — thin adapter around the vendored Brahe
//! implementation.
//!
//! The heavy lifting is in [`crate::aero::nrlmsise00_brahe`]; this module maps
//! Apogee's `AtmosphereInput` / `AtmosphereOutput` types to the vendored model's
//! low-level `NrlmsiseInput` / `NrlmsiseOutput` and provides a default-switch
//! configuration that evaluates the full model (all perturbations enabled).

use apogee_common::units::{Density, Kelvins};

use crate::aero::model::{AtmosphereInput, AtmosphereModel, AtmosphereOutput, SpeciesDensities};
use crate::aero::nrlmsise00_brahe::{gtd7, NrlmsiseFlags, NrlmsiseInput, NrlmsiseOutput};

/// Model version for diagnostics.
pub const NRLMSISE00_VERSION: &str = "00";

/// 7-element magnetic-index array used by NRLMSISE-00 for geomagnetic history.
/// Layout (Fortran convention):
///   [0] ap for current time
///   [1] ap 3 hr before
///   [2] ap 6 hr before
///   [3] ap 9 hr before
///   [4] average of last 24 hr
///   [5] average of last 36 hr (centered on 18 hr)
///   [6] ap at 54 hr
pub type ApArray = [f64; 7];

/// NRLMSISE-00 model.
#[derive(Debug, Clone, Copy, Default)]
pub struct Nrlmsise00;

impl Nrlmsise00 {
    /// Evaluate the model with a simple daily-Ap assumption.
    pub fn evaluate_simple(input: &AtmosphereInput) -> AtmosphereOutput {
        let ap_array = [input.ap; 7];
        Self::evaluate_with_ap_array(input, &ap_array)
    }

    /// Evaluate the model with a full 3-hourly Ap history array.
    pub fn evaluate_with_ap_array(input: &AtmosphereInput, ap_array: &ApArray) -> AtmosphereOutput {
        let mut flags = full_model_flags();
        let mut model_input = NrlmsiseInput {
            year: 0,
            doy: input.day_of_year as i32,
            sec: input.seconds_utc,
            alt: input.altitude_m.into_value() / 1000.0,
            g_lat: input.latitude_rad.to_degrees(),
            g_lon: input.longitude_rad.to_degrees(),
            lst: local_solar_time(input),
            f107a: input.f107a,
            f107: input.f107,
            ap: input.ap,
            ap_array: *ap_array,
        };
        let mut model_output = NrlmsiseOutput::default();

        gtd7(&mut model_input, &mut flags, &mut model_output);

        AtmosphereOutput {
            density: Density::new(model_output.d[5]),
            temperature: Kelvins::new(model_output.t[0]),
            temperature_alt: Kelvins::new(model_output.t[1]),
            number_densities: SpeciesDensities {
                he: model_output.d[0],
                o: model_output.d[1],
                n2: model_output.d[2],
                o2: model_output.d[3],
                ar: model_output.d[4],
                h: model_output.d[6],
                n: model_output.d[7],
                anomalous_o: model_output.d[8],
            },
        }
    }
}

impl AtmosphereModel for Nrlmsise00 {
    fn evaluate(&self, input: &AtmosphereInput) -> AtmosphereOutput {
        Self::evaluate_simple(input)
    }
}

/// Build flags that enable the complete NRLMSISE-00 model, with SI output
/// units (kg/m³ for mass density, m⁻³ for number densities).
fn full_model_flags() -> NrlmsiseFlags {
    let mut flags = NrlmsiseFlags::default();
    // switches[0] = 1 selects SI output units.
    flags.switches[0] = 1;
    // Enable all standard model terms.
    for i in 1..24 {
        flags.switches[i] = 1;
    }
    flags.tselec();
    flags
}

/// Compute local apparent solar time in hours from longitude and UTC seconds.
fn local_solar_time(input: &AtmosphereInput) -> f64 {
    let utc_hours = input.seconds_utc / 3600.0;
    let lon_hours = input.longitude_rad.to_degrees() / 15.0;
    (utc_hours + lon_hours).rem_euclid(24.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_returns_finite_density() {
        let input = AtmosphereInput::at_altitude(400_000.0);
        let out = Nrlmsise00::evaluate_simple(&input);
        assert!(out.density.into_value().is_finite() && out.density.into_value() > 0.0);
        assert!(out.temperature.into_value() > 500.0);
        assert!(out.temperature_alt.into_value() > 500.0);
    }
}
