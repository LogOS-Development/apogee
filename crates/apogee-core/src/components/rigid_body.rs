//! Rigid-body properties: mass and inertia.

use apogee_common::units::Kilograms;
use nalgebra::Matrix3;

/// Mass and inertia properties of a rigid body.
#[derive(Debug, Clone)]
pub struct RigidBody {
    /// Total mass.
    pub mass: Kilograms<f64>,
    /// Inertia tensor in body frame (kg m^2).
    pub inertia: Matrix3<f64>,
    /// Center of mass offset from reference point (m).
    pub cg_offset: nalgebra::Vector3<f64>,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self {
            mass: Kilograms::new(1.0),
            inertia: Matrix3::identity(),
            cg_offset: nalgebra::Vector3::zeros(),
        }
    }
}

/// Environment / simulation configuration for force models.
///
/// This is intentionally separate from per-entity properties because a single
/// federation simulation can contain many spacecraft but only one set of
/// space-weather / epoch inputs at a given time. It can be federated with an
/// external solar-system ephemeris by updating `celestial` and `clock` inputs
/// at the simulation's own cadence.
#[derive(Debug, Clone, Copy)]
pub struct SimulationConfig {
    /// 10.7 cm solar flux (sfu).
    pub f107: f64,
    /// 81-day averaged 10.7 cm solar flux (sfu).
    pub f107a: f64,
    /// Geomagnetic activity index.
    pub ap: f64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            f107: 150.0,
            f107a: 150.0,
            ap: 4.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn dynamics_default() {
        let d = RigidBody::default();
        assert_relative_eq!(d.mass.value, 1.0);
        assert_relative_eq!(d.inertia[(0, 0)], 1.0);
        assert_relative_eq!(d.cg_offset.norm(), 0.0);
    }

    #[test]
    fn simulation_config_default() {
        let s = SimulationConfig::default();
        assert_relative_eq!(s.f107, 150.0);
        assert_relative_eq!(s.f107a, 150.0);
        assert_relative_eq!(s.ap, 4.0);
    }
}
