//! Atmospheric drag force model.

use apogee_common::constants::R_EARTH_EQ;
use apogee_common::units::{AccelerationVector, Area, Density, Kilograms};
use apogee_common::Position;
use nalgebra::Vector3;

use crate::aero::model::{AtmosphereInput, AtmosphereModel};

/// Atmospheric drag acceleration on a spacecraft.
#[derive(Debug, Clone, Copy, Default)]
pub struct AtmosphericDrag;

impl AtmosphericDrag {
    /// Compute drag acceleration in the inertial frame.
    ///
    /// Arguments:
    /// - `spacecraft_position`: inertial position (m)
    /// - `spacecraft_velocity`: inertial velocity (m/s)
    /// - `density`: atmospheric mass density (kg/m³)
    /// - `drag_area`: cross-sectional area exposed to the flow (m²)
    /// - `mass`: spacecraft mass (kg)
    ///
    /// Returns the drag acceleration as [`AccelerationVector`] (m/s²) in the
    /// inertial frame.
    pub fn acceleration(
        &self,
        spacecraft_position: &Position,
        spacecraft_velocity: &Vector3<f64>,
        density: Density<f64>,
        drag_area: Area<f64>,
        mass: Kilograms<f64>,
    ) -> AccelerationVector {
        // Approximate Earth rotation velocity at equator; inertial atmosphere is
        // assumed co-rotating for this simple model.
        let omega_earth = Vector3::new(0.0, 0.0, 7.2921159e-5);
        let r = spacecraft_position.norm();
        let altitude_m = r - R_EARTH_EQ;

        // Effective velocity relative to rotating atmosphere.
        let vel_rel = spacecraft_velocity - omega_earth.cross(spacecraft_position);
        let v_rel = vel_rel.norm();
        let density_value = density.into_value();
        if v_rel == 0.0 || density_value <= 0.0 || altitude_m < 0.0 {
            return AccelerationVector::new(Vector3::zeros());
        }

        // F_drag = 0.5 * rho * v^2 * Cd*A  (N).  Divide by mass to get m/s².
        let force_magnitude = 0.5 * density_value * v_rel * v_rel * drag_area.into_value();
        let accel_magnitude = force_magnitude / mass.into_value();
        AccelerationVector::new(-vel_rel / v_rel * accel_magnitude)
    }

    /// Compute drag acceleration using an atmosphere model to obtain density.
    pub fn acceleration_with_model<M: AtmosphereModel>(
        &self,
        spacecraft_position: &Position,
        spacecraft_velocity: &Vector3<f64>,
        model: &M,
        input: &AtmosphereInput,
        drag_area: Area<f64>,
        mass: Kilograms<f64>,
    ) -> AccelerationVector {
        let output = model.evaluate(input);
        self.acceleration(
            spacecraft_position,
            spacecraft_velocity,
            output.density,
            drag_area,
            mass,
        )
    }
}

impl crate::systems::force_model::ForceModel for AtmosphericDrag {
    fn name(&self) -> &str {
        "atmospheric drag"
    }

    fn acceleration(&self, ctx: &crate::systems::force_model::ForceContext) -> AccelerationVector {
        use crate::aero::model::AtmosphereInput;
        use crate::aero::nrlmsise00::Nrlmsise00;

        // Derive day-of-year and seconds-into-day from the epoch.
        let doy_f64 = ctx.epoch.day_of_year(); // 1-based, fractional
        let day_of_year = doy_f64 as u16;
        let seconds_utc = (doy_f64 - doy_f64.floor()) * 86_400.0;

        let model = Nrlmsise00;
        let latlon = crate::systems::force_aggregator::ecef_lat_lon_from_inertial(
            &ctx.kinematics.position,
            day_of_year,
            seconds_utc,
        );
        let input = AtmosphereInput {
            altitude_m: apogee_common::units::Meters::new(latlon.altitude_m),
            latitude_rad: latlon.latitude_rad,
            longitude_rad: latlon.longitude_rad,
            day_of_year,
            seconds_utc,
            f107: ctx.sim_config.f107,
            f107a: ctx.sim_config.f107a,
            ap: ctx.sim_config.ap,
        };
        let drag_area = ctx.config.drag_area(ctx.rigid_body.mass);
        self.acceleration_with_model(
            &ctx.kinematics.position,
            &ctx.kinematics.velocity,
            &model,
            &input,
            drag_area,
            ctx.rigid_body.mass,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector3;

    #[test]
    fn test_drag_opposes_velocity() {
        let drag = AtmosphericDrag;
        let pos = Vector3::new(R_EARTH_EQ + 400_000.0, 0.0, 0.0);
        let vel = Vector3::new(0.0, 7_500.0, 0.0);
        let acc = drag.acceleration(
            &pos,
            &vel,
            Density::new(1e-12),
            Area::new(10.0),
            Kilograms::new(1_000.0),
        );
        assert!(acc.raw().norm() > 0.0);
        assert!(acc.raw().dot(&vel) < 0.0);
    }

    #[test]
    fn test_zero_drag_below_surface() {
        let drag = AtmosphericDrag;
        let pos = Vector3::new(R_EARTH_EQ - 1000.0, 0.0, 0.0);
        let vel = Vector3::new(0.0, 100.0, 0.0);
        let acc = drag.acceleration(
            &pos,
            &vel,
            Density::new(1.225),
            Area::new(10.0),
            Kilograms::new(1_000.0),
        );
        assert_eq!(acc.raw().norm(), 0.0);
    }
}
