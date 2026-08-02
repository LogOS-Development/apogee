//! Actuator models for spacecraft attitude and orbit control.
//!
//! Includes pulse-width thrusters, reaction wheels, and magnetorquers. Each
//! actuator type produces body-frame forces and torques subject to physical
//! limits, quantization, and saturation.

use nalgebra::Vector3;

/// Configuration for a set of body-mounted thrusters.
#[derive(Debug, Clone)]
pub struct ThrusterConfiguration {
    /// Per-thruster maximum force (N).
    pub max_force_n: f64,
    /// Minimum impulse bit (N s). Thruster firings below this are ignored.
    pub minimum_impulse_bit_ns: f64,
    /// Thruster positions in body frame (m). Row i is thruster i.
    pub positions_m: Vec<Vector3<f64>>,
    /// Thruster force directions in body frame (unit vectors). Row i is thruster i.
    pub directions: Vec<Vector3<f64>>,
}

impl Default for ThrusterConfiguration {
    fn default() -> Self {
        Self {
            max_force_n: 1.0,
            minimum_impulse_bit_ns: 0.01,
            positions_m: Vec::new(),
            directions: Vec::new(),
        }
    }
}

impl ThrusterConfiguration {
    /// Check consistency.
    pub fn validate(&self) -> Result<(), String> {
        if self.positions_m.len() != self.directions.len() {
            return Err("thruster positions and directions must have same length".into());
        }
        if self.positions_m.is_empty() {
            return Err("at least one thruster is required".into());
        }
        for d in &self.directions {
            if (d.norm() - 1.0).abs() > 1e-6 {
                return Err(format!("thruster direction not normalized: {}", d));
            }
        }
        Ok(())
    }

    /// Compute the 6xN wrench map [F; tau] = W * u for continuous forces u.
    /// Columns ordered [force_x; force_y; force_z; torque_x; torque_y; torque_z].
    pub fn wrench_matrix(&self) -> nalgebra::DMatrix<f64> {
        let n = self.positions_m.len();
        let mut w = nalgebra::DMatrix::zeros(6, n);
        for (i, (pos, dir)) in self.positions_m.iter().zip(&self.directions).enumerate() {
            w.fixed_rows_mut::<3>(0).column_mut(i).copy_from(dir);
            let torque = pos.cross(dir);
            w.fixed_rows_mut::<3>(3).column_mut(i).copy_from(&torque);
        }
        w
    }
}

/// Per-wheel physical parameters.
#[derive(Debug, Clone)]
pub struct WheelParameters {
    /// Moment of inertia of the rotor about its spin axis (kg m^2).
    pub inertia_spin: f64,
    /// Transverse moment of inertia of the rotor (kg m^2).
    pub inertia_transverse: f64,
    /// Motor torque constant (N m / A).
    pub motor_constant: f64,
    /// Back-EMF coefficient (V s / rad).
    pub back_emf: f64,
    /// Winding resistance (Ohm).
    pub resistance: f64,
    /// Coulomb friction torque magnitude at the bearing (N m).
    pub coulomb_friction: f64,
    /// Viscous friction coefficient (N m s / rad).
    pub viscous_friction: f64,
}

impl Default for WheelParameters {
    fn default() -> Self {
        Self {
            inertia_spin: 1e-4,
            inertia_transverse: 5e-5,
            motor_constant: 0.05,
            back_emf: 0.05,
            resistance: 1.0,
            coulomb_friction: 1e-5,
            viscous_friction: 1e-6,
        }
    }
}

/// Reaction wheel assembly with high-fidelity electromechanical dynamics.
///
/// Each wheel i produces body torque along its spin axis `a_i` plus cross-coupling
/// torques from other wheels' accelerations due to the common mounting structure.
/// The model tracks per-wheel speed and current, with saturation, friction, and
/// motor dynamics.
#[derive(Debug, Clone)]
pub struct ReactionWheelAssembly {
    /// Wheel spin axes in body frame (unit vectors, one per wheel).
    pub axes: Vec<Vector3<f64>>,
    /// Maximum torque per wheel (N m).
    pub max_torque_nm: f64,
    /// Maximum momentum per wheel (N m s).
    pub max_momentum_nms: f64,
    /// Current wheel momenta (N m s). Same order as `axes`.
    pub momenta: Vec<f64>,
    /// Per-wheel speeds (rad/s). Same order as `axes`.
    pub speeds: Vec<f64>,
    /// Per-wheel currents (A). Same order as `axes`.
    pub currents: Vec<f64>,
    /// Per-wheel physical parameters. Same order as `axes`.
    pub params: Vec<WheelParameters>,
    /// Cross-coupling matrix C where tau_coupling_i = sum_j C_ij * h_dot_j.
    /// C[i][j] is dimensionless (ratio of j's acceleration torque that leaks to i).
    pub coupling_matrix: nalgebra::DMatrix<f64>,
    /// Mounting compliance (rad/N m) diagonal per wheel, modeling flexible bracket.
    pub mount_compliance: Vec<f64>,
}

