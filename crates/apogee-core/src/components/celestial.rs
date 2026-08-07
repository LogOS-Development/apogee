//! Celestial body components and registry.
//!
//! The hybrid model stores celestial bodies in two categories:
//!
//! - **Kinematic** bodies (Sun, planets, moons): positions are injected from
//!   the ephemeris service (SPK kernel) each step. They are not integrated.
//! - **Propagated** bodies (user-spawned debris, asteroids): positions are
//!   integrated each step, just like spacecraft. These bodies also
//!   contribute to the gravity field.
//!
//! Both categories live in a [`CelestialRegistry`] on the [`World`], so that
//! `aggregate_forces` can iterate all massive bodies (kinematic + propagated)
//! when computing point-mass gravity on a spacecraft.

use apogee_common::gravitational_parameter;
use apogee_common::units::Kilograms;
use apogee_common::{NaifId, Position, Velocity};

/// Kind of celestial body: kinematic (ephemeris-driven) or propagated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CelestialKind {
    /// Position is set from an external ephemeris service each step.
    /// The body is never integrated by `step_world`.
    Kinematic,

    /// Position is integrated by `step_world` like a spacecraft.
    /// The body contributes its own GM to the gravity field.
    Propagated,
}

/// A celestial body in the simulation.
#[derive(Debug, Clone)]
pub struct CelestialBody {
    /// NAIF ID of the body. Used for GM lookup and ephemeris matching.
    pub naif_id: NaifId,
    /// Whether this body is kinematic (ephemeris-driven) or propagated.
    pub kind: CelestialKind,
    /// Inertial position (m).
    pub position: Position,
    /// Inertial velocity (m/s).
    pub velocity: Velocity,
    /// Gravitational parameter GM (m^3/s^2). If the NAIF ID is known, this is
    /// looked up from the built-in table; otherwise the caller supplies it.
    pub gm: f64,
    /// Mass (kg). Derived from GM = G * M for propagated bodies; for kinematic
    /// bodies it is informational only (not used in point-mass gravity).
    pub mass: Kilograms<f64>,
}

impl CelestialBody {
    /// Create a kinematic celestial body from an ephemeris state.
    ///
    /// GM is looked up from the built-in NAIF table. If the NAIF ID is not
    /// known, GM defaults to 0 (no gravity contribution).
    pub fn kinematic(naif_id: NaifId, position: Position, velocity: Velocity) -> Self {
        let gm = gravitational_parameter(naif_id).unwrap_or(0.0);
        let mass = Kilograms::new(gm / apogee_common::constants::G);
        Self {
            naif_id,
            kind: CelestialKind::Kinematic,
            position,
            velocity,
            gm,
            mass,
        }
    }

    /// Create a propagated celestial body (e.g. an asteroid or debris cloud)
    /// with an explicit GM and mass.
    pub fn propagated(
        naif_id: NaifId,
        position: Position,
        velocity: Velocity,
        gm: f64,
        mass: Kilograms<f64>,
    ) -> Self {
        Self {
            naif_id,
            kind: CelestialKind::Propagated,
            position,
            velocity,
            gm,
            mass,
        }
    }

    /// Create a propagated celestial body with GM derived from mass.
    pub fn propagated_from_mass(
        naif_id: NaifId,
        position: Position,
        velocity: Velocity,
        mass: Kilograms<f64>,
    ) -> Self {
        let gm = mass.into_value() * apogee_common::constants::G;
        Self {
            naif_id,
            kind: CelestialKind::Propagated,
            position,
            velocity,
            gm,
            mass,
        }
    }

    /// Is this body kinematic (ephemeris-driven)?
    pub fn is_kinematic(&self) -> bool {
        self.kind == CelestialKind::Kinematic
    }

    /// Is this body propagated (integrated each step)?
    pub fn is_propagated(&self) -> bool {
        self.kind == CelestialKind::Propagated
    }
}

