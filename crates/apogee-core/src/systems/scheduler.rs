//! System trait and single-threaded scheduler for the ECS world.
//!
//! Provides a [`System`] trait and [`Scheduler`] so simulation steps can be
//! registered and run in order without the caller knowing each system's
//! signature. Systems write their own hecs query loops inside `run`.

use apogee_common::units::Seconds;

use crate::systems::step;
use crate::world::World;

/// Error returned by a [`System`] during execution.
#[derive(Debug, Clone)]
pub enum SystemError {
    /// A system encountered a runtime failure (e.g. numerical issue,
    /// resource limit). The scheduler logs the error and continues
    /// running remaining systems.
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

/// A system: a unit of simulation work that operates on the ECS [`World`].
///
/// Implementations write their own hecs query loops inside `run`, identical
/// to the existing free functions (`step_world`, `aggregate_forces`). The
/// trait decouples system *definition* from system *registration* — callers
/// register systems with a [`Scheduler`] and call `scheduler.run(&mut world, dt)`
/// instead of invoking each system by name.
///
/// The `dt` parameter is per-call: the caller controls the step size and can
/// vary it between ticks for adaptive timestepping. Per-system sub-stepping
/// within a single tick (multi-rate scheduling) is not supported yet.
///
/// Systems return [`Result`] so the scheduler can handle failures gracefully
/// rather than propagating panics. The scheduler collects errors and continues
/// running remaining systems.
///
/// Single-threaded. Multithreaded dispatch and dependency graphs are tracked
/// separately.
pub trait System: Send {
    /// Advance the simulation world by `dt` seconds.
    fn run(&mut self, world: &mut World, dt: Seconds<f64>) -> Result<(), SystemError>;
}

/// A single-threaded system scheduler.
///
/// Systems are registered with [`Scheduler::add`] and run in registration
/// order when [`Scheduler::run`] is called. Registration order = execution
/// order; explicit dependency declarations can come later.
///
/// The scheduler owns error handling: if a system returns [`Err`], the
/// scheduler records it and continues running remaining systems. Collected
/// errors are accessible via [`Scheduler::errors`] after `run` returns.
pub struct Scheduler {
    systems: Vec<Box<dyn System>>,
    errors: Vec<(usize, SystemError)>,
}

impl Scheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Register a system. Systems run in the order they are added.
    pub fn add(&mut self, system: impl System + 'static) {
        self.systems.push(Box::new(system));
    }

    /// Run all registered systems in order, each advancing the world by `dt`.
    ///
    /// If a system returns [`Err`], the scheduler records the error and
    /// continues running remaining systems. This is graceful degradation:
    /// a failure in one system (e.g. a force model) does not prevent
    /// others (e.g. the integrator) from running.
    pub fn run(&mut self, world: &mut World, dt: Seconds<f64>) {
        self.errors.clear();
        for (i, system) in self.systems.iter_mut().enumerate() {
            if let Err(e) = system.run(world, dt) {
                self.errors.push((i, e));
            }
        }
    }

    /// Errors collected during the most recent [`Scheduler::run`] call.
    ///
    /// Each entry is `(system_index, error)` — the index is the position
    /// in registration order (0-based).
    pub fn errors(&self) -> &[(usize, SystemError)] {
        &self.errors
    }

    /// Whether the most recent [`Scheduler::run`] produced any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Number of registered systems.
    pub fn len(&self) -> usize {
        self.systems.len()
    }

    /// Whether no systems are registered.
    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------
// System implementations wrapping the existing free functions.
// ------------------------------------------------------------------

/// System that advances the entire simulation world by one fixed step.
///
/// Wraps the existing [`step_world`] free function. The physics logic is
/// unchanged — this struct is a thin adapter that lets the simulation loop
/// register stepping as a system rather than calling `step_world` by name.
///
/// [`step_world`]: crate::systems::step::step_world
pub struct StepWorldSystem;

impl System for StepWorldSystem {
    fn run(&mut self, world: &mut World, dt: Seconds<f64>) -> Result<(), SystemError> {
        step::step_world(world, dt);
        Ok(())
    }
}

/// A diagnostic system that records simulation tick metadata.
///
/// Captures the tick count and the world epoch after each step, providing
/// a lightweight audit trail when registered alongside physics systems.
pub struct LoggingSystem {
    ticks: u64,
    last_epoch: Option<hifitime::Epoch>,
}

impl LoggingSystem {
    /// Create a new logging system with zero ticks.
    #[allow(clippy::new_without_default)]
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use nalgebra::Vector3;

