//! Spherical harmonics gravity engine (Cunningham recursion) — stub.

/// Spherical harmonics gravity model.
#[derive(Debug, Default)]
pub struct SphericalHarmonics {
    pub degree: usize,
    pub order: usize,
    // TODO: C/S coefficient arrays
}

impl SphericalHarmonics {
    /// Load EGM2008 coefficients from file.
    pub fn load_egm2008(_path: &str) -> Result<Self, std::io::Error> {
        // TODO: parse coefficient file
        Ok(Self::default())
    }
}
