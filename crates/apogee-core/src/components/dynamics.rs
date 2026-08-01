//! Dynamic properties: mass, inertia, and spacecraft aerodynamic config.

use nalgebra::Matrix3;

/// Mass and inertia properties of a rigid body.
#[derive(Debug, Clone)]
pub struct Dynamics {
    /// Total mass (kg).
    pub mass: f64,
    /// Inertia tensor in body frame (kg m^2).
    pub inertia: Matrix3<f64>,
    /// Center of mass offset from reference point (m).
    pub cg_offset: nalgebra::Vector3<f64>,
}

impl Default for Dynamics {
    fn default() -> Self {
        Self {
            mass: 1.0,
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
    /// Cross-sectional area exposed to solar radiation (m²).
    pub srp_area_m2: f64,
    /// Reflectivity coefficient (0.0 = fully absorbing, 1.0 = perfectly reflecting).
    pub reflectivity: f64,
    /// Reference mass used to scale drag area if only Cd*A/m is known (kg).
    pub reference_mass_kg: f64,
}

impl Default for SpacecraftConfig {
    fn default() -> Self {
        Self {
            ballistic_coefficient: 0.01,
            srp_area_m2: 10.0,
            reflectivity: 1.2,
            reference_mass_kg: 1.0,
        }
    }
}

impl SpacecraftConfig {
    /// Effective drag area Cd*A (m²) for the current spacecraft mass.
    pub fn drag_area_m2(&self, mass: f64) -> f64 {
        self.ballistic_coefficient * mass
    }
}
