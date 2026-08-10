//! `ApogeeWorld` — Godot GDExtension class wrapping `apogee_core::World`.
//!
//! Exposes the ECS simulation to Godot so scenes can create, step, and query
//! entities at runtime without holding Rust references across frames. The
//! FFI mirrors the generic `World::spawn` — callers specify which components
//! to attach, and the entity is composed from the provided set.
//!
//! Entity IDs are passed as `i64` across the FFI boundary (the `Entity` handle
//! is a 64-bit value accessible via `Entity::to_bits` / `Entity::from_bits`).
//!
//! Note: godot 0.5 uses `real = f32` by default. Positions and velocities are
//! converted from `f64` (internal) to `f32` (Godot) at the boundary.

use apogee_common::units::{Area, Kilograms, Seconds};
use apogee_core::components::celestial::CelestialBodySpec;
use apogee_core::components::kinematics::Kinematics;
use apogee_core::components::rigid_body::{RigidBody, SimulationConfig, SpacecraftConfig};
use apogee_core::systems::step::step_world;
use apogee_core::world::Entity;
use apogee_core::world::World as CoreWorld;
use godot::classes::Node;
use godot::prelude::*;
use hifitime::Epoch;
use nalgebra::{Matrix3, Quaternion as NaQuaternion, Vector3 as NaVector3};

// -----------------------------------------------------------------------
// Pure-Rust component parsing (testable without the Godot runtime)
// -----------------------------------------------------------------------

/// Raw field values for a `Kinematics` component, extracted from a
/// Godot Dictionary by the FFI layer. All fields use plain Rust types so
/// the parsing logic can be unit-tested without the Godot engine.
#[derive(Debug, Clone, Default)]
pub struct KinematicsInput {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
}

/// Raw field values for a `RigidBody` component.
#[derive(Debug, Clone, Default)]
pub struct RigidBodyInput {
    pub mass: Option<f64>,
    pub cg_offset: [f64; 3],
}

/// Raw field values for a `SpacecraftConfig` component.
#[derive(Debug, Clone, Default)]
pub struct SpacecraftConfigInput {
    pub ballistic_coefficient: Option<f64>,
    pub srp_area: Option<f64>,
    pub reflectivity: Option<f64>,
    pub reference_mass_kg: Option<f64>,
}

/// A parsed component set ready for spawning into the ECS World.
///
/// Only the components the caller provided are present; the FFI layer spawns
/// the appropriate tuple shape based on which fields are `Some`.
#[derive(Debug, Clone, Default)]
pub struct ComponentSet {
    pub kinematics: Option<KinematicsInput>,
    pub rigid_body: Option<RigidBodyInput>,
    pub spacecraft_config: Option<SpacecraftConfigInput>,
}

impl ComponentSet {
    /// Build a `Kinematics` from the input, or `None` if required fields
    /// (position, velocity) are missing.
    fn build_kinematics(input: &KinematicsInput) -> Kinematics {
        let [px, py, pz] = input.position;
        let [vx, vy, vz] = input.velocity;
        Kinematics {
            position: NaVector3::new(px, py, pz),
            velocity: NaVector3::new(vx, vy, vz),
            attitude: NaQuaternion::identity(),
            angular_velocity: NaVector3::zeros(),
        }
    }

    /// Build a `RigidBody` from the input. Returns `None` if `mass` is missing.
    fn build_rigid_body(input: &RigidBodyInput) -> Option<RigidBody> {
        let [cx, cy, cz] = input.cg_offset;
        Some(RigidBody {
            mass: Kilograms::new(input.mass?),
            inertia: Matrix3::identity(),
            cg_offset: NaVector3::new(cx, cy, cz),
        })
    }

    /// Build a `SpacecraftConfig` from the input, defaulting unset fields.
    /// `reference_mass_kg` defaults to the rigid_body mass if not specified.
    fn build_spacecraft_config(
        input: &SpacecraftConfigInput,
        fallback_mass: f64,
    ) -> SpacecraftConfig {
        SpacecraftConfig {
            ballistic_coefficient: input.ballistic_coefficient.unwrap_or(0.01),
            srp_area: Area::new(input.srp_area.unwrap_or(10.0)),
            reflectivity: input.reflectivity.unwrap_or(1.2),
            reference_mass_kg: input.reference_mass_kg.unwrap_or(fallback_mass),
        }
    }

