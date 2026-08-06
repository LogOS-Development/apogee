//! Low-fidelity analytic star-system state for visualization and mission planning.
//!
//! This module is intentionally simple: analytic formulas for major bodies
//! that are fast and good enough for visual rendering (~1° accuracy for the
//! Sun/Earth/Moon). Higher precision ephemeris can be swapped in later without
//! changing the public API.
//!
//! All vector positions/velocities are stored in SI base units (meters,
//! meters/second) in the solar-system barycentric (SSB) equatorial J2000 frame.
//! The scalar unit types from `apogee_common::units` are used for GM and for
//! any scalar magnitudes. Godot-facing helpers convert to astronomical units
//! (AU) because that is the natural scale for the visualizer scene.

use std::collections::HashMap;

use apogee_common::constants::AU;
use apogee_common::units::GravitationalParameter;
use apogee_common::{Position, Velocity};
use hifitime::Epoch;

/// Seconds per day, used to convert AU/day analytic velocities to SI.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// A celestial body with barycentric state and physical parameters.
///
/// This is the foundation for a future ECS/config-driven star system. Each
/// body knows its name, NAIF-style ID, gravitational parameter, and current
/// barycentric position/velocity. Propagators can be attached later.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CelestialBody {
    /// Human-readable name, e.g. "Sun", "Earth", "Moon".
    pub name: &'static str,
    /// NAIF-style body identifier, if known.
    pub naif_id: Option<i32>,
    /// Gravitational parameter GM.
    pub gm: GravitationalParameter<f64>,
    /// Barycentric position in meters, SSB J2000 equatorial.
    pub position_m: Position,
    /// Barycentric velocity in meters per second, SSB J2000 equatorial.
    pub velocity_m_s: Velocity,
    /// Barycentric acceleration in meters per second squared, SSB J2000 equatorial.
    pub acceleration_m_s2: Position,
}

impl CelestialBody {
    /// Position in astronomical units.
    pub fn position_au(&self) -> Position {
        self.position_m / AU
    }

    /// Velocity in astronomical units per day.
    pub fn velocity_au_day(&self) -> Velocity {
        self.velocity_m_s * SECONDS_PER_DAY / AU
    }

    /// Acceleration in astronomical units per day squared.
    pub fn acceleration_au_day2(&self) -> Position {
        self.acceleration_m_s2 * (SECONDS_PER_DAY * SECONDS_PER_DAY) / AU
    }

    /// Scalar distance from an observer at `origin_m` to this body (AU).
    pub fn distance_au_to(&self, origin_m: &Position) -> f64 {
        (self.position_m - origin_m).norm() / AU
    }

    /// Unit vector from an observer at `origin_m` to this body.
    pub fn direction_to(&self, origin_m: &Position) -> Position {
        (self.position_m - origin_m).normalize()
    }

    /// Unit vector from this body to an observer at `origin_m`.
    pub fn direction_from(&self, origin_m: &Position) -> Position {
        (origin_m - self.position_m).normalize()
    }

    /// Displacement vector from an observer at `origin_m` to this body (AU).
    pub fn vector_to(&self, origin_m: &Position) -> Position {
        (self.position_m - origin_m) / AU
    }

    /// Displacement vector from this body to an observer at `origin_m` (AU).
    pub fn vector_from(&self, origin_m: &Position) -> Position {
        (origin_m - self.position_m) / AU
    }
}

/// Snapshot of a star system at a single epoch.
#[derive(Debug, Clone, PartialEq)]
pub struct StarSystem {
    /// Epoch of the snapshot.
    pub epoch: Epoch,
    /// Julian Day (TT) corresponding to `epoch`.
    pub julian_day: f64,
    /// Days since J2000.0 (JD 2451545.0).
    pub days_since_j2000: f64,
    /// Bodies keyed by name.
    pub bodies: HashMap<String, CelestialBody>,
    /// Earth orientation: rotation angle about the celestial pole (radians).
    pub earth_rotation_rad: f64,
    /// Earth's obliquity of the ecliptic at the epoch (radians).
    pub earth_obliquity_rad: f64,
}

