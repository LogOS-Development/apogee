//! ECS World: generational arena storage for spacecraft entities.
//!
//! The [`World`] owns all entity state via a slotmap-style arena. Each slot
//! stores a [`SpacecraftBundle`](crate::components::SpacecraftBundle) and a
//! generation counter, so that despawned entities produce stale
//! [`Entity`] handles that correctly return `None` from `get`/`get_mut`
//! instead of aliasing a newly-spawned occupant.
//!
//! Simulation-level configuration ([`SimulationConfig`]) and celestial
//! ephemeris state ([`SolarSystemState`]) live on the `World` so that
//! system functions (Phase 2, issue #102) can take a single `&mut World`
//! argument.

mod arena;
mod entity;

pub use arena::Arena;
pub use entity::Entity;

use crate::components::rigid_body::SimulationConfig;
use crate::components::spacecraft::SpacecraftBundle;
use crate::ephemeris::kernel::SolarSystemState;

/// The simulation world.
///
/// Holds a generational arena of spacecraft bundles and shared simulation
/// context. System functions (Phase 2) will take `&mut World` instead of
/// threading individual component references through every call site.
#[derive(Debug)]
pub struct World {
    /// Generational arena of spacecraft entities.
    entities: Arena<SpacecraftBundle>,
    /// Space-weather / environment configuration for force models.
    pub sim_config: SimulationConfig,
    /// Celestial ephemeris state (positions and velocities of all bodies).
    pub celestial: SolarSystemState,
    /// Day of year [1, 365/366], used by the atmosphere model.
    pub day_of_year: u16,
    /// Seconds since UTC midnight [0, 86400).
    pub seconds_utc: f64,
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
            entities: Arena::new(),
            sim_config: SimulationConfig::default(),
            celestial: SolarSystemState::default(),
            day_of_year: 1,
            seconds_utc: 0.0,
        }
    }

    /// Create an empty world with the given simulation context.
    pub fn with_config(sim_config: SimulationConfig, celestial: SolarSystemState) -> Self {
        Self {
            entities: Arena::new(),
            sim_config,
            celestial,
            day_of_year: 1,
            seconds_utc: 0.0,
        }
    }

    // ------------------------------------------------------------------
    // Entity API
    // ------------------------------------------------------------------

    /// Spawn a spacecraft bundle, returning the [`Entity`] handle.
    pub fn spawn(&mut self, bundle: SpacecraftBundle) -> Entity {
        self.entities.insert(bundle)
    }

    /// Despawn an entity. Returns `true` if the handle was valid and the
    /// entity was removed.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        self.entities.remove(entity).is_some()
    }

    /// Get an immutable reference to the bundle, or `None` if the handle is
    /// stale.
    pub fn get(&self, entity: Entity) -> Option<&SpacecraftBundle> {
        self.entities.get(entity)
    }

    /// Get a mutable reference to the bundle, or `None` if the handle is
    /// stale.
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut SpacecraftBundle> {
        self.entities.get_mut(entity)
    }

    /// Iterate over all live entity handles.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.entities()
    }

    /// Iterate over immutable references to all live bundles.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &SpacecraftBundle)> + '_ {
        self.entities.iter()
    }

    /// Iterate over mutable references to all live bundles.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Entity, &mut SpacecraftBundle)> + '_ {
        self.entities.iter_mut()
    }

    /// Number of live entities.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Is the world empty?
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Remove all entities.
    pub fn clear(&mut self) {
        self.entities.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apogee_common::units::{Area, Kilograms};
    use approx::assert_relative_eq;
    use nalgebra::{Matrix3, Quaternion, Vector3};

    fn make_bundle(id: f64) -> SpacecraftBundle {
        SpacecraftBundle {
            kinematics: crate::components::kinematics::Kinematics {
                position: Vector3::new(id, 0.0, 0.0),
                velocity: Vector3::zeros(),
                attitude: Quaternion::identity(),
                angular_velocity: Vector3::zeros(),
            },
            rigid_body: crate::components::rigid_body::RigidBody {
                mass: Kilograms::new(1000.0),
                inertia: Matrix3::identity(),
                cg_offset: Vector3::zeros(),
            },
            config: crate::components::rigid_body::SpacecraftConfig {
                ballistic_coefficient: 0.01,
                srp_area: Area::new(10.0),
                reflectivity: 1.2,
                reference_mass_kg: 1000.0,
            },
        }
    }

    #[test]
    fn spawn_and_get() {
        let mut world = World::new();
        let e = world.spawn(make_bundle(1.0));
        assert_eq!(world.len(), 1);
        let bundle = world.get(e).unwrap();
        assert_relative_eq!(bundle.kinematics.position.x, 1.0);
    }

    #[test]
    fn despawn() {
        let mut world = World::new();
        let e = world.spawn(make_bundle(1.0));
        assert!(world.despawn(e));
        assert_eq!(world.len(), 0);
        assert!(world.get(e).is_none());
    }

    #[test]
    fn despawn_stale_handle() {
        let mut world = World::new();
        let e0 = world.spawn(make_bundle(1.0));
        world.despawn(e0);
        let e1 = world.spawn(make_bundle(2.0));
        // The old handle should not resolve to the new occupant.
        assert!(world.get(e0).is_none());
        assert!(world.get(e1).is_some());
    }

    #[test]
    fn get_mut_modifies_bundle() {
        let mut world = World::new();
        let e = world.spawn(make_bundle(1.0));
        {
            let bundle = world.get_mut(e).unwrap();
            bundle.kinematics.position = Vector3::new(99.0, 0.0, 0.0);
        }
        assert_relative_eq!(world.get(e).unwrap().kinematics.position.x, 99.0);
    }

    #[test]
    fn entities_iterator() {
        let mut world = World::new();
        let e0 = world.spawn(make_bundle(1.0));
        let e1 = world.spawn(make_bundle(2.0));
        let e2 = world.spawn(make_bundle(3.0));
        world.despawn(e1);

        let collected: Vec<_> = world.entities().collect();
        assert_eq!(collected, vec![e0, e2]);
    }

    #[test]
    fn iter_iter_mut() {
        let mut world = World::new();
        let e0 = world.spawn(make_bundle(1.0));
        let e1 = world.spawn(make_bundle(2.0));

        let positions: Vec<_> = world
            .iter()
            .map(|(e, b)| (e, b.kinematics.position.x))
            .collect();
        assert_eq!(positions.len(), 2);

        for (_, b) in world.iter_mut() {
            b.kinematics.position = Vector3::new(42.0, 0.0, 0.0);
        }
        assert_relative_eq!(world.get(e0).unwrap().kinematics.position.x, 42.0);
        assert_relative_eq!(world.get(e1).unwrap().kinematics.position.x, 42.0);
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
    fn clear_removes_all() {
        let mut world = World::new();
        let e0 = world.spawn(make_bundle(1.0));
        let e1 = world.spawn(make_bundle(2.0));
        world.clear();
        assert_eq!(world.len(), 0);
        assert!(world.get(e0).is_none());
        assert!(world.get(e1).is_none());
    }

    #[test]
    fn despawn_returns_false_for_invalid() {
        let mut world = World::new();
        let _ = world.spawn(make_bundle(1.0));
        let bad = Entity::pack(999, 0);
        assert!(!world.despawn(bad));
    }
}
