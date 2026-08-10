//! ECS World built on [`hecs`].
//!
//! The [`World`] wraps a [`hecs::World`] for entity storage and holds shared
//! simulation context ([`SimulationConfig`], celestial ephemeris state, the
//! current simulation epoch). Entities are composed of individual components
//! — [`Kinematics`], [`RigidBody`], [`SpacecraftConfig`] — rather than a
//! monolithic bundle. This lets systems query only the components they need
//! and allows future entity types (stations, asteroids, debris) to reuse
//! shared components without fitting into a spacecraft-shaped bundle.
//!
//! [`hecs::Entity`] is a lightweight `Copy` handle. It wraps a `u64` internally
//! and is safe to pass across the FFI boundary via [`Entity::to_bits`] /
//! [`Entity::from_bits`].

pub use hecs::Entity;

use crate::components::celestial::CelestialRegistry;
use crate::components::rigid_body::SimulationConfig;
use crate::ephemeris::kernel::SolarSystemState;

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
    /// Celestial ephemeris state (positions and velocities of all bodies).
    ///
    /// This is kept for backward compatibility and is rebuilt from the
    /// registry when `build_celestial_state()` is called. Direct mutation
    /// should be replaced by `celestial_registry` operations.
    pub celestial: SolarSystemState,
    /// Registry of celestial bodies (kinematic + dynamic).
    pub celestial_registry: CelestialRegistry,
    /// Current simulation epoch.
    pub epoch: hifitime::Epoch,
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
            celestial: SolarSystemState::default(),
            celestial_registry: CelestialRegistry::new(),
            epoch: hifitime::Epoch::from_tai_duration(hifitime::Duration::ZERO),
        }
    }

    /// Create an empty world with the given simulation context.
    pub fn with_config(sim_config: SimulationConfig, celestial: SolarSystemState) -> Self {
        Self {
            ecs: hecs::World::new(),
            sim_config,
            celestial,
            celestial_registry: CelestialRegistry::new(),
            epoch: hifitime::Epoch::from_tai_duration(hifitime::Duration::ZERO),
        }
    }

    /// Create an empty world with the given simulation context and epoch.
    pub fn with_config_and_epoch(
        sim_config: SimulationConfig,
        celestial: SolarSystemState,
        epoch: hifitime::Epoch,
    ) -> Self {
        Self {
            ecs: hecs::World::new(),
            sim_config,
            celestial,
            celestial_registry: CelestialRegistry::new(),
            epoch,
        }
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

    /// Add a celestial body to the registry.
    pub fn add_celestial_body(&mut self, body: crate::components::celestial::CelestialBody) {
        self.celestial_registry.add(body);
    }

    /// Rebuild the `celestial` (`SolarSystemState`) from the celestial
    /// registry. This should be called before each `step_world` if the
    /// registry has been modified (kinematic bodies updated from ephemeris,
    /// or dynamic bodies integrated).
    pub fn build_celestial_state(&mut self) {
        self.celestial =
            crate::components::celestial::celestial_state_from_registry(&self.celestial_registry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::kinematics::Kinematics;
    use crate::components::rigid_body::{RigidBody, SpacecraftConfig};
    use apogee_common::units::{Area, Kilograms};
    use approx::assert_relative_eq;
    use nalgebra::{Matrix3, Quaternion, Vector3};

    fn make_spacecraft(id: f64) -> (Kinematics, RigidBody, SpacecraftConfig) {
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
            SpacecraftConfig {
                ballistic_coefficient: 0.01,
                srp_area: Area::new(10.0),
                reflectivity: 1.2,
                reference_mass_kg: 1000.0,
            },
        )
    }

    #[test]
    fn spawn_and_get_component() {
        let mut world = World::new();
        let (kin, rb, cfg) = make_spacecraft(1.0);
        let e = world.spawn((kin, rb, cfg));
        assert_eq!(world.len(), 1);
        let kin = world.get_component::<Kinematics>(e).unwrap();
        assert_relative_eq!(kin.position.x, 1.0);
    }

    #[test]
    fn spawn_heterogeneous_entities() {
        // A spacecraft with full component set and a bare-body with just kinematics.
        let mut world = World::new();
        let (kin, rb, cfg) = make_spacecraft(1.0);
        let _sc = world.spawn((kin, rb, cfg));
        let bare = world.spawn((Kinematics::default(),));
        assert_eq!(world.len(), 2);
        // The bare entity has Kinematics but no RigidBody.
        assert!(world.get_component::<Kinematics>(bare).is_some());
        assert!(world.get_component::<RigidBody>(bare).is_none());
    }

    #[test]
    fn despawn() {
        let mut world = World::new();
        let (kin, rb, cfg) = make_spacecraft(1.0);
        let e = world.spawn((kin, rb, cfg));
        assert!(world.despawn(e));
        assert_eq!(world.len(), 0);
        assert!(world.get_component::<Kinematics>(e).is_none());
    }

    #[test]
    fn despawn_stale_handle() {
        let mut world = World::new();
        let (kin, rb, cfg) = make_spacecraft(1.0);
        let e0 = world.spawn((kin, rb, cfg));
        world.despawn(e0);
        let (kin, rb, cfg) = make_spacecraft(2.0);
        let e1 = world.spawn((kin, rb, cfg));
        assert!(world.get_component::<Kinematics>(e0).is_none());
        assert!(world.get_component::<Kinematics>(e1).is_some());
    }

    #[test]
    fn get_component_mut_modifies() {
        let mut world = World::new();
        let (kin, rb, cfg) = make_spacecraft(1.0);
        let e = world.spawn((kin, rb, cfg));
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
        let (kin, rb, cfg) = make_spacecraft(1.0);
        let e0 = world.spawn((kin, rb.clone(), cfg));
        let (kin, rb, cfg) = make_spacecraft(2.0);
        let e1 = world.spawn((kin, rb, cfg));

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
        let (kin, rb, cfg) = make_spacecraft(1.0);
        let _e0 = world.spawn((kin, rb.clone(), cfg));
        let (kin, rb, cfg) = make_spacecraft(2.0);
        let _e1 = world.spawn((kin, rb, cfg));

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
        let celestial = SolarSystemState {
            states: vec![crate::ephemeris::kernel::BodyState {
                naif_id: 399,
                position: Vector3::zeros(),
                velocity: Vector3::zeros(),
            }],
        };
        let world = World::with_config(sim_config, celestial.clone());
        assert_relative_eq!(world.sim_config.f107, 200.0);
        assert_eq!(world.celestial.states.len(), 1);
        assert_eq!(world.len(), 0);
    }

    #[test]
    fn with_config_and_epoch() {
        let epoch = hifitime::Epoch::from_gregorian_utc(2026, 3, 20, 12, 0, 0, 0);
        let world = World::with_config_and_epoch(
            SimulationConfig::default(),
            SolarSystemState::default(),
            epoch,
        );
        assert_eq!(world.epoch, epoch);
    }

    #[test]
    fn clear_removes_all() {
        let mut world = World::new();
        let (kin, rb, cfg) = make_spacecraft(1.0);
        let e0 = world.spawn((kin, rb.clone(), cfg));
        let (kin, rb, cfg) = make_spacecraft(2.0);
        let e1 = world.spawn((kin, rb, cfg));
        world.clear();
        assert_eq!(world.len(), 0);
        assert!(world.get_component::<Kinematics>(e0).is_none());
        assert!(world.get_component::<Kinematics>(e1).is_none());
    }

    #[test]
    fn despawn_returns_false_for_invalid() {
        let mut world = World::new();
        let (kin, rb, cfg) = make_spacecraft(1.0);
        let _ = world.spawn((kin, rb, cfg));
        let bad = Entity::from_bits(u64::MAX).expect("non-zero bits should produce an Entity");
        assert!(!world.despawn(bad));
    }
}