    /// Spawn the component set into the world, choosing the right tuple
    /// shape based on which components are present. Returns `None` if
    /// `kinematics` is missing (required for any propagated entity).
    ///
    /// This is a pure-Rust method: it takes `&mut CoreWorld` directly, so
    /// it can be unit-tested without the Godot engine.
    pub fn spawn_into(&self, world: &mut CoreWorld) -> Option<Entity> {
        let kin_input = self.kinematics.as_ref()?;
        let kin = Self::build_kinematics(kin_input);

        match (&self.rigid_body, &self.spacecraft_config) {
            (Some(rb_input), Some(sc_input)) => {
                let rb = Self::build_rigid_body(rb_input)?;
                let fallback_mass = rb.mass.into_value();
                let cfg = Self::build_spacecraft_config(sc_input, fallback_mass);
                Some(world.spawn((kin, rb, cfg)))
            }
            (Some(rb_input), None) => {
                let rb = Self::build_rigid_body(rb_input)?;
                Some(world.spawn((kin, rb)))
            }
            (None, _) => Some(world.spawn((kin,))),
        }
    }
}

// -----------------------------------------------------------------------
// Godot Dictionary ↔ Rust type adapters (thin, untestable in isolation)
// -----------------------------------------------------------------------

/// Extract a Godot `Vector3` from a `VarDictionary` key, or `None`.
fn dict_get_vec3(dict: &VarDictionary, key: &str) -> Option<Vector3> {
    dict.get(key).map(|v| v.to::<Vector3>())
}

/// Extract an `f64` from a `VarDictionary` key, or `None`.
fn dict_get_f64(dict: &VarDictionary, key: &str) -> Option<f64> {
    dict.get(key).map(|v| v.to::<f64>())
}

/// Extract an `i32` from a `VarDictionary` key, or `None`.
fn dict_get_i32(dict: &VarDictionary, key: &str) -> Option<i32> {
    dict.get(key).map(|v| v.to::<i32>())
}

/// Convert a Godot `Vector3` to a `[f64; 3]`.
fn vec3_to_array(v: Vector3) -> [f64; 3] {
    [v.x as f64, v.y as f64, v.z as f64]
}

/// Parse a Godot `VarDictionary` of component type names → field dicts
/// into a `ComponentSet`. Unknown component types are silently skipped
/// (future versions could register custom parsers).
fn parse_component_dict(components: &VarDictionary) -> ComponentSet {
    let mut set = ComponentSet::default();
    for (key, value) in components.iter_shared() {
        let key_str = key.to::<GString>().to_string();
        let fields = value.to::<VarDictionary>();
        match key_str.as_str() {
            "kinematics" => {
                let position = dict_get_vec3(&fields, "position").map(vec3_to_array);
                let velocity = dict_get_vec3(&fields, "velocity").map(vec3_to_array);
                if let (Some(position), Some(velocity)) = (position, velocity) {
                    set.kinematics = Some(KinematicsInput { position, velocity });
                }
            }
            "rigid_body" => {
                set.rigid_body = Some(RigidBodyInput {
                    mass: dict_get_f64(&fields, "mass"),
                    cg_offset: dict_get_vec3(&fields, "cg_offset")
                        .map(vec3_to_array)
                        .unwrap_or([0.0; 3]),
                });
            }
            "spacecraft_config" => {
                set.spacecraft_config = Some(SpacecraftConfigInput {
                    ballistic_coefficient: dict_get_f64(&fields, "ballistic_coefficient"),
                    srp_area: dict_get_f64(&fields, "srp_area"),
                    reflectivity: dict_get_f64(&fields, "reflectivity"),
                    reference_mass_kg: dict_get_f64(&fields, "reference_mass_kg"),
                });
            }
            _ => { /* unknown component — skip */ }
        }
    }
    set
}

// -----------------------------------------------------------------------
// ApogeeWorld Godot class
// -----------------------------------------------------------------------

/// Godot node wrapping the Apogee ECS `World`.
///
/// Create one of these in your scene to manage the simulation. Call
/// `spawn_entity` to add entities (spacecraft, asteroids, debris — anything
/// with a kinematic state), `step` to advance the simulation, and
/// `get_position`/`get_velocity`/`get_attitude` to read state.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct ApogeeWorld {
    base: Base<Node>,

    /// Simulation-level config: solar flux and geomagnetic index.
    #[var]
    f107: f64,
    #[var]
    f107a: f64,
    #[var]
    ap: f64,

    /// Day of year [1, 366].
    #[var]
    day_of_year: i32,

    /// UTC seconds since midnight [0, 86400).
    #[var]
    seconds_utc: f64,

    /// Underlying ECS world.
    world: CoreWorld,
}

