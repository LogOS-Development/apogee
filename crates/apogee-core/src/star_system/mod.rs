//! Low-fidelity analytic star-system state for visualization and mission planning.
//!
//! This module is intentionally simple: analytic formulas for major bodies
//! that are fast and good enough for visual rendering (~1° accuracy for the
//! Sun/Earth/Moon). Higher precision ephemeris can be swapped in later without
//! changing the public API.
//!
//! All state is stored in SI units: positions in meters, velocities in
//! meters per second, accelerations in meters per second squared, and GM in
//! m^3/s^2. The analytic formulas internally use AU/day because that is their
//! natural scale, but results are converted to SI at construction time.
//! Callers that need AU, km/s, etc. convert at output boundaries.

use std::collections::HashMap;

use apogee_common::constants::AU;
use apogee_common::math::modulo;
use apogee_common::units::GravitationalParameter;
use apogee_common::units::{PositionVector, VelocityVector, AccelerationVector, AngularVelocityVector, Radians};
use hifitime::Epoch;

/// A celestial body with barycentric state, physical parameters, and orientation.
///
/// This is the foundation for a future ECS/config-driven star system. Each
/// body knows its name, NAIF-style ID, gravitational parameter, current
/// barycentric state, and rotation. Propagators can be attached later.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CelestialBody {
    /// Human-readable name, e.g. "Sun", "Earth", "Moon".
    pub name: &'static str,
    /// NAIF-style body identifier, if known.
    pub naif_id: Option<i32>,
    /// Gravitational parameter GM (m^3/s^2).
    pub gm: GravitationalParameter<f64>,
    /// Barycentric position in meters, SSB J2000 equatorial.
    pub position: PositionVector,
    /// Barycentric velocity in meters per second, SSB J2000 equatorial.
    pub velocity: VelocityVector,
    /// Barycentric acceleration in meters per second squared, SSB J2000 equatorial.
    pub acceleration: AccelerationVector,
    /// Angular velocity vector (rad/s) — magnitude is spin rate, direction is rotation pole.
    pub angular_velocity: AngularVelocityVector,
    /// Current rotation angle about `angular_velocity` (radians).
    pub rotation_angle: Radians<f64>,
}

impl CelestialBody {
    /// Unit direction from this body to an observer at `origin`.
    /// Convention: vector points from self to the observer.
    pub fn direction_to(&self, origin: &PositionVector) -> PositionVector {
        PositionVector::new((origin.value() - self.position.value()).normalize())
    }

    /// Displacement vector from this body to an observer at `origin` (meters).
    /// Convention: vector points from self to the observer.
    pub fn vector_to(&self, origin: &PositionVector) -> PositionVector {
        PositionVector::new(origin.value() - self.position.value())
    }

    /// Scalar distance from this body to an observer at `origin` (meters).
    pub fn distance_to(&self, origin: &PositionVector) -> f64 {
        self.position.distance_to(origin)
    }

    /// Relative velocity of `origin` with respect to this body (m/s).
    pub fn velocity_to(&self, origin: &VelocityVector) -> VelocityVector {
        VelocityVector::new(origin.value() - self.velocity.value())
    }

    /// Relative acceleration of `origin` with respect to this body (m/s²).
    pub fn acceleration_to(&self, origin: &AccelerationVector) -> AccelerationVector {
        AccelerationVector::new(origin.value() - self.acceleration.value())
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
}

impl StarSystem {
    /// Compute the star-system state at the given epoch.
    ///
    /// Uses the low-precision analytic model from "Astronomical Algorithms" by
    /// Jean Meeus (Ch. 25) for the Sun's apparent position as seen from Earth.
    /// For Phase 1 the solar-system barycenter is approximated as the Sun;
    /// later this will be replaced by a proper JPL/NAIF ephemeris backend.
    ///
    /// Returned state is in SI units.
    pub fn at_epoch(epoch: Epoch) -> Self {
        let julian_day = epoch.to_jde_tt_days();
        let days_since_j2000 = julian_day - 2451545.0;

        // Mean longitude of the Sun (degrees).
        let l = modulo(280.460 + 0.9856474 * days_since_j2000, 360.0);

        // Mean anomaly of the Sun (degrees).
        let g = modulo(357.528 + 0.9856003 * days_since_j2000, 360.0);
        let g_rad = g.to_radians();

        // Ecliptic longitude of the Sun (degrees).
        let lambda = modulo(
            l + 1.915 * g_rad.sin() + 0.020 * (2.0_f64 * g_rad).sin(),
            360.0,
        );
        let lambda_rad = lambda.to_radians();

        // Obliquity of the ecliptic (degrees), simplified.
        let epsilon: f64 = 23.439 - 0.0000004 * days_since_j2000;
        let epsilon_rad = epsilon.to_radians();

        // Distance from Earth to Sun (AU).
        let r_au = 1.00014 - 0.01671 * g_rad.cos() - 0.00014 * (2.0_f64 * g_rad).cos();

        // Sun unit vector in geocentric equatorial J2000 frame.
        let sun_dir_x = lambda_rad.cos();
        let sun_dir_y = epsilon_rad.cos() * lambda_rad.sin();
        let sun_dir_z = epsilon_rad.sin() * lambda_rad.sin();
        let sun_direction = nalgebra::Vector3::new(sun_dir_x, sun_dir_z, -sun_dir_y).normalize();

        // Phase 1: SSB ≈ Sun.
        let sun_position_m = PositionVector::new(nalgebra::Vector3::zeros());

        // Earth is opposite the Sun direction at distance r_au.
        let earth_position_m = PositionVector::new(-sun_direction * r_au * AU);

        // Earth rotation angle: Greenwich Mean Sidereal Time.
        let gmst_deg = modulo(280.46061837 + 360.98564736629 * days_since_j2000, 360.0);
        let earth_rotation_rad = gmst_deg.to_radians();

        // Celestial pole for Earth in J2000 equatorial frame.
        let earth_pole = nalgebra::Vector3::new(
            epsilon_rad.sin() * lambda_rad.sin(),
            -epsilon_rad.cos() * lambda_rad.sin(),
            lambda_rad.cos(),
        )
        .normalize();
        let earth_rotation_axis = AngularVelocityVector::new(earth_pole);

        let mut bodies = HashMap::new();
        bodies.insert(
            "Sun".to_string(),
            CelestialBody {
                name: "Sun",
                naif_id: Some(10),
                gm: GravitationalParameter::new(apogee_common::constants::GM_SUN),
                position: sun_position_m,
                velocity: VelocityVector::default(),
                acceleration: AccelerationVector::default(),
                angular_velocity: AngularVelocityVector::new(nalgebra::Vector3::z()),
                rotation_angle: Radians::new(0.0),
            },
        );
        bodies.insert(
            "Earth".to_string(),
            CelestialBody {
                name: "Earth",
                naif_id: Some(399),
                gm: GravitationalParameter::new(apogee_common::constants::GM_EARTH),
                position: earth_position_m,
                velocity: VelocityVector::default(),
                acceleration: AccelerationVector::default(),
                angular_velocity: earth_rotation_axis,
                rotation_angle: Radians::new(earth_rotation_rad),
            },
        );

        Self {
            epoch,
            julian_day,
            days_since_j2000,
            bodies,
        }
    }