impl StarSystem {
    /// Compute the star-system state at the given epoch.
    ///
    /// Uses the low-precision analytic model from "Astronomical Algorithms" by
    /// Jean Meeus (Ch. 25) for the Sun's apparent position as seen from Earth.
    /// For Phase 1 the solar-system barycenter is approximated as the Sun;
    /// later this will be replaced by a proper JPL/NAIF ephemeris backend.
    pub fn at_epoch(epoch: Epoch) -> Self {
        let julian_day = epoch.to_jde_tt_days();
        let days_since_j2000 = julian_day - 2451545.0;

        // Mean longitude of the Sun (degrees).
        let l = wrap_degrees(280.460 + 0.9856474 * days_since_j2000);

        // Mean anomaly of the Sun (degrees).
        let g = wrap_degrees(357.528 + 0.9856003 * days_since_j2000);
        let g_rad = g.to_radians();

        // Ecliptic longitude of the Sun (degrees).
        let lambda = wrap_degrees(l + 1.915 * g_rad.sin() + 0.020 * (2.0 * g_rad).sin());
        let lambda_rad = lambda.to_radians();

        // Obliquity of the ecliptic (degrees), simplified.
        let epsilon: f64 = 23.439 - 0.0000004 * days_since_j2000;
        let epsilon_rad = epsilon.to_radians();

        // Distance from Earth to Sun (AU).
        let r_au = 1.00014 - 0.01671 * g_rad.cos() - 0.00014 * (2.0 * g_rad).cos();

        // Sun unit vector in geocentric equatorial J2000 frame.
        let sun_dir_x = lambda_rad.cos();
        let sun_dir_y = epsilon_rad.cos() * lambda_rad.sin();
        let sun_dir_z = epsilon_rad.sin() * lambda_rad.sin();
        let sun_direction = nalgebra::Vector3::new(sun_dir_x, sun_dir_z, -sun_dir_y).normalize();

        // Phase 1: SSB ≈ Sun.
        let sun_position_m = Position::zeros();

        // Earth is opposite the Sun direction at distance r_au.
        let earth_position_m = -sun_direction * r_au * AU;

        // Earth rotation angle: Greenwich Mean Sidereal Time.
        let gmst_deg = wrap_degrees(280.46061837 + 360.98564736629 * days_since_j2000);
        let earth_rotation_rad = gmst_deg.to_radians();

        let mut bodies = HashMap::new();
        bodies.insert(
            "Sun".to_string(),
            CelestialBody {
                name: "Sun",
                naif_id: Some(10),
                gm: GravitationalParameter::new(apogee_common::constants::GM_SUN),
                position_m: sun_position_m,
                velocity_m_s: Velocity::zeros(),
                acceleration_m_s2: Position::zeros(),
            },
        );
        bodies.insert(
            "Earth".to_string(),
            CelestialBody {
                name: "Earth",
                naif_id: Some(399),
                gm: GravitationalParameter::new(apogee_common::constants::GM_EARTH),
                position_m: earth_position_m,
                velocity_m_s: Velocity::zeros(),
                acceleration_m_s2: Position::zeros(),
            },
        );

        Self {
            epoch,
            julian_day,
            days_since_j2000,
            bodies,
            earth_rotation_rad,
            earth_obliquity_rad: epsilon_rad,
        }
    }

    /// Convenience accessor for a body by name.
    pub fn body(&self, name: &str) -> Option<&CelestialBody> {
        self.bodies.get(name)
    }

    /// Displacement vector from `observer_name` to `target_name` (AU).
    pub fn vector_between(&self, observer_name: &str, target_name: &str) -> Option<Position> {
        let observer = self.body(observer_name)?;
        let target = self.body(target_name)?;
        Some(target.vector_to(&observer.position_m))
    }

    /// Unit direction from `observer_name` to `target_name`.
    pub fn direction_between(&self, observer_name: &str, target_name: &str) -> Option<Position> {
        let observer = self.body(observer_name)?;
        let target = self.body(target_name)?;
        Some(target.direction_to(&observer.position_m))
    }

    /// Distance from `observer_name` to `target_name` (AU).
    pub fn distance_between(&self, observer_name: &str, target_name: &str) -> Option<f64> {
        let observer = self.body(observer_name)?;
        let target = self.body(target_name)?;
        Some(target.distance_au_to(&observer.position_m))
    }
}

fn wrap_degrees(angle: f64) -> f64 {
    let mut a = angle % 360.0;
    if a < 0.0 {
        a += 360.0;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use hifitime::{TimeScale, Unit};

    #[test]
    fn j2000_sun_direction_matches_analytic_reference() {
        let epoch = Epoch::from_jde_in_time_scale(2451545.0, TimeScale::TT);
        let system = StarSystem::at_epoch(epoch);

        let dir = system.direction_between("Earth", "Sun").unwrap();

        // Reference values in our swapped-Y/Z frame.
        assert!(
            (dir.x - 0.180).abs() < 0.02,
            "expected X ≈ +0.18, got {:?}",
            dir
        );
        assert!(
            (dir.y - (-0.392)).abs() < 0.02,
            "expected swapped-Y ≈ -0.392, got {:?}",
            dir
        );
        assert!(
            (dir.z - 0.902).abs() < 0.02,
            "expected swapped-Z ≈ +0.902, got {:?}",
            dir
        );

        assert!((system.distance_between("Earth", "Sun").unwrap() - 1.0).abs() < 0.02);

        assert!((dir.norm() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn six_months_later_sun_is_opposite() {
        let j2000 = Epoch::from_jde_in_time_scale(2451545.0, TimeScale::TT);
        let later = j2000 + 180.0 * Unit::Day;
        let system_later = StarSystem::at_epoch(later);
        let system_j2000 = StarSystem::at_epoch(j2000);

        let a = system_j2000.direction_between("Earth", "Sun").unwrap();
        let b = system_later.direction_between("Earth", "Sun").unwrap();
        let dot = a.dot(&b);
        assert!(
            dot < -0.95,
            "expected Sun direction to flip ~180°, dot={}",
            dot
        );
    }
}
