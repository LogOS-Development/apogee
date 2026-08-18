//! Celestial body ECS components.
//!
//! Celestial bodies (Sun, planets, moons, asteroids) are first-class ECS
//! entities, not a separate data structure. Each body is spawned with a
//! combination of components:
//!
//! - [`Kinematics`] — position and velocity (shared with spacecraft entities).
//! - [`GravitySource`] — gravitational parameter GM, used by point-mass gravity.
//! - [`NaifId`] — body identifier for ephemeris lookup and GM resolution.
//! - [`CelestialKind`] — kinematic (ephemeris-driven) or dynamic (integrated).
//!
//! Not every body needs every component. An asteroid has
//! `Kinematics + GravitySource + NaifId + CelestialKind::Dynamic` and nothing
//! else. Earth may carry future components (`Atmosphere`, `MagneticField`,
//! `RotationState`) — the ECS model handles this naturally.
//!
//! ## Design
//!
//! The force aggregator queries `hecs::World` for `(&GravitySource,
//! &Kinematics)` to compute point-mass gravity, eliminating the separate
//! `SolarSystemState` that previously duplicated celestial body data.
//! Kinematic bodies (Sun, planets, moons) have their `Kinematics` updated
//! from the ephemeris service each step; dynamic bodies (asteroids, debris)
//! are integrated by `step_world` like spacecraft.

use apogee_common::gravitational_parameter;
use apogee_common::units::{GravitationalParameter, Kilograms};
use apogee_common::NaifId;

/// Kind of celestial body: kinematic (ephemeris-driven) or dynamic.
///
/// Stored as an ECS component on each celestial body entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CelestialKind {
    /// Position is set from an external ephemeris service each step.
    /// The body is never integrated by `step_world`, but it **does**
    /// contribute its GM to the gravity field acting on all other bodies.
    #[default]
    Kinematic,

    /// Position is integrated by `step_world` like a spacecraft.
    /// The body contributes its own GM to the gravity field.
    Dynamic,
}

impl CelestialKind {
    /// Is this body kinematic (ephemeris-driven)?
    pub fn is_kinematic(&self) -> bool {
        *self == Self::Kinematic
    }

    /// Is this body dynamic (integrated each step)?
    pub fn is_dynamic(&self) -> bool {
        *self == Self::Dynamic
    }
}

/// NAIF body identifier component.
///
/// Used for ephemeris lookup and GM resolution. Stored as a component so
/// systems can query bodies by NAIF ID without a separate registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NaifIdComponent(pub NaifId);

impl NaifIdComponent {
    /// Create a new NAIF ID component.
    pub fn new(id: NaifId) -> Self {
        Self(id)
    }

    /// The underlying NAIF ID value.
    pub fn id(&self) -> NaifId {
        self.0
    }
}

/// Gravitational source component.
///
/// Carries the gravitational parameter GM (m³/s²) for point-mass gravity
/// computation. Attached to any entity that contributes to the gravity field
/// — planets, moons, asteroids, the Sun.
///
/// For bodies with a known NAIF ID, GM is looked up from the built-in table
/// at construction time. For user-spawned bodies (asteroids, debris), the
/// caller supplies GM directly or derives it from mass.
#[derive(Debug, Clone, Copy, Default)]
pub struct GravitySource {
    /// Gravitational parameter GM (m³/s²).
    pub gm: GravitationalParameter<f64>,
}

impl GravitySource {
    /// Create a gravity source from a known GM value.
    pub fn from_gm(gm: f64) -> Self {
        Self {
            gm: GravitationalParameter::new(gm),
        }
    }

    /// Create a gravity source from a mass, deriving GM = G * M.
    pub fn from_mass(mass: Kilograms<f64>) -> Self {
        Self {
            gm: GravitationalParameter::new(mass.into_value() * apogee_common::constants::G),
        }
    }

    /// Create a gravity source by looking up GM from the NAIF ID table.
    /// Returns `None` if the NAIF ID is not in the built-in table.
    pub fn from_naif_id(naif_id: NaifId) -> Option<Self> {
        gravitational_parameter(naif_id).map(Self::from_gm)
    }
}

/// Mass component for celestial bodies.
///
/// Stored separately from [`GravitySource`] because not all gravity sources
/// need a mass (e.g. a kinematic planet's gravity comes from GM, not from
/// mass-based integration). Dynamic bodies (asteroids) carry both so the
/// integrator can compute their acceleration as F/m.
///
/// This is an alias for `Kilograms<f64>` — the component is a thin wrapper
/// so hecs can store it as a distinct component type.
#[derive(Debug, Clone, Copy, Default)]
pub struct CelestialMass(pub Kilograms<f64>);

impl CelestialMass {
    /// Create a new celestial mass component.
    pub fn new(mass: Kilograms<f64>) -> Self {
        Self(mass)
    }

    /// The underlying mass value.
    pub fn mass(&self) -> Kilograms<f64> {
        self.0
    }
}