    /// Convenience accessor for a body by name.
    pub fn body(&self, name: &str) -> Option<&CelestialBody> {
        self.bodies.get(name)
    }

    /// Displacement vector from `observer_name` to `target_name` (meters).
    /// Convention: vector points from target to observer.
    pub fn vector_between(&self, observer_name: &str, target_name: &str) -> Option<PositionVector> {
        let observer = self.body(observer_name)?;
        let target = self.body(target_name)?;
        Some(observer.vector_to(&target.position))
    }

    /// Unit direction from `observer_name` to `target_name`.
    /// Convention: vector points from target to observer.
    pub fn direction_between(&self, observer_name: &str, target_name: &str) -> Option<PositionVector> {
        let observer = self.body(observer_name)?;
        let target = self.body(target_name)?;
        Some(observer.direction_to(&target.position))
    }

    /// Distance from `observer_name` to `target_name` (meters).
    pub fn distance_between(&self, observer_name: &str, target_name: &str) -> Option<f64> {
        let observer = self.body(observer_name)?;
        let target = self.body(target_name)?;
        Some(observer.distance_to(&target.position))
    }
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
            dir.value()
        );
        assert!(
            (dir.y - (-0.392)).abs() < 0.02,
            "expected swapped-Y ≈ -0.392, got {:?}",
            dir.value()
        );
        assert!(
            (dir.z - 0.902).abs() < 0.02,
            "expected swapped-Z ≈ +0.902, got {:?}",
            dir.value()
        );

        assert!((system.distance_between("Earth", "Sun").unwrap() - AU).abs() < 0.02 * AU);

        assert!((dir.value().norm() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn six_months_later_sun_is_opposite() {
        let j2000 = Epoch::from_jde_in_time_scale(2451545.0, TimeScale::TT);
        let later = j2000 + 180.0 * Unit::Day;
        let system_later = StarSystem::at_epoch(later);
        let system_j2000 = StarSystem::at_epoch(j2000);

        let a = system_j2000.direction_between("Earth", "Sun").unwrap();
        let b = system_later.direction_between("Earth", "Sun").unwrap();
        let dot = a.value().dot(b.value());
        assert!(
            dot < -0.95,
            "expected Sun direction to flip ~180°, dot={}",
            dot
        );
    }

    #[test]
    fn celestial_body_velocity_and_acceleration_to() {
        let epoch = Epoch::from_jde_in_time_scale(2451545.0, TimeScale::TT);
        let system = StarSystem::at_epoch(epoch);
        let earth = system.body("Earth").unwrap();
        let sun = system.body("Sun").unwrap();

        // Relative velocity of Sun wrt Earth (both ~zero at J2000 in this model)
        let rel_vel = earth.velocity_to(&sun.velocity);
        assert_relative_eq!(rel_vel.vector.norm(), 0.0);

        // Relative acceleration
        let rel_acc = earth.acceleration_to(&sun.acceleration);
        assert_relative_eq!(rel_acc.vector.norm(), 0.0);
    }

    #[test]
    fn celestial_body_angular_velocity_is_set() {
        let epoch = Epoch::from_jde_in_time_scale(2451545.0, TimeScale::TT);
        let system = StarSystem::at_epoch(epoch);
        let earth = system.body("Earth").unwrap();
        // Angular velocity direction should be the pole (not zero)
        assert!(earth.angular_velocity.vector.norm() > 0.0);
        // Rotation angle should be wrapped
        assert!(earth.rotation_angle.value >= 0.0);
    }
}