    use super::*;

    #[test]
    fn test_scheduler_new_is_empty() {
        let scheduler = Scheduler::new();
        assert!(scheduler.is_empty());
        assert_eq!(scheduler.len(), 0);
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
        scheduler.add(CounterSystem {
            label: "first",
            log: Arc::clone(&log),
        });
        scheduler.add(CounterSystem {
            label: "second",
            log: Arc::clone(&log),
        });
        scheduler.add(CounterSystem {
            label: "third",
            log: Arc::clone(&log),
        });

        assert_eq!(scheduler.len(), 3);
        assert!(!scheduler.is_empty());

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_scheduler_run_multiple_ticks_preserves_order() {
        let log = Arc::new(Mutex::new(Vec::new()));

        let mut scheduler = Scheduler::new();
        scheduler.add(CounterSystem {
            label: "a",
            log: Arc::clone(&log),
        });
        scheduler.add(CounterSystem {
            label: "b",
            log: Arc::clone(&log),
        });

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["a", "b", "a", "b"]);
    }

    // ------------------------------------------------------------------
    // StepWorldSystem — wraps the existing `step_world` free function.
    // ------------------------------------------------------------------

    use crate::components::celestial::CelestialBodySpec;
    use crate::components::kinematics::Kinematics;
    use crate::components::rigid_body::RigidBody;
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
    use apogee_common::units::Kilograms;

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
        scheduler.add(StepWorldSystem);
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
        let r = pos.norm();
        let v2 = vel.norm_squared();
        v2 / 2.0 - GM_EARTH / r
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
        // The scheduler should run both in order each tick.
        let mut scheduler = Scheduler::new();
        scheduler.add(StepWorldSystem);
        scheduler.add(LoggingSystem::new());

        assert_eq!(scheduler.len(), 2);

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

    #[test]
    fn test_logging_system_records_tick_count() {
        let mut logging = LoggingSystem::new();
        let mut world = World::new();

        for _ in 0..5 {
            logging.run(&mut world, Seconds::new(1.0)).unwrap();
        }

        assert_eq!(logging.tick_count(), 5);
        assert!(logging.last_epoch().is_some());
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
        scheduler.add(CounterSystem {
            label: "before",
            log: Arc::clone(&log),
        });
        scheduler.add(FailingSystem);
        scheduler.add(CounterSystem {
            label: "after",
            log: Arc::clone(&log),
        });

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));

        // Both CounterSystems should have run despite FailingSystem's error.
        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["before", "after"]);

        // One error should be recorded, for system index 1 (FailingSystem).
        assert!(scheduler.has_errors());
        assert_eq!(scheduler.errors().len(), 1);
        assert_eq!(scheduler.errors()[0].0, 1);
        assert!(scheduler.errors()[0]
            .1
            .to_string()
            .contains("intentional failure"));
    }

    #[test]
    fn test_scheduler_no_errors_when_all_succeed() {
        let mut scheduler = Scheduler::new();
        scheduler.add(CounterSystem {
            label: "a",
            log: Arc::new(Mutex::new(Vec::new())),
        });

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));

        assert!(!scheduler.has_errors());
        assert!(scheduler.errors().is_empty());
    }

    #[test]
    fn test_scheduler_errors_cleared_between_runs() {
        let mut scheduler = Scheduler::new();
        scheduler.add(FailingSystem);

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));
        assert!(scheduler.has_errors());

        // Replace with a succeeding system and run again — errors should clear.
        let mut scheduler2 = Scheduler::new();
        scheduler2.add(CounterSystem {
            label: "ok",
            log: Arc::new(Mutex::new(Vec::new())),
        });
        scheduler2.run(&mut world, Seconds::new(1.0));
        assert!(!scheduler2.has_errors());
    }
}
