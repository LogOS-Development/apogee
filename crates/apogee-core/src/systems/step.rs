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
//!
//! Issue #149: Celestial bodies are now first-class ECS entities. The force
//! aggregator queries the ECS world for `(&GravitySource, &Kinematics)` to
//! compute point-mass gravity, eliminating the separate `SolarSystemState`.
//! Dynamic celestial bodies (asteroids, debris) are integrated like spacecraft
//! — they have `Kinematics + GravitySource + CelestialKind::Dynamic +
//! CelestialMass` components.
//!
//! Issue #150: `SpacecraftConfig` has been replaced by per-component
//! `DragSurfaces` and `SrpSurfaces`. Entities without these components get
//! zero drag/SRP — the force aggregator skips them automatically.

use apogee_common::units::Seconds;
use nalgebra::Vector3;

use crate::components::celestial::{CelestialKind, CelestialMass, GravitySource, NaifIdComponent};
use crate::components::drag_surfaces::DragSurfaces;
use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::{RigidBody, SimulationConfig};
use crate::components::srp_surfaces::SrpSurfaces;
use crate::gravity::{GravitySources, SphericalHarmonics};
use crate::integrator::{IntegrationResult, Integrator, Rk4, StateDerivative, StateVector};
use crate::systems::force_aggregator::aggregate_forces;
use crate::world::World;
use hifitime::{Epoch, Unit};

/// Shared simulation environment passed to propagation functions.
///
/// Groups the values that are constant across all entities during a single
/// integration step: space-weather configuration, gravity source snapshot,
/// the Sun's position, and the current simulation epoch. Extracting them into
/// a struct keeps function signatures manageable and makes the environment
/// boundary explicit.
#[derive(Debug, Clone)]
pub struct SimContext {
    /// Space-weather / environment configuration for force models.
    pub sim_config: SimulationConfig,
    /// Gravity source snapshot (GM + position of all massive bodies).
    pub gravity_sources: GravitySources,
    /// Position of the Sun (NAIF ID 10), for SRP calculation.
    pub sun_position: apogee_common::Position,
    /// Current simulation epoch.
    pub epoch: Epoch,
    /// Optional spherical harmonics gravity model for the central body.
    ///
    /// When present, the force aggregator uses this instead of point-mass
    /// gravity for the primary body. The SH model includes its own GM and
    /// reference radius, so the central body's point-mass contribution is
    /// replaced (not added to) by the SH acceleration. Third-body point-mass
    /// perturbations from other gravity sources are still added on top.
    pub gravity_model: Option<SphericalHarmonics>,
}

impl SimContext {
    /// Build a `SimContext` from a `World`'s shared state.
    ///
    /// Collects gravity sources from all ECS entities that have
    /// `GravitySource + Kinematics` components, and finds the Sun's position
    /// by looking up NAIF ID 10.
    pub fn from_world(world: &World) -> Self {
        let mut gravity_sources = GravitySources::new();
        for (_, (gs, kin)) in world.ecs.query::<(&GravitySource, &Kinematics)>().iter() {
            gravity_sources.push(gs.gm, kin.position);
        }

        let sun_position = find_sun_position(world);

        Self {
            sim_config: world.sim_config,
            gravity_sources,
            sun_position,
            epoch: world.epoch,
            gravity_model: None,
        }
    }

    /// Create a `SimContext` with a single gravity source at the origin.
    ///
    /// Convenience for tests that need a simple Earth-at-origin gravity model
    /// without setting up a full ECS world.
    pub fn single_body(gm: f64, epoch: Epoch) -> Self {
        let mut gravity_sources = GravitySources::new();
        gravity_sources.push(gm, Vector3::zeros());
        Self {
            sim_config: SimulationConfig::default(),
            gravity_sources,
            sun_position: Vector3::new(-apogee_common::constants::AU, 0.0, 0.0),
            epoch,
            gravity_model: None,
        }
    }
}

/// Find the Sun's position from the ECS world (NAIF ID 10).
///
/// Returns a default position (-1 AU on x-axis) if no Sun entity exists,
/// so SRP falls back to a heliocentric approximation.
fn find_sun_position(world: &World) -> apogee_common::Position {
    for (_, (id, kin)) in world.ecs.query::<(&NaifIdComponent, &Kinematics)>().iter() {
        if id.0 == 10 {
            return kin.position;
        }
    }
    Vector3::new(-apogee_common::constants::AU, 0.0, 0.0)
}

