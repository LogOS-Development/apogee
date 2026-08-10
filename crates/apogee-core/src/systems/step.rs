//! Fixed-step translation propagation system.
//!
//! Phase 1.6 uses a single-rate RK4 integrator for the 6DOF milestone. The
//! full multi-rate / adaptive integrator will be introduced in a follow-up
//! Phase 1.5 issue.
//!
//! Phase 2 (issue #102): `step_world` operates on the ECS `World` directly,
//! iterating all entities and stepping each one in-place via hecs queries.
//! `step_spacecraft` now takes individual component references instead of a
//! monolithic bundle.

use apogee_common::units::Seconds;

use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::{RigidBody, SimulationConfig, SpacecraftConfig};
use crate::ephemeris::kernel::SolarSystemState;
use crate::integrator::{IntegrationResult, Integrator, Rk4, StateDerivative, StateVector};
use crate::systems::force_aggregator::aggregate_forces;
use crate::world::World;
use hifitime::{Epoch, Unit};

/// Shared simulation environment passed to propagation functions.
///
/// Groups the three values that are constant across all entities during a
/// single integration step: space-weather configuration, celestial ephemeris
/// state, and the current simulation epoch. Extracting them into a struct
/// keeps function signatures manageable and makes the environment boundary
/// explicit.
#[derive(Debug, Clone)]
pub struct SimContext {
    /// Space-weather / environment configuration for force models.
    pub sim_config: SimulationConfig,
    /// Celestial ephemeris state (positions and velocities of all bodies).
    pub celestial: SolarSystemState,
    /// Current simulation epoch.
    pub epoch: Epoch,
}

impl SimContext {
    /// Build a `SimContext` from a `World`'s shared state.
    /// Clones the celestial ephemeris; copies `sim_config` and `epoch`.
    pub fn from_world(world: &World) -> Self {
        Self {
            sim_config: world.sim_config,
            celestial: world.celestial.clone(),
            epoch: world.epoch,
        }
    }
}

/// Advance a single spacecraft's translational state by `dt` seconds using
/// the fixed-step RK4 integrator configured by `integrator`.
///
/// Attitude and angular velocity are left unchanged in this first milestone.
///
/// Selectable propagators and adaptive step sizing are tracked in follow-up
/// issues for per-object fidelity and federated simulation support.
pub fn step_spacecraft(
    kinematics: &mut Kinematics,
    rigid_body: &RigidBody,
    config: &SpacecraftConfig,
    ctx: &SimContext,
    integrator: &mut Rk4,
    dt: Seconds<f64>,
) -> IntegrationResult {
    let mut state = StateVector::from_kinematics(kinematics);

    let inertia = rigid_body.inertia;
    let inertia_inv = inertia
        .try_inverse()
        .unwrap_or_else(nalgebra::Matrix3::identity);
    let _mass_inv = 1.0 / rigid_body.mass.into_value();

    // Snapshot the immutable parts so the derivative closure does not conflict
    // with the mutable kinematics write-back.
    let rb = rigid_body.clone();
    let cfg = *config;
    let sim_config = ctx.sim_config;
    let celestial = ctx.celestial.clone();
    let epoch = ctx.epoch;

    let derivative_fn = |s: &StateVector| {
        // Reconstruct a temporary kinematics from the integrator state so
        // force models see the trial position/velocity/attitude/rate.
        let trial_kinematics = Kinematics {
            position: s.position,
            velocity: s.velocity,
            attitude: s.attitude,
            angular_velocity: s.angular_velocity,
        };
        let forces = aggregate_forces(&trial_kinematics, &rb, &cfg, &sim_config, &celestial, epoch);

        // Translational acceleration = F / m. The unit-aware newtype collapses
        // to a raw vector for the integrator's hot path; the type tag is
        // preserved in `AggregatedForces` at the API surface.
        let acceleration = forces.total();

        // Rotational acceleration: alpha = I^-1 * (tau - omega x (I * omega)).
        let h = inertia * s.angular_velocity;
        let gyroscopic = s.angular_velocity.cross(&h);
        let net_torque_raw = forces.torque().raw() - gyroscopic;
        let angular_acceleration = inertia_inv * net_torque_raw;

        StateDerivative {
            velocity: s.velocity,
            // `acceleration` is m/s², but the integrator field is the same
            // SI base as `Position` (m); the convention is that the field
            // carries m/s² in the integrand role, even though the alias is
            // reused to keep the integrator allocation-free.
            acceleration: *acceleration.raw(),
            attitude_derivative: nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
            angular_acceleration,
        }
    };

    let result = integrator.step(&mut state, &derivative_fn, dt);
    state.write_to_kinematics(kinematics);
    result
}

