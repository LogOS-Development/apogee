//! Atmospheric drag force model.

use apogee_common::constants::R_EARTH_EQ;
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
    /// - `drag_area`: Cd * A (m²)
    /// - `mass`: spacecraft mass (kg)
    pub fn acceleration(
        &self,
        spacecraft_position: &Position,
        spacecraft_velocity: &Vector3<f64>,
        density: f64,
        drag_area_m2: f64,
        mass: f64,
    ) -> Vector3<f64> {
        // Approximate Earth rotation velocity at equator; inertial atmosphere is
        // assumed co-rotating for this simple model.
        let omega_earth = Vector3::new(0.0, 0.0, 7.2921159e-5);
        let r = spacecraft_position.norm();
        let altitude_m = r - R_EARTH_EQ;

        // Effective velocity relative to rotating atmosphere.
        let vel_rel = spacecraft_velocity - omega_earth.cross(spacecraft_position);
        let v_rel = vel_rel.norm();
        if v_rel == 0.0 || density <= 0.0 || altitude_m < 0.0 {
            return Vector3::zeros();
        }

        let force_magnitude = 0.5 * density * v_rel * v_rel * drag_area_m2;
        let accel_magnitude = force_magnitude / mass;
        -vel_rel / v_rel * accel_magnitude
    }

    /// Compute drag acceleration using an atmosphere model to obtain density.
    pub fn acceleration_with_model<M: AtmosphereModel>(
        &self,
        spacecraft_position: &Position,
        spacecraft_velocity: &Vector3<f64>,
        model: &M,
        input: &AtmosphereInput,
        drag_area_m2: f64,
        mass: f64,
    ) -> Vector3<f64> {
        let output = model.evaluate(input);
        self.acceleration(
            spacecraft_position,
            spacecraft_velocity,
            output.density,
            drag_area_m2,
            mass,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector3;

    #[test]
    fn test_drag_opposes_velocity() {
        let drag = AtmosphericDrag::default();
        let pos = Vector3::new(R_EARTH_EQ + 400_000.0, 0.0, 0.0);
        let vel = Vector3::new(0.0, 7_500.0, 0.0);
        let acc = drag.acceleration(&pos, &vel, 1e-12, 10.0, 1_000.0);
        assert!(acc.norm() > 0.0);
        assert!(acc.dot(&vel) < 0.0);
    }

    #[test]
    fn test_zero_drag_below_surface() {
        let drag = AtmosphericDrag::default();
        let pos = Vector3::new(R_EARTH_EQ - 1000.0, 0.0, 0.0);
        let vel = Vector3::new(0.0, 100.0, 0.0);
        let acc = drag.acceleration(&pos, &vel, 1.225, 10.0, 1_000.0);
        assert_eq!(acc.norm(), 0.0);
    }
}