/// Advance a single spacecraft's translational state by `dt` seconds using
/// the fixed-step RK4 integrator configured by `integrator`.
///
/// Attitude and angular velocity are left unchanged in this first milestone.
///
/// `drag_surfaces` and `srp_surfaces` are `Option` — `None` means the entity
/// has no surfaces of that type and the corresponding force is zero.
///
/// Selectable propagators and adaptive step sizing are tracked in follow-up
/// issues for per-object fidelity and federated simulation support.
pub fn step_spacecraft(
    kinematics: &mut Kinematics,
    rigid_body: &RigidBody,
    drag_surfaces: Option<&DragSurfaces>,
    srp_surfaces: Option<&SrpSurfaces>,
    ctx: &SimContext,
    integrator: &mut Rk4,
    dt: Seconds<f64>,
) -> IntegrationResult {
    let mut state = StateVector::from_kinematics(kinematics);

    let inertia = rigid_body.inertia;
    let inertia_inv = inertia
        .try_inverse()
        .unwrap_or_else(nalgebra::Matrix3::identity);

    // Snapshot the immutable parts so the derivative closure does not conflict
    // with the mutable kinematics write-back.
    let rb = rigid_body.clone();
    let drag = drag_surfaces.cloned();
    let srp = srp_surfaces.cloned();
    let sim_config = ctx.sim_config;
    let gravity_sources = ctx.gravity_sources.clone();
    let gravity_model = ctx.gravity_model.clone();
    let sun_position = ctx.sun_position;
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
        let forces = aggregate_forces(
            &trial_kinematics,
            &rb,
            drag.as_ref(),
            srp.as_ref(),
            &sim_config,
            &gravity_sources,
            gravity_model.as_ref(),
            sun_position,
            epoch,
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
/// Iterates all live entities that have `Kinematics + RigidBody`, calling
/// `step_spacecraft` on each one in-place. `DragSurfaces` and `SrpSurfaces`
/// are queried as optional components — entities without them get zero
/// drag/SRP. Then integrates dynamic celestial bodies (entities with
/// `Kinematics + GravitySource + CelestialKind::Dynamic + CelestialMass`).
/// Kinematic celestial bodies are left untouched (their positions are driven
/// by the ephemeris service). The world epoch is advanced by `dt` after all
/// entities have been stepped.
pub fn step_world(world: &mut World, dt: Seconds<f64>) {
    // Build the simulation context from the ECS world — this collects
    // gravity sources and the Sun's position from celestial body entities.
    let ctx = SimContext::from_world(world);

    // Each entity gets its own integrator instance. In Phase 1.6 all
    // entities share the same fixed step size.
    let mut integrator = Rk4::new(dt);

    // Step all spacecraft entities (Kinematics + RigidBody). DragSurfaces
    // and SrpSurfaces are optional — queried separately per entity.
    // We collect entity handles first to avoid holding a borrow while
    // mutating.
    let entities: Vec<hecs::Entity> = world
        .ecs
        .query::<(&mut Kinematics, &RigidBody)>()
        .iter()
        .map(|(e, _)| e)
        .collect();

    for entity in entities {
        // Check for celestial kind — skip dynamic celestial bodies here;
        // they are handled by integrate_dynamic_celestials below.
        let is_dynamic_celestial = world
            .get_component::<CelestialKind>(entity)
            .map(|k| k.is_dynamic())
            .unwrap_or(false);

        if is_dynamic_celestial {
            continue;
        }

        // Read the drag and SRP components (optional).
        let drag = world
            .get_component::<DragSurfaces>(entity)
            .map(|d| (*d).clone());
        let srp = world
            .get_component::<SrpSurfaces>(entity)
            .map(|s| (*s).clone());
        let rb = world
            .get_component::<RigidBody>(entity)
            .map(|r| (*r).clone());

        if let Some(rb) = rb {
            if let Some(mut kin) = world.get_component_mut::<Kinematics>(entity) {
                step_spacecraft(
                    &mut kin,
                    &rb,
                    drag.as_ref(),
                    srp.as_ref(),
                    &ctx,
                    &mut integrator,
                    dt,
                );
            }
        }
    }

    // Integrate dynamic celestial bodies under point-mass gravity from
    // all gravity sources. Kinematic bodies are left untouched.
    integrate_dynamic_celestials(world, &ctx, &mut integrator, dt);
}

/// Step the world and advance the epoch by `dt`. Use this when calling
/// directly instead of through a [`Scheduler`].
pub fn step_and_advance(world: &mut World, dt: Seconds<f64>) {
    step_world(world, dt);
    world.epoch += dt.into_value() * Unit::Second;
}

/// Integrate all dynamic celestial bodies in the ECS world one step forward.
///
/// Each dynamic body is accelerated by point-mass gravity from every other
/// gravity source in the world (excluding itself — a body does not feel its
/// own gravity). Kinematic bodies are skipped: their positions are driven by
/// the ephemeris service.
fn integrate_dynamic_celestials(
    world: &mut World,
    ctx: &SimContext,
    integrator: &mut Rk4,
    dt: Seconds<f64>,
) {
    // Collect entity IDs + positions of dynamic celestial bodies first, so we
    // can borrow the world mutably without holding a query borrow alive.
    let dynamic_bodies: Vec<(hecs::Entity, apogee_common::Position)> = world
        .ecs
        .query::<(&CelestialKind, &Kinematics)>()
        .iter()
        .filter(|(_, (kind, _))| kind.is_dynamic())
        .map(|(e, (_, kin))| (e, kin.position))
        .collect();

    for (entity, body_position) in dynamic_bodies {
        // Read mass for this dynamic body.
        let mass = match world.get_component::<CelestialMass>(entity) {
            Some(m) => m.mass(),
            None => continue,
        };

        let mut kin = match world.get_component_mut::<Kinematics>(entity) {
            Some(k) => k,
            None => continue,
        };

        // Build a gravity sources snapshot excluding this body's own
        // gravity source (identified by matching position). A body should
        // not feel its own gravity — including it produces a singularity
        // at the initial position that silently zeroes the k1 acceleration.
        let mut body_gs = GravitySources::new();
        for &(gm, pos) in &ctx.gravity_sources.sources {
            if pos != body_position {
                body_gs.push(gm, pos);
            }
        }

        let body_ctx = SimContext {
            sim_config: ctx.sim_config,
            gravity_sources: body_gs,
            sun_position: ctx.sun_position,
            epoch: ctx.epoch,
            gravity_model: ctx.gravity_model.clone(),
        };

        // Dynamic bodies only feel gravity — no drag/SRP surfaces.
        let rb = RigidBody {
            mass,
            inertia: nalgebra::Matrix3::identity(),
            cg_offset: Vector3::zeros(),
        };

        step_spacecraft(&mut kin, &rb, None, None, &body_ctx, integrator, dt);
    }
}

/// Propagate a single spacecraft for `duration_s` seconds with a fixed `dt` step.
///
/// The epoch is advanced in place on `ctx` as simulation time progresses, so
/// callers that reuse the context across calls will see the updated epoch.
///
/// This is a convenience wrapper around `step_spacecraft` for single-entity
/// use cases that do not need a full `World`.
pub fn propagate(
    kinematics: &mut Kinematics,
    rigid_body: &RigidBody,
    drag_surfaces: Option<&DragSurfaces>,
    srp_surfaces: Option<&SrpSurfaces>,
    ctx: &mut SimContext,
    dt: Seconds<f64>,
    duration_s: Seconds<f64>,
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
            kinematics,
            rigid_body,
            drag_surfaces,
            srp_surfaces,
            ctx,
            &mut integrator,
            Seconds::new(step),
        );
        elapsed += step;
        ctx.epoch += step * Unit::Second;
    }
}

