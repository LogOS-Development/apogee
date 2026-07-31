//! Frame transformation service: ICRF, ECI, ECEF, ECLIPJ2000.

pub mod eop;
pub mod leap_seconds;

pub use eop::*;
pub use leap_seconds::*;

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

/// Frame transformation service.
#[derive(Debug, Default)]
pub struct FrameService {
    // TODO: EOP data, nutation/precession models
}

impl FrameService {
    pub fn new() -> Self {
        Self::default()
    }

    // TODO: transform(position, from, to, epoch)
}
