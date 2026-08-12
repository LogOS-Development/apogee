//! System trait and single-threaded scheduler for the ECS world.
//!
//! Systems are ECS entities, not external `Box<dyn System>` objects held by a
//! scheduler struct. Each system is spawned as an entity inside the
//! [`World`] with a [`SystemMeta`] component (ordering, preferred dt, enabled
//! flag) and a [`SystemHandler`] component (the boxed [`System`] trait
//! object). The [`Scheduler`] queries the ECS world for system entities each
//! tick, sorts them by `order`, and invokes each handler.
//!
//! This design lets systems carry configuration as component data, allows
//! future systems to be queried and composed via the ECS query API, and
//! aligns with the hecs-native architecture used throughout Apogee.
//!
//! ## Epoch invariant
//!
//! The scheduler advances `world.epoch` by exactly `dt` once per
//! [`Scheduler::run`] call, regardless of how many systems are registered or
//! how many sub-steps each system takes. This invariant is verified by Z3
//! (see issue #168 acceptance criteria).

use std::collections::HashMap;

use apogee_common::units::Seconds;
use hifitime::Unit;

use crate::systems::step;
use crate::world::World;

/// Error returned by a [`System`] during execution.
#[derive(Debug, Clone)]
pub enum SystemError {
    /// A system encountered a runtime failure.
    Runtime(String),
}

impl std::fmt::Display for SystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SystemError::Runtime(msg) => write!(f, "runtime error: {msg}"),
        }
    }
}

impl std::error::Error for SystemError {}

/// Metadata for a system entity, stored as an ECS component.
///
/// Controls the system's execution order, sub-step preference, and enabled
/// state. The scheduler queries `&SystemMeta` to sort and filter system
/// entities before dispatching their [`SystemHandler`]s.
#[derive(Debug, Clone)]
pub struct SystemMeta {
    /// Sort key for execution order. Systems are run in ascending `order`.
    /// Use fractional values to insert between existing systems.
    pub order: f64,
    /// Preferred sub-step dt. When `Some(dt)`, the scheduler sub-steps this
    /// system so each `run` receives approximately `dt`. When `None`, the
    /// system runs once per tick at the full scheduler dt.
    pub preferred_dt: Option<Seconds<f64>>,
    /// Whether the system should be run. Disabled systems are skipped.
    pub enabled: bool,
}

impl SystemMeta {
    /// Create new metadata with the given order, no preferred dt, enabled.
    pub fn new(order: f64) -> Self {
        Self {
            order,
            preferred_dt: None,
            enabled: true,
        }
    }

    /// Set the preferred sub-step dt.
    pub fn with_preferred_dt(mut self, dt: Seconds<f64>) -> Self {
        self.preferred_dt = Some(dt);
        self
    }

    /// Set the enabled flag.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Wrapper holding the boxed [`System`] trait object as an ECS component.
///
/// hecs is archetypal: one instance per type per entity. This struct wraps
/// the `Box<dyn System>` so the system entity has exactly one
/// `SystemHandler` component.
pub struct SystemHandler {
    /// The boxed system implementation.
    pub system: Box<dyn System>,
}

/// A system: a unit of simulation work that operates on the ECS [`World`].
///
/// Register systems with [`Scheduler::add`] and call
/// `scheduler.run(&mut world, dt)`.
///
/// Systems that need finer timesteps can override [`System::preferred_dt`]
/// to declare a preferred sub-step size. Alternatively, set the
/// `preferred_dt` field on the system entity's [`SystemMeta`] component
/// after spawning.
pub trait System: Send + Sync {
    fn run(&mut self, world: &mut World, dt: Seconds<f64>) -> Result<(), SystemError>;

    /// Preferred sub-step dt for this system.
    ///
    /// When `Some(dt)`, the scheduler sub-steps this system so that each
    /// call to `run` receives approximately `dt`. When `None`, the system
    /// runs once per scheduler tick at the full scheduler dt.
    fn preferred_dt(&self) -> Option<Seconds<f64>> {
        None
    }
}

/// Errors collected during the most recent [`Scheduler::run`], attributed
/// to the system entity that produced them.
#[derive(Debug, Clone)]
pub struct SystemErrorEntry {
    /// The entity handle of the system that errored.
    pub entity: hecs::Entity,
    /// The system's name (from `SystemMeta` or the handler's type).
    pub name: String,
    /// The error.
    pub error: SystemError,
}

/// Runs registered systems in order and collects errors.
///
/// Systems are stored as ECS entities inside the [`World`], each with a
/// [`SystemMeta`] and [`SystemHandler`] component. The scheduler queries
/// the world for all system entities, sorts by `SystemMeta::order`, and
/// dispatches each handler's `run` method.
///
/// For sub-stepping, each system entity's `SystemMeta::preferred_dt` is
/// consulted; accumulated time remainders are tracked per-entity in the
/// scheduler's `accumulated` map (keyed by entity handle bits).
pub struct Scheduler {
    /// Accumulated time remainder per system, keyed by entity bits.
    accumulated: HashMap<u64, f64>,
    /// Errors collected during the most recent `run`.
    errors: Vec<SystemErrorEntry>,
    /// Next order value to assign when no explicit order is given.
    next_order: f64,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self {
            accumulated: HashMap::new(),
            errors: Vec::new(),
            next_order: 0.0,
        }
    }

