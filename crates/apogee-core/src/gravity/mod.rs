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
/// This replaces the old `SolarSystemState` as the input to point-mass
/// gravity computation. The force aggregator builds a `GravitySources` by
/// querying the ECS world for `(&GravitySource, &Kinematics)` entities,
/// then passes it to `PointMassGravity::acceleration`.
///
/// Each entry is `(gm, position)` — the gravitational parameter and the
/// inertial position of the massive body.
#[derive(Debug, Clone, Default)]
pub struct GravitySources {
    /// (GM, position) pairs for all massive bodies.
    pub sources: Vec<(GravitationalParameter<f64>, apogee_common::Position)>,
}

impl GravitySources {
    /// Create an empty gravity source set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a gravity source.
    pub fn push(&mut self, gm: GravitationalParameter<f64>, position: apogee_common::Position) {
        self.sources.push((gm, position));
    }

    /// Number of gravity sources.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Is the source set empty?
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Iterate over all (GM, position) pairs.
    pub fn iter(&self) -> impl Iterator<Item = &(GravitationalParameter<f64>, apogee_common::Position)> {
        self.sources.iter()
    }

    /// Find the position of the body with the given NAIF ID, if present.
    ///
    /// This requires the caller to have stored the NAIF ID alongside the
    /// gravity source when building the snapshot. Since `GravitySources` is
    /// a lightweight `(gm, position)` list, the NAIF ID lookup is done at
    /// the ECS query level before building this snapshot. For the Sun's
    /// position (needed by SRP), use [`World::find_celestial`] to locate the
    /// Sun entity and read its `Kinematics`.
    pub fn position_of(&self, index: usize) -> Option<apogee_common::Position> {
        self.sources.get(index).map(|(_, pos)| *pos)
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
