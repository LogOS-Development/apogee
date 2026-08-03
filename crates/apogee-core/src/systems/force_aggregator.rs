//! Force aggregator system: collects gravity + drag + SRP forces.

use apogee_common::units::{AccelerationVec, Dimensionless, Kilograms, TorqueVec};
use apogee_common::Position;
use nalgebra::Vector3;

use crate::aero::model::AtmosphereInput;
use crate::aero::nrlmsise00::Nrlmsise00;
use crate::aero::{AtmosphericDrag, SolarRadiationPressure};
use crate::components::dynamics::{SimulationConfig, SpacecraftConfig};
use crate::components::kinematics::Kinematics;
use crate::ephemeris::kernel::SolarSystemState;
use crate::gravity::PointMassGravity;

/// Aggregated forces and torques on a body.
#[derive(Debug, Clone, Default)]
pub struct AggregatedForces {
    /// Gravitational acceleration (m/s²).
    pub gravity: AccelerationVec,
    /// Atmospheric drag acceleration (m/s²).
    pub drag: AccelerationVec,
    /// Solar radiation pressure acceleration (m/s²).
    pub srp: AccelerationVec,
    /// Thrust acceleration (m/s²).
    pub thrust: AccelerationVec,
    /// External control torque (N·m), e.g. from reaction wheels or thrusters.
    pub control_torque: TorqueVec,
}

impl AggregatedForces {
    /// Sum all force contributions into total acceleration.
    pub fn total(&self) -> AccelerationVec {
        // Sum component-wise via raw escape hatches, then re-wrap.
        let raw = self.gravity.raw() + self.drag.raw() + self.srp.raw() + self.thrust.raw();
        AccelerationVec::from_mps2(raw)
    }

    /// Sum all torque contributions (N·m).
    pub fn torque(&self) -> TorqueVec {
        self.control_torque
    }
}

/// Control inputs (force + torque) to apply during a propagation step.
#[derive(Debug, Clone, Default)]
pub struct ControlInputs {
    /// Body-frame torque (N·m).
    pub torque_nm: Vector3<f64>,
    /// Body-frame force (N). Converted to acceleration using the supplied mass.
    pub force_n: Vector3<f64>,
}

impl AggregatedForces {
    /// Apply control inputs, converting force to acceleration using `mass_kg`.
    pub fn apply_control(&mut self, inputs: &ControlInputs, mass: Kilograms<f64>) {
        self.control_torque = TorqueVec::from_nm(inputs.torque_nm);
        let accel = inputs.force_n / mass.into_value();
        self.thrust = AccelerationVec::from_mps2(accel);
    }
}