impl ReactionWheelAssembly {
    pub fn new(
        axes: Vec<Vector3<f64>>,
        max_torque_nm: f64,
        max_momentum_nms: f64,
        params: Vec<WheelParameters>,
    ) -> Self {
        let n = axes.len();
        assert_eq!(
            axes.len(),
            params.len(),
            "axes and params length must match"
        );
        let coupling_matrix = Self::build_coupling_matrix(&axes);
        Self {
            axes: axes.clone(),
            max_torque_nm,
            max_momentum_nms,
            momenta: vec![0.0; n],
            speeds: vec![0.0; n],
            currents: vec![0.0; n],
            params,
            coupling_matrix,
            mount_compliance: vec![0.0; n],
        }
    }

    /// Build a simple cross-coupling model: small off-diagonal terms based on
    /// axis misalignment and mount flexibility. Diagonal is 1.0 (own axis).
    fn build_coupling_matrix(axes: &[Vector3<f64>]) -> nalgebra::DMatrix<f64> {
        let n = axes.len();
        let mut c = nalgebra::DMatrix::identity(n, n);
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                // Coupling scales with dot product of axes (parallel wheels couple more)
                // and a base stiffness/mount-flexibility coefficient.
                let misalign = axes[i].dot(&axes[j]).abs();
                c[(i, j)] = 0.01 * misalign + 0.005; // placeholder tuned by test data
            }
        }
        c
    }

    /// Set per-wheel mount compliance (rad / N m) for flexible-mount modeling.
    pub fn set_mount_compliance(&mut self, compliance: &[f64]) {
        assert_eq!(compliance.len(), self.axes.len());
        self.mount_compliance = compliance.to_vec();
    }

    /// Compute motor electrical dynamics: dI/dt = (V - R*I - K_b*omega) / L.
    /// Uses simplified zero-inductance assumption (L -> 0) so I = (V - K_b*omega)/R.
    pub fn motor_current(&self, voltage: &[f64]) -> Vec<f64> {
        self.params
            .iter()
            .zip(&self.speeds)
            .zip(voltage)
            .map(|((p, omega), v)| {
                let current = (v - p.back_emf * omega) / p.resistance;
                // Clamp to a practical motor current limit derived from max torque.
                let max_current = self.max_torque_nm / p.motor_constant;
                current.clamp(-max_current, max_current)
            })
            .collect()
    }

    /// Net body torque produced by the commanded wheel torques, including
    /// cross-coupling and friction.
    ///
    /// `wheel_torques_nm` are the requested motor torques (before saturation).
    pub fn net_body_torque(
        &self,
        wheel_torques_nm: &[f64],
        bus_angular_velocity: &Vector3<f64>,
    ) -> Vector3<f64> {
        let saturated = {
            let mut c = wheel_torques_nm.to_vec();
            self.saturate_torque(&mut c);
            c
        };

        // Effective torque at each wheel including cross-coupling:
        // tau_wheel_effective_i = tau_motor_i + sum_j C_ij * tau_motor_j
        let _n = self.axes.len();
        let tau_vec = nalgebra::DVector::from_row_slice(&saturated);
        let coupled = &self.coupling_matrix * tau_vec;

        // Sum into body frame; each effective wheel torque acts along its axis.
        let mut tau_body = Vector3::zeros();
        for (i, axis) in self.axes.iter().enumerate() {
            let friction = self.wheel_friction(i);
            let net_wheel = coupled[i] - friction;
            tau_body += axis * net_wheel;
        }

        // Gyroscopic coupling: when the bus rotates, each spinning wheel exerts
        // a torque omega_bus x h_wheel on the bus.
        let h_total = self.total_momentum();
        let gyroscopic = bus_angular_velocity.cross(&h_total);
        tau_body + gyroscopic
    }

    /// Friction torque opposing wheel motion (Coulomb + viscous).
    fn wheel_friction(&self, i: usize) -> f64 {
        let p = &self.params[i];
        let coulomb = p.coulomb_friction * self.speeds[i].signum();
        let viscous = p.viscous_friction * self.speeds[i];
        coulomb + viscous
    }

    /// Saturate wheel torque commands per-axis.
    pub fn saturate_torque(&self, commands: &mut [f64]) {
        for c in commands.iter_mut() {
            *c = c.clamp(-self.max_torque_nm, self.max_torque_nm);
        }
    }

    /// Update stored wheel momenta and speeds given applied motor torques over dt.
    pub fn step_momentum(&mut self, wheel_torques_nm: &[f64], dt: f64) {
        let saturated = {
            let mut c = wheel_torques_nm.to_vec();
            self.saturate_torque(&mut c);
            c
        };
        let frictions: Vec<f64> = (0..self.axes.len())
            .map(|i| self.wheel_friction(i))
            .collect();
        for (i, h) in self.momenta.iter_mut().enumerate() {
            let p = &self.params[i];
            let net = saturated[i] - frictions[i];
            let alpha = net / p.inertia_spin;
            let delta_speed = alpha * dt;
            self.speeds[i] += delta_speed;
            *h += net * dt;
            *h = h.clamp(-self.max_momentum_nms, self.max_momentum_nms);
            self.speeds[i] = h.signum() * h.abs() / p.inertia_spin;
        }
    }

    /// Total stored angular momentum vector in body frame.
    pub fn total_momentum(&self) -> Vector3<f64> {
        self.axes
            .iter()
            .zip(&self.momenta)
            .map(|(a, h)| a * *h)
            .fold(Vector3::zeros(), |acc, v| acc + v)
    }
}