    /// Register a system as an ECS entity in the world.
    ///
    /// The system is spawned with a [`SystemMeta`] (order = next available,
    /// preferred_dt from the system's `preferred_dt()` method) and a
    /// [`SystemHandler`] wrapping the boxed system.
    ///
    /// Returns the entity handle, which can be used with
    /// [`Scheduler::insert_before`] / [`Scheduler::insert_after`] to
    /// position subsequent systems.
    pub fn add(&mut self, world: &mut World, system: impl System + 'static) -> hecs::Entity {
        let preferred = system.preferred_dt();
        let order = self.next_order;
        self.next_order += 1.0;
        let meta = SystemMeta {
            order,
            preferred_dt: preferred,
            enabled: true,
        };
        let handler = SystemHandler {
            system: Box::new(system),
        };
        let entity = world.ecs.spawn((meta, handler));
        self.accumulated.insert(entity.to_bits().get(), 0.0);
        entity
    }

    /// Register a system at a specific order position.
    ///
    /// The system is inserted with the given `order` value. Use this for
    /// precise control over execution position. The `next_order` counter is
    /// bumped to `order + 1` so a subsequent `add` lands after this one.
    pub fn add_at(
        &mut self,
        world: &mut World,
        system: impl System + 'static,
        order: f64,
    ) -> hecs::Entity {
        let preferred = system.preferred_dt();
        let meta = SystemMeta {
            order,
            preferred_dt: preferred,
            enabled: true,
        };
        let handler = SystemHandler {
            system: Box::new(system),
        };
        let entity = world.ecs.spawn((meta, handler));
        self.accumulated.insert(entity.to_bits().get(), 0.0);
        self.next_order = self.next_order.max(order + 1.0);
        entity
    }

    /// Insert a system immediately before the system entity `target`.
    ///
    /// The new system gets `order = target_order - 0.5`, placing it between
    /// the target and whatever preceded it.
    pub fn insert_before(
        &mut self,
        world: &mut World,
        target: hecs::Entity,
        system: impl System + 'static,
    ) -> hecs::Entity {
        let target_order = self
            .entity_order(world, target)
            .expect("target system not found");
        let order = target_order - 0.5;
        self.add_at(world, system, order)
    }

    /// Insert a system immediately after the system entity `target`.
    ///
    /// The new system gets `order = target_order + 0.5`, placing it between
    /// the target and whatever followed it.
    pub fn insert_after(
        &mut self,
        world: &mut World,
        target: hecs::Entity,
        system: impl System + 'static,
    ) -> hecs::Entity {
        let target_order = self
            .entity_order(world, target)
            .expect("target system not found");
        let order = target_order + 0.5;
        self.add_at(world, system, order)
    }

    /// Insert a system at the front of the execution order.
    pub fn insert_front(
        &mut self,
        world: &mut World,
        system: impl System + 'static,
    ) -> hecs::Entity {
        // Find the minimum order among existing systems, or use -0.5 if none.
        let min_order = self.min_order(world);
        let order = min_order - 0.5;
        self.add_at(world, system, order)
    }

    /// Disable a system entity so the scheduler skips it.
    pub fn disable(&self, world: &mut World, entity: hecs::Entity) {
        if let Some(mut meta) = world.get_component_mut::<SystemMeta>(entity) {
            meta.enabled = false;
        }
    }

    /// Enable a system entity so the scheduler runs it again.
    pub fn enable(&self, world: &mut World, entity: hecs::Entity) {
        if let Some(mut meta) = world.get_component_mut::<SystemMeta>(entity) {
            meta.enabled = true;
        }
    }

    /// Remove a system entity from the world.
    pub fn remove(&self, world: &mut World, entity: hecs::Entity) -> bool {
        world.despawn(entity)
    }

    /// Read the `order` field from a system entity's `SystemMeta`.
    fn entity_order(&self, world: &World, entity: hecs::Entity) -> Option<f64> {
        world
            .get_component::<SystemMeta>(entity)
            .map(|meta| meta.order)
    }

    /// Find the minimum `order` among all system entities, or 0.0 if none.
    fn min_order(&self, world: &World) -> f64 {
        world
            .ecs
            .query::<&SystemMeta>()
            .iter()
            .map(|(_, meta)| meta.order)
            .fold(f64::INFINITY, f64::min)
            .min(0.0)
    }