#[cfg(test)]
mod tests {
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
    use apogee_common::units::{Kilograms, Seconds};
    use approx::assert_relative_eq;
    use nalgebra::Vector3;

    use super::*;
    use crate::components::celestial::CelestialBodySpec;
    use crate::components::kinematics::Kinematics;

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

    fn test_epoch() -> Epoch {
        Epoch::from_gregorian_utc(2026, 3, 21, 0, 0, 0, 0)
    }

    #[test]
    fn test_two_body_orbit_energy_conservation() {
        let (mut kin, rb) = make_orbit_components();
        let mut ctx = SimContext::single_body(GM_EARTH, test_epoch());

        let e0 = orbital_energy(&kin.position, &kin.velocity);
        propagate(
            &mut kin,
            &rb,
            None,
            None,
            &mut ctx,
            Seconds::new(60.0),
            Seconds::new(3_600.0),
        );
        let e1 = orbital_energy(&kin.position, &kin.velocity);
        let rel_err = (e1 - e0).abs() / e0.abs();
        assert!(rel_err < 1e-5, "energy drift too large: {}", rel_err);
    }

    #[test]
    fn test_step_world_single_entity() {
        let (kin, rb) = make_orbit_components();
        let mut world = World::with_config_and_epoch(SimulationConfig::default(), test_epoch());
        // Spawn Earth as a kinematic celestial body at the origin.
        world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));
        let _entity = world.spawn((kin, rb));

        let e0 = {
            // Find the spacecraft entity (the one with Kinematics + RigidBody
            // but no CelestialKind).
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

        // Step 60 seconds at a time for 1 hour.
        for _ in 0..60 {
            step_and_advance(&mut world, Seconds::new(60.0));
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
            "step_world energy drift too large: {}",
            rel_err
        );
    }

    #[test]
    fn test_step_world_multi_entity() {
        let mut world = World::with_config_and_epoch(SimulationConfig::default(), test_epoch());
        world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));

        // Two entities with different initial positions.
        let (kin0, rb0) = make_orbit_components();
        let e0 = world.spawn((kin0, rb0));

        let mut kin1 = make_orbit_components().0;
        kin1.position = Vector3::new(R_EARTH_EQ + 500_000.0, 0.0, 0.0);
        kin1.velocity = Vector3::new(0.0, (GM_EARTH / (R_EARTH_EQ + 500_000.0)).sqrt(), 0.0);
        let (_, rb1) = make_orbit_components();
        let e1 = world.spawn((kin1, rb1));

        for _ in 0..10 {
            step_and_advance(&mut world, Seconds::new(60.0));
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

    // ------------------------------------------------------------------
    // Celestial ECS entity integration tests
    // ------------------------------------------------------------------

    #[test]
    fn test_kinematic_body_does_not_move() {
        // A kinematic Earth at the origin should not be moved by step_world.
        let mut world = World::new();
        world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));

        for _ in 0..10 {
            step_and_advance(&mut world, Seconds::new(60.0));
        }

        let earth = world.find_celestial(399).unwrap();
        let kin = world.get_component::<Kinematics>(earth).unwrap();
        assert_relative_eq!(kin.position.norm(), 0.0);
        assert_relative_eq!(kin.velocity.norm(), 0.0);
    }

    #[test]
    fn test_dynamic_celestial_orbits_kinematic_body() {
        // A small dynamic body (asteroid) should orbit a kinematic Earth.
        let mut world = World::new();
        world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));

        // Asteroid at 400 km altitude, circular orbit velocity.
        let r = R_EARTH_EQ + 400_000.0;
        let v = (GM_EARTH / r).sqrt();
        world.add_celestial_body(CelestialBodySpec::dynamic_from_mass(
            2_000_001,
            Vector3::new(r, 0.0, 0.0),
            Vector3::new(0.0, v, 0.0),
            Kilograms::new(1e6),
        ));

        let asteroid = world.find_celestial(2_000_001).unwrap();
        let e0 = {
            let kin = world.get_component::<Kinematics>(asteroid).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };

        // Step for ~1 orbit (92 min).
        for _ in 0..92 {
            step_and_advance(&mut world, Seconds::new(60.0));
        }

        let e1 = {
            let kin = world.get_component::<Kinematics>(asteroid).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };
        let rel_err = (e1 - e0).abs() / e0.abs();
        assert!(
            rel_err < 1e-4,
            "dynamic celestial energy drift too large: {}",
            rel_err
        );
    }

    #[test]
    fn test_spacecraft_orbits_with_kinematic_and_propagated_bodies() {
        // Spacecraft orbits a kinematic Earth while a propagated asteroid
        // also orbits. Both should maintain stable orbits.
        let mut world = World::new();
        world.add_celestial_body(CelestialBodySpec::kinematic(
            399,
            Vector3::zeros(),
            Vector3::zeros(),
        ));

        // Propagated asteroid at 1000 km altitude.
        let r_ast = R_EARTH_EQ + 1_000_000.0;
        let v_ast = (GM_EARTH / r_ast).sqrt();
        world.add_celestial_body(CelestialBodySpec::dynamic_from_mass(
            2_000_001,
            Vector3::new(r_ast, 0.0, 0.0),
            Vector3::new(0.0, v_ast, 0.0),
            Kilograms::new(1e10),
        ));

        // Spacecraft at 400 km altitude (well inside the asteroid orbit).
        let r_sc = R_EARTH_EQ + 400_000.0;
        let v_sc = (GM_EARTH / r_sc).sqrt();
        let kin = Kinematics {
            position: Vector3::new(r_sc, 0.0, 0.0),
            velocity: Vector3::new(0.0, v_sc, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: Vector3::zeros(),
        };
        let rb = crate::components::rigid_body::RigidBody {
            mass: Kilograms::new(1_000.0),
            inertia: nalgebra::Matrix3::identity(),
            cg_offset: Vector3::zeros(),
        };
        let sc_entity = world.spawn((kin, rb));

        let sc_e0 = {
            let kin = world.get_component::<Kinematics>(sc_entity).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };

        // Step 10 minutes.
        for _ in 0..10 {
            step_and_advance(&mut world, Seconds::new(60.0));
        }

        let sc_e1 = {
            let kin = world.get_component::<Kinematics>(sc_entity).unwrap();
            orbital_energy(&kin.position, &kin.velocity)
        };
        let sc_rel_err = (sc_e1 - sc_e0).abs() / sc_e0.abs();
        assert!(
            sc_rel_err < 1e-4,
            "spacecraft energy drift with celestial registry: {}",
            sc_rel_err
        );

        // Spacecraft should still be in LEO.
        let kin = world.get_component::<Kinematics>(sc_entity).unwrap();
        let sc_alt = kin.position.norm() - R_EARTH_EQ;
        assert!(sc_alt > 350_000.0 && sc_alt < 500_000.0);
    }
}
