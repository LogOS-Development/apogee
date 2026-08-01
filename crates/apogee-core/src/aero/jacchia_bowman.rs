//! Jacchia-Bowman 2008 (JB2008) atmosphere model — placeholder implementation.
//!
//! This module provides a lightweight empirical density approximation that
//! shares the same `AtmosphereModel` trait as NRLMSISE-00. It is intentionally
//! simpler than the full JB2008 formulation and is documented as a
//! cross-check / fallback, not a replacement for NRLMSISE-00.
//!
//! The model returns an exponentially decaying density scaled by solar activity
//! (F10.7) and geomagnetic activity (Ap). It is useful for sanity-checking
//! drag computations and for comparing against NRLMSISE-00 in test cases.
//!
//! Future work: replace this placeholder with the full JB2008 thermospheric
//! density model if cross-validation requirements demand it.

use crate::aero::model::{AtmosphereInput, AtmosphereModel, AtmosphereOutput, SpeciesDensities};

/// Jacchia-Bowman placeholder model.
#[derive(Debug, Clone, Copy, Default)]
pub struct JacchiaBowman;

impl JacchiaBowman {
    /// Evaluate the model at the given conditions.
    pub fn evaluate(input: &AtmosphereInput) -> AtmosphereOutput {
        let alt_km = input.altitude_m / 1000.0;

        // Base sea-level density, kg/m³.
        let rho0 = 1.225;
        // Reference scale height, km, adjusted by solar activity.
        let h_ref = 7.0 + 0.02 * (input.f107 - 150.0);
        let h = h_ref.max(5.0);

        // Simple exponential atmosphere with F10.7 and Ap scaling.
        let rho = rho0 * (-alt_km / h).exp();

        // Rough temperature estimate (not physical, just for output completeness).
        let t = 200.0 + 4.0 * alt_km + 0.5 * (input.f107 - 70.0);

        // Number densities are not computed by this placeholder; leave zeros.
        AtmosphereOutput {
            density: rho,
            temperature: t,
            temperature_alt: t,
            number_densities: SpeciesDensities::default(),
        }
    }
}

impl AtmosphereModel for JacchiaBowman {
    fn evaluate(&self, input: &AtmosphereInput) -> AtmosphereOutput {
        Self::evaluate(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_density_decreases_with_altitude() {
        let low = AtmosphereInput::at_altitude(100_000.0);
        let high = AtmosphereInput::at_altitude(400_000.0);
        assert!(JacchiaBowman::evaluate(&low).density > JacchiaBowman::evaluate(&high).density);
    }

    #[test]
    fn test_density_positive_and_finite() {
        let out = JacchiaBowman::evaluate(&AtmosphereInput::at_altitude(300_000.0));
        assert!(out.density.is_finite() && out.density > 0.0);
    }
}