/// Compute all perturbative accelerations for a spacecraft.
///
/// This is the main force aggregation for Phase 1.6. It uses:
/// - point-mass gravity from all bodies in `celestial`
/// - NRLMSISE-00 atmospheric drag
/// - solar radiation pressure with cylindrical eclipse detection
///
/// The `day_of_year` and `seconds_utc` inputs are used to build the
/// atmosphere-model input. For a real simulation these would come from a
/// `ClockService` tied to the current epoch.
///
/// Space-weather values are taken from `sim_config` rather than hardcoded so
/// a federation can drive them from an external simulation.
#[allow(clippy::too_many_arguments)]
pub fn aggregate_forces(
    kinematics: &Kinematics,
    dynamics: &crate::components::dynamics::Dynamics,
    config: &SpacecraftConfig,
    sim_config: &SimulationConfig,
    celestial: &SolarSystemState,
    day_of_year: u16,
    seconds_utc: f64,
) -> AggregatedForces {
    let gravity = PointMassGravity
        .acceleration(&kinematics.position, celestial)
        .unwrap_or_else(|_| AccelerationVec::from_mps2(Vector3::zeros()));

    let drag = {
        let model = Nrlmsise00;
        let latlon = ecef_lat_lon_from_inertial(&kinematics.position, day_of_year, seconds_utc);
        let input = AtmosphereInput {
            altitude_m: apogee_common::units::Meters::new(latlon.altitude_m),
            latitude_rad: latlon.latitude_rad,
            longitude_rad: latlon.longitude_rad,
            day_of_year,
            seconds_utc,
            f107: sim_config.f107,
            f107a: sim_config.f107a,
            ap: sim_config.ap,
        };
        let drag_area = config.drag_area(dynamics.mass);
        AtmosphericDrag.acceleration_with_model(
            &kinematics.position,
            &kinematics.velocity,
            &model,
            &input,
            drag_area,
            dynamics.mass,
        )
    };

    let srp = {
        let sun_pos = celestial
            .states
            .iter()
            .find(|s| s.naif_id == 10)
            .map(|s| s.position)
            .unwrap_or_else(|| Vector3::new(-apogee_common::constants::AU, 0.0, 0.0));
        SolarRadiationPressure.acceleration(
            &kinematics.position,
            &sun_pos,
            config.srp_area,
            Dimensionless::new(config.reflectivity),
            dynamics.mass,
        )
    };

    AggregatedForces {
        gravity,
        drag,
        srp,
        thrust: AccelerationVec::from_mps2(Vector3::zeros()),
        control_torque: TorqueVec::from_nm(Vector3::zeros()),
    }
}

/// Approximate geodetic latitude, longitude, and altitude from an inertial
/// position. This is a coarse spherical approximation sufficient for
/// atmosphere-model inputs in a first-pass 6DOF demo.
struct LatLonAlt {
    latitude_rad: f64,
    longitude_rad: f64,
    altitude_m: f64,
}

fn ecef_lat_lon_from_inertial(
    position: &Position,
    _day_of_year: u16,
    _seconds_utc: f64,
) -> LatLonAlt {
    let r = position.norm();
    let lat = position
        .z
        .atan2((position.x * position.x + position.y * position.y).sqrt());
    let lon = position.y.atan2(position.x);
    let alt = r - apogee_common::constants::R_EARTH_EQ;
    LatLonAlt {
        latitude_rad: lat,
        longitude_rad: lon,
        altitude_m: alt,
    }
}

#[cfg(test)]
mod tests {
    use apogee_common::constants::{GM_EARTH, R_EARTH_EQ};
    use apogee_common::units::{Area, Kilograms};
    use nalgebra::Vector3;

    use super::*;
    use crate::components::dynamics::Dynamics;

    fn make_iss_state() -> Kinematics {
        let r = R_EARTH_EQ + 408_000.0;
        let v = (GM_EARTH / r).sqrt();
        Kinematics {
            position: Vector3::new(r, 0.0, 0.0),
            velocity: Vector3::new(0.0, v, 0.0),
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: Vector3::zeros(),
        }
    }

    #[test]
    fn test_aggregate_forces_finite() {
        let kinematics = make_iss_state();
        let dynamics = Dynamics {
            mass: Kilograms::new(420_000.0),
            inertia: nalgebra::Matrix3::identity(),
            cg_offset: Vector3::zeros(),
        };
        let config = SpacecraftConfig {
            ballistic_coefficient: 1e-4,
            srp_area: Area::new(2_500.0),
            reflectivity: 1.2,
            reference_mass_kg: 420_000.0,
        };
        let sim_config = SimulationConfig::default();
        let celestial = SolarSystemState {
            states: vec![crate::ephemeris::kernel::BodyState {
                naif_id: 399,
                position: Vector3::zeros(),
                velocity: Vector3::zeros(),
            }],
        };
        let forces = aggregate_forces(
            &kinematics,
            &dynamics,
            &config,
            &sim_config,
            &celestial,
            80,
            12.0 * 3600.0,
        );
        let total = forces.total();
        assert!(total.raw().iter().all(|v| v.is_finite()));
        assert!(forces.gravity.raw().norm() > 0.0);
    }
}
