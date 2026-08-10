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
use apogee_core::components::kinematics::Kinematics;
use apogee_core::components::rigid_body::{RigidBody, SimulationConfig, SpacecraftConfig};
use apogee_core::ephemeris::kernel::{BodyState, SolarSystemState};
use apogee_core::systems::step::step_world;
use apogee_core::world::Entity;
use apogee_core::world::World as CoreWorld;
use godot::classes::Node;
use godot::prelude::*;
use hifitime::Epoch;
use nalgebra::{Matrix3, Quaternion as NaQuaternion, Vector3 as NaVector3};

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
        let world = CoreWorld::with_config(sim_config, SolarSystemState::default());
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

/// Build a `Kinematics` from a Godot Dictionary.
///
/// Required keys: `position` (Vector3), `velocity` (Vector3).
/// Optional: `attitude` (Quaternion, default identity),
///           `angular_velocity` (Vector3, default zero).
fn parse_kinematics(d: &VarDictionary) -> Option<Kinematics> {
    let position = dict_get_vec3(d, "position")?;
    let velocity = dict_get_vec3(d, "velocity")?;
    let attitude = dict_get_vec3(d, "attitude")
        .map(|v| NaQuaternion::new(0.0, v.x as f64, v.y as f64, v.z as f64))
        .unwrap_or_else(NaQuaternion::identity);
    let angular_velocity = dict_get_vec3(d, "angular_velocity")
        .map(|v| NaVector3::new(v.x as f64, v.y as f64, v.z as f64))
        .unwrap_or_else(NaVector3::zeros);
    Some(Kinematics {
        position: NaVector3::new(position.x as f64, position.y as f64, position.z as f64),
        velocity: NaVector3::new(velocity.x as f64, velocity.y as f64, velocity.z as f64),
        attitude,
        angular_velocity,
    })
}

/// Build a `RigidBody` from a Godot Dictionary.
///
/// Required: `mass` (float, kg).
/// Optional: `inertia` (Vector3 diagonal, default identity matrix),
///           `cg_offset` (Vector3, default zero).
fn parse_rigid_body(d: &VarDictionary) -> Option<RigidBody> {
    let mass = dict_get_f64(d, "mass")?;
    let cg_offset = dict_get_vec3(d, "cg_offset")
        .map(|v| NaVector3::new(v.x as f64, v.y as f64, v.z as f64))
        .unwrap_or_else(NaVector3::zeros);
    Some(RigidBody {
        mass: Kilograms::new(mass),
        inertia: Matrix3::identity(),
        cg_offset,
    })
}

/// Build a `SpacecraftConfig` from a Godot Dictionary.
///
/// Optional (defaults shown):
/// - `ballistic_coefficient`: float, default 0.01
/// - `srp_area`: float (m²), default 10.0
/// - `reflectivity`: float, default 1.2
/// - `reference_mass_kg`: float, defaults to `mass` from the rigid_body dict.
fn parse_spacecraft_config(d: &VarDictionary, reference_mass_kg: f64) -> SpacecraftConfig {
    SpacecraftConfig {
        ballistic_coefficient: dict_get_f64(d, "ballistic_coefficient").unwrap_or(0.01),
        srp_area: Area::new(dict_get_f64(d, "srp_area").unwrap_or(10.0)),
        reflectivity: dict_get_f64(d, "reflectivity").unwrap_or(1.2),
        reference_mass_kg: dict_get_f64(d, "reference_mass_kg").unwrap_or(reference_mass_kg),
    }
}

/// Parse a component Dictionary by its `type` key.
///
/// Returns a boxed dynamic bundle that can be spawned into the World.
/// The caller is responsible for providing the correct component set for
/// the entity type they want.
fn parse_component_set(components: &VarDictionary) -> Option<Vec<(&'static str, ComponentSpec)>> {
    // The Dictionary maps component type names to their field dictionaries.
    // e.g. { "kinematics": { "position": ..., "velocity": ... },
    //        "rigid_body": { "mass": ... },
    //        "spacecraft_config": { "ballistic_coefficient": ... } }
    //
    // We collect the parsed components and spawn them as a tuple.
    let mut specs = Vec::new();
    for (key, value) in components.iter_shared() {
        let key_str = key.to::<GString>().to_string();
        let fields = value.to::<VarDictionary>();
        match key_str.as_str() {
            "kinematics" => {
                let kin = parse_kinematics(&fields)?;
                specs.push(("kinematics", ComponentSpec::Kinematics(kin)));
            }
            "rigid_body" => {
                let rb = parse_rigid_body(&fields)?;
                specs.push(("rigid_body", ComponentSpec::RigidBody(rb)));
            }
            "spacecraft_config" => {
                // reference_mass_kg defaults to the rigid_body mass if present
                let ref_mass = specs
                    .iter()
                    .find_map(|(_, s)| match s {
                        ComponentSpec::RigidBody(rb) => Some(rb.mass.into_value()),
                        _ => None,
                    })
                    .unwrap_or(1.0);
                let cfg = parse_spacecraft_config(&fields, ref_mass);
                specs.push(("spacecraft_config", ComponentSpec::SpacecraftConfig(cfg)));
            }
            _ => {
                // Unknown component type — skip for now. Future versions
                // could register custom component parsers.
            }
        }
    }
    if specs.is_empty() {
        return None;
    }
    Some(specs)
}

