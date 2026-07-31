//! Frame transformation service: ICRF, ECI, ECEF, ECLIPJ2000.

pub mod clock;
pub mod eop;
pub mod frame_service;
pub mod leap_seconds;
pub mod nutation_precession;

pub use clock::*;
pub use eop::*;
pub use frame_service::*;
pub use leap_seconds::*;
pub use nutation_precession::*;

/// Reference frames supported by Apogee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// International Celestial Reference Frame.
    Icrf,
    /// Earth-Centered Inertial (Earth Mean Equator, J2000).
    Eci,
    /// Earth-Centered Earth-Fixed.
    Ecef,
    /// Ecliptic J2000.
    EclipticJ2000,
    /// Body-fixed frame for a specific body.
    BodyFixed(u32),
}