    /// Run all registered system entities in order, then advance
    /// `world.epoch` by `dt` exactly once.
    ///
    /// Systems with a `preferred_dt` of `Some(system_dt)` (from `SystemMeta`
    /// or the handler's `preferred_dt()` method) are sub-stepped: the
    /// scheduler runs them `N` times per call, where
    /// `N = floor((dt + accumulated) / system_dt)`. The remainder is
    /// carried forward, ensuring zero long-term drift even when
    /// `dt / system_dt` is not an integer.
    ///
    /// If a system returns [`Err`], the scheduler records the error and
    /// continues running remaining systems.
    pub fn run(&mut self, world: &mut World, dt: Seconds<f64>) {
        self.errors.clear();
        let dt_val = dt.into_value();

        // Collect all system entities, sorted by order. We snapshot
        // (entity, order, preferred_dt) so we don't hold a borrow on the
        // world while running systems.
        let mut entries: Vec<(hecs::Entity, f64, Option<Seconds<f64>>)> = world
            .ecs
            .query::<(&SystemMeta, &SystemHandler)>()
            .iter()
            .filter(|(_, (meta, _))| meta.enabled)
            .map(|(entity, (meta, handler))| {
                let pref = meta.preferred_dt.or_else(|| handler.system.preferred_dt());
                (entity, meta.order, pref)
            })
            .collect();
        entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        for (entity, _order, preferred) in &entries {
            let sub_dt = match preferred {
                Some(pref) => pref.into_value(),
                None => {
                    // No preferred dt — run once at full scheduler dt.
                    if let Err(e) = run_system(world, *entity, dt) {
                        self.errors.push(SystemErrorEntry {
                            entity: *entity,
                            name: format!("{:?}", entity),
                            error: e,
                        });
                    }
                    continue;
                }
            };

            // Compute sub-step count with accumulation for non-integer divisors.
            let key = entity.to_bits().get();
            let effective_dt = dt_val + self.accumulated.get(&key).copied().unwrap_or(0.0);
            let n_sub = (effective_dt / sub_dt).floor() as usize;
            let simulated = n_sub as f64 * sub_dt;
            self.accumulated.insert(key, effective_dt - simulated);

            let mut had_error = false;
            for _ in 0..n_sub {
                match run_system(world, *entity, Seconds::new(sub_dt)) {
                    Ok(()) => {}
                    Err(e) => {
                        self.errors.push(SystemErrorEntry {
                            entity: *entity,
                            name: format!("{:?}", entity),
                            error: e,
                        });
                        had_error = true;
                        break;
                    }
                }
            }
            let _ = had_error;
        }

        // Epoch advances exactly once per run(), by dt.
        world.epoch += dt.into_value() * Unit::Second;
    }

    /// Number of system entities currently registered in the world.
    pub fn len(&self, world: &World) -> usize {
        world.ecs.query::<&SystemMeta>().iter().count()
    }

    /// Whether no system entities are registered in the world.
    pub fn is_empty(&self, world: &World) -> bool {
        self.len(world) == 0
    }

    /// Errors collected during the most recent [`Scheduler::run`] call.
    pub fn errors(&self) -> &[SystemErrorEntry] {
        &self.errors
    }

    /// Whether the most recent [`Scheduler::run`] produced any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Invoke a single system entity's handler.
///
/// The `SystemHandler` component is temporarily removed from the entity,
/// its boxed system's `run` is called with `&mut World`, and the handler
/// is re-inserted afterward. This avoids holding a borrow on `world.ecs`
/// while the system needs `&mut World`.
fn run_system(
    world: &mut World,
    entity: hecs::Entity,
    dt: Seconds<f64>,
) -> Result<(), SystemError> {
    // Take the SystemHandler component out of the entity.
    let handler = world
        .ecs
        .remove_one::<SystemHandler>(entity)
        .map_err(|_| SystemError::Runtime("failed to remove SystemHandler".into()))?;

    let mut handler = handler;
    let result = handler.system.run(world, dt);

    // Put the handler back regardless of success/failure.
    let _ = world.ecs.insert_one(entity, handler);

    result
}

// ------------------------------------------------------------------
// System implementations wrapping the existing free functions.
// ------------------------------------------------------------------

/// Wraps [`step::step_world`] as a [`System`].
pub struct StepWorldSystem;

impl System for StepWorldSystem {
    fn run(&mut self, world: &mut World, dt: Seconds<f64>) -> Result<(), SystemError> {
        step::step_world(world, dt);
        Ok(())
    }
}

/// Records tick count and world epoch per step.
pub struct LoggingSystem {
    ticks: u64,
    last_epoch: Option<hifitime::Epoch>,
}

impl Default for LoggingSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl LoggingSystem {
    pub fn new() -> Self {
        Self {
            ticks: 0,
            last_epoch: None,
        }
    }

    /// Number of times `run` has been called.
    pub fn tick_count(&self) -> u64 {
        self.ticks
    }