/// Registry of all celestial bodies in the simulation world.
///
/// Stores both kinematic bodies (updated from ephemeris) and propagated
/// bodies (integrated each step). The registry is iterated by the force
/// aggregator to compute point-mass gravity on spacecraft.
#[derive(Debug, Clone, Default)]
pub struct CelestialRegistry {
    /// All celestial bodies, in insertion order.
    bodies: Vec<CelestialBody>,
}

impl CelestialRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a celestial body to the registry.
    pub fn add(&mut self, body: CelestialBody) {
        self.bodies.push(body);
    }

    /// Number of celestial bodies.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// Iterate over all celestial bodies (immutable).
    pub fn iter(&self) -> impl Iterator<Item = &CelestialBody> {
        self.bodies.iter()
    }

    /// Iterate over all celestial bodies (mutable).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut CelestialBody> {
        self.bodies.iter_mut()
    }

    /// Iterate over only the kinematic bodies (mutable) — used by the
    /// ephemeris service to update positions each step.
    pub fn kinematic_mut(&mut self) -> impl Iterator<Item = &mut CelestialBody> {
        self.bodies.iter_mut().filter(|b| b.is_kinematic())
    }

    /// Iterate over only the propagated bodies (mutable) — used by
    /// `step_world` to integrate positions.
    pub fn propagated_mut(&mut self) -> impl Iterator<Item = &mut CelestialBody> {
        self.bodies.iter_mut().filter(|b| b.is_propagated())
    }

    /// Iterate over only the propagated bodies (immutable).
    pub fn propagated(&self) -> impl Iterator<Item = &CelestialBody> {
        self.bodies.iter().filter(|b| b.is_propagated())
    }

    /// Iterate over only the kinematic bodies (immutable).
    pub fn kinematic(&self) -> impl Iterator<Item = &CelestialBody> {
        self.bodies.iter().filter(|b| b.is_kinematic())
    }

    /// Find a body by NAIF ID.
    pub fn find(&self, naif_id: NaifId) -> Option<&CelestialBody> {
        self.bodies.iter().find(|b| b.naif_id == naif_id)
    }

    /// Find a body by NAIF ID (mutable).
    pub fn find_mut(&mut self, naif_id: NaifId) -> Option<&mut CelestialBody> {
        self.bodies.iter_mut().find(|b| b.naif_id == naif_id)
    }

    /// Remove a body by NAIF ID. Returns the body if it was found.
    pub fn remove(&mut self, naif_id: NaifId) -> Option<CelestialBody> {
        self.bodies
            .iter()
            .position(|b| b.naif_id == naif_id)
            .map(|i| self.bodies.remove(i))
    }

    /// Update the position and velocity of a kinematic body from ephemeris
    /// data. Does nothing if the body is not found or is not kinematic.
    pub fn update_kinematic(&mut self, naif_id: NaifId, position: Position, velocity: Velocity) {
        if let Some(body) = self.find_mut(naif_id) {
            if body.is_kinematic() {
                body.position = position;
                body.velocity = velocity;
            }
        }
    }

    /// Clear all bodies from the registry.
    pub fn clear(&mut self) {
        self.bodies.clear();
    }
}

