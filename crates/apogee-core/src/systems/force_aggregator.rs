//! Force aggregator system: collects gravity + drag + SRP forces.
//!
//! Issue #150: drag and SRP are now per-component force models. The
//! aggregator takes `Option<&DragSurfaces>` and `Option<&SrpSurfaces>` —
//! `None` means the entity has no surfaces of that type and the force is
//! zero. The `SpacecraftConfig` struct has been replaced by
//! `SpacecraftDefinition` (a load-time blueprint, not a runtime component).

use apogee_common::units::{AccelerationVector, Kilograms, TorqueVector};
use apogee_common::Position;
use nalgebra::Vector3;

use crate::aero::model::AtmosphereModel;
use crate::components::drag_surfaces::DragSurfaces;
use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::{RigidBody, SimulationConfig};
use crate::components::srp_surfaces::SrpSurfaces;
use crate::gravity::{GravitySources, PointMassGravity};
use hifitime::Epoch;

/// Aggregated forces and torques on a body.
#[derive(Debug, Clone, Default)]
pub struct AggregatedForces {
    /// Gravitational acceleration (m/s^2).
    pub gravity: AccelerationVector,
    /// Atmospheric drag acceleration (m/s^2).
    pub drag: AccelerationVector,
    /// Solar radiation pressure acceleration (m/s^2).
    pub srp: AccelerationVector,
    /// Thrust acceleration (m/s^2).
    pub thrust: AccelerationVector,
    /// External control torque (N m), e.g. from reaction wheels or thrusters.
    pub control_torque: TorqueVector,
}

impl AggregatedForces {
    /// Sum all force contributions into total acceleration.
    pub fn total(&self) -> AccelerationVector {
        self.gravity + self.drag + self.srp + self.thrust
    }

    /// Sum all torque contributions (N m).
    pub fn torque(&self) -> TorqueVector {
        self.control_torque
    }
}

/// Control inputs (force + torque) to apply during a propagation step.
#[derive(Debug, Clone, Default)]
pub struct ControlInputs {
    /// Body-frame torque (N m).
    pub torque_nm: Vector3<f64>,
    /// Body-frame force (N). Converted to acceleration using the supplied mass.
    pub force_n: Vector3<f64>,
}

impl AggregatedForces {
    /// Apply control inputs, converting force to acceleration using `mass`.
    pub fn apply_control(&mut self, inputs: &ControlInputs, mass: Kilograms<f64>) {
        self.control_torque = TorqueVector::new(inputs.torque_nm);
        let accel = inputs.force_n / mass.into_value();
        self.thrust = AccelerationVector::new(accel);
    }
}

/// Compute all perturbative accelerations for a spacecraft.
///
/// This is the main force aggregation for Phase 1.6. It uses:
/// - point-mass gravity from all bodies in `gravity_sources`
/// - NRLMSISE-00 atmospheric drag via `DragSurfaces` (per-surface linear
///   superposition)
/// - solar radiation pressure via `SrpSurfaces` (per-surface linear
///   superposition)
///
/// `drag_surfaces` and `srp_surfaces` are `Option` — entities without
/// those components get zero drag/SRP acceleration. This is the
/// automatic skip: no zero-filled fields, no special-casing.
///
/// The epoch supplies `day_of_year` and `seconds_utc` to the atmosphere
/// model via hifitime's `day_of_year()` accessor (1-based, fractional).
/// Space-weather values are taken from `sim_config`.
///
/// `sun_position` is the inertial position of the Sun (NAIF ID 10), used
/// for SRP eclipse detection. If no Sun entity exists in the world, the
/// caller should pass a default position (SRP will fall back to a
/// heliocentric approximation).
#[allow(clippy::too_many_arguments)]
pub fn aggregate_forces(
    kinematics: &Kinematics,
    rigid_body: &RigidBody,
    drag_surfaces: Option<&DragSurfaces>,
    srp_surfaces: Option<&SrpSurfaces>,
    sim_config: &SimulationConfig,
    gravity_sources: &GravitySources,
    sun_position: Position,
    epoch: Epoch,
) -> AggregatedForces {
    let gravity = PointMassGravity
        .acceleration(&kinematics.position, gravity_sources)
        .unwrap_or_else(|_| AccelerationVector::new(Vector3::zeros()));

    let drag = if let Some(ds) = drag_surfaces {
        let doy_f64 = epoch.day_of_year();
        let day_of_year = doy_f64 as u16;
        let seconds_utc = (doy_f64 - doy_f64.floor()) * 86_400.0;

        let model = crate::aero::nrlmsise00::Nrlmsise00;
        let latlon = ecef_lat_lon_from_inertial(&kinematics.position);
        let input = crate::aero::model::AtmosphereInput {
            altitude_m: apogee_common::units::Meters::new(latlon.altitude_m),
            latitude_rad: latlon.latitude_rad,
            longitude_rad: latlon.longitude_rad,
            day_of_year,
            seconds_utc,
            f107: sim_config.f107,
            f107a: sim_config.f107a,
            ap: sim_config.ap,
        };
        let output = model.evaluate(&input);
        ds.drag_acceleration(
            &kinematics.position,
            &kinematics.velocity,
            &kinematics.attitude,
            output.density,
            rigid_body.mass,
        )
    } else {
        AccelerationVector::new(Vector3::zeros())
    };

    let srp = if let Some(ss) = srp_surfaces {
        ss.srp_acceleration(
            &kinematics.position,
            &sun_position,
            &kinematics.attitude,
            rigid_body.mass,
        )
    } else {
        AccelerationVector::new(Vector3::zeros())
    };

    AggregatedForces {
        gravity,
        drag,
        srp,
        thrust: AccelerationVector::new(Vector3::zeros()),
        control_torque: TorqueVector::new(Vector3::zeros()),
    }
}

