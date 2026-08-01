//! Horizontal Wind Model (HWM) — placeholder implementation.
//!
//! The full Horizontal Wind Model (e.g., HWM14 or HWM07) predicts meridional
//! and zonal thermospheric wind velocities as a function of altitude,
//! latitude, local time, season, solar activity, and geomagnetic activity.
//!
//! This module provides the API surface and a trivial placeholder that
//! returns zero wind, allowing drag simulations to depend on a typed
//! `HorizontalWindModel` without blocking on a full HWM port. A future PR
//! will either vendor or implement HWM14.

use nalgebra::Vector3;

/// Geodetic location and conditions for wind evaluation.
#[derive(Debug, Clone, Copy)]
pub struct WindInput {
    /// Altitude above the ellipsoid, metres.
    pub altitude_m: f64,
    /// Geodetic latitude, radians.
    pub latitude_rad: f64,
    /// Geodetic longitude, radians.
    pub longitude_rad: f64,
    /// Local apparent solar time, hours.
    pub local_solar_time_hours: f64,
    /// Day of year (1..=366).
    pub day_of_year: u16,
    /// Daily F10.7 solar flux, sfu.
    pub f107: f64,
    /// Daily Ap geomagnetic index.
    pub ap: f64,
}

/// Wind velocity output, m/s, in the local East/North/Up frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindOutput {
    /// Eastward component, m/s.
    pub east_mps: f64,
    /// Northward component, m/s.
    pub north_mps: f64,
    /// Upward component, m/s.
    pub up_mps: f64,
}

impl WindOutput {
    /// Wind vector in the local ENU frame.
    pub fn enu(&self) -> Vector3<f64> {
        Vector3::new(self.east_mps, self.north_mps, self.up_mps)
    }
}

/// Trait for empirical horizontal wind models.
pub trait HorizontalWindModel: Send + Sync {
    /// Evaluate the wind at the given location and activity conditions.
    fn evaluate(&self, input: &WindInput) -> WindOutput;
}

/// HWM placeholder model that returns zero wind.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hwm;

impl Hwm {
    /// Evaluate the placeholder model.
    pub fn evaluate(_input: &WindInput) -> WindOutput {
        WindOutput::default()
    }
}

impl HorizontalWindModel for Hwm {
    fn evaluate(&self, input: &WindInput) -> WindOutput {
        Self::evaluate(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_returns_zero() {
        let input = WindInput {
            altitude_m: 400_000.0,
            latitude_rad: 0.0,
            longitude_rad: 0.0,
            local_solar_time_hours: 12.0,
            day_of_year: 80,
            f107: 150.0,
            ap: 4.0,
        };
        let out = Hwm::evaluate(&input);
        assert_eq!(out.east_mps, 0.0);
        assert_eq!(out.north_mps, 0.0);
        assert_eq!(out.up_mps, 0.0);
    }
}
