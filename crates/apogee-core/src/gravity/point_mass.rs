//! Point-mass N-body gravity.
//!
//! Computes the total gravitational acceleration acting on a small body
//! (spacecraft) due to a set of massive celestial bodies supplied by the
//! ephemeris service.

use crate::ephemeris::kernel::{BodyState, SolarSystemState};
use apogee_common::units::AccelerationVec;
use apogee_common::{gravitational_parameter, Position};
use nalgebra::Vector3;

/// Point-mass gravity model.
///
/// The model is stateless; all required masses and positions are taken from
/// the provided [`SolarSystemState`]. Bodies whose NAIF ID is not present in
/// the built-in GM table are skipped.
#[derive(Debug, Default)]
pub struct PointMassGravity;

impl PointMassGravity {
    /// Compute gravitational acceleration from all celestial bodies.
    ///
    /// `position` is the inertial position of the spacecraft. `celestial`
    /// contains the positions (and NAIF IDs) of the massive bodies. The
    /// acceleration is the sum over all bodies of:
    ///
    ///   a_i = GM_i * (r_i - r) / |r_i - r|^3
    ///
    /// where `r` is the spacecraft position and `r_i` is the body position.
    /// The returned vector carries an m/s² unit tag at the public API surface
    /// while the internal math remains on raw `Vector3<f64>`.
    ///
    /// This is O(N) in the number of celestial bodies.
    pub fn acceleration(
        &self,
        position: &Position,
        celestial: &SolarSystemState,
    ) -> Result<AccelerationVec, String> {
        let mut acc = Vector3::zeros();

        for BodyState {
            naif_id,
            position: body_pos,
            velocity: _,
        } in &celestial.states
        {
            let Some(gm) = gravitational_parameter(*naif_id) else {
                continue;
            };

            let delta = body_pos - position;
            let r2 = delta.norm_squared();
            if r2 == 0.0 {
                return Err(format!(
                    "coincident positions for body {naif_id}: singularity in point-mass gravity"
                ));
            }
            let r3 = r2 * r2.sqrt();
            // GM has units m³/s², dividing by m³ (delta/r3) yields m/s². The
            // type tag is preserved in the wrapper; the underlying arithmetic
            // is dimensionally consistent.
            acc += gm * delta / r3;
        }

        Ok(AccelerationVec::new(acc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn body_state(naif_id: i32, position: [f64; 3]) -> BodyState {
        BodyState {
            naif_id,
            position: Vector3::new(position[0], position[1], position[2]),
            velocity: Vector3::zeros(),
        }
    }

    #[test]
    fn test_sun_only_at_1au() {
        let gravity = PointMassGravity {};
        let spacecraft = Vector3::new(apogee_common::constants::AU, 0.0, 0.0);

        let celestial = SolarSystemState {
            states: vec![body_state(10, [0.0, 0.0, 0.0])],
        };

        let acc = gravity.acceleration(&spacecraft, &celestial).unwrap();
        let raw = acc.raw();
        let expected = -apogee_common::constants::GM_SUN / apogee_common::constants::AU.powi(2);

        assert_relative_eq!(raw.x, expected, epsilon = 1e-6);
        assert_relative_eq!(raw.y, 0.0, epsilon = 1e-15);
        assert_relative_eq!(raw.z, 0.0, epsilon = 1e-15);
        // Unit tag survives the conversion.
        assert_relative_eq!(acc.x().into_value(), expected, epsilon = 1e-6);
    }

    #[test]
    fn test_earth_dominates_near_earth() {
        let gravity = PointMassGravity {};
        // Spacecraft 400 km above Earth's equator.
        let r = apogee_common::constants::R_EARTH_EQ + 400_000.0;
        let spacecraft = Vector3::new(r, 0.0, 0.0);

        // Earth at origin, Sun 1 AU away on y-axis. The spacecraft is close
        // to Earth, so Earth dominates; the Sun adds a small orthogonal term.
        let celestial = SolarSystemState {
            states: vec![
                body_state(399, [0.0, 0.0, 0.0]),
                body_state(10, [0.0, apogee_common::constants::AU, 0.0]),
            ],
        };

        let acc = gravity.acceleration(&spacecraft, &celestial).unwrap();
        let earth_acc = apogee_common::constants::GM_EARTH / r.powi(2);

        // Total magnitude is slightly larger than the Earth-only radial
        // acceleration because of the orthogonal Sun contribution.
        let total_mag = acc.raw().norm();
        assert!(
            total_mag > earth_acc && total_mag < earth_acc * 1.01,
            "unexpected acceleration magnitude: {} m/s^2",
            total_mag
        );
        // x component is Earth pull.
        assert_relative_eq!(acc.raw().x, -earth_acc, epsilon = 1e-6);
        // y component is small Sun pull toward +y.
        assert!(acc.raw().y > 0.0 && acc.raw().y < 0.01);
    }

    #[test]
    fn test_unknown_body_is_skipped() {
        let gravity = PointMassGravity {};
        let spacecraft = Vector3::new(apogee_common::constants::AU, 0.0, 0.0);

        let celestial = SolarSystemState {
            states: vec![
                body_state(10, [0.0, 0.0, 0.0]),
                body_state(123_456, [0.0, apogee_common::constants::AU, 0.0]),
            ],
        };

        let acc = gravity.acceleration(&spacecraft, &celestial).unwrap();
        // Only the Sun contributes.
        let expected = -apogee_common::constants::GM_SUN / apogee_common::constants::AU.powi(2);
        assert_relative_eq!(acc.raw().x, expected, epsilon = 1e-6);
    }

    #[test]
    fn test_earth_moon_two_body_cancellation_line() {
        // Spacecraft on the Earth-Moon line, closer to Earth. Net acceleration
        // should point toward the more massive body (Earth) and be continuous.
        let gravity = PointMassGravity {};
        let moon_distance = 384_400_000.0;
        // Spacecraft 1/4 of the way from Earth to Moon.
        let spacecraft = Vector3::new(moon_distance * 0.25, 0.0, 0.0);
        let celestial = SolarSystemState {
            states: vec![
                body_state(399, [0.0, 0.0, 0.0]),
                body_state(301, [moon_distance, 0.0, 0.0]),
            ],
        };

        let acc = gravity.acceleration(&spacecraft, &celestial).unwrap();

        // Net pull is toward Earth (negative x) because Earth dominates.
        assert!(
            acc.raw().x < 0.0,
            "expected net pull toward Earth, got {}",
            acc.raw().x
        );
        assert_relative_eq!(acc.raw().y, 0.0, epsilon = 1e-15);
        assert_relative_eq!(acc.raw().z, 0.0, epsilon = 1e-15);

        // Verify by closed-form two-body sum.
        let r_se = moon_distance * 0.25;
        let r_sm = moon_distance * 0.75;
        let earth_acc = -apogee_common::constants::GM_EARTH / r_se.powi(2);
        let moon_acc = apogee_common::constants::GM_MOON / r_sm.powi(2);
        assert_relative_eq!(acc.raw().x, earth_acc + moon_acc, epsilon = 1e-9);
    }

    #[test]
    fn test_singularity_returns_error() {
        let gravity = PointMassGravity {};
        let spacecraft = Vector3::new(0.0, 0.0, 0.0);
        let celestial = SolarSystemState {
            states: vec![body_state(10, [0.0, 0.0, 0.0])],
        };

        let result = gravity.acceleration(&spacecraft, &celestial);
        assert!(result.is_err());
    }
}