/// Approximate geodetic latitude, longitude, and altitude from an inertial
/// position. This is a coarse spherical approximation sufficient for
/// atmosphere-model inputs in a first-pass 6DOF demo.
pub(crate) struct LatLonAlt {
    pub(crate) latitude_rad: f64,
    pub(crate) longitude_rad: f64,
    pub(crate) altitude_m: f64,
}

pub(crate) fn ecef_lat_lon_from_inertial(position: &Position) -> LatLonAlt {
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
    use crate::components::drag_surfaces::{DragSurface, DragSurfaces};
    use crate::components::kinematics::Kinematics;
    use crate::components::srp_surfaces::{SrpSurface, SrpSurfaces};
    use approx::assert_relative_eq;

    fn make_iss_components() -> (Kinematics, RigidBody, DragSurfaces, SrpSurfaces) {
        let r = R_EARTH_EQ + 408_000.0;
        let v = (GM_EARTH / r).sqrt();
        (
            Kinematics {
                position: Vector3::new(r, 0.0, 0.0),
                velocity: Vector3::new(0.0, v, 0.0),
                attitude: nalgebra::Quaternion::identity(),
                angular_velocity: Vector3::zeros(),
            },
            RigidBody {
                mass: Kilograms::new(420_000.0),
                inertia: nalgebra::Matrix3::identity(),
                cg_offset: Vector3::zeros(),
            },
            DragSurfaces::from_surfaces(vec![DragSurface::new(
                // ISS ballistic coefficient ~1e-4 m^2/kg, mass 420000 kg
                // → Cd*A = 42 m^2. Use Cd=2.2, A≈19 m^2.
                Area::new(19.0),
                2.2,
            )]),
            SrpSurfaces::from_surfaces(vec![SrpSurface::new(Area::new(2_500.0), 1.2)]),
        )
    }

    #[test]
    fn test_aggregate_forces_finite() {
        let (kin, rb, drag, srp) = make_iss_components();
        let sim_config = SimulationConfig::default();
        let gravity_sources = GravitySources {
            sources: vec![(GM_EARTH, Vector3::zeros())],
        };
        let sun_position = Vector3::new(-apogee_common::constants::AU, 0.0, 0.0);
        let epoch = Epoch::from_gregorian_utc(2026, 3, 21, 12, 0, 0, 0);
        let forces = aggregate_forces(
            &kin,
            &rb,
            Some(&drag),
            Some(&srp),
            &sim_config,
            &gravity_sources,
            sun_position,
            epoch,
        );
        let total = forces.total();
        assert!(total.raw().iter().all(|v| v.is_finite()));
        assert!(forces.gravity.raw().norm() > 0.0);
    }

    #[test]
    fn test_aggregate_forces_no_surfaces() {
        let (kin, rb, _, _) = make_iss_components();
        let sim_config = SimulationConfig::default();
        let gravity_sources = GravitySources {
            sources: vec![(GM_EARTH, Vector3::zeros())],
        };
        let sun_position = Vector3::new(-apogee_common::constants::AU, 0.0, 0.0);
        let epoch = Epoch::from_gregorian_utc(2026, 3, 21, 12, 0, 0, 0);
        let forces = aggregate_forces(
            &kin,
            &rb,
            None,
            None,
            &sim_config,
            &gravity_sources,
            sun_position,
            epoch,
        );
        assert_eq!(forces.drag.raw().norm(), 0.0);
        assert_eq!(forces.srp.raw().norm(), 0.0);
        assert!(forces.gravity.raw().norm() > 0.0);
    }

    #[test]
    fn test_apply_control_sets_torque_and_thrust() {
        let mut forces = AggregatedForces::default();
        let inputs = ControlInputs {
            torque_nm: Vector3::new(0.0, 0.0, 5.0),
            force_n: Vector3::new(10.0, 0.0, 0.0),
        };
        forces.apply_control(&inputs, Kilograms::new(500.0));
        assert_relative_eq!(forces.control_torque.vector.z, 5.0);
        assert_relative_eq!(forces.thrust.vector.x, 10.0 / 500.0);
    }

    #[test]
    fn test_total_sums_all_accelerations() {
        let forces = AggregatedForces {
            gravity: AccelerationVector::from_xyz(1.0, 0.0, 0.0),
            drag: AccelerationVector::from_xyz(0.0, 2.0, 0.0),
            srp: AccelerationVector::from_xyz(0.0, 0.0, 3.0),
            thrust: AccelerationVector::from_xyz(0.5, 0.5, 0.5),
            ..Default::default()
        };
        let total = forces.total();
        assert_relative_eq!(total.vector.x, 1.5);
        assert_relative_eq!(total.vector.y, 2.5);
        assert_relative_eq!(total.vector.z, 3.5);
    }
}
