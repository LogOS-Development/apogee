//! Fixed-step translation propagation system.
//!
//! Phase 1.6 uses a single-rate RK4 integrator for the 6DOF milestone. The
//! full multi-rate / adaptive integrator will be introduced in a follow-up
//! Phase 1.5 issue.

use apogee_common::units::Seconds;

use crate::components::rigid_body::{RigidBody, SimulationConfig, SpacecraftConfig};
use crate::components::kinematics::Kinematics;
use crate::ephemeris::kernel::SolarSystemState;
use crate::integrator::{IntegrationResult, Integrator, Rk4, StateDerivative, StateVector};
use crate::systems::force_aggregator::aggregate_forces;

/// Advance a single spacecraft's translational state by `dt` seconds using
/// the fixed-step RK4 integrator configured by `integrator`.
///
/// Attitude and angular velocity are left unchanged in this first milestone.
///
/// Selectable propagators and adaptive step sizing are tracked in follow-up
/// issues for per-object fidelity and federated simulation support.
#[allow(clippy::too_many_arguments)]
pub fn step_spacecraft(
    kinematics: &mut Kinematics,
    rigid_body: &RigidBody,
    config: &SpacecraftConfig,
    sim_config: &SimulationConfig,
    celestial: &SolarSystemState,
    integrator: &mut Rk4,
    dt: Seconds<f64>,
    day_of_year: u16,
    seconds_utc: f64,
) -> IntegrationResult {
    let mut state = StateVector::from_kinematics(kinematics);

    let inertia = rigid_body.inertia;
    let inertia_inv = inertia
        .try_inverse()
        .unwrap_or_else(nalgebra::Matrix3::identity);
    let _mass_inv = 1.0 / rigid_body.mass.into_value();

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
            rigid_body,
            config,
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
    state.write_to_kinematics(kinematics);
    result
}

/// Propagate `kinematics` for `duration_s` seconds with a fixed `dt` step.
///
/// `seconds_utc` is advanced linearly with simulation time; `day_of_year`
/// is kept constant for simplicity in this milestone.
#[allow(clippy::too_many_arguments)]
pub fn propagate(
    kinematics: &mut Kinematics,
    rigid_body: &RigidBody,
    config: &SpacecraftConfig,
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
            kinematics,
            rigid_body,
            config,
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

#[cfg(test)]
mod tests {
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
    use apogee_common::units::{Area, Kilograms};
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
        let rigid_body = RigidBody {
            mass: Kilograms::new(1_000.0),
            inertia: nalgebra::Matrix3::identity(),
            cg_offset: Vector3::zeros(),
        };
        let config = SpacecraftConfig {
            ballistic_coefficient: 0.0,
            srp_area: Area::new(0.0),
            reflectivity: 0.0,
            reference_mass_kg: 1_000.0,
        };
        let sim_config = SimulationConfig::default();
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
            &rigid_body,
            &config,
            &sim_config,
            &celestial,
            Seconds::new(60.0),
            Seconds::new(3_600.0),
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
