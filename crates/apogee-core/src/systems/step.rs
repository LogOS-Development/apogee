//! Fixed-step translation propagation system.
//!
//! Phase 1.6 uses a single-rate RK4 integrator for the 6DOF milestone. The
//! full multi-rate / adaptive integrator will be introduced in a follow-up
//! Phase 1.5 issue.
//!
//! Phase 2 (issue #102): `step_world` operates on the ECS `World` directly,
//! iterating all entities and stepping each one in-place. `step_spacecraft`
//! now takes a `SpacecraftBundle` instead of individual component refs.

use apogee_common::units::Seconds;

use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::SimulationConfig;
use crate::components::spacecraft::SpacecraftBundle;
use crate::ephemeris::kernel::SolarSystemState;
use crate::integrator::{IntegrationResult, Integrator, Rk4, StateDerivative, StateVector};
use crate::systems::force_aggregator::aggregate_forces;
use crate::world::World;

/// Advance a single spacecraft's translational state by `dt` seconds using
/// the fixed-step RK4 integrator configured by `integrator`.
///
/// Attitude and angular velocity are left unchanged in this first milestone.
///
/// Selectable propagators and adaptive step sizing are tracked in follow-up
/// issues for per-object fidelity and federated simulation support.
pub fn step_spacecraft(
    bundle: &mut SpacecraftBundle,
    sim_config: &SimulationConfig,
    celestial: &SolarSystemState,
    integrator: &mut Rk4,
    dt: Seconds<f64>,
    day_of_year: u16,
    seconds_utc: f64,
) -> IntegrationResult {
    let mut state = StateVector::from_kinematics(&bundle.kinematics);

    let inertia = bundle.rigid_body.inertia;
    let inertia_inv = inertia
        .try_inverse()
        .unwrap_or_else(nalgebra::Matrix3::identity);
    let _mass_inv = 1.0 / bundle.rigid_body.mass.into_value();

    // Capture the immutable parts of the bundle for the derivative closure.
    // We need a snapshot of the rigid_body and config so the closure borrows
    // do not conflict with the mutable kinematics write-back.
    let rigid_body = &bundle.rigid_body;
    let config = &bundle.config;

    let derivative_fn = |s: &StateVector| {
        // Reconstruct a temporary kinematics from the integrator state so
        // force models see the trial position/velocity/attitude/rate.
        let trial_kinematics = Kinematics {
            position: s.position,
            velocity: s.velocity,
            attitude: s.attitude,
            angular_velocity: s.angular_velocity,
        };
        let trial_bundle = SpacecraftBundle {
            kinematics: trial_kinematics,
            rigid_body: rigid_body.clone(),
            config: *config,
        };
        let forces = aggregate_forces(
            &trial_bundle,
            sim_config,
            celestial,
            day_of_year,
            seconds_utc,
        );

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
    state.write_to_kinematics(&mut bundle.kinematics);
    result
}

/// Step the entire simulation world forward by `dt` seconds.
///
/// Iterates all live entities, calling `step_spacecraft` on each one
/// in-place. The simulation config, celestial state, and clock values are
/// read from the `World`.
pub fn step_world(world: &mut World, dt: Seconds<f64>) {
    // Snapshot the simulation context so we can borrow world.entities mutably
    // without simultaneously borrowing world.sim_config / world.celestial.
    let sim_config = world.sim_config;
    let celestial = world.celestial.clone();
    let day_of_year = world.day_of_year;
    let seconds_utc = world.seconds_utc;

    // Each entity gets its own integrator instance. In Phase 1.6 all
    // entities share the same fixed step size.
    let mut integrator = Rk4::new(dt);

    // Collect entity handles first to avoid borrow issues during iteration.
    let entities: Vec<_> = world.entities().collect();
    for entity in entities {
        if let Some(bundle) = world.get_mut(entity) {
            step_spacecraft(
                bundle,
                &sim_config,
                &celestial,
                &mut integrator,
                dt,
                day_of_year,
                seconds_utc,
            );
        }
    }

    // Advance the world clock.
    let dt_value = dt.into_value();
    world.seconds_utc += dt_value;
    if world.seconds_utc >= 86_400.0 {
        world.seconds_utc -= 86_400.0;
        world.day_of_year = world.day_of_year.saturating_add(1);
    }
}

/// Propagate `kinematics` for `duration_s` seconds with a fixed `dt` step.
///
/// `seconds_utc` is advanced linearly with simulation time; `day_of_year`
/// is kept constant for simplicity in this milestone.
///
/// This is a convenience wrapper around `step_spacecraft` for single-entity
/// use cases that do not need a full `World`.
pub fn propagate(
    bundle: &mut SpacecraftBundle,
    sim_config: &SimulationConfig,
    celestial: &SolarSystemState,
    dt: Seconds<f64>,
    duration_s: Seconds<f64>,
    mut day_of_year: u16,
    mut seconds_utc: f64,
) {
    let mut integrator = Rk4::new(dt);
    let mut elapsed = 0.0_f64;
    let total = duration_s.into_value();
    while elapsed < total {
        let remaining = total - elapsed;
        let dt_value = dt.into_value();
        let step = if remaining < dt_value {
            remaining
        } else {
            dt_value
        };
        step_spacecraft(
            bundle,
            sim_config,
            celestial,
            &mut integrator,
            Seconds::new(step),
            day_of_year,
            seconds_utc,
        );
        elapsed += step;
        seconds_utc += step;
        if seconds_utc >= 86_400.0 {
            seconds_utc -= 86_400.0;
            day_of_year += 1;
        }
    }
}

/// Propagate a single entity in the `World` for `duration_s` seconds.
///
/// Convenience wrapper for the common single-spacecraft case: creates a
/// temporary `World` from the given bundle and context, calls `step_world`
/// in a loop, then returns the propagated bundle.
pub fn propagate_single(
    bundle: SpacecraftBundle,
    sim_config: SimulationConfig,
    celestial: SolarSystemState,
    dt: Seconds<f64>,
    duration_s: Seconds<f64>,
    day_of_year: u16,
    seconds_utc: f64,
) -> SpacecraftBundle {
    let mut world = World::with_config(sim_config, celestial);
    world.day_of_year = day_of_year;
    world.seconds_utc = seconds_utc;
    let _entity = world.spawn(bundle);

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

    // Return the single entity's bundle.
    let entity = world.entities().next().expect("entity should exist");
    world.get(entity).unwrap().clone()
}

#[cfg(test)]
mod tests {
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
    use apogee_common::units::{Area, Kilograms, Seconds};
    use nalgebra::Vector3;

    use super::*;
    use crate::components::kinematics::Kinematics;
    use crate::components::spacecraft::SpacecraftBundle;

    fn make_orbit_bundle() -> SpacecraftBundle {
        let r = R_EARTH_EQ + 400_000.0;
        let v = (GM_EARTH / r).sqrt();
        SpacecraftBundle {
            kinematics: Kinematics {
                position: Vector3::new(r, 0.0, 0.0),
                velocity: Vector3::new(0.0, v, 0.0),
                attitude: nalgebra::Quaternion::identity(),
                angular_velocity: Vector3::zeros(),
            },
            rigid_body: crate::components::rigid_body::RigidBody {
                mass: Kilograms::new(1_000.0),
                inertia: nalgebra::Matrix3::identity(),
                cg_offset: Vector3::zeros(),
            },
            config: crate::components::rigid_body::SpacecraftConfig {
                ballistic_coefficient: 0.0,
                srp_area: Area::new(0.0),
                reflectivity: 0.0,
                reference_mass_kg: 1_000.0,
            },
        }
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

    #[test]
    fn test_two_body_orbit_energy_conservation() {
        let mut bundle = make_orbit_bundle();
        let sim_config = SimulationConfig::default();
        let celestial = earth_only();

        let e0 = orbital_energy(&bundle.kinematics.position, &bundle.kinematics.velocity);
        propagate(
            &mut bundle,
            &sim_config,
            &celestial,
            Seconds::new(60.0),
            Seconds::new(3_600.0),
            80,
            0.0,
        );
        let e1 = orbital_energy(&bundle.kinematics.position, &bundle.kinematics.velocity);
        let rel_err = (e1 - e0).abs() / e0.abs();
        assert!(rel_err < 1e-5, "energy drift too large: {}", rel_err);
    }

    #[test]
    fn test_step_world_single_entity() {
        let bundle = make_orbit_bundle();
        let mut world = World::with_config(SimulationConfig::default(), earth_only());
        world.day_of_year = 80;
        world.seconds_utc = 0.0;
        let _entity = world.spawn(bundle);

        let e0 = {
            let b = world.iter().next().unwrap().1;
            orbital_energy(&b.kinematics.position, &b.kinematics.velocity)
        };

        // Step 60 seconds at a time for 1 hour.
        for _ in 0..60 {
            step_world(&mut world, Seconds::new(60.0));
        }

        let e1 = {
            let b = world.iter().next().unwrap().1;
            orbital_energy(&b.kinematics.position, &b.kinematics.velocity)
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
        let mut world = World::with_config(SimulationConfig::default(), earth_only());
        world.day_of_year = 80;

        // Two entities with different initial positions.
        let b0 = make_orbit_bundle();
        let mut b1 = make_orbit_bundle();
        b1.kinematics.position = Vector3::new(R_EARTH_EQ + 500_000.0, 0.0, 0.0);
        b1.kinematics.velocity =
            Vector3::new(0.0, (GM_EARTH / (R_EARTH_EQ + 500_000.0)).sqrt(), 0.0);
        let e0 = world.spawn(b0);
        let e1 = world.spawn(b1);

        for _ in 0..10 {
            step_world(&mut world, Seconds::new(60.0));
        }

        // Both entities should have moved.
        let b0 = world.get(e0).unwrap();
        let b1 = world.get(e1).unwrap();
        assert!(b0.kinematics.position.norm() > R_EARTH_EQ);
        assert!(b1.kinematics.position.norm() > R_EARTH_EQ);
    }

    fn orbital_energy(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
        let r = pos.norm();
        let v2 = vel.norm_squared();
        v2 / 2.0 - GM_EARTH / r
    }
}
