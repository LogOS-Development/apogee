//! Dynamic properties: mass, inertia, and spacecraft aerodynamic config.

use apogee_common::units::{Area, Kilograms};
use nalgebra::Matrix3;

/// Mass and inertia properties of a rigid body.
#[derive(Debug, Clone)]
pub struct Dynamics {
    /// Total mass.
    pub mass: Kilograms<f64>,
    /// Inertia tensor in body frame (kg m^2).
    pub inertia: Matrix3<f64>,
    /// Center of mass offset from reference point (m).
    pub cg_offset: nalgebra::Vector3<f64>,
}

impl Default for Dynamics {
    fn default() -> Self {
        Self {
            mass: Kilograms::new(1.0),
            inertia: Matrix3::identity(),
            cg_offset: nalgebra::Vector3::zeros(),
        }
    }
}

/// Spacecraft-specific configuration used by force models.
#[derive(Debug, Clone, Copy)]
pub struct SpacecraftConfig {
    /// Ballistic coefficient for atmospheric drag: Cd * A / m (m²/kg).
    pub ballistic_coefficient: f64,
    /// Cross-sectional area exposed to solar radiation.
    pub srp_area: Area<f64>,
    /// Reflectivity coefficient (0.0 = fully absorbing, 1.0 = perfectly reflecting).
    pub reflectivity: f64,
    /// Reference mass used to scale drag area if only Cd*A/m is known (kg).
    pub reference_mass_kg: f64,
}

impl Default for SpacecraftConfig {
    fn default() -> Self {
        Self {
            ballistic_coefficient: 0.01,
            srp_area: Area::new(10.0),
            reflectivity: 1.2,
            reference_mass_kg: 1.0,
        }
    }
}

impl SpacecraftConfig {
    /// Effective drag area Cd*A for the supplied spacecraft mass.
    pub fn drag_area(&self, mass: Kilograms<f64>) -> Area<f64> {
        Area::new(self.ballistic_coefficient * mass.into_value())
    }
}

/// Environment / simulation configuration for force models.
///
/// This is intentionally separate from `SpacecraftConfig` because a single
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