#[godot_api]
impl INode for ApogeeWorld {
    fn init(base: Base<Node>) -> Self {
        let sim_config = SimulationConfig::default();
        let world = CoreWorld::with_config(sim_config);
        Self {
            base,
            f107: sim_config.f107,
            f107a: sim_config.f107a,
            ap: sim_config.ap,
            day_of_year: 1,
            seconds_utc: 0.0,
            world,
        }
    }
}

#[godot_api]
impl ApogeeWorld {
    /// Spawn an entity from a Dictionary mapping component type names to
    /// their field dictionaries.
    ///
    /// Component types (keys) and their required/optional fields:
    ///
    /// `kinematics` (required for any propagated body):
    /// - `position`: Vector3 (inertial, meters) — required
    /// - `velocity`: Vector3 (inertial, m/s) — required
    /// - `angular_velocity`: Vector3 (rad/s) — optional, default zero
    ///
    /// `rigid_body` (optional, for bodies with mass):
    /// - `mass`: float (kg) — required
    /// - `cg_offset`: Vector3 (m) — optional, default zero
    ///
    /// `spacecraft_config` (optional, for spacecraft with drag/SRP):
    /// - `ballistic_coefficient`: float — optional, default 0.01
    /// - `srp_area`: float (m²) — optional, default 10.0
    /// - `reflectivity`: float — optional, default 1.2
    ///
    /// Example (GDScript):
    /// ```
    /// var entity = world.spawn_entity({
    ///     "kinematics": { "position": Vector3(6.7e6, 0, 0), "velocity": Vector3(0, 7700, 0) },
    ///     "rigid_body": { "mass": 1000.0 },
    ///     "spacecraft_config": { "ballistic_coefficient": 0.01 }
    /// })
    /// ```
    ///
    /// Returns the entity ID as an i64, or -1 if a required field is missing.
    #[func]
    fn spawn_entity(&mut self, components: VarDictionary) -> i64 {
        let set = parse_component_dict(&components);
        match set.spawn_into(&mut self.world) {
            Some(entity) => entity.to_bits().get() as i64,
            None => -1,
        }
    }

    /// Advance the simulation by `delta_time` seconds.
    #[func]
    fn step(&mut self, delta_time: f64) {
        // Sync Godot-exposed sim config into the world before stepping.
        self.world.sim_config.f107 = self.f107;
        self.world.sim_config.f107a = self.f107a;
        self.world.sim_config.ap = self.ap;

        // Reconstruct the epoch from Godot-exposed day_of_year + seconds_utc
        // so Godot-side changes to those vars are respected. The epoch is
        // built from the start of the current year plus the day/second offset.
        let year = self.world.epoch.year();
        let year_start = Epoch::from_gregorian_utc_at_midnight(year, 1, 1);
        let doy_offset = hifitime::Duration::from_seconds(
            (self.day_of_year.clamp(1, 366) as f64 - 1.0) * 86_400.0 + self.seconds_utc,
        );
        self.world.epoch = year_start + doy_offset;

        step_world(&mut self.world, Seconds::new(delta_time));

        // Read back the advanced clock.
        let doy_f64 = self.world.epoch.day_of_year();
        self.day_of_year = doy_f64 as i32;
        self.seconds_utc = (doy_f64 - doy_f64.floor()) * 86_400.0;
    }

    /// Get the inertial position of the entity as a Godot Vector3.
    #[func]
    fn get_position(&self, entity_id: i64) -> Vector3 {
        let Some(entity) = Entity::from_bits(entity_id as u64) else {
            return Vector3::ZERO;
        };
        match self.world.get_component::<Kinematics>(entity) {
            Some(kin) => Vector3::new(
                kin.position.x as real,
                kin.position.y as real,
                kin.position.z as real,
            ),
            None => Vector3::ZERO,
        }
    }

    /// Get the inertial velocity of the entity as a Godot Vector3.
    #[func]
    fn get_velocity(&self, entity_id: i64) -> Vector3 {
        let Some(entity) = Entity::from_bits(entity_id as u64) else {
            return Vector3::ZERO;
        };
        match self.world.get_component::<Kinematics>(entity) {
            Some(kin) => Vector3::new(
                kin.velocity.x as real,
                kin.velocity.y as real,
                kin.velocity.z as real,
            ),
            None => Vector3::ZERO,
        }
    }