/// A complete set of components for spawning a celestial body entity.
///
/// This is a convenience builder — the caller constructs it, then spawns the
/// appropriate component tuple into the ECS world. Not all fields are
/// required for every body:
///
/// - `naif_id` and `kind` are always needed.
/// - `gm` is needed if the body contributes to gravity.
/// - `mass` is needed for dynamic bodies (integrated by `step_world`).
/// - `kinematics` (position + velocity) is always needed.
#[derive(Debug, Clone)]
pub struct CelestialBodySpec {
    /// NAIF ID of the body.
    pub naif_id: NaifId,
    /// Kinematic (ephemeris-driven) or dynamic (integrated).
    pub kind: CelestialKind,
    /// Inertial position (m).
    pub position: apogee_common::Position,
    /// Inertial velocity (m/s).
    pub velocity: apogee_common::Velocity,
    /// Gravitational parameter GM (m³/s²). If `None`, looked up from NAIF ID.
    pub gm: Option<GravitationalParameter<f64>>,
    /// Mass (kg). If `None`, derived from GM for dynamic bodies.
    pub mass: Option<Kilograms<f64>>,
}

impl CelestialBodySpec {
    /// Create a kinematic celestial body from an ephemeris state.
    ///
    /// GM is looked up from the built-in NAIF table. If the NAIF ID is not
    /// known, GM defaults to 0 (no gravity contribution).
    pub fn kinematic(
        naif_id: NaifId,
        position: apogee_common::Position,
        velocity: apogee_common::Velocity,
    ) -> Self {
        let gm = gravitational_parameter(naif_id).map(GravitationalParameter::new);
        let mass = gm.map(|g| Kilograms::new(g.into_value() / apogee_common::constants::G));
        Self {
            naif_id,
            kind: CelestialKind::Kinematic,
            position,
            velocity,
            gm,
            mass,
        }
    }

    /// Create a dynamic celestial body with an explicit GM and mass.
    pub fn dynamic(
        naif_id: NaifId,
        position: apogee_common::Position,
        velocity: apogee_common::Velocity,
        gm: f64,
        mass: Kilograms<f64>,
    ) -> Self {
        Self {
            naif_id,
            kind: CelestialKind::Dynamic,
            position,
            velocity,
            gm: Some(GravitationalParameter::new(gm)),
            mass: Some(mass),
        }
    }

    /// Create a dynamic celestial body with GM derived from mass.
    pub fn dynamic_from_mass(
        naif_id: NaifId,
        position: apogee_common::Position,
        velocity: apogee_common::Velocity,
        mass: Kilograms<f64>,
    ) -> Self {
        let gm = mass.into_value() * apogee_common::constants::G;
        Self {
            naif_id,
            kind: CelestialKind::Dynamic,
            position,
            velocity,
            gm: Some(GravitationalParameter::new(gm)),
            mass: Some(mass),
        }
    }

    /// The resolved GM value (0 if not set and NAIF lookup failed).
    pub fn resolved_gm(&self) -> GravitationalParameter<f64> {
        self.gm
            .unwrap_or_else(|| GravitationalParameter::new(gravitational_parameter(self.naif_id).unwrap_or(0.0)))
    }

    /// The resolved mass (derived from GM if not explicitly set).
    pub fn resolved_mass(&self) -> Kilograms<f64> {
        self.mass.unwrap_or_else(|| {
            let gm = self.resolved_gm();
            Kilograms::new(gm.into_value() / apogee_common::constants::G)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use nalgebra::Vector3;

    #[test]
    fn kinematic_spec_has_gm_from_naif_id() {
        let spec = CelestialBodySpec::kinematic(399, Vector3::zeros(), Vector3::zeros());
        assert_eq!(spec.kind, CelestialKind::Kinematic);
        assert_relative_eq!(spec.resolved_gm().into_value(), apogee_common::constants::GM_EARTH);
        assert!(spec.resolved_gm().into_value() > 0.0);
    }

    #[test]
    fn kinematic_spec_unknown_naif_has_zero_gm() {
        let spec = CelestialBodySpec::kinematic(12345, Vector3::zeros(), Vector3::zeros());
        assert_relative_eq!(spec.resolved_gm().into_value(), 0.0);
    }

    #[test]
    fn dynamic_spec_from_mass() {
        let mass = Kilograms::new(1e15);
        let spec =
            CelestialBodySpec::dynamic_from_mass(2000001, Vector3::zeros(), Vector3::zeros(), mass);
        assert_eq!(spec.kind, CelestialKind::Dynamic);
        assert!(spec.resolved_gm().into_value() > 0.0);
        assert_relative_eq!(spec.resolved_gm().into_value(), 1e15 * apogee_common::constants::G);
    }

    #[test]
    fn gravity_source_from_naif_id() {
        let gs = GravitySource::from_naif_id(399).unwrap();
        assert_relative_eq!(gs.gm.into_value(), apogee_common::constants::GM_EARTH);
    }

    #[test]
    fn gravity_source_from_unknown_naif_returns_none() {
        assert!(GravitySource::from_naif_id(12345).is_none());
    }

    #[test]
    fn gravity_source_from_mass() {
        let gs = GravitySource::from_mass(Kilograms::new(1e15));
        assert_relative_eq!(gs.gm.into_value(), 1e15 * apogee_common::constants::G);
    }

    #[test]
    fn celestial_kind_defaults_to_kinematic() {
        assert_eq!(CelestialKind::default(), CelestialKind::Kinematic);
        assert!(CelestialKind::Kinematic.is_kinematic());
        assert!(!CelestialKind::Kinematic.is_dynamic());
        assert!(CelestialKind::Dynamic.is_dynamic());
        assert!(!CelestialKind::Dynamic.is_kinematic());
    }
}
