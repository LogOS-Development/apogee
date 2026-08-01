//! TLE state-vector conversion and orbital-element helpers.

use std::f64::consts::PI;

use apogee_common::{constants::GM_EARTH, Position, Velocity};
use nalgebra::{Rotation3, Vector3};

use super::parser::Tle;

/// Keplerian orbital elements in TLE-compatible units.
#[derive(Debug, Clone, Copy)]
pub struct KeplerianElements {
    /// Semi-major axis (m).
    pub semi_major_axis: f64,
    /// Eccentricity (dimensionless).
    pub eccentricity: f64,
    /// Inclination (rad).
    pub inclination: f64,
    /// Right ascension of ascending node (rad).
    pub raan: f64,
    /// Argument of perigee (rad).
    pub arg_perigee: f64,
    /// True anomaly (rad).
    pub true_anomaly: f64,
}

impl Tle {
    /// Convert the TLE to Keplerian elements in SI units.
    pub fn to_keplerian(&self) -> KeplerianElements {
        let n = self.mean_motion * 2.0 * PI / 86_400.0; // rad/s
        let a = (GM_EARTH / n.powi(2)).cbrt(); // m

        let i = self.inclination.to_radians();
        let raan = self.raan.to_radians();
        let arg_perigee = self.arg_perigee.to_radians();
        let mean_anomaly = self.mean_anomaly.to_radians();
        let e = self.eccentricity;

        let true_anomaly = mean_anomaly_to_eccentric_to_true(mean_anomaly, e);

        KeplerianElements {
            semi_major_axis: a,
            eccentricity: e,
            inclination: i,
            raan,
            arg_perigee,
            true_anomaly,
        }
    }

    /// Convert the TLE to an inertial state vector (position, velocity) at epoch.
    ///
    /// The output is in the J2000/ICRF-equivalent inertial frame used internally
    /// by Apogee. Note that TLE elements are mean elements; this conversion is
    /// accurate to ~1 km for the purpose of initializing a numerical propagator.
    pub fn to_state_vector(&self) -> (Position, Velocity) {
        self.to_keplerian().to_state_vector()
    }

    /// Return the TLE epoch as a hifitime `Epoch` (UTC).
    pub fn epoch(&self) -> hifitime::Epoch {
        let day = self.epoch_day.floor();
        let fraction = self.epoch_day - day;
        let seconds = fraction * 86_400.0;
        let year = self.epoch_year as i32;
        hifitime::Epoch::from_gregorian_utc_at_midnight(year, 1, 1)
            + hifitime::Unit::Day * (day - 1.0)
            + hifitime::Unit::Second * seconds
    }
}

impl KeplerianElements {
    /// Convert Keplerian elements to inertial position/velocity.
    pub fn to_state_vector(&self) -> (Position, Velocity) {
        let a = self.semi_major_axis;
        let e = self.eccentricity;
        let i = self.inclination;
        let raan = self.raan;
        let arg = self.arg_perigee;
        let nu = self.true_anomaly;

        let p = a * (1.0 - e * e);
        let r = p / (1.0 + e * nu.cos());

        let position_perifocal = Vector3::new(r * nu.cos(), r * nu.sin(), 0.0);
        let velocity_perifocal = Vector3::new(
            -(GM_EARTH / p).sqrt() * nu.sin(),
            (GM_EARTH / p).sqrt() * (e + nu.cos()),
            0.0,
        );

        let r3 = Rotation3::from_euler_angles(0.0, 0.0, raan)
            * Rotation3::from_euler_angles(0.0, i, 0.0)
            * Rotation3::from_euler_angles(0.0, 0.0, arg);

        let position = r3 * position_perifocal;
        let velocity = r3 * velocity_perifocal;
        (position, velocity)
    }
}

/// Solve Kepler's equation for eccentric anomaly from mean anomaly, then convert
/// to true anomaly. Newton iteration.
fn mean_anomaly_to_eccentric_to_true(mean_anomaly: f64, e: f64) -> f64 {
    let mut ecc = mean_anomaly;
    for _ in 0..50 {
        let f = ecc - e * ecc.sin() - mean_anomaly;
        let fp = 1.0 - e * ecc.cos();
        let delta = f / fp;
        ecc -= delta;
        if delta.abs() < 1e-12 {
            break;
        }
    }

    let beta = e / (1.0 + (1.0 - e * e).sqrt());
    ecc + 2.0 * (beta * ecc.sin() / (1.0 - beta * ecc.cos())).atan()
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    #[test]
    fn test_circular_equatorial() {
        let elems = KeplerianElements {
            semi_major_axis: apogee_common::constants::R_EARTH_EQ + 400_000.0,
            eccentricity: 0.0,
            inclination: 0.0,
            raan: 0.0,
            arg_perigee: 0.0,
            true_anomaly: 0.0,
        };
        let (pos, vel) = elems.to_state_vector();
        assert_relative_eq!(pos.y, 0.0, epsilon = 1e-9);
        assert_relative_eq!(pos.z, 0.0, epsilon = 1e-9);
        assert_relative_eq!(vel.x, 0.0, epsilon = 1e-9);
        assert_relative_eq!(vel.z, 0.0, epsilon = 1e-9);
        assert!(vel.y > 0.0);
    }
}