/// Build a `SolarSystemState` (the flat `Vec<BodyState>` used by the gravity
/// model) from the registry. All bodies — kinematic and propagated — are
/// included, since both contribute to point-mass gravity.
pub fn celestial_state_from_registry(
    registry: &CelestialRegistry,
) -> crate::ephemeris::kernel::SolarSystemState {
    crate::ephemeris::kernel::SolarSystemState {
        states: registry
            .iter()
            .map(|body| crate::ephemeris::kernel::BodyState {
                naif_id: body.naif_id,
                position: body.position,
                velocity: body.velocity,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use nalgebra::Vector3;

    fn earth() -> CelestialBody {
        CelestialBody::kinematic(399, Vector3::zeros(), Vector3::zeros())
    }

    #[test]
    fn kinematic_body_has_gm_from_naif_id() {
        let body = earth();
        assert_eq!(body.kind, CelestialKind::Kinematic);
        assert_relative_eq!(body.gm, apogee_common::constants::GM_EARTH);
        assert!(body.gm > 0.0);
    }

    #[test]
    fn kinematic_body_unknown_naif_has_zero_gm() {
        let body = CelestialBody::kinematic(12345, Vector3::zeros(), Vector3::zeros());
        assert_relative_eq!(body.gm, 0.0);
    }

    #[test]
    fn propagated_body_from_mass() {
        let mass = Kilograms::new(1e15);
        let body =
            CelestialBody::propagated_from_mass(2000001, Vector3::zeros(), Vector3::zeros(), mass);
        assert_eq!(body.kind, CelestialKind::Propagated);
        assert!(body.is_propagated());
        assert!(body.gm > 0.0);
        assert_relative_eq!(body.gm, 1e15 * apogee_common::constants::G);
    }

    #[test]
    fn registry_add_iter_find() {
        let mut reg = CelestialRegistry::new();
        reg.add(earth());
        reg.add(CelestialBody::propagated_from_mass(
            2000001,
            Vector3::new(1e6, 0.0, 0.0),
            Vector3::zeros(),
            Kilograms::new(1e12),
        ));
        assert_eq!(reg.len(), 2);
        assert!(reg.find(399).is_some());
        assert!(reg.find(2000001).is_some());
        assert!(reg.find(999).is_none());
    }

    #[test]
    fn registry_kinematic_propagated_filters() {
        let mut reg = CelestialRegistry::new();
        reg.add(earth());
        reg.add(CelestialBody::propagated_from_mass(
            2000001,
            Vector3::zeros(),
            Vector3::zeros(),
            Kilograms::new(1e12),
        ));
        assert_eq!(reg.kinematic().count(), 1);
        assert_eq!(reg.propagated().count(), 1);
        assert_eq!(reg.kinematic_mut().count(), 1);
        assert_eq!(reg.propagated_mut().count(), 1);
    }

    #[test]
    fn registry_update_kinematic() {
        let mut reg = CelestialRegistry::new();
        reg.add(earth());
        reg.update_kinematic(
            399,
            Vector3::new(1e9, 0.0, 0.0),
            Vector3::new(1e3, 0.0, 0.0),
        );
        let body = reg.find(399).unwrap();
        assert_relative_eq!(body.position.x, 1e9);
        assert_relative_eq!(body.velocity.x, 1e3);
    }

    #[test]
    fn registry_update_kinematic_ignores_propagated() {
        let mut reg = CelestialRegistry::new();
        reg.add(CelestialBody::propagated_from_mass(
            2000001,
            Vector3::zeros(),
            Vector3::zeros(),
            Kilograms::new(1e12),
        ));
        // Should not update a propagated body.
        reg.update_kinematic(2000001, Vector3::new(1e9, 0.0, 0.0), Vector3::zeros());
        let body = reg.find(2000001).unwrap();
        assert_relative_eq!(body.position.x, 0.0);
    }

    #[test]
    fn registry_remove() {
        let mut reg = CelestialRegistry::new();
        reg.add(earth());
        assert_eq!(reg.len(), 1);
        let removed = reg.remove(399);
        assert!(removed.is_some());
        assert_eq!(reg.len(), 0);
        assert!(reg.remove(399).is_none());
    }

    #[test]
    fn celestial_state_from_registry_includes_all() {
        let mut reg = CelestialRegistry::new();
        reg.add(earth());
        reg.add(CelestialBody::propagated_from_mass(
            2000001,
            Vector3::new(1e6, 0.0, 0.0),
            Vector3::zeros(),
            Kilograms::new(1e12),
        ));
        let state = celestial_state_from_registry(&reg);
        assert_eq!(state.states.len(), 2);
        assert_eq!(state.states[0].naif_id, 399);
        assert_eq!(state.states[1].naif_id, 2000001);
    }
}
