//! Atmospheric model trait and shared output types.
//!
//! The `AtmosphereModel` trait abstracts over NRLMSISE-00, Jacchia-Bowman,
//! and any future empirical density model. Inputs are geodetic altitude and
//! latitude, plus space-weather indices; outputs are total mass density,
//! temperature, and (optionally) number densities for major species.

use apogee_common::units::{Density, Kelvins, Meters};

/// Geodetic location and local conditions used by atmosphere models.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtmosphereInput {
    /// Altitude above the ellipsoid.
    pub altitude_m: Meters<f64>,
    /// Geodetic latitude, radians.
    pub latitude_rad: f64,
    /// Geodetic longitude, radians.
    pub longitude_rad: f64,
    /// Day of year (1..=366).
    pub day_of_year: u16,
    /// Seconds since local midnight, UTC.
    pub seconds_utc: f64,
    /// Daily F10.7 solar flux at 1 AU (sfu, 10⁻²² W/m²/Hz).
    pub f107: f64,
    /// 81-day centred smoothed F10.7 (sfu).
    pub f107a: f64,
    /// Daily Ap geomagnetic index.
    pub ap: f64,
}

impl AtmosphereInput {
    /// Convenience constructor for a simple altitude-only test case at the
    /// equator with default indices.
    pub fn at_altitude(altitude_m: f64) -> Self {
        Self {
            altitude_m: Meters::new(altitude_m),
            latitude_rad: 0.0,
            longitude_rad: 0.0,
            day_of_year: 80,
            seconds_utc: 0.0,
            f107: 150.0,
            f107a: 150.0,
            ap: 4.0,
        }
    }
}

/// Species number densities, m⁻³.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpeciesDensities {
    pub he: f64,
    pub o: f64,
    pub n2: f64,
    pub o2: f64,
    pub ar: f64,
    pub h: f64,
    pub n: f64,
    pub anomalous_o: f64,
}

/// Atmospheric density and temperature output.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AtmosphereOutput {
    /// Total mass density.
    pub density: Density<f64>,
    /// Exospheric temperature.
    pub temperature: Kelvins<f64>,
    /// Temperature at the requested altitude.
    pub temperature_alt: Kelvins<f64>,
    /// Number densities for individual species, m⁻³.
    pub number_densities: SpeciesDensities,
}

/// Trait for empirical atmosphere models.
///
/// All implementations must be deterministic and thread-safe; models hold no
/// mutable state between calls.
pub trait AtmosphereModel: Send + Sync {
    /// Evaluate the atmosphere at the given location and solar/geomagnetic
    /// conditions.
    fn evaluate(&self, input: &AtmosphereInput) -> AtmosphereOutput;
}