/// Step the entire simulation world forward by `dt` seconds.
///
/// Iterates all live entities that have `Kinematics + RigidBody +
/// SpacecraftConfig`, calling `step_spacecraft` on each one in-place. The
/// simulation config, celestial state, and epoch are read from the `World`.
/// The world epoch is advanced by `dt` after all entities have been stepped.
pub fn step_world(world: &mut World, dt: Seconds<f64>) {
    // Snapshot the simulation context so we can borrow world.ecs mutably
    // without simultaneously borrowing world.sim_config / world.celestial.
    let ctx = SimContext::from_world(world);

    // Each entity gets its own integrator instance. In Phase 1.6 all
    // entities share the same fixed step size.
    let mut integrator = Rk4::new(dt);

    // Query all entities with the full spacecraft component set.
    for (_entity, (kin, rb, cfg)) in world
        .ecs
        .query::<(&mut Kinematics, &RigidBody, &SpacecraftConfig)>()
        .iter()
    {
        step_spacecraft(kin, rb, cfg, &ctx, &mut integrator, dt);
    }

    // Advance the world clock.
    world.epoch += dt.into_value() * Unit::Second;
}

/// Propagate a single spacecraft for `duration_s` seconds with a fixed `dt` step.
///
/// `epoch` is advanced linearly with simulation time.
///
/// This is a convenience wrapper around `step_spacecraft` for single-entity
/// use cases that do not need a full `World`.
pub fn propagate(
    kinematics: &mut Kinematics,
    rigid_body: &RigidBody,
    config: &SpacecraftConfig,
    ctx: &SimContext,
    dt: Seconds<f64>,
    duration_s: Seconds<f64>,
) {
    let mut integrator = Rk4::new(dt);
    let mut elapsed = 0.0_f64;
    let total = duration_s.into_value();
    let mut epoch = ctx.epoch;
    while elapsed < total {
        let remaining = total - elapsed;
        let dt_value = dt.into_value();
        let step = if remaining < dt_value {
            remaining
        } else {
            dt_value
        };
        // Temporarily advance the epoch for this sub-step.
        let step_ctx = SimContext {
            epoch,
            ..ctx.clone()
        };
        step_spacecraft(
            kinematics,
            rigid_body,
            config,
            &step_ctx,
            &mut integrator,
            Seconds::new(step),
        );
        elapsed += step;
        epoch += step * Unit::Second;
    }
}

/// Propagate a single entity in a `World` for `duration_s` seconds.
///
/// Convenience wrapper for the common single-spacecraft case: creates a
/// temporary `World` from the given components and context, calls
/// `step_world` in a loop, then returns the propagated components.
pub fn propagate_single(
    kinematics: Kinematics,
    rigid_body: RigidBody,
    config: SpacecraftConfig,
    ctx: SimContext,
    dt: Seconds<f64>,
    duration_s: Seconds<f64>,
) -> (Kinematics, RigidBody, SpacecraftConfig) {
    let mut world = World::with_config_and_epoch(ctx.sim_config, ctx.celestial.clone(), ctx.epoch);
    let _entity = world.spawn((kinematics, rigid_body, config));

    let total = duration_s.into_value();
    let dt_value = dt.into_value();
    let mut elapsed = 0.0_f64;
    while elapsed < total {
        let remaining = total - elapsed;
        let step = if remaining < dt_value {
            remaining
        } else {
            dt_value
        };
        step_world(&mut world, Seconds::new(step));
        elapsed += step;
    }

    // Return the single entity's components.
    let entity = world.entities().next().expect("entity should exist");
    let kin = world.get_component::<Kinematics>(entity).unwrap();
    let rb = world.get_component::<RigidBody>(entity).unwrap();
    let cfg = world.get_component::<SpacecraftConfig>(entity).unwrap();
    ((*kin).clone(), (*rb).clone(), *cfg)
}

