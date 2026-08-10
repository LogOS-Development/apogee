//! System trait and single-threaded scheduler.
//!
//! Issue #156: decouple system definition from query iteration. hecs is
//! deliberately minimal — no `System` trait, no scheduler. This module adds
//! a thin scheduling layer on top of hecs so systems can be registered and
//! run in order without the caller knowing each system's signature.
//!
//! Design: Option B from the issue — `Fn(&mut World)` style. The query is
//! written inside `run`, same as the existing free functions, but the system
//! is a registered object rather than a free function the caller must remember
//! to call.

use apogee_common::units::Seconds;

use crate::systems::step;
use crate::world::World;

/// A system: a unit of simulation work that operates on the ECS [`World`].
///
/// Implementations write their own hecs query loops inside `run`, identical
/// to the existing free functions (`step_world`, `aggregate_forces`). The
/// trait decouples system *definition* from system *registration* — callers
/// register systems with a [`Scheduler`] and call `scheduler.run(&mut world, dt)`
/// instead of invoking each system by name.
///
/// Single-threaded. Multithreaded dispatch and dependency graphs are non-goals
/// (tracked separately).
pub trait System: Send {
    /// Advance the simulation world by `dt` seconds.
    fn run(&mut self, world: &mut World, dt: Seconds<f64>);
}

/// A single-threaded system scheduler.
///
/// Systems are registered with [`Scheduler::add`] and run in registration
/// order when [`Scheduler::run`] is called. Registration order = execution
/// order; explicit dependency declarations can come later.
pub struct Scheduler {
    systems: Vec<Box<dyn System>>,
}

impl Scheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    /// Register a system. Systems run in the order they are added.
    pub fn add(&mut self, system: impl System + 'static) {
        self.systems.push(Box::new(system));
    }

    /// Run all registered systems in order, each advancing the world by `dt`.
    pub fn run(&mut self, world: &mut World, dt: Seconds<f64>) {
        for system in &mut self.systems {
            system.run(world, dt);
        }
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
    fn run(&mut self, world: &mut World, dt: Seconds<f64>) {
        step::step_world(world, dt);
    }
}

/// A no-op system that counts how many times it has been run.
///
/// Demonstrates a third system registering and running alongside
/// `StepWorldSystem` via the scheduler. Useful as a diagnostic stub and
/// for verifying execution order in tests.
pub struct LoggingSystem {
    ticks: u64,
}

impl LoggingSystem {
    /// Create a new logging system with zero ticks.
    pub fn new() -> Self {
        Self { ticks: 0 }
    }

    /// Number of times `run` has been called.
    pub fn tick_count(&self) -> u64 {
        self.ticks
    }
}

impl Default for LoggingSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for LoggingSystem {
    fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) {
        self.ticks += 1;
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
        fn run(&mut self, _world: &mut World, _dt: Seconds<f64>) {
            self.log.lock().unwrap().push(self.label);
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
            logging.run(&mut world, Seconds::new(1.0));
        }

        assert_eq!(logging.tick_count(), 5);
    }
}