    /// Get the attitude quaternion of the entity as a Godot Quaternion.
    #[func]
    fn get_attitude(&self, entity_id: i64) -> Quaternion {
        let Some(entity) = Entity::from_bits(entity_id as u64) else {
            return Quaternion::IDENTITY;
        };
        match self.world.get_component::<Kinematics>(entity) {
            Some(kin) => Quaternion::new(
                kin.attitude.i as real,
                kin.attitude.j as real,
                kin.attitude.k as real,
                kin.attitude.w as real,
            ),
            None => Quaternion::IDENTITY,
        }
    }

    /// Despawn an entity by ID. Returns true if the entity was found and removed.
    #[func]
    fn despawn(&mut self, entity_id: i64) -> bool {
        let Some(entity) = Entity::from_bits(entity_id as u64) else {
            return false;
        };
        self.world.despawn(entity)
    }

    /// Get the number of live entities.
    #[func]
    fn entity_count(&self) -> i32 {
        self.world.len() as i32
    }

    /// Set the celestial state from a Dictionary.
    ///
    /// The Dictionary should contain an `entries` key holding an array of
    /// Dictionaries, each with:
    /// - `naif_id`: int
    /// - `position`: Vector3 (meters)
    /// - `velocity`: Vector3 (m/s)
    ///
    /// Each entry is spawned as an ECS entity with `Kinematics + NaifId +
    /// GravitySource + CelestialKind::Kinematic` components. The GM is looked
    /// up from the built-in NAIF table; unknown NAIF IDs get GM = 0 (no
    /// gravity contribution).
    #[func]
    fn set_celestial_state(&mut self, state: VarDictionary) {
        let Some(entries) = state.get("entries") else {
            return;
        };
        let arr = entries.to::<VarArray>();

        for entry in arr.iter_shared() {
            let d = entry.to::<VarDictionary>();
            let naif_id = dict_get_i32(&d, "naif_id").unwrap_or(0);
            let pos = dict_get_vec3(&d, "position").unwrap_or(Vector3::ZERO);
            let vel = dict_get_vec3(&d, "velocity").unwrap_or(Vector3::ZERO);

            let spec = CelestialBodySpec::kinematic(
                naif_id,
                NaVector3::new(pos.x as f64, pos.y as f64, pos.z as f64),
                NaVector3::new(vel.x as f64, vel.y as f64, vel.z as f64),
            );
            self.world.add_celestial_body(spec);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the ApogeeWorld FFI surface.
    //!
    //! Two categories:
    //! 1. **ComponentSet::spawn_into** — exercises the pure-Rust parsing and
    //!    spawning logic (component composition, default filling, error
    //!    handling) without the Godot engine.
    //! 2. **CoreWorld integration** — end-to-end spawn → step → verify cycles
    //!    that exercise the underlying ECS API directly.

    use super::*;
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};

    // -- ComponentSet::spawn_into tests (no Godot runtime needed) --

    #[test]
    fn test_spawn_kinematics_only() {
        let mut world = CoreWorld::new();
        let set = ComponentSet {
            kinematics: Some(KinematicsInput {
                position: [1.0, 2.0, 3.0],
                velocity: [0.0, 0.0, 0.0],
            }),
            ..Default::default()
        };
        let entity = set.spawn_into(&mut world).unwrap();
        assert!(world.get_component::<Kinematics>(entity).is_some());
        assert!(world.get_component::<RigidBody>(entity).is_none());
    }

    #[test]
    fn test_spawn_kinematics_and_rigid_body() {
        let mut world = CoreWorld::new();
        let set = ComponentSet {
            kinematics: Some(KinematicsInput {
                position: [7.0e6, 0.0, 0.0],
                velocity: [0.0, 7700.0, 0.0],
            }),
            rigid_body: Some(RigidBodyInput {
                mass: Some(500.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let entity = set.spawn_into(&mut world).unwrap();
        assert!(world.get_component::<Kinematics>(entity).is_some());
        assert!(world.get_component::<RigidBody>(entity).is_some());
        assert!(world.get_component::<SpacecraftConfig>(entity).is_none());
    }

    #[test]
    fn test_spawn_full_spacecraft() {
        let mut world = CoreWorld::new();
        let set = ComponentSet {
            kinematics: Some(KinematicsInput {
                position: [6.7e6, 0.0, 0.0],
                velocity: [0.0, 7700.0, 0.0],
            }),
            rigid_body: Some(RigidBodyInput {
                mass: Some(1000.0),
                ..Default::default()
            }),
            spacecraft_config: Some(SpacecraftConfigInput {
                ballistic_coefficient: Some(0.02),
                srp_area: Some(5.0),
                reflectivity: Some(1.0),
                reference_mass_kg: None, // should default to rigid_body mass
            }),
        };
        let entity = set.spawn_into(&mut world).unwrap();
        assert!(world.get_component::<Kinematics>(entity).is_some());
        assert!(world.get_component::<RigidBody>(entity).is_some());
        let cfg = world.get_component::<SpacecraftConfig>(entity).unwrap();
        assert_eq!(cfg.reference_mass_kg, 1000.0);
    }

    #[test]
    fn test_spawn_without_kinematics_fails() {
        let mut world = CoreWorld::new();
        let set = ComponentSet {
            rigid_body: Some(RigidBodyInput {
                mass: Some(500.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(set.spawn_into(&mut world).is_none());
    }

    #[test]
    fn test_spawn_rigid_body_without_mass_fails() {
        let mut world = CoreWorld::new();
        let set = ComponentSet {
            kinematics: Some(KinematicsInput {
                position: [1.0; 3],
                velocity: [0.0; 3],
            }),
            rigid_body: Some(RigidBodyInput {
                mass: None,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(set.spawn_into(&mut world).is_none());
    }

    #[test]
    fn test_spacecraft_config_defaults() {
        let mut world = CoreWorld::new();
        let set = ComponentSet {
            kinematics: Some(KinematicsInput {
                position: [1.0; 3],
                velocity: [0.0; 3],
            }),
            rigid_body: Some(RigidBodyInput {
                mass: Some(250.0),
                ..Default::default()
            }),
            spacecraft_config: Some(SpacecraftConfigInput::default()),
        };
        let entity = set.spawn_into(&mut world).unwrap();
        let cfg = world.get_component::<SpacecraftConfig>(entity).unwrap();
        assert_eq!(cfg.ballistic_coefficient, 0.01);
        assert_eq!(cfg.srp_area.value, 10.0);
        assert_eq!(cfg.reflectivity, 1.2);
        assert_eq!(cfg.reference_mass_kg, 250.0);
    }

    #[test]
    fn test_entity_count_after_multiple_spawns() {
        let mut world = CoreWorld::new();
        for _ in 0..5 {
            let set = ComponentSet {
                kinematics: Some(KinematicsInput {
                    position: [1.0; 3],
                    velocity: [0.0; 3],
                }),
                ..Default::default()
            };
            set.spawn_into(&mut world).unwrap();
        }
        assert_eq!(world.len(), 5);
    }

    // -- CoreWorld integration tests --

    #[test]
    fn test_spawn_step_query_cycle() {
        let mut world = CoreWorld::with_config(SimulationConfig::default());
        world.epoch = Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0);
        // Spawn Earth as a kinematic celestial body at the origin.
        world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            NaVector3::zeros(),
            NaVector3::zeros(),
        ));

        let r = R_EARTH_EQ + 400_000.0;
        let v = (GM_EARTH / r).sqrt();
        let kinematics = Kinematics {
            position: NaVector3::new(r, 0.0, 0.0),
            velocity: NaVector3::new(0.0, v, 0.0),
            attitude: NaQuaternion::identity(),
            angular_velocity: NaVector3::zeros(),
        };
        let rigid_body = RigidBody {
            mass: Kilograms::new(1_000.0),
            inertia: Matrix3::identity(),
            cg_offset: NaVector3::zeros(),
        };
        let config = SpacecraftConfig {
            ballistic_coefficient: 0.0,
            srp_area: Area::new(0.0),
            reflectivity: 0.0,
            reference_mass_kg: 1_000.0,
        };

        let entity = world.spawn((kinematics, rigid_body, config));
        let pos0 = world.get_component::<Kinematics>(entity).unwrap().position;

        for _ in 0..100 {
            step_world(&mut world, Seconds::new(60.0));
        }

        let kin = world.get_component::<Kinematics>(entity).unwrap();
        let pos1 = (*kin).clone().position;

        let displacement = (pos1 - pos0).norm();
        assert!(
            displacement > 1_000.0,
            "entity did not move: displacement = {displacement} m"
        );

        let altitude = pos1.norm() - R_EARTH_EQ;
        assert!(
            altitude > 350_000.0 && altitude < 500_000.0,
            "altitude out of LEO range: {altitude:.0} m"
        );
    }

    #[test]
    fn test_despawn_removes_entity() {
        let mut world = CoreWorld::new();
        let entity = world.spawn((
            Kinematics::default(),
            RigidBody::default(),
            SpacecraftConfig::default(),
        ));
        assert_eq!(world.len(), 1);
        assert!(world.despawn(entity));
        assert_eq!(world.len(), 0);
        assert!(world.get_component::<Kinematics>(entity).is_none());
    }
}