#[cfg(test)]
mod tests {
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
    use apogee_common::units::{Area, Kilograms, Seconds};
    use nalgebra::Vector3;

    use super::*;
    use crate::components::kinematics::Kinematics;

    fn make_orbit_components() -> (Kinematics, RigidBody, SpacecraftConfig) {
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
            SpacecraftConfig {
                ballistic_coefficient: 0.0,
                srp_area: Area::new(0.0),
                reflectivity: 0.0,
                reference_mass_kg: 1_000.0,
            },
        )
    }

    fn earth_only() -> SolarSystemState {
        SolarSystemState {
            states: vec![crate::ephemeris::kernel::BodyState {
                naif_id: 399,
                position: Vector3::zeros(),
                velocity: Vector3::zeros(),
            }],
        }
    }

    fn test_epoch() -> Epoch {
        Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0)
    }

    #[test]
    fn test_two_body_orbit_energy_conservation() {
        let (mut kin, rb, cfg) = make_orbit_components();
        let ctx = SimContext {
            sim_config: SimulationConfig::default(),
            celestial: earth_only(),
            epoch: test_epoch(),
        };

        let e0 = orbital_energy(&kin.position, &kin.velocity);
        propagate(
            &mut kin,
            &rb,
            &cfg,
            &ctx,
            Seconds::new(60.0),
            Seconds::new(3_600.0),
        );
        let e1 = orbital_energy(&kin.position, &kin.velocity);
        let rel_err = (e1 - e0).abs() / e0.abs();
        assert!(rel_err < 1e-5, "energy drift too large: {}", rel_err);
    }

    #[test]
    fn test_step_world_single_entity() {
        let (kin, rb, cfg) = make_orbit_components();
        let mut world =
            World::with_config_and_epoch(SimulationConfig::default(), earth_only(), test_epoch());
        let _entity = world.spawn((kin, rb, cfg));

        let e0 = {
            let entity = world.entities().next().unwrap();
            let kin = world.get_component::<Kinematics>(entity).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };

        // Step 60 seconds at a time for 1 hour.
        for _ in 0..60 {
            step_world(&mut world, Seconds::new(60.0));
        }

        let e1 = {
            let entity = world.entities().next().unwrap();
            let kin = world.get_component::<Kinematics>(entity).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };
        let rel_err = (e1 - e0).abs() / e0.abs();
        assert!(
            rel_err < 1e-5,
            "step_world energy drift too large: {}",
            rel_err
        );
    }

    #[test]
    fn test_step_world_multi_entity() {
        let mut world =
            World::with_config_and_epoch(SimulationConfig::default(), earth_only(), test_epoch());

        // Two entities with different initial positions.
        let (kin0, rb0, cfg0) = make_orbit_components();
        let e0 = world.spawn((kin0, rb0, cfg0));

        let mut kin1 = make_orbit_components().0;
        kin1.position = Vector3::new(R_EARTH_EQ + 500_000.0, 0.0, 0.0);
        kin1.velocity = Vector3::new(0.0, (GM_EARTH / (R_EARTH_EQ + 500_000.0)).sqrt(), 0.0);
        let (_, rb1, cfg1) = make_orbit_components();
        let e1 = world.spawn((kin1, rb1, cfg1));

        for _ in 0..10 {
            step_world(&mut world, Seconds::new(60.0));
        }

        // Both entities should have moved.
        let kin0 = world.get_component::<Kinematics>(e0).unwrap();
        let kin1 = world.get_component::<Kinematics>(e1).unwrap();
        assert!(kin0.position.norm() > R_EARTH_EQ);
        assert!(kin1.position.norm() > R_EARTH_EQ);
    }

    fn orbital_energy(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
        let r = pos.norm();
        let v2 = vel.norm_squared();
        v2 / 2.0 - GM_EARTH / r
    }
}
