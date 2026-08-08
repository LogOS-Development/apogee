//! Solar radiation pressure model with eclipse detection.

use apogee_common::constants::{AU, R_EARTH_EQ, SRP_1AU};
use apogee_common::units::{AccelerationVector, Area, Dimensionless, Kilograms};
use apogee_common::Position;
use nalgebra::Vector3;

/// Solar radiation pressure acceleration on a flat plate / cannonball spacecraft.
#[derive(Debug, Clone, Copy, Default)]
pub struct SolarRadiationPressure;

impl SolarRadiationPressure {
    /// Compute SRP acceleration given the spacecraft inertial position,
    /// the Sun's inertial position, the spacecraft SRP area, reflectivity,
    /// and mass.
    ///
    /// Returns the acceleration as [`AccelerationVector`] (m/s²) in the
    /// inertial frame.
    pub fn acceleration(
        &self,
        spacecraft_position: &Position,
        sun_position: &Position,
        srp_area: Area<f64>,
        reflectivity: Dimensionless<f64>,
        mass: Kilograms<f64>,
    ) -> AccelerationVector {
        let to_sun = sun_position - spacecraft_position;
        let r = to_sun.norm();
        if r == 0.0 {
            return AccelerationVector::new(Vector3::zeros());
        }

        if is_eclipsed(spacecraft_position, sun_position) {
            return AccelerationVector::new(Vector3::zeros());
        }

        let flux_factor = AU * AU / (r * r);
        let pressure = SRP_1AU * flux_factor;
        // (1 + reflectivity) factor for flat plate normal to Sun; use as effective
        // scaling for cannonball model. F = P * A * (1 + r) gives newtons; a = F/m.
        let force_magnitude = pressure * srp_area.into_value() * (1.0 + reflectivity.into_value());
        let direction = to_sun / r;
        let accel_magnitude = force_magnitude / mass.into_value();

        AccelerationVector::new(direction * accel_magnitude)
    }

    /// SRP acceleration using the Sun fixed at origin (heliocentric
    /// approximation for Earth-orbiting craft).
    pub fn acceleration_sun_at_origin(
        &self,
        spacecraft_position: &Position,
        srp_area: Area<f64>,
        reflectivity: Dimensionless<f64>,
        mass: Kilograms<f64>,
    ) -> AccelerationVector {
        // Place the Sun at +1 AU so the spacecraft at origin is on the sunlit
        // side of Earth and not eclipsed.
        self.acceleration(
            spacecraft_position,
            &Position::new(AU, 0.0, 0.0),
            srp_area,
            reflectivity,
            mass,
        )
    }
}

/// Simple cylindrical eclipse check: spacecraft is eclipsed if it is in
/// Earth's shadow cylinder opposite the Sun. Uses WGS84 equatorial radius
/// as a conservative approximation.
fn is_eclipsed(spacecraft_position: &Position, sun_position: &Position) -> bool {
    // Vector from Sun to spacecraft.
    let sun_to_sc = spacecraft_position - sun_position;
    // Unit Sun direction.
    let sun_dir = sun_position / sun_position.norm();
    // Project Sun->spacecraft onto Sun direction.
    let projection = sun_to_sc.dot(&sun_dir);
    // Perpendicular distance from Earth's center to Sun-spacecraft line.
    let closest_approach = sun_to_sc - sun_dir * projection;
    let distance = closest_approach.norm();

    // Eclipse only if spacecraft is between Sun and Earth.
    if projection > 0.0 {
        return false;
    }

    distance < R_EARTH_EQ
}

impl crate::systems::force_model::ForceModel for SolarRadiationPressure {
    fn name(&self) -> &str {
        "solar radiation pressure"
    }

    fn acceleration(&self, ctx: &crate::systems::force_model::ForceContext) -> AccelerationVector {
        let sun_pos = ctx
            .celestial
            .states
            .iter()
            .find(|s| s.naif_id == 10)
            .map(|s| s.position)
            .unwrap_or_else(|| Vector3::new(-apogee_common::constants::AU, 0.0, 0.0));
        self.acceleration(
            &ctx.kinematics.position,
            &sun_pos,
            ctx.config.srp_area,
            apogee_common::units::Dimensionless::new(ctx.config.reflectivity),
            ctx.rigid_body.mass,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_srp_at_1au() {
        let srp = SolarRadiationPressure;
        let acc = srp.acceleration(
            &Position::new(AU, 0.0, 0.0),
            &Position::new(0.0, 0.0, 0.0),
            Area::new(1.0),
            Dimensionless::new(1.0),
            Kilograms::new(1.0),
        );
        let expected_magnitude = SRP_1AU * 2.0 / 1.0;
        assert_relative_eq!(acc.raw().norm(), expected_magnitude, epsilon = 1e-12);
        assert_relative_eq!(-acc.raw().x, expected_magnitude, epsilon = 1e-12);
    }

    #[test]
    fn test_eclipse_blocks_srp() {
        let srp = SolarRadiationPressure;
        let sc = Position::new(-(R_EARTH_EQ + 100_000.0), 0.0, 0.0);
        let sun = Position::new(AU, 0.0, 0.0);
        let acc = srp.acceleration(
            &sc,
            &sun,
            Area::new(1.0),
            Dimensionless::new(1.0),
            Kilograms::new(1.0),
        );
        assert_eq!(acc.raw().norm(), 0.0);
    }
}