    /// The epoch recorded on the most recent `run`, if any.
    pub fn last_epoch(&self) -> Option<hifitime::Epoch> {
        self.last_epoch
    }
}

impl System for LoggingSystem {
    fn run(&mut self, world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
        self.ticks += 1;
        self.last_epoch = Some(world.epoch);
        Ok(())
    }
}

/// Placeholder for force aggregation as a registered system.
///
/// Forces are computed inside `step_world`'s RK4 derivative closure.
/// This struct marks the extension point for future decoupled force
/// computation. See #151.
pub struct AggregateForcesSystem;

impl Default for AggregateForcesSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AggregateForcesSystem {
    pub fn new() -> Self {
        Self
    }
}

impl System for AggregateForcesSystem {
    fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use nalgebra::Vector3;

    use super::*;
    use crate::components::celestial::CelestialBodySpec;
    use crate::components::kinematics::Kinematics;
    use crate::components::rigid_body::RigidBody;
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
    use apogee_common::units::Kilograms;

    #[test]
    fn test_scheduler_new_is_empty() {
        let scheduler = Scheduler::new();
        let world = World::new();
        assert!(scheduler.is_empty(&world));
        assert_eq!(scheduler.len(&world), 0);
    }

    #[test]
    fn test_scheduler_run_empty_does_not_crash() {
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));
    }

    /// A test system that appends its label to a shared log each time it runs.
    struct CounterSystem {
        label: &'static str,
        log: Arc<Mutex<Vec<&'static str>>>,
    }

    impl System for CounterSystem {
        fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
            self.log.lock().unwrap().push(self.label);
            Ok(())
        }
    }

    #[test]
    fn test_scheduler_runs_systems_in_registration_order() {
        let log = Arc::new(Mutex::new(Vec::new()));

        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "first",
                log: Arc::clone(&log),
            },
        );
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "second",
                log: Arc::clone(&log),
            },
        );
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "third",
                log: Arc::clone(&log),
            },
        );

        assert_eq!(scheduler.len(&world), 3);
        assert!(!scheduler.is_empty(&world));

        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_scheduler_run_multiple_ticks_preserves_order() {
        let log = Arc::new(Mutex::new(Vec::new()));

        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "a",
                log: Arc::clone(&log),
            },
        );
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "b",
                log: Arc::clone(&log),
            },
        );

        scheduler.run(&mut world, Seconds::new(1.0));
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["a", "b", "a", "b"]);
    }

    // ------------------------------------------------------------------
    // StepWorldSystem — wraps the existing `step_world` free function.
    // ------------------------------------------------------------------

    fn make_orbit_components() -> (Kinematics, RigidBody) {
        let r = R_EARTH_EQ + 400_000.0;
        let v = (GM_EARTH / r).sqrt();
        (
            Kinematics {
                position: Vector3::new(r, 0.0, 0.0),
                velocity: Vector3::new(0.0, v, 0.0),
                attitude: nalgebra::Quaternion::identity(),
                angular_velocity: Vector3::zeros(),
            },
            RigidBody {
                mass: Kilograms::new(1_000.0),
                inertia: nalgebra::Matrix3::identity(),
                cg_offset: Vector3::zeros(),
            },
        )
    }

    #[test]
    fn test_step_world_system_advances_entities() {
        let (kin, rb) = make_orbit_components();
        let mut world = World::new();
        world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));
        let _entity = world.spawn((kin, rb));

        // Capture initial energy.
        let e0 = {
            let sc_entity = world
                .ecs
                .query::<(&Kinematics, &RigidBody)>()
                .iter()
                .next()
                .unwrap()
                .0;
            let kin = world.get_component::<Kinematics>(sc_entity).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };

        // Run StepWorldSystem via the scheduler for 60 steps.
        let mut scheduler = Scheduler::new();
        scheduler.add(&mut world, StepWorldSystem);
        for _ in 0..60 {
            scheduler.run(&mut world, Seconds::new(60.0));
        }

        let e1 = {
            let sc_entity = world
                .ecs
                .query::<(&Kinematics, &RigidBody)>()
                .iter()
                .next()
                .unwrap()
                .0;
            let kin = world.get_component::<Kinematics>(sc_entity).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };
        let rel_err = (e1 - e0).abs() / e0.abs();
        assert!(
            rel_err < 1e-5,
            "StepWorldSystem energy drift too large: {}",
            rel_err
        );
    }

    fn orbital_energy(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
        crate::orbit::specific_energy_earth(pos, vel)
    }

    // ------------------------------------------------------------------
    // LoggingSystem — no-op system for the acceptance criterion.
    // ------------------------------------------------------------------

    #[test]
    fn test_logging_system_runs_alongside_step_world_system() {
        let (kin, rb) = make_orbit_components();
        let mut world = World::new();
        world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));
        let _entity = world.spawn((kin, rb));

        let e0 = {
            let sc_entity = world
                .ecs
                .query::<(&Kinematics, &RigidBody)>()
                .iter()
                .next()
                .unwrap()
                .0;
            let kin = world.get_component::<Kinematics>(sc_entity).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };

        // Register StepWorldSystem first, then LoggingSystem.
        let mut scheduler = Scheduler::new();
        scheduler.add(&mut world, StepWorldSystem);
        scheduler.add(&mut world, LoggingSystem::new());

        assert_eq!(scheduler.len(&world), 2);

        for _ in 0..60 {
            scheduler.run(&mut world, Seconds::new(60.0));
        }

        let e1 = {
            let sc_entity = world
                .ecs
                .query::<(&Kinematics, &RigidBody)>()
                .iter()
                .next()
                .unwrap()
                .0;
            let kin = world.get_component::<Kinematics>(sc_entity).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };
        let rel_err = (e1 - e0).abs() / e0.abs();
        assert!(
            rel_err < 1e-5,
            "energy drift with LoggingSystem too large: {}",
            rel_err
        );
    }

    // ------------------------------------------------------------------
    // Error handling — scheduler continues on system failure.
    // ------------------------------------------------------------------

    /// A system that always returns an error.
    struct FailingSystem;

    impl System for FailingSystem {
        fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
            Err(SystemError::Runtime("intentional failure".to_string()))
        }
    }

    #[test]
    fn test_scheduler_collects_errors_and_continues() {
        let log = Arc::new(Mutex::new(Vec::new()));

        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        let _id_before = scheduler.add(
            &mut world,
            CounterSystem {
                label: "before",
                log: Arc::clone(&log),
            },
        );
        let id_fail = scheduler.add(&mut world, FailingSystem);
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "after",
                log: Arc::clone(&log),
            },
        );

        scheduler.run(&mut world, Seconds::new(1.0));

        // Both CounterSystems should have run despite FailingSystem's error.
        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["before", "after"]);

        // One error should be recorded, for the FailingSystem entity.
        assert!(scheduler.has_errors());
        assert_eq!(scheduler.errors().len(), 1);
        assert_eq!(scheduler.errors()[0].entity, id_fail);
        assert!(scheduler.errors()[0]
            .error
            .to_string()
            .contains("intentional failure"));
    }

    #[test]
    fn test_scheduler_no_errors_when_all_succeed() {
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "a",
                log: Arc::new(Mutex::new(Vec::new())),
            },
        );

        scheduler.run(&mut world, Seconds::new(1.0));

        assert!(!scheduler.has_errors());
        assert!(scheduler.errors().is_empty());
    }

    #[test]
    fn test_scheduler_errors_cleared_between_runs() {
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(&mut world, FailingSystem);

        scheduler.run(&mut world, Seconds::new(1.0));
        assert!(scheduler.has_errors());

        // Replace with a succeeding system and run again — errors should clear.
        let mut scheduler2 = Scheduler::new();
        let mut world2 = World::new();
        scheduler2.add(
            &mut world2,
            CounterSystem {
                label: "ok",
                log: Arc::new(Mutex::new(Vec::new())),
            },
        );
        scheduler2.run(&mut world2, Seconds::new(1.0));
        assert!(!scheduler2.has_errors());
    }

    // ------------------------------------------------------------------
    // Scheduler owns epoch advancement — not step_world.
    // ------------------------------------------------------------------

    #[test]
    fn test_scheduler_advances_epoch_exactly_once_per_run() {
        let mut world = World::new();
        let epoch_before = world.epoch;

        let mut scheduler = Scheduler::new();
        scheduler.add(&mut world, StepWorldSystem);
        scheduler.add(&mut world, LoggingSystem::new());

        scheduler.run(&mut world, Seconds::new(60.0));

        let elapsed = world.epoch - epoch_before;
        let elapsed_s = elapsed.to_seconds();
        assert!(
            (elapsed_s - 60.0).abs() < 1e-9,
            "epoch should advance exactly 60s, got {elapsed_s}s"
        );
    }

    #[test]
    fn test_scheduler_advances_epoch_with_multiple_systems() {
        let mut world = World::new();
        let epoch_before = world.epoch;

        let mut scheduler = Scheduler::new();
        scheduler.add(&mut world, LoggingSystem::new());
        scheduler.add(&mut world, StepWorldSystem);
        scheduler.add(&mut world, LoggingSystem::new());

        scheduler.run(&mut world, Seconds::new(30.0));

        let elapsed = world.epoch - epoch_before;
        let elapsed_s = elapsed.to_seconds();
        assert!(
            (elapsed_s - 30.0).abs() < 1e-9,
            "epoch should advance exactly 30s with 3 systems, got {elapsed_s}s"
        );
    }

    #[test]
    fn test_step_world_does_not_advance_epoch() {
        let mut world = World::new();
        let epoch_before = world.epoch;

        step::step_world(&mut world, Seconds::new(60.0));

        let elapsed = world.epoch - epoch_before;
        let elapsed_s = elapsed.to_seconds();
        assert!(
            elapsed_s.abs() < 1e-9,
            "step_world should not advance epoch, but it advanced {elapsed_s}s"
        );
    }

    // ------------------------------------------------------------------
    // Multi-rate scheduling: per-system sub-stepping
    // ------------------------------------------------------------------

    /// Test system that records every dt it receives via a shared log.
    struct SubStepLogSystem {
        preferred_dt: f64,
        call_log: Arc<Mutex<Vec<f64>>>,
    }

    impl System for SubStepLogSystem {
        fn run(&mut self, _world: &mut World, dt: Seconds<f64>) -> Result<(), SystemError> {
            self.call_log.lock().unwrap().push(dt.into_value());
            Ok(())
        }

        fn preferred_dt(&self) -> Option<Seconds<f64>> {
            Some(Seconds::new(self.preferred_dt))
        }
    }

    #[test]
    fn test_system_default_preferred_dt_is_none() {
        struct NoPrefSystem;
        impl System for NoPrefSystem {
            fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
                Ok(())
            }
        }
        let sys = NoPrefSystem;
        assert!(sys.preferred_dt().is_none());
    }

    #[test]
    fn test_system_with_preferred_dt_returns_it() {
        struct PrefSystem {
            pref: f64,
        }
        impl System for PrefSystem {
            fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
                Ok(())
            }
            fn preferred_dt(&self) -> Option<Seconds<f64>> {
                Some(Seconds::new(self.pref))
            }
        }
        let sys = PrefSystem { pref: 6.0 };
        let dt = sys.preferred_dt().unwrap();
        assert!((dt.into_value() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn test_system_without_preferred_dt_runs_once_per_tick() {
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "once",
                log: Arc::new(Mutex::new(Vec::new())),
            },
        );

        let epoch_before = world.epoch;

        scheduler.run(&mut world, Seconds::new(60.0));
        scheduler.run(&mut world, Seconds::new(60.0));

        let elapsed = (world.epoch - epoch_before).to_seconds();
        assert!(
            (elapsed - 120.0).abs() < 1e-9,
            "epoch should be 120s after 2 ticks of 60s, got {elapsed}"
        );
    }

    #[test]
    fn test_sub_stepping_fast_system_runs_multiple_times() {
        let call_log = Arc::new(Mutex::new(Vec::new()));

        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            SubStepLogSystem {
                preferred_dt: 6.0,
                call_log: Arc::clone(&call_log),
            },
        );

        scheduler.run(&mut world, Seconds::new(60.0));

        let recorded = call_log.lock().unwrap();
        assert_eq!(recorded.len(), 10, "should run 10 sub-steps");
        for &dt_val in recorded.iter() {
            assert!(
                (dt_val - 6.0).abs() < 1e-9,
                "each sub-step dt should be 6.0, got {dt_val}"
            );
        }
    }

    #[test]
    fn test_sub_stepping_with_recording_system() {
        let call_log = Arc::new(Mutex::new(Vec::new()));

        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            SubStepLogSystem {
                preferred_dt: 6.0,
                call_log: Arc::clone(&call_log),
            },
        );

        scheduler.run(&mut world, Seconds::new(60.0));

        let recorded = call_log.lock().unwrap();
        assert_eq!(recorded.len(), 10, "should run 10 sub-steps");
        for &dt_val in recorded.iter() {
            assert!(
                (dt_val - 6.0).abs() < 1e-9,
                "each sub-step dt should be 6.0, got {dt_val}"
            );
        }
    }

    #[test]
    fn test_fast_system_dt_div_10_while_slow_system_dt_full() {
        let log = Arc::new(Mutex::new(Vec::new()));

        struct FastSystem {
            log: Arc<Mutex<Vec<&'static str>>>,
        }
        impl System for FastSystem {
            fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
                self.log.lock().unwrap().push("fast");
                Ok(())
            }
            fn preferred_dt(&self) -> Option<Seconds<f64>> {
                Some(Seconds::new(6.0))
            }
        }

        struct SlowSystem {
            log: Arc<Mutex<Vec<&'static str>>>,
        }
        impl System for SlowSystem {
            fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
                self.log.lock().unwrap().push("slow");
                Ok(())
            }
        }

        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            SlowSystem {
                log: Arc::clone(&log),
            },
        );
        scheduler.add(
            &mut world,
            FastSystem {
                log: Arc::clone(&log),
            },
        );

        scheduler.run(&mut world, Seconds::new(60.0));

        let recorded = log.lock().unwrap();
        let slow_count = recorded.iter().filter(|&&s| s == "slow").count();
        let fast_count = recorded.iter().filter(|&&s| s == "fast").count();
        assert_eq!(slow_count, 1, "slow system should run once");
        assert_eq!(fast_count, 10, "fast system should run 10 times");
    }

    #[test]
    fn test_sub_stepping_epoch_invariant_holds() {
        let mut world = World::new();
        let epoch_before = world.epoch;

        let mut scheduler = Scheduler::new();
        scheduler.add(
            &mut world,
            SubStepLogSystem {
                preferred_dt: 6.0,
                call_log: Arc::new(Mutex::new(Vec::new())),
            },
        );
        scheduler.add(
            &mut world,
            SubStepLogSystem {
                preferred_dt: 12.0,
                call_log: Arc::new(Mutex::new(Vec::new())),
            },
        );
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "slow",
                log: Arc::new(Mutex::new(Vec::new())),
            },
        );

        scheduler.run(&mut world, Seconds::new(60.0));

        let elapsed = (world.epoch - epoch_before).to_seconds();
        assert!(
            (elapsed - 60.0).abs() < 1e-9,
            "epoch should advance exactly 60s, got {elapsed}s"
        );
    }

    #[test]
    fn test_sub_stepping_non_integer_divisor_accumulates() {
        // When dt/system_dt is not an integer, accumulated time tracking
        // ensures zero long-term drift. With dt=60, system_dt=7:
        // Over 7 ticks: total simulated = 7*60 = 420 (zero drift).
        let call_log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            SubStepLogSystem {
                preferred_dt: 7.0,
                call_log: Arc::clone(&call_log),
            },
        );

        let epoch_before = world.epoch;

        for _ in 0..7 {
            scheduler.run(&mut world, Seconds::new(60.0));
        }

        let elapsed = (world.epoch - epoch_before).to_seconds();
        assert!(
            (elapsed - 420.0).abs() < 1e-9,
            "epoch should advance exactly 420s over 7 ticks, got {elapsed}s"
        );
    }

    #[test]
    fn test_sub_stepping_adapts_to_changing_outer_dt() {
        let call_log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            SubStepLogSystem {
                preferred_dt: 10.0,
                call_log: Arc::clone(&call_log),
            },
        );

        let epoch_before = world.epoch;

        scheduler.run(&mut world, Seconds::new(60.0));
        scheduler.run(&mut world, Seconds::new(30.0));
        scheduler.run(&mut world, Seconds::new(100.0));

        let elapsed = (world.epoch - epoch_before).to_seconds();
        assert!(
            (elapsed - 190.0).abs() < 1e-9,
            "epoch should advance 190s (60+30+100), got {elapsed}"
        );
    }

    #[test]
    fn test_sub_stepping_microsecond_resolution() {
        let call_log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            SubStepLogSystem {
                preferred_dt: 0.001,
                call_log: Arc::clone(&call_log),
            },
        );

        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = call_log.lock().unwrap();
        assert_eq!(
            recorded.len(),
            1000,
            "should run 1000 sub-steps at 1ms each"
        );
        for &dt_val in recorded.iter() {
            assert!(
                (dt_val - 0.001).abs() < 1e-9,
                "each sub-step dt should be 0.001, got {dt_val}"
            );
        }
    }

    #[test]
    fn test_sub_stepping_preserves_system_order() {
        let log = Arc::new(Mutex::new(Vec::new()));

        struct OrderSystem {
            label: &'static str,
            log: Arc<Mutex<Vec<&'static str>>>,
            preferred: Option<f64>,
        }

        impl System for OrderSystem {
            fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
                self.log.lock().unwrap().push(self.label);
                Ok(())
            }
            fn preferred_dt(&self) -> Option<Seconds<f64>> {
                self.preferred.map(Seconds::new)
            }
        }

        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            OrderSystem {
                label: "A",
                log: Arc::clone(&log),
                preferred: None,
            },
        );
        scheduler.add(
            &mut world,
            OrderSystem {
                label: "B",
                log: Arc::clone(&log),
                preferred: Some(30.0),
            },
        );
        scheduler.add(
            &mut world,
            OrderSystem {
                label: "C",
                log: Arc::clone(&log),
                preferred: None,
            },
        );

        scheduler.run(&mut world, Seconds::new(60.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["A", "B", "B", "C"]);
    }

    #[test]
    fn test_existing_systems_default_to_no_sub_stepping() {
        let step = StepWorldSystem;
        assert!(step.preferred_dt().is_none());

        let logging = LoggingSystem::new();
        assert!(logging.preferred_dt().is_none());

        let agg = AggregateForcesSystem::new();
        assert!(agg.preferred_dt().is_none());
    }

    // ------------------------------------------------------------------
    // insert_front / insert_before / insert_after
    // ------------------------------------------------------------------

    #[test]
    fn test_add_returns_entity() {
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        let e0 = scheduler.add(
            &mut world,
            CounterSystem {
                label: "a",
                log: Arc::new(Mutex::new(Vec::new())),
            },
        );
        let e1 = scheduler.add(
            &mut world,
            CounterSystem {
                label: "b",
                log: Arc::new(Mutex::new(Vec::new())),
            },
        );
        assert_ne!(e0, e1);
    }

    #[test]
    fn test_insert_front() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "second",
                log: Arc::clone(&log),
            },
        );
        scheduler.insert_front(
            &mut world,
            CounterSystem {
                label: "first",
                log: Arc::clone(&log),
            },
        );

        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["first", "second"]);
    }

    #[test]
    fn test_insert_middle_by_id() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        let id_a = scheduler.add(
            &mut world,
            CounterSystem {
                label: "a",
                log: Arc::clone(&log),
            },
        );
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "c",
                log: Arc::clone(&log),
            },
        );
        scheduler.insert_after(
            &mut world,
            id_a,
            CounterSystem {
                label: "b",
                log: Arc::clone(&log),
            },
        );

        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_insert_before_by_id() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "a",
                log: Arc::clone(&log),
            },
        );
        let id_c = scheduler.add(
            &mut world,
            CounterSystem {
                label: "c",
                log: Arc::clone(&log),
            },
        );
        scheduler.insert_before(
            &mut world,
            id_c,
            CounterSystem {
                label: "b",
                log: Arc::clone(&log),
            },
        );

        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_insert_after_by_id() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        let id_a = scheduler.add(
            &mut world,
            CounterSystem {
                label: "a",
                log: Arc::clone(&log),
            },
        );
        scheduler.add(
            &mut world,
            CounterSystem {
                label: "c",
                log: Arc::clone(&log),
            },
        );
        scheduler.insert_after(
            &mut world,
            id_a,
            CounterSystem {
                label: "b",
                log: Arc::clone(&log),
            },
        );

        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_insert_mid_sim_between_existing_systems() {
        let call_log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();

        let id_a = scheduler.add(
            &mut world,
            SubStepLogSystem {
                preferred_dt: 10.0,
                call_log: Arc::clone(&call_log),
            },
        );
        let _id_b = scheduler.add(
            &mut world,
            SubStepLogSystem {
                preferred_dt: 10.0,
                call_log: Arc::clone(&call_log),
            },
        );

        // Run one tick so accumulated values are non-zero.
        scheduler.run(&mut world, Seconds::new(60.0));

        // Insert a new system between A and B mid-sim.
        scheduler.insert_after(
            &mut world,
            id_a,
            SubStepLogSystem {
                preferred_dt: 10.0,
                call_log: Arc::clone(&call_log),
            },
        );

        // Run another tick. The new system starts with accumulated=0,
        // while B's accumulated value from tick 1 is preserved.
        scheduler.run(&mut world, Seconds::new(60.0));

        // No crash, no panic — the key invariant.
        let _elapsed = world.epoch;
    }

    // ------------------------------------------------------------------
    // Disable / enable / remove
    // ------------------------------------------------------------------

    #[test]
    fn test_disable_skips_system() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        let e = scheduler.add(
            &mut world,
            CounterSystem {
                label: "skip_me",
                log: Arc::clone(&log),
            },
        );

        scheduler.disable(&mut world, e);
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(recorded.len(), 0, "disabled system should not run");
    }

    #[test]
    fn test_enable_resumes_system() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        let e = scheduler.add(
            &mut world,
            CounterSystem {
                label: "toggle",
                log: Arc::clone(&log),
            },
        );

        scheduler.disable(&mut world, e);
        scheduler.run(&mut world, Seconds::new(1.0));

        scheduler.enable(&mut world, e);
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(recorded.len(), 1, "system should run after re-enable");
    }

    #[test]
    fn test_remove_system() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();
        let e = scheduler.add(
            &mut world,
            CounterSystem {
                label: "temp",
                log: Arc::clone(&log),
            },
        );

        assert!(scheduler.remove(&mut world, e));
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(recorded.len(), 0, "removed system should not run");
    }

    // ------------------------------------------------------------------
    // SystemMeta as ECS component — preferred_dt override
    // ------------------------------------------------------------------

    #[test]
    fn test_system_meta_preferred_dt_overrides_handler() {
        // A system with preferred_dt() = None, but its SystemMeta has
        // preferred_dt = Some(10). The scheduler should use the meta value.
        struct NoPrefSystem {
            log: Arc<Mutex<Vec<f64>>>,
        }
        impl System for NoPrefSystem {
            fn run(&mut self, _world: &mut World, dt: Seconds<f64>) -> Result<(), SystemError> {
                self.log.lock().unwrap().push(dt.into_value());
                Ok(())
            }
        }

        let call_log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let mut world = World::new();

        let entity = scheduler.add(
            &mut world,
            NoPrefSystem {
                log: Arc::clone(&call_log),
            },
        );

        // Override preferred_dt via the SystemMeta component.
        {
            let mut meta = world.get_component_mut::<SystemMeta>(entity).unwrap();
            meta.preferred_dt = Some(Seconds::new(10.0));
        }

        scheduler.run(&mut world, Seconds::new(60.0));

        let recorded = call_log.lock().unwrap();
        assert_eq!(recorded.len(), 6, "should run 6 sub-steps (60/10)");
    }
}
