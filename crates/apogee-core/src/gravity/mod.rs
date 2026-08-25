//! Gravity models: point-mass N-body, spherical harmonics, gradient torque.

pub mod gradient_torque;
pub mod point_mass;
pub mod spherical_harmonics;

pub use gradient_torque::*;
pub use point_mass::*;
pub use spherical_harmonics::*;

use apogee_common::units::{AccelerationVector, GravitationalParameter, TorqueVector};

/// A snapshot of all gravity sources in the simulation, collected from the
/// ECS world before a force evaluation.
///
/// Each entry carries the gravitational parameter, position, and optional
/// spherical harmonics model for a massive body. The force aggregator
/// iterates all entries: bodies with SH coefficients get SH acceleration;
/// bodies without get point-mass acceleration.
#[derive(Debug, Clone, Default)]
pub struct GravitySources {
    /// Gravity source entries, each with GM, position, and optional SH.
    pub sources: Vec<GravitySourceEntry>,
}

/// A single gravity source entry in the snapshot.
#[derive(Debug, Clone)]
pub struct GravitySourceEntry {
    /// Gravitational parameter GM (m³/s²).
    pub gm: GravitationalParameter<f64>,
    /// Inertial position of the body (m).
    pub position: apogee_common::Position,
    /// Optional spherical harmonics model for this body.
    pub spherical_harmonics: Option<SphericalHarmonics>,
}

impl GravitySources {
    /// Create an empty gravity source set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a point-mass gravity source (no SH).
    pub fn push(&mut self, gm: GravitationalParameter<f64>, position: apogee_common::Position) {
        self.sources.push(GravitySourceEntry {
            gm,
            position,
            spherical_harmonics: None,
        });
    }

    /// Add a gravity source with optional spherical harmonics.
    pub fn push_with_sh(
        &mut self,
        gm: GravitationalParameter<f64>,
        position: apogee_common::Position,
        sh: Option<SphericalHarmonics>,
    ) {
        self.sources.push(GravitySourceEntry {
            gm,
            position,
            spherical_harmonics: sh,
        });
    }

    /// Number of gravity sources.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Is the source set empty?
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &GravitySourceEntry> {
        self.sources.iter()
    }

    /// Find the position of the body at the given index, if present.
    pub fn position_of(&self, index: usize) -> Option<apogee_common::Position> {
        self.sources.get(index).map(|e| e.position)
    }
}

/// Trait for gravity acceleration computation.
///
/// Implementors compute gravitational acceleration from a set of massive
/// bodies. The `GravitySources` snapshot is built from the ECS world before
/// each force evaluation.
pub trait GravityModel: Send + Sync {
    /// Returns acceleration in inertial frame, tagged as [`AccelerationVector`]
    /// (m/s²) at the public API surface.
    fn acceleration(
        &self,
        position: &apogee_common::Position,
        sources: &GravitySources,
    ) -> AccelerationVector;

    /// Returns gravity gradient torque in body frame as a [`TorqueVector`] (N·m).
    fn gradient_torque(
        &self,
        position: &apogee_common::Position,
        inertia: &nalgebra::Matrix3<f64>,
        sources: &GravitySources,
    ) -> TorqueVector;
}