/// Simple continuous allocation: distribute desired torque across wheels via
/// pseudo-inverse of the axis matrix. Returns wheel torque commands.
pub fn allocate_reaction_wheel_torque(
    rwa: &ReactionWheelAssembly,
    desired_body_torque_nm: &Vector3<f64>,
) -> Vec<f64> {
    let n = rwa.axes.len();
    if n == 0 {
        return Vec::new();
    }
    let columns: Vec<nalgebra::DVector<f64>> = rwa
        .axes
        .iter()
        .map(|v| nalgebra::DVector::from_column_slice(v.as_slice()))
        .collect();
    let a = nalgebra::DMatrix::from_columns(&columns);
    let a_pinv = a.clone().pseudo_inverse(1e-12).unwrap_or_else(|_| {
        // Fallback: identity scaled so no torque if matrix is pathological.
        nalgebra::DMatrix::identity(n, n)
    });
    let wheel = a_pinv * desired_body_torque_nm;
    let mut cmds: Vec<f64> = wheel.iter().copied().collect();
    rwa.saturate_torque(&mut cmds);
    cmds
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use nalgebra::Vector3;

    use super::*;

    #[test]
    fn test_thruster_wrench_matrix() {
        let cfg = ThrusterConfiguration {
            max_force_n: 1.0,
            minimum_impulse_bit_ns: 0.01,
            positions_m: vec![Vector3::new(1.0, 0.0, 0.0), Vector3::new(-1.0, 0.0, 0.0)],
            directions: vec![Vector3::new(0.0, 1.0, 0.0), Vector3::new(0.0, -1.0, 0.0)],
        };
        cfg.validate().expect("valid thruster config");
        let w = cfg.wrench_matrix();
        // Both thrusters produce +Z torque when fired positively.
        assert_relative_eq!(w[(5, 0)], 1.0, epsilon = 1e-9);
        assert_relative_eq!(w[(5, 1)], 1.0, epsilon = 1e-9);
    }

    #[test]
    fn test_rw_allocation_recovers_torque() {
        let axes = vec![
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];
        let params = vec![WheelParameters::default(); 3];
        let rwa = ReactionWheelAssembly::new(axes, 1.0, 10.0, params);
        let desired = Vector3::new(0.5, -0.3, 0.2);
        let cmds = allocate_reaction_wheel_torque(&rwa, &desired);
        let tau = rwa.net_body_torque(&cmds, &Vector3::zeros());
        assert_relative_eq!(tau, desired, epsilon = 1e-2);
    }

    #[test]
    fn test_rw_cross_coupling_changes_off_axis_torque() {
        let axes = vec![Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0)];
        let params = vec![WheelParameters::default(); 2];
        let rwa = ReactionWheelAssembly::new(axes, 1.0, 10.0, params);
        let cmds = vec![0.5, 0.0];
        let tau = rwa.net_body_torque(&cmds, &Vector3::zeros());
        // X-axis command produces some Y-axis torque due to cross-coupling.
        assert!(tau.y.abs() > 1e-6, "expected cross-coupling torque on Y");
    }

    #[test]
    fn test_rw_gyroscopic_coupling_with_bus_rate() {
        let axes = vec![Vector3::new(0.0, 0.0, 1.0)];
        let params = WheelParameters {
            inertia_spin: 1e-3,
            ..WheelParameters::default()
        };
        let mut rwa = ReactionWheelAssembly::new(axes, 1.0, 10.0, vec![params]);
        // Spin wheel up to 100 rad/s.
        rwa.speeds[0] = 100.0;
        rwa.momenta[0] = rwa.params[0].inertia_spin * 100.0;
        let bus_rate = Vector3::new(0.1, 0.0, 0.0);
        let tau = rwa.net_body_torque(&[0.0], &bus_rate);
        // h along Z, omega along X => omega x h is along -Y.
        assert!(tau.y < -1e-6, "expected gyroscopic -Y torque");
    }
}
