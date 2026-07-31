//! NRLMSISE-00 atmosphere model — stub (Fortran port pending).

/// Atmospheric density and temperature output.
#[derive(Debug, Clone, Default)]
pub struct AtmosphereOutput {
    pub density: f64,     // kg/m^3
    pub temperature: f64, // K
}

/// NRLMSISE-00 model.
#[derive(Debug, Default)]
pub struct Nrlmsise00 {
    // TODO: F10.7, Ap inputs
}

impl Nrlmsise00 {
    /// Compute atmospheric density at given altitude and conditions.
    pub fn density(&self, _altitude_km: f64, _f107: f64, _ap: f64) -> AtmosphereOutput {
        // TODO: implement model
        AtmosphereOutput::default()
    }
}
