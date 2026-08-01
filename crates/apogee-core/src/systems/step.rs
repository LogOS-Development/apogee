//! Fixed-step translation propagation system.
//!
//! Phase 1.6 uses a single-rate RK4 integrator for the 6DOF milestone. The
//! full multi-rate / adaptive integrator will be introduced in a follow-up
//! Phase 1.5 issue.

use crate::components::dynamics::{Dynamics, SpacecraftConfig};
use crate::components::kinematics::Kinematics;
use crate::ephemeris::kernel::SolarSystemState;
use crate::integrator::{IntegrationResult, Integrator, Rk4, StateDerivative, StateVector};
use crate::systems::force_aggregator::aggregate_forces;

/// Advance a single spacecraft's translational state by `dt` seconds using
/// the fixed-step RK4 integrator configured by `integrator`.
///
/// Attitude and angular velocity are left unchanged in this first milestone.
#[allow(clippy::too_many_arguments)]
pub fn step_spacecraft(
    kinematics: &mut Kinematics,
    dynamics: &Dynamics,
    config: &SpacecraftConfig,
    celestial: &SolarSystemState,
    integrator: &mut Rk4,
    dt: f64,
    day_of_year: u16,
    seconds_utc: f64,
) -> IntegrationResult {
    let mut state = StateVector {
        position: kinematics.position,
        velocity: kinematics.velocity,
    };

    let derivative_fn = |s: &StateVector| {
        // Reconstruct a temporary kinematics from the integrator state so
        // force models see the trial position/velocity.
        let trial_kinematics = Kinematics {
            position: s.position,
            velocity: s.velocity,
            attitude: kinematics.attitude,
            angular_velocity: kinematics.angular_velocity,
        };
        let forces = aggregate_forces(
            &trial_kinematics,
            dynamics,
            config,
            celestial,
            day_of_year,
            seconds_utc,
        );
        StateDerivative {
            velocity: s.velocity,
            acceleration: forces.total(),
        }
    };

    let result = integrator.step(&mut state, &derivative_fn, dt);
    kinematics.position = state.position;
    kinematics.velocity = state.velocity;
    result
}

/// Propagate `kinematics` for `duration_s` seconds with a fixed `dt` step.
///
/// `seconds_utc` is advanced linearly with simulation time; `day_of_year`
/// is kept constant for simplicity in this milestone.
#[allow(clippy::too_many_arguments)]
pub fn propagate(
    kinematics: &mut Kinematics,
    dynamics: &Dynamics,
    config: &SpacecraftConfig,
    celestial: &SolarSystemState,
    dt: f64,
    duration_s: f64,
    mut day_of_year: u16,
    mut seconds_utc: f64,
) {
    let mut integrator = Rk4::new(dt);
    let mut elapsed = 0.0;
    while elapsed < duration_s {
        let step = (duration_s - elapsed).min(dt);
        step_spacecraft(
            kinematics,
            dynamics,
            config,
            celestial,
            &mut integrator,
            step,
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

#[cfg(test)]
mod tests {
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
    use nalgebra::Vector3;

    use super::*;

    #[test]
    fn test_two_body_orbit_energy_conservation() {
        let r = R_EARTH_EQ + 400_000.0;
        let v = (GM_EARTH / r).sqrt();
        let mut kinematics = Kinematics {
            position: Vector3::new(r, 0.0, 0.0),
            velocity: Vector3::new(0.0, v, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: Vector3::zeros(),
        };
        let dynamics = Dynamics {
            mass: 1_000.0,
            inertia: nalgebra::Matrix3::identity(),
            cg_offset: Vector3::zeros(),
        };
        let config = SpacecraftConfig::default();
        let celestial = SolarSystemState {
            states: vec![crate::ephemeris::kernel::BodyState {
                naif_id: 399,
                position: Vector3::zeros(),
                velocity: Vector3::zeros(),
            }],
        };

        let e0 = orbital_energy(&kinematics.position, &kinematics.velocity);
        propagate(
            &mut kinematics,
            &dynamics,
            &config,
            &celestial,
            60.0,
            3_600.0,
            80,
            0.0,
        );
        let e1 = orbital_energy(&kinematics.position, &kinematics.velocity);
        let rel_err = (e1 - e0).abs() / e0.abs();
        assert!(rel_err < 1e-5, "energy drift too large: {}", rel_err);
    }

    fn orbital_energy(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
        let r = pos.norm();
        let v2 = vel.norm_squared();
        v2 / 2.0 - GM_EARTH / r
    }
}
