//! ECS World built on [`hecs`].
//!
//! The [`World`] wraps a [`hecs::World`] for entity and component storage and
//! holds shared simulation context ([`SimulationConfig`], the current
//! simulation epoch). Entities are composed of individual components —
//! [`Kinematics`], [`RigidBody`], [`DragSurfaces`], [`SrpSurfaces`],
//! [`GravitySource`], [`NaifIdComponent`], [`CelestialKind`] — rather than a
//! monolithic bundle. This lets systems query only the components they need
//! and allows heterogeneous entity types (spacecraft, planets, asteroids,
//! debris) to coexist with different component sets.
//!
//! Celestial bodies (Sun, planets, moons, asteroids) are first-class ECS
//! entities, not a separate data structure. The force aggregator queries
//! the ECS world for `(&GravitySource, &Kinematics)` to compute point-mass
//! gravity, eliminating the old `SolarSystemState` / `CelestialRegistry`.
//!
//! [`hecs::Entity`] is a lightweight `Copy` handle. It wraps a `u64` internally
//! and is safe to pass across the FFI boundary via [`Entity::to_bits`] /
//! [`Entity::from_bits`].

pub use hecs::Entity;

use crate::components::celestial::{
    CelestialBodySpec, CelestialKind, GravitySource, NaifIdComponent,
};
use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::SimulationConfig;
use crate::ephemeris::EphemerisService;
use crate::frames::ClockService;
use apogee_common::NaifId;

/// The simulation world.
///
/// Owns a [`hecs::World`] (exposed as [`World::ecs`]) for entity/component
/// storage plus shared simulation context. System functions take `&mut World`
/// and query the inner [`hecs::World`] for the components they need.
pub struct World {
    /// Archetypal ECS storage for all entities and their components.
    pub ecs: hecs::World,
    /// Space-weather / environment configuration for force models.
    pub sim_config: SimulationConfig,
    /// Current simulation epoch.
    pub epoch: hifitime::Epoch,
    /// Optional clock service for time scale conversions (UT1, EOP).
    pub clock: Option<ClockService>,
    /// Optional ephemeris service for kinematic body updates.
    pub ephemeris: Option<EphemerisService>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    /// Create an empty world with default simulation context.
    pub fn new() -> Self {
        Self {
            ecs: hecs::World::new(),
            sim_config: SimulationConfig::default(),
            epoch: hifitime::Epoch::from_tai_duration(hifitime::Duration::ZERO),
            clock: None,
            ephemeris: None,
        }
    }

    /// Create an empty world with the given simulation configuration.
    pub fn with_config(sim_config: SimulationConfig) -> Self {
        Self {
            ecs: hecs::World::new(),
            sim_config,
            epoch: hifitime::Epoch::from_tai_duration(hifitime::Duration::ZERO),
            clock: None,
            ephemeris: None,
        }
    }

    /// Create an empty world with the given simulation configuration and epoch.
    pub fn with_config_and_epoch(sim_config: SimulationConfig, epoch: hifitime::Epoch) -> Self {
        Self {
            ecs: hecs::World::new(),
            sim_config,
            epoch,
            clock: None,
            ephemeris: None,
        }
    }

    /// Attach a clock service for time scale conversions (UT1, EOP).
    pub fn with_clock(mut self, clock: ClockService) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Attach an ephemeris service for kinematic body updates.
    pub fn with_ephemeris(mut self, ephemeris: EphemerisService) -> Self {
        self.ephemeris = Some(ephemeris);
        self
    }

    /// Spawn a celestial body and return `&mut self` for chaining.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut world = World::new()
    ///     .with_epoch(epoch)
    ///     .with_ephemeris(eph);
    /// world.add_body(CelestialBodySpec::kinematic(399, pos, vel))
    ///     .add_body(CelestialBodySpec::kinematic(301, moon_pos, moon_vel));
    /// ```
    pub fn add_body(&mut self, spec: CelestialBodySpec) -> &mut Self {
        self.add_celestial_body(spec);
        self
    }

    /// Spawn a spacecraft with kinematics and rigid body, return `&mut self`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// world.add_spacecraft(kinematics, rigid_body)
    ///     .add_spacecraft(kin2, rb2);
    /// ```
    pub fn add_spacecraft(
        &mut self,
        kinematics: Kinematics,
        rigid_body: crate::components::rigid_body::RigidBody,
    ) -> &mut Self {
        self.spawn((kinematics, rigid_body));
        self
    }

