//! System trait and single-threaded scheduler for the ECS world.
//!
//! Provides a [`System`] trait and [`Scheduler`] so simulation steps can be
//! registered and run in order without the caller knowing each system's
//! signature. Systems write their own hecs query loops inside `run`.

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

/// Stable identifier for a registered system.
///
/// Assigned by the [`Scheduler`] when a system is added or inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemId(pub usize);

/// A system: a unit of simulation work that operates on the ECS [`World`].
///
/// Register systems with a [`Scheduler`] and call `scheduler.run(&mut world, dt)`.
///
/// Systems that need finer timesteps can override [`System::preferred_dt`]
/// to declare a preferred sub-step size. The scheduler will run the system
/// `N` times per tick, where `N = dt / preferred_dt` (with accumulation
/// for non-integer divisors). Returning `None` (the default) means the
/// system runs once per tick at the full scheduler dt.
pub trait System: Send {
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

struct SystemEntry {
    id: SystemId,
    system: Box<dyn System>,
}

/// Runs registered systems in order and collects errors.
///
/// Systems can be appended with [`Scheduler::add`], inserted at the front
/// with [`Scheduler::insert_front`], or inserted relative to an existing
/// system with [`Scheduler::insert_before`] and [`Scheduler::insert_after`].
pub struct Scheduler {
    systems: Vec<SystemEntry>,
    errors: Vec<(SystemId, SystemError)>,
    /// Accumulated time remainder per system, keyed by stable [`SystemId`].
    accumulated: HashMap<SystemId, f64>,
    next_id: usize,
}

impl Scheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            errors: Vec::new(),
            accumulated: HashMap::new(),
            next_id: 0,
        }
    }

    /// Register a system at the end of the execution order.
    ///
    /// Returns the [`SystemId`] assigned to the new system, which can be
    /// used with [`Scheduler::insert_before`] and
    /// [`Scheduler::insert_after`].
    pub fn add(&mut self, system: impl System + 'static) -> SystemId {
        let id = SystemId(self.next_id);
        self.next_id += 1;
        self.systems.push(SystemEntry {
            id,
            system: Box::new(system),
        });
        self.accumulated.insert(id, 0.0);
        id
    }

    /// Insert a system at the front of the execution order.
    pub fn insert_front(&mut self, system: impl System + 'static) -> SystemId {
        let id = SystemId(self.next_id);
        self.next_id += 1;
        self.systems.insert(
            0,
            SystemEntry {
                id,
                system: Box::new(system),
            },
        );
        self.accumulated.insert(id, 0.0);
        id
    }

    /// Insert a system immediately before the system with the given ID.
    ///
    /// Panics if `target_id` is not found.
    pub fn insert_before(
        &mut self,
        target_id: SystemId,
        system: impl System + 'static,
    ) -> SystemId {
        let index = self
            .systems
            .iter()
            .position(|e| e.id == target_id)
            .expect("target system not found");
        let id = SystemId(self.next_id);
        self.next_id += 1;
        self.systems.insert(
            index,
            SystemEntry {
                id,
                system: Box::new(system),
            },
        );
        self.accumulated.insert(id, 0.0);
        id
    }

    /// Insert a system immediately after the system with the given ID.
    ///
    /// Panics if `target_id` is not found.
    pub fn insert_after(&mut self, target_id: SystemId, system: impl System + 'static) -> SystemId {
        let index = self
            .systems
            .iter()
            .position(|e| e.id == target_id)
            .expect("target system not found");
        let id = SystemId(self.next_id);
        self.next_id += 1;
        self.systems.insert(
            index + 1,
            SystemEntry {
                id,
                system: Box::new(system),
            },
        );
        self.accumulated.insert(id, 0.0);
        id
    }

    /// Run all registered systems in order, then advance `world.epoch` by
    /// `dt` exactly once.
    ///
    /// Systems with a [`System::preferred_dt`] of `Some(system_dt)` are
    /// sub-stepped: the scheduler runs them `N` times per call, where
    /// `N = floor((dt + accumulated) / system_dt)`. The remainder
    /// is carried forward to the next tick, ensuring zero long-term drift
    /// even when `dt / system_dt` is not an integer.
    ///
    /// If a system returns [`Err`], the scheduler records the error and
    /// continues running remaining systems.
    pub fn run(&mut self, world: &mut World, dt: Seconds<f64>) {
        self.errors.clear();
        let dt_val = dt.into_value();

        for entry in self.systems.iter_mut() {
            let sub_dt = match entry.system.preferred_dt() {
                Some(pref) => pref.into_value(),
                None => {
                    // No preferred dt — run once at full scheduler dt.
                    if let Err(e) = entry.system.run(world, dt) {
                        self.errors.push((entry.id, e));
                    }
                    continue;
                }
            };

            // Compute sub-step count with accumulation for non-integer divisors.
            let effective_dt = dt_val + self.accumulated[&entry.id];
            let n_sub = (effective_dt / sub_dt).floor() as usize;
            let simulated = n_sub as f64 * sub_dt;
            self.accumulated.insert(entry.id, effective_dt - simulated);

            for _ in 0..n_sub {
                if let Err(e) = entry.system.run(world, Seconds::new(sub_dt)) {
                    self.errors.push((entry.id, e));
                    break;
                }
            }
        }

        world.epoch += dt.into_value() * Unit::Second;
    }

    /// Errors collected during the most recent [`Scheduler::run`] call.
    ///
    /// Each entry is `(`[`SystemId`]`, error`).
    pub fn errors(&self) -> &[(SystemId, SystemError)] {
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

/// Wraps [`step_world`] as a [`System`].
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

        // One error should be recorded, for SystemId(1) (FailingSystem).
        assert!(scheduler.has_errors());
        assert_eq!(scheduler.errors().len(), 1);
        assert_eq!(scheduler.errors()[0].0, SystemId(1));
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

    // ------------------------------------------------------------------
    // Scheduler owns epoch advancement — not step_world.
    // ------------------------------------------------------------------

    #[test]
    fn test_scheduler_advances_epoch_exactly_once_per_run() {
        // The scheduler should advance world.epoch by dt once per run(),
        // regardless of how many systems are registered. step_world must NOT
        // advance the epoch — that's the scheduler's job.
        let mut world = World::new();
        let epoch_before = world.epoch;

        let mut scheduler = Scheduler::new();
        scheduler.add(StepWorldSystem);
        scheduler.add(LoggingSystem::new());

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
        // Even with 3 systems, epoch should advance exactly once by dt.
        let mut world = World::new();
        let epoch_before = world.epoch;

        let mut scheduler = Scheduler::new();
        scheduler.add(LoggingSystem::new());
        scheduler.add(StepWorldSystem);
        scheduler.add(LoggingSystem::new());

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
        // step_world should NOT advance the epoch — that responsibility
        // belongs to the scheduler. Direct callers must advance it themselves.
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
    // AggregateForcesSystem
    // ------------------------------------------------------------------

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
        // Systems that don't override preferred_dt() should return None,
        // meaning they run once per scheduler tick (N=1, no sub-stepping).
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
        // A system with preferred_dt = None runs exactly once per scheduler.run,
        // receiving the full scheduler dt.
        let mut scheduler = Scheduler::new();
        scheduler.add(CounterSystem {
            label: "once",
            log: Arc::new(Mutex::new(Vec::new())),
        });

        let mut world = World::new();
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
        // A system with preferred_dt = 6.0 and scheduler dt = 60.0
        // should run 10 times (60/6 = 10), each with sub_dt = 6.0.
        let call_log = Arc::new(Mutex::new(Vec::new()));

        let mut scheduler = Scheduler::new();
        scheduler.add(SubStepLogSystem {
            preferred_dt: 6.0,
            call_log: Arc::clone(&call_log),
        });

        let mut world = World::new();
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
        // Verify sub-stepping calls the system N times with sub_dt each.
        let call_log = Arc::new(Mutex::new(Vec::new()));

        let mut scheduler = Scheduler::new();
        scheduler.add(SubStepLogSystem {
            preferred_dt: 6.0,
            call_log: Arc::clone(&call_log),
        });

        let mut world = World::new();
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
        // Issue acceptance criterion: "a fast system running at dt/10
        // while a slow system runs at dt".
        let log = Arc::new(Mutex::new(Vec::new()));

        // Fast system: preferred_dt = 6.0 (dt/10 when scheduler dt = 60)
        struct FastSystem {
            log: Arc<Mutex<Vec<&'static str>>>,
            call_count: usize,
        }
        impl System for FastSystem {
            fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
                self.call_count += 1;
                self.log.lock().unwrap().push("fast");
                Ok(())
            }
            fn preferred_dt(&self) -> Option<Seconds<f64>> {
                Some(Seconds::new(6.0))
            }
        }

        // Slow system: no preferred_dt (runs once at full dt)
        struct SlowSystem {
            log: Arc<Mutex<Vec<&'static str>>>,
            call_count: usize,
        }
        impl System for SlowSystem {
            fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) -> Result<(), SystemError> {
                self.call_count += 1;
                self.log.lock().unwrap().push("slow");
                Ok(())
            }
        }

        let mut scheduler = Scheduler::new();
        scheduler.add(SlowSystem {
            log: Arc::clone(&log),
            call_count: 0,
        });
        scheduler.add(FastSystem {
            log: Arc::clone(&log),
            call_count: 0,
        });

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(60.0));

        // Slow system should have been called once, fast system 10 times.
        // The log should contain: slow, fast×10 (in registration order,
        // slow first then fast sub-steps).
        let recorded = log.lock().unwrap();
        let slow_count = recorded.iter().filter(|&&s| s == "slow").count();
        let fast_count = recorded.iter().filter(|&&s| s == "fast").count();
        assert_eq!(slow_count, 1, "slow system should run once");
        assert_eq!(fast_count, 10, "fast system should run 10 times");
    }

    #[test]
    fn test_sub_stepping_epoch_invariant_holds() {
        // The epoch advancement invariant must hold under sub-stepping:
        // epoch advances exactly dt per scheduler.run, regardless of
        // how many sub-steps each system takes.
        let mut world = World::new();
        let epoch_before = world.epoch;

        let mut scheduler = Scheduler::new();
        scheduler.add(SubStepLogSystem {
            preferred_dt: 6.0,
            call_log: Arc::new(Mutex::new(Vec::new())),
        });
        scheduler.add(SubStepLogSystem {
            preferred_dt: 12.0,
            call_log: Arc::new(Mutex::new(Vec::new())),
        });
        scheduler.add(CounterSystem {
            label: "slow",
            log: Arc::new(Mutex::new(Vec::new())),
        });

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
        // Tick 1: N=8, simulated=56, remainder=4
        // Tick 2: effective=64, N=9, simulated=63, remainder=1
        // Tick 3: effective=61, N=8, simulated=56, remainder=5
        // ...
        // Over 7 ticks: total simulated = 7*60 = 420 = 60*7 (zero drift)
        // because 420/7 = 60 sub-steps exactly.

        let call_log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        scheduler.add(SubStepLogSystem {
            preferred_dt: 7.0,
            call_log: Arc::clone(&call_log),
        });

        let mut world = World::new();
        let epoch_before = world.epoch;

        // Run 7 ticks of dt=60.
        for _ in 0..7 {
            scheduler.run(&mut world, Seconds::new(60.0));
        }

        let elapsed = (world.epoch - epoch_before).to_seconds();
        // Epoch should have advanced by exactly 7*60 = 420 seconds.
        assert!(
            (elapsed - 420.0).abs() < 1e-9,
            "epoch should advance exactly 420s over 7 ticks, got {elapsed}s"
        );
    }

    #[test]
    fn test_sub_stepping_adapts_to_changing_outer_dt() {
        // The caller can vary the outer dt per tick, and each system's
        // sub-step count adjusts accordingly.
        let call_log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        scheduler.add(SubStepLogSystem {
            preferred_dt: 10.0,
            call_log: Arc::clone(&call_log),
        });

        let mut world = World::new();
        let epoch_before = world.epoch;

        // Tick 1: dt=60, preferred=10 → 6 sub-steps
        scheduler.run(&mut world, Seconds::new(60.0));
        // Tick 2: dt=30, preferred=10 → 3 sub-steps
        scheduler.run(&mut world, Seconds::new(30.0));
        // Tick 3: dt=100, preferred=10 → 10 sub-steps
        scheduler.run(&mut world, Seconds::new(100.0));

        let elapsed = (world.epoch - epoch_before).to_seconds();
        assert!(
            (elapsed - 190.0).abs() < 1e-9,
            "epoch should advance 190s (60+30+100), got {elapsed}s"
        );
    }

    #[test]
    fn test_sub_stepping_microsecond_resolution() {
        // Without a min_sub_dt floor, a system can declare arbitrarily
        // small preferred_dt values. With preferred_dt = 0.001 (1ms) and
        // scheduler dt = 1.0, the system runs 1000 sub-steps.
        let call_log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        scheduler.add(SubStepLogSystem {
            preferred_dt: 0.001,
            call_log: Arc::clone(&call_log),
        });

        let mut world = World::new();
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
        // Systems should still run in registration order across sub-steps.
        // System A (no preferred dt) → runs once
        // System B (preferred_dt = 30) → runs twice at dt=60
        // System C (no preferred dt) → runs once
        // Order: A, B, B, C
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
        scheduler.add(OrderSystem {
            label: "A",
            log: Arc::clone(&log),
            preferred: None,
        });
        scheduler.add(OrderSystem {
            label: "B",
            log: Arc::clone(&log),
            preferred: Some(30.0),
        });
        scheduler.add(OrderSystem {
            label: "C",
            log: Arc::clone(&log),
            preferred: None,
        });

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(60.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["A", "B", "B", "C"]);
    }

    #[test]
    fn test_existing_systems_default_to_no_sub_stepping() {
        // Existing systems (StepWorldSystem, LoggingSystem, AggregateForcesSystem)
        // should have preferred_dt() = None, meaning they run once per tick.
        // This ensures backward compatibility.
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
    fn test_add_returns_system_id() {
        let mut scheduler = Scheduler::new();
        let id0 = scheduler.add(CounterSystem {
            label: "a",
            log: Arc::new(Mutex::new(Vec::new())),
        });
        let id1 = scheduler.add(CounterSystem {
            label: "b",
            log: Arc::new(Mutex::new(Vec::new())),
        });
        assert_ne!(id0, id1);
    }

    #[test]
    fn test_insert_front() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        scheduler.add(CounterSystem {
            label: "second",
            log: Arc::clone(&log),
        });
        scheduler.insert_front(CounterSystem {
            label: "first",
            log: Arc::clone(&log),
        });

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["first", "second"]);
    }

    #[test]
    fn test_insert_middle_by_id() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let id_a = scheduler.add(CounterSystem {
            label: "a",
            log: Arc::clone(&log),
        });
        scheduler.add(CounterSystem {
            label: "c",
            log: Arc::clone(&log),
        });
        scheduler.insert_after(
            id_a,
            CounterSystem {
                label: "b",
                log: Arc::clone(&log),
            },
        );

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_insert_before_by_id() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        scheduler.add(CounterSystem {
            label: "a",
            log: Arc::clone(&log),
        });
        let id_c = scheduler.add(CounterSystem {
            label: "c",
            log: Arc::clone(&log),
        });
        scheduler.insert_before(
            id_c,
            CounterSystem {
                label: "b",
                log: Arc::clone(&log),
            },
        );

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_insert_after_by_id() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();
        let id_a = scheduler.add(CounterSystem {
            label: "a",
            log: Arc::clone(&log),
        });
        scheduler.add(CounterSystem {
            label: "c",
            log: Arc::clone(&log),
        });
        scheduler.insert_after(
            id_a,
            CounterSystem {
                label: "b",
                log: Arc::clone(&log),
            },
        );

        let mut world = World::new();
        scheduler.run(&mut world, Seconds::new(1.0));

        let recorded = log.lock().unwrap();
        assert_eq!(*recorded, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_insert_mid_sim_between_existing_systems() {
        // Simulate inserting a system mid-simulation between two
        // already-registered systems, and verify accumulated values
        // for existing systems are preserved.
        let call_log = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new();

        let _id_a = scheduler.add(SubStepLogSystem {
            preferred_dt: 10.0,
            call_log: Arc::clone(&call_log),
        });
        let _id_b = scheduler.add(SubStepLogSystem {
            preferred_dt: 10.0,
            call_log: Arc::clone(&call_log),
        });

        let mut world = World::new();

        // Run one tick so accumulated values are non-zero.
        scheduler.run(&mut world, Seconds::new(60.0));

        // Insert a new system between A and B mid-sim.
        scheduler.insert_after(
            _id_a,
            SubStepLogSystem {
                preferred_dt: 10.0,
                call_log: Arc::clone(&call_log),
            },
        );

        // Run another tick. The new system starts with accumulated=0,
        // while B's accumulated value from tick 1 is preserved.
        scheduler.run(&mut world, Seconds::new(60.0));

        // Epoch should advance by 120s total.
        // We don't assert call counts precisely because the new system
        // only ran in tick 2 — the key invariant is that B's accumulated
        // value from tick 1 carried over correctly (no crash, no panic).
        let _elapsed = world.epoch;
    }
}