/// Parsed component ready for spawning.
enum ComponentSpec {
    Kinematics(Kinematics),
    RigidBody(RigidBody),
    SpacecraftConfig(SpacecraftConfig),
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
        let Some(specs) = parse_component_set(&components) else {
            return -1;
        };

        // Spawn by matching on which components were provided. hecs requires
        // owned bundles, so we destructure the parsed specs into the right
        // tuple shape.
        let has_rigid = specs.iter().any(|(n, _)| *n == "rigid_body");
        let has_sc_config = specs.iter().any(|(n, _)| *n == "spacecraft_config");

        let entity = {
            let kin = specs.iter().find_map(|(_, s)| match s {
                ComponentSpec::Kinematics(k) => Some(k.clone()),
                _ => None,
            });
            let Some(kin) = kin else {
                return -1;
            };

            if has_rigid && has_sc_config {
                let rb = specs
                    .iter()
                    .find_map(|(_, s)| match s {
                        ComponentSpec::RigidBody(rb) => Some(rb.clone()),
                        _ => None,
                    })
                    .unwrap();
                let cfg = specs
                    .iter()
                    .find_map(|(_, s)| match s {
                        ComponentSpec::SpacecraftConfig(cfg) => Some(*cfg),
                        _ => None,
                    })
                    .unwrap();
                self.world.spawn((kin, rb, cfg))
            } else if has_rigid {
                let rb = specs
                    .iter()
                    .find_map(|(_, s)| match s {
                        ComponentSpec::RigidBody(rb) => Some(rb.clone()),
                        _ => None,
                    })
                    .unwrap();
                self.world.spawn((kin, rb))
            } else {
                self.world.spawn((kin,))
            }
        };

        entity.to_bits().get() as i64
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
    #[func]
    fn set_celestial_state(&mut self, state: VarDictionary) {
        let Some(entries) = state.get("entries") else {
            return;
        };
        let arr = entries.to::<VarArray>();

        let mut bodies = Vec::with_capacity(arr.len());
        for entry in arr.iter_shared() {
            let d = entry.to::<VarDictionary>();
            let naif_id = dict_get_i32(&d, "naif_id").unwrap_or(0);
            let pos = dict_get_vec3(&d, "position").unwrap_or(Vector3::ZERO);
            let vel = dict_get_vec3(&d, "velocity").unwrap_or(Vector3::ZERO);

            bodies.push(BodyState {
                naif_id,
                position: NaVector3::new(pos.x as f64, pos.y as f64, pos.z as f64),
                velocity: NaVector3::new(vel.x as f64, vel.y as f64, vel.z as f64),
            });
        }

        self.world.celestial = SolarSystemState { states: bodies };
    }
}

#[cfg(test)]
mod tests {
    //! Integration tests for the ApogeeWorld FFI surface.
    //!
    //! These tests exercise the underlying `CoreWorld` API (which `ApogeeWorld`
    //! wraps) rather than the Godot class directly, since instantiating
    //! `ApogeeWorld` requires the Godot engine runtime. The tests verify the
    //! full path: spawn → step 100x → verify position changes.

    use super::*;
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};

    #[test]
    fn test_spawn_step_query_cycle() {
        let mut world = CoreWorld::with_config(
            SimulationConfig::default(),
            SolarSystemState {
                states: vec![BodyState {
                    naif_id: 399,
                    position: NaVector3::zeros(),
                    velocity: NaVector3::zeros(),
                }],
            },
        );
        // Set epoch to day 80, midnight UTC (matches old day_of_year=80, seconds_utc=0).
        world.epoch = Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0);

        // Spawn an entity in a circular LEO orbit — full spacecraft component set.
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

        // Step 100 times at 60s per step.
        for _ in 0..100 {
            step_world(&mut world, Seconds::new(60.0));
        }

        let kin = world.get_component::<Kinematics>(entity).unwrap();
        let pos1 = (*kin).clone().position;

        // Position must have changed.
        let displacement = (pos1 - pos0).norm();
        assert!(
            displacement > 1_000.0,
            "entity did not move: displacement = {displacement} m"
        );

        // Entity should still be in LEO.
        let altitude = pos1.norm() - R_EARTH_EQ;
        assert!(
            altitude > 350_000.0 && altitude < 500_000.0,
            "altitude out of LEO range: {altitude:.0} m"
        );
    }

    #[test]
    fn test_spawn_kinematics_only() {
        // An entity with only Kinematics (no RigidBody or SpacecraftConfig)
        // should still be spawnable — the generic spawn accepts any component set.
        let mut world = CoreWorld::new();
        let kin = Kinematics {
            position: NaVector3::new(1.0, 2.0, 3.0),
            velocity: NaVector3::zeros(),
            attitude: NaQuaternion::identity(),
            angular_velocity: NaVector3::zeros(),
        };
        let entity = world.spawn((kin,));
        assert_eq!(world.len(), 1);
        assert!(world.get_component::<Kinematics>(entity).is_some());
        assert!(world.get_component::<RigidBody>(entity).is_none());
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

    #[test]
    fn test_entity_count_after_multiple_spawns() {
        let mut world = CoreWorld::new();
        for _ in 0..5 {
            world.spawn((
                Kinematics::default(),
                RigidBody::default(),
                SpacecraftConfig::default(),
            ));
        }
        assert_eq!(world.len(), 5);
    }
}
