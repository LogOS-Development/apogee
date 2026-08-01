//! Magnetorquer model and control laws.
//!
//! Magnetorquers generate torque via `tau = m x B` where m is the magnetic
//! dipole moment commanded in body frame and B is the local geomagnetic field
//! vector in body frame. This module also includes classic B-dot detumble and
//! Y-momentum unloading laws.

use nalgebra::{UnitQuaternion, Vector3};

/// Magnetorquer configuration: dipole capacity per axis.
#[derive(Debug, Clone)]
pub struct MagnetorquerConfiguration {
    /// Maximum dipole moment per axis (A m^2).
    pub max_dipole_am2: f64,
    /// Minimum effective dipole step (A m^2). Commands below this are zeroed.
    pub resolution_am2: f64,
}

impl Default for MagnetorquerConfiguration {
    fn default() -> Self {
        Self {
            max_dipole_am2: 10.0,
            resolution_am2: 0.01,
        }
    }
}

/// Magnetorquer model.
#[derive(Debug, Clone, Default)]
pub struct MagnetorquerSet {
    pub config: MagnetorquerConfiguration,
}

impl MagnetorquerSet {
    /// Compute body torque from commanded dipole moment and body-frame B field.
    pub fn torque(
        &self,
        dipole_am2: &Vector3<f64>,
        b_body_t: &Vector3<f64>,
    ) -> Vector3<f64> {
        let m = self.quantize(dipole_am2);
        m.cross(b_body_t)
    }

    /// Saturate and quantize the dipole command.
    fn quantize(&self, m: &Vector3<f64>) -> Vector3<f64> {
        let mut out = *m;
        for v in out.iter_mut() {
            *v = if v.abs() < self.config.resolution_am2 {
                0.0
            } else {
                v.clamp(-self.config.max_dipole_am2, self.config.max_dipole_am2)
            };
        }
        out
    }
}

/// B-dot detumble controller. Commands dipole proportional to negative rate of
/// change of the body-frame magnetic field.
#[derive(Debug, Clone)]
pub struct BdotController {
    gain: f64,
    last_b_body: Option<Vector3<f64>>,
}

impl BdotController {
    pub fn new(gain_am2_per_t_s: f64) -> Self {
        Self {
            gain: gain_am2_per_t_s,
            last_b_body: None,
        }
    }

    /// Compute dipole command given current body-frame B field and dt since last call.
    pub fn compute(
        &mut self,
        b_body_t: &Vector3<f64>,
        dt: f64,
    ) -> Vector3<f64> {
        let dot = match &self.last_b_body {
            Some(prev) => {
                if dt > 0.0 {
                    (b_body_t - prev) / dt
                } else {
                    Vector3::zeros()
                }
            }
            None => Vector3::zeros(),
        };
        self.last_b_body = Some(*b_body_t);
        // Command dipole opposing dB/dt
        -dot * self.gain
    }

    pub fn reset(&mut self) {
        self.last_b_body = None;
    }
}

/// Y-wheel momentum dumping using the local B field.
///
/// Dumps excess angular momentum along a target body axis (commonly the orbit
/// normal / Y axis). The magnetorquer dipole is chosen so that `m x B` aligns
/// opposite to the excess momentum component.
#[derive(Debug, Clone)]
pub struct MomentumDumpController {
    config: MagnetorquerConfiguration,
    /// Body axis along which to dump momentum.
    pub dump_axis: Vector3<f64>,
    /// Gain: A m^2 per N m s of excess momentum.
    pub gain_am2_per_nms: f64,
}

impl MomentumDumpController {
    pub fn new(config: MagnetorquerConfiguration, dump_axis: &Vector3<f64>) -> Self {
        Self {
            config,
            dump_axis: dump_axis.normalize(),
            gain_am2_per_nms: 1.0,
        }
    }

    /// Compute dipole command given body-frame B and stored momentum H.
    pub fn compute(
        &self,
        b_body_t: &Vector3<f64>,
        body_momentum_nms: &Vector3<f64>,
    ) -> Vector3<f64> {
        let h_excess = body_momentum_nms.dot(&self.dump_axis) * self.dump_axis;
        // Desired torque opposite to excess momentum.
        let tau_desired = -h_excess * self.gain_am2_per_nms;
        // We want m x B = tau. A simple approximate solution: m perpendicular to both.
        if b_body_t.norm() < 1e-12 {
            return Vector3::zeros();
        }
        let m = b_body_t.cross(&tau_desired) / b_body_t.norm_squared();
        let mut out = m;
        for v in out.iter_mut() {
            *v = v.clamp(-self.config.max_dipole_am2, self.config.max_dipole_am2);
        }
        out
    }
}

/// Transform an inertial-frame geomagnetic field vector into body frame.
pub fn b_body_from_inertial(
    attitude: &UnitQuaternion<f64>,
    b_inertial_t: &Vector3<f64>,
) -> Vector3<f64> {
    attitude.inverse().transform_vector(b_inertial_t)
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use nalgebra::Vector3;

    use super::*;

    #[test]
    fn test_torque_perpendicular_to_b() {
        let mtq = MagnetorquerSet::default();
        let b = Vector3::new(0.0, 0.0, 3e-5);
        let m = Vector3::new(1.0, 0.0, 0.0);
        let tau = mtq.torque(&m, &b);
        // m x B should be along +Y.
        assert_relative_eq!(tau, Vector3::new(0.0, -3e-5, 0.0), epsilon = 1e-12);
    }

    #[test]
    fn test_bdot_opposes_field_rate() {
        let mut ctrl = BdotController::new(1.0);
        let b0 = Vector3::new(0.0, 0.0, 3e-5);
        ctrl.compute(&b0, 1.0); // first call initializes
        let b1 = Vector3::new(0.0, 0.0, 4e-5);
        let m = ctrl.compute(&b1, 1.0);
        // dBz/dt > 0, so command should oppose: negative z dipole.
        assert!(m.z < 0.0);
    }

    #[test]
    fn test_momentum_dump_opposes_excess() {
        let cfg = MagnetorquerConfiguration::default();
        let dump_axis = Vector3::new(0.0, 1.0, 0.0);
        let ctrl = MomentumDumpController::new(cfg, &dump_axis);
        let b = Vector3::new(0.0, 0.0, 3e-5);
        let h = Vector3::new(0.0, 1.0, 0.0);
        let m = ctrl.compute(&b, &h);
        let tau = m.cross(&b);
        // Torque should oppose +Y momentum.
        assert!(tau.y < 0.0);
    }
}
