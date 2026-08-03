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

/// HWM14 empirical horizontal wind model (Fortran via FFI).
///
/// Available only when the `hwm14` feature is enabled. The model coefficient
/// files are vendored and extracted to a temporary directory on first use.
#[cfg(feature = "hwm14")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Hwm14;

#[cfg(feature = "hwm14")]
impl Hwm14 {
    /// Evaluate HWM14 for the given input.
    pub fn evaluate(input: &WindInput) -> WindOutput {
        let iyd = two_digit_year_and_doy(input.day_of_year);
        let sec = input.local_solar_time_hours * 3600.0; // HWM14 expects UT seconds; using LST as approximation
        let (meridional, zonal) = hwm14_sys::Hwm14::evaluate(
            iyd,
            sec,
            input.altitude_m / 1000.0,
            input.latitude_rad.to_degrees(),
            input.longitude_rad.to_degrees(),
            input.local_solar_time_hours,
            -1.0,
            -1.0,
            input.ap,
        );
        WindOutput {
            east_mps: zonal,
            north_mps: meridional,
            up_mps: 0.0,
        }
    }
}

#[cfg(feature = "hwm14")]
impl HorizontalWindModel for Hwm14 {
    fn evaluate(&self, input: &WindInput) -> WindOutput {
        Self::evaluate(input)
    }
}

#[cfg(feature = "hwm14")]
fn two_digit_year_and_doy(doy: u16) -> i32 {
    // HWM14's iyd is yyddd. The model is not sensitive to the year for
    // climatological winds, so we use a neutral reference year (1993).
    let year = 93;
    year * 1000 + i32::from(doy)
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