    // ------------------------------------------------------------------
    // Entity API
    // ------------------------------------------------------------------

    /// Spawn an entity with any combination of components.
    ///
    /// Accepts any [`hecs::DynamicBundle`] — typically a tuple of components.
    /// This is intentionally generic so the world can hold heterogeneous entity
    /// types (spacecraft, asteroids, stations, debris) with different
    /// component sets.
    ///
    /// ```
    /// # use apogee_core::world::World;
    /// # use apogee_core::components::kinematics::Kinematics;
    /// let mut world = World::new();
    /// let entity = world.spawn((Kinematics::default(),));
    /// ```
    pub fn spawn(&mut self, components: impl hecs::DynamicBundle) -> Entity {
        self.ecs.spawn(components)
    }

    /// Despawn an entity. Returns `true` if the handle was valid.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        self.ecs.despawn(entity).is_ok()
    }

    /// Get an immutable reference to a single component, or `None`.
    ///
    /// Returns a [`hecs::Ref`] guard that derefs to `&T`.
    pub fn get_component<T: 'static + Send + Sync>(
        &self,
        entity: Entity,
    ) -> Option<hecs::Ref<'_, T>> {
        self.ecs.get::<&T>(entity).ok()
    }

    /// Get a mutable reference to a single component, or `None`.
    ///
    /// Returns a [`hecs::RefMut`] guard that derefs to `&mut T`.
    pub fn get_component_mut<T: 'static + Send + Sync>(
        &mut self,
        entity: Entity,
    ) -> Option<hecs::RefMut<'_, T>> {
        self.ecs.get::<&mut T>(entity).ok()
    }

    /// Number of live entities.
    pub fn len(&self) -> usize {
        self.ecs.len() as usize
    }

    /// Is the world empty?
    pub fn is_empty(&self) -> bool {
        self.ecs.len() == 0
    }

    /// Iterate over all live entity handles.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.ecs.iter().map(|r| r.entity())
    }

    /// Query all entities that have the given component tuple, returning
    /// immutable references.
    pub fn query<Q: hecs::Query>(&self) -> hecs::QueryBorrow<'_, Q> {
        self.ecs.query::<Q>()
    }

    /// Query all entities that have the given component tuple, returning
    /// mutable references.
    pub fn query_mut<Q: hecs::Query>(&mut self) -> hecs::QueryMut<'_, Q> {
        self.ecs.query_mut::<Q>()
    }

    /// Remove all entities.
    pub fn clear(&mut self) {
        self.ecs.clear();
    }

    // ------------------------------------------------------------------
    // Celestial body API
    // ------------------------------------------------------------------

    /// Spawn a celestial body as an ECS entity.
    ///
    /// The body is spawned with `Kinematics + NaifIdComponent + CelestialKind +
    /// GravitySource` (if GM is non-zero). Dynamic bodies also get a
    /// `CelestialMass` component so the integrator can compute acceleration.
    ///
    /// Returns the entity handle.
    pub fn add_celestial_body(&mut self, spec: CelestialBodySpec) -> Entity {
        let kinematics = Kinematics {
            position: spec.position,
            velocity: spec.velocity,
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };
        let naif_id = NaifIdComponent::new(spec.naif_id);
        let kind = spec.kind;
        let gm = spec.resolved_gm();
        let mass = spec.resolved_mass();

        if gm.value > 0.0 {
            let mut gravity = GravitySource::from_gm(gm);
            if let Some(ref sh) = spec.spherical_harmonics {
                gravity = gravity.with_spherical_harmonics(sh.clone());
            }
            if kind.is_dynamic() {
                let celestial_mass = crate::components::celestial::CelestialMass::new(mass);
                self.spawn((kinematics, naif_id, kind, gravity, celestial_mass))
            } else {
                self.spawn((kinematics, naif_id, kind, gravity))
            }
        } else {
            // No gravity contribution — still spawn with identity components
            // so the body is visible to ephemeris-update queries.
            self.spawn((kinematics, naif_id, kind))
        }
    }

    /// Find a celestial body entity by NAIF ID.
    pub fn find_celestial(&self, naif_id: NaifId) -> Option<Entity> {
        for (entity, id) in self.ecs.query::<&NaifIdComponent>().iter() {
            if id.0 == naif_id {
                return Some(entity);
            }
        }
        None
    }

    /// Update the position and velocity of a kinematic celestial body from
    /// ephemeris data. Does nothing if the body is not found or is not
    /// kinematic.
    pub fn update_kinematic_celestial(
        &mut self,
        naif_id: NaifId,
        position: apogee_common::Position,
        velocity: apogee_common::Velocity,
    ) {
        let entity = match self.find_celestial(naif_id) {
            Some(e) => e,
            None => return,
        };

        // Check that the body is kinematic before updating.
        if let Some(kind) = self.get_component::<CelestialKind>(entity) {
            if !kind.is_kinematic() {
                return;
            }
        }

        if let Some(mut kin) = self.get_component_mut::<Kinematics>(entity) {
            kin.position = position;
            kin.velocity = velocity;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::celestial::{
        CelestialBodySpec, CelestialKind, GravitySource, NaifIdComponent,
    };
    use crate::components::rigid_body::RigidBody;
    use apogee_common::units::Kilograms;
    use approx::assert_relative_eq;
    use nalgebra::{Matrix3, Quaternion, Vector3};

    fn make_spacecraft(id: f64) -> (Kinematics, RigidBody) {
        (
            Kinematics {
                position: Vector3::new(id, 0.0, 0.0),
                velocity: Vector3::zeros(),
                attitude: Quaternion::identity(),
                angular_velocity: Vector3::zeros(),
            },
            RigidBody {
                mass: Kilograms::new(1000.0),
                inertia: Matrix3::identity(),
                cg_offset: Vector3::zeros(),
            },
        )
    }

    #[test]
    fn spawn_and_get_component() {
        let mut world = World::new();
        let (kin, rb) = make_spacecraft(1.0);
        let e = world.spawn((kin, rb));
        assert_eq!(world.len(), 1);
        let kin = world.get_component::<Kinematics>(e).unwrap();
        assert_relative_eq!(kin.position.x, 1.0);
    }

    #[test]
    fn spawn_heterogeneous_entities() {
        // A spacecraft with full component set and a bare-body with just kinematics.
        let mut world = World::new();
        let (kin, rb) = make_spacecraft(1.0);
        let _sc = world.spawn((kin, rb));
        let bare = world.spawn((Kinematics::default(),));
        assert_eq!(world.len(), 2);
        // The bare entity has Kinematics but no RigidBody.
        assert!(world.get_component::<Kinematics>(bare).is_some());
        assert!(world.get_component::<RigidBody>(bare).is_none());
    }

    #[test]
    fn despawn() {
        let mut world = World::new();
        let (kin, rb) = make_spacecraft(1.0);
        let e = world.spawn((kin, rb));
        assert!(world.despawn(e));
        assert_eq!(world.len(), 0);
        assert!(world.get_component::<Kinematics>(e).is_none());
    }

    #[test]
    fn despawn_stale_handle() {
        let mut world = World::new();
        let (kin, rb) = make_spacecraft(1.0);
        let e0 = world.spawn((kin, rb));
        world.despawn(e0);
        let (kin, rb) = make_spacecraft(2.0);
        let e1 = world.spawn((kin, rb));
        assert!(world.get_component::<Kinematics>(e0).is_none());
        assert!(world.get_component::<Kinematics>(e1).is_some());
    }

    #[test]
    fn get_component_mut_modifies() {
        let mut world = World::new();
        let (kin, rb) = make_spacecraft(1.0);
        let e = world.spawn((kin, rb));
        {
            let mut kin = world.get_component_mut::<Kinematics>(e).unwrap();
            kin.position = Vector3::new(99.0, 0.0, 0.0);
        }
        assert_relative_eq!(
            world.get_component::<Kinematics>(e).unwrap().position.x,
            99.0
        );
    }

    #[test]
    fn query_multi_entity() {
        let mut world = World::new();
        let (kin, rb) = make_spacecraft(1.0);
        let e0 = world.spawn((kin, rb.clone()));
        let (kin, rb) = make_spacecraft(2.0);
        let e1 = world.spawn((kin, rb));

        let positions: Vec<_> = world
            .query::<(&Kinematics,)>()
            .iter()
            .map(|(e, (kin,))| (e, kin.position.x))
            .collect();
        assert_eq!(positions.len(), 2);
        let ids: Vec<_> = positions.iter().map(|(e, _)| *e).collect();
        assert!(ids.contains(&e0));
        assert!(ids.contains(&e1));
    }

    #[test]
    fn query_mut_updates_all() {
        let mut world = World::new();
        let (kin, rb) = make_spacecraft(1.0);
        let _e0 = world.spawn((kin, rb.clone()));
        let (kin, rb) = make_spacecraft(2.0);
        let _e1 = world.spawn((kin, rb));

        for (_, (kin,)) in world.query_mut::<(&mut Kinematics,)>() {
            kin.position = Vector3::new(42.0, 0.0, 0.0);
        }

        for (_, (kin,)) in world.query::<(&Kinematics,)>().iter() {
            assert_relative_eq!(kin.position.x, 42.0);
        }
    }

    #[test]
    fn with_config() {
        let sim_config = SimulationConfig {
            f107: 200.0,
            f107a: 180.0,
            ap: 12.0,
        };
        let world = World::with_config(sim_config);
        assert_relative_eq!(world.sim_config.f107, 200.0);
        assert_eq!(world.len(), 0);
    }

    #[test]
    fn with_config_and_epoch() {
        let epoch = hifitime::Epoch::from_gregorian_utc(2026, 3, 20, 12, 0, 0, 0);
        let world = World::with_config_and_epoch(SimulationConfig::default(), epoch);
        assert_eq!(world.epoch, epoch);
    }

    #[test]
    fn clear_removes_all() {
        let mut world = World::new();
        let (kin, rb) = make_spacecraft(1.0);
        let e0 = world.spawn((kin, rb.clone()));
        let (kin, rb) = make_spacecraft(2.0);
        let e1 = world.spawn((kin, rb));
        world.clear();
        assert_eq!(world.len(), 0);
        assert!(world.get_component::<Kinematics>(e0).is_none());
        assert!(world.get_component::<Kinematics>(e1).is_none());
    }

    #[test]
    fn despawn_returns_false_for_invalid() {
        let mut world = World::new();
        let (kin, rb) = make_spacecraft(1.0);
        let _ = world.spawn((kin, rb));
        let bad = Entity::from_bits(u64::MAX).expect("non-zero bits should produce an Entity");
        assert!(!world.despawn(bad));
    }

    // ------------------------------------------------------------------
    // Celestial body ECS entity tests
    // ------------------------------------------------------------------

    #[test]
    fn add_kinematic_celestial_body() {
        let mut world = World::new();
        let entity = world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));
        assert!(world.get_component::<Kinematics>(entity).is_some());
        assert!(world.get_component::<NaifIdComponent>(entity).is_some());
        assert!(world.get_component::<CelestialKind>(entity).is_some());
        let gs = world.get_component::<GravitySource>(entity);
        assert!(gs.is_some());
        assert_relative_eq!(
            gs.unwrap().gm.into_value(),
            apogee_common::constants::GM_EARTH
        );
    }

    #[test]
    fn add_dynamic_celestial_body() {
        let mut world = World::new();
        let entity = world.add_celestial_body(CelestialBodySpec::dynamic_from_mass(
            2_000_001,
            Vector3::new(1e6, 0.0, 0.0),
            Vector3::zeros(),
            Kilograms::new(1e12),
        ));
        assert!(world.get_component::<Kinematics>(entity).is_some());
        let kind = world.get_component::<CelestialKind>(entity).unwrap();
        assert!(kind.is_dynamic());
        let gs = world.get_component::<GravitySource>(entity);
        assert!(gs.is_some());
        assert!(gs.unwrap().gm.into_value() > 0.0);
    }

    #[test]
    fn find_celestial_by_naif_id() {
        let mut world = World::new();
        let e = world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));
        assert_eq!(world.find_celestial(399), Some(e));
        assert!(world.find_celestial(999).is_none());
    }

    #[test]
    fn update_kinematic_celestial() {
        let mut world = World::new();
        world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));
        world.update_kinematic_celestial(
            399,
            Vector3::new(1e9, 0.0, 0.0),
            Vector3::new(1e3, 0.0, 0.0),
        );
        let entity = world.find_celestial(399).unwrap();
        let kin = world.get_component::<Kinematics>(entity).unwrap();
        assert_relative_eq!(kin.position.x, 1e9);
        assert_relative_eq!(kin.velocity.x, 1e3);
    }

    #[test]
    fn update_kinematic_ignores_dynamic() {
        let mut world = World::new();
        world.add_celestial_body(CelestialBodySpec::dynamic_from_mass(
            2_000_001,
            Vector3::zeros(),
            Vector3::zeros(),
            Kilograms::new(1e12),
        ));
        world.update_kinematic_celestial(2_000_001, Vector3::new(1e9, 0.0, 0.0), Vector3::zeros());
        let entity = world.find_celestial(2_000_001).unwrap();
        let kin = world.get_component::<Kinematics>(entity).unwrap();
        assert_relative_eq!(kin.position.x, 0.0);
    }
}
