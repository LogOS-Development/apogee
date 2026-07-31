//! Apogee common types, constants, and error definitions.
//!
//! Shared across all Apogee crates. No I/O, no dependencies on other
//! Apogee crates.

use nalgebra::Vector3;

/// NAIF-style body identifier (e.g. 10 = Sun, 399 = Earth).
pub type NaifId = i32;

/// Position in meters, inertial frame.
pub type Position = Vector3<f64>;

/// Velocity in meters per second.
pub type Velocity = Vector3<f64>;

/// Unified error type for the Apogee workspace.
#[derive(Debug, thiserror::Error)]
pub enum ApogeeError {
    #[error("ephemeris error: {0}")]
    Ephemeris(String),

    #[error("frame transform error: {0}")]
    Frame(String),

    #[error("gravity model error: {0}")]
    Gravity(String),

    #[error("integrator error: {0}")]
    Integrator(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("data error: {0}")]
    Data(String),
}

pub type ApogeeResult<T> = Result<T, ApogeeError>;

/// Physical constants (SI units).
pub mod constants {
    /// Pi to 15 significant digits.
    ///
    /// Note: Rust's `std::f64::consts::PI` is identical (both are the
    /// nearest f64 to mathematical π). This constant exists for
    /// explicitness and discoverability alongside other project constants.
    #[allow(clippy::approx_constant)]
    pub const PI: f64 = 3.141592653589793;

    /// Gravitational constant (m^3 kg^-1 s^-2).
    pub const G: f64 = 6.67430e-11;

    /// Speed of light (m/s).
    pub const C: f64 = 299_792_458.0;

    /// Astronomical unit (m).
    pub const AU: f64 = 1.495978707e11;

    /// Earth gravitational parameter (m^3/s^2), GM from GGM03C.
    pub const GM_EARTH: f64 = 3.986004415e14;

    /// Sun gravitational parameter (m^3/s^2).
    pub const GM_SUN: f64 = 1.32712440018e20;

    /// Moon gravitational parameter (m^3/s^2).
    pub const GM_MOON: f64 = 4.902800118e12;

    /// Earth equatorial radius (m), WGS84.
    pub const R_EARTH_EQ: f64 = 6_378_137.0;

    /// Earth polar radius (m), WGS84.
    pub const R_EARTH_POLAR: f64 = 6_356_752.314245;

    /// Earth flattening, WGS84.
    pub const F_EARTH: f64 = 1.0 / 298.257223563;

    /// Solar radiation pressure at 1 AU (N/m^2).
    pub const SRP_1AU: f64 = 4.5391e-6;
}
