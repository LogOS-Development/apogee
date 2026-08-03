//! Extended Kalman filter for spacecraft attitude and gyro bias estimation.
//!
//! State: [q (4), beta (3)] where beta is the gyro bias in body frame (rad/s).
//! Measurement update uses a star tracker (attitude quaternion) and optionally
//! a magnetometer (body-frame magnetic field vector). Process model is a simple
//! gyro-driven quaternion propagation.

use apogee_common::units::Seconds;
use nalgebra::{DMatrix, DVector, Matrix3, Quaternion, UnitQuaternion, Vector3};

use crate::control::integrate_attitude;

/// EKF state: unit quaternion + gyro bias.
#[derive(Debug, Clone)]
pub struct AttitudeEstimate {
    /// Estimated body-to-inertial attitude.
    pub attitude: UnitQuaternion<f64>,
    /// Estimated gyro bias in body frame (rad/s).
    pub gyro_bias: Vector3<f64>,
    /// Error-state covariance (6x6): [theta error, bias error].
    pub covariance: Matrix6,
}

type Matrix6 = nalgebra::SMatrix<f64, 6, 6>;

/// Sensor noise parameters.
#[derive(Debug, Clone)]
pub struct EkfNoise {
    /// Gyro angular random walk (rad/s^0.5 per sqrt(Hz)).
    pub gyro_arw: f64,
    /// Gyro bias instability random walk (rad/s^1.5 per sqrt(Hz)).
    pub gyro_bias_rw: f64,
    /// Star tracker attitude noise (rad, 1-sigma per axis).
    pub star_tracker_sigma_rad: f64,
    /// Magnetometer vector noise (unitless, relative to measured unit vector).
    pub magnetometer_sigma: f64,
}

impl Default for EkfNoise {
    fn default() -> Self {
        Self {
            gyro_arw: 1e-4,
            gyro_bias_rw: 1e-6,
            star_tracker_sigma_rad: 1e-4,
            magnetometer_sigma: 1e-2,
        }
    }
}

/// Attitude + gyro-bias EKF.
#[derive(Debug, Clone)]
pub struct AttitudeEkf {
    estimate: AttitudeEstimate,
    noise: EkfNoise,
}

impl AttitudeEkf {
    pub fn new(initial: AttitudeEstimate, noise: EkfNoise) -> Self {
        Self {
            estimate: initial,
            noise,
        }
    }

    pub fn estimate(&self) -> &AttitudeEstimate {
        &self.estimate
    }

    /// Propagation step using gyro measurement `omega_meas` (body rad/s) over dt.
    pub fn predict(&mut self, omega_meas: &Vector3<f64>, dt: Seconds<f64>) {
        let dt_value = dt.into_value();
        let omega_est = omega_meas - self.estimate.gyro_bias;
        let q = self.estimate.attitude.quaternion();
        let q_next = integrate_attitude(q, &omega_est, dt);
        self.estimate.attitude = UnitQuaternion::from_quaternion(q_next);

        // Linearized state transition: F = I - [omega_x, -I; 0, 0] * dt
        let omega_cross = skew_symmetric(&omega_est);
        let mut f = Matrix6::identity();
        f.fixed_view_mut::<3, 3>(0, 0)
            .copy_from(&(-omega_cross * dt_value));
        f.fixed_view_mut::<3, 3>(0, 3)
            .copy_from(&(-Matrix3::identity() * dt_value));

        // Process noise Q
        let q_theta = self.noise.gyro_arw.powi(2) * dt_value
            + self.noise.gyro_bias_rw.powi(2) * dt_value.powi(3) / 3.0;
        let q_beta = self.noise.gyro_bias_rw.powi(2) * dt_value;
        let q_cross = self.noise.gyro_bias_rw.powi(2) * dt_value.powi(2) / 2.0;
        let mut q = Matrix6::zeros();
        q.fixed_view_mut::<3, 3>(0, 0)
            .copy_from(&(q_theta * Matrix3::identity()));
        q.fixed_view_mut::<3, 3>(3, 3)
            .copy_from(&(q_beta * Matrix3::identity()));
        q.fixed_view_mut::<3, 3>(0, 3)
            .copy_from(&(q_cross * Matrix3::identity()));
        q.fixed_view_mut::<3, 3>(3, 0)
            .copy_from(&(q_cross * Matrix3::identity()));

        self.estimate.covariance = f * self.estimate.covariance * f.transpose() + q;
    }

    /// Update from a star-tracker attitude quaternion measurement.
    pub fn update_star_tracker(&mut self, measured_q: &UnitQuaternion<f64>) {
        // Measurement residual z = 2 * vector(q_err)
        let q_err = self.estimate.attitude.inverse() * measured_q;
        let q = q_err.quaternion();
        let sign = if q.w < 0.0 { -1.0 } else { 1.0 };
        let z = Vector3::new(sign * q.i, sign * q.j, sign * q.k) * 2.0;

        // H maps theta error directly.
        let h = observation_matrix_star_tracker();
        let r = Matrix3::identity() * self.noise.star_tracker_sigma_rad.powi(2);

        self.apply_kalman_update(&z, &h, &r);
    }

    /// Update from body-frame magnetic field unit vector `b_body`.
    /// `b_inertial` must be the corresponding inertial-frame unit vector.
    pub fn update_magnetometer(&mut self, b_body: &Vector3<f64>, b_inertial: &Vector3<f64>) {
        let b_est = self
            .estimate
            .attitude
            .inverse()
            .transform_vector(b_inertial);
        let z = b_body - b_est;

        // H = [b_est_x, 0]
        let b_cross = skew_symmetric(&b_est);
        let mut h = DMatrix::zeros(3, 6);
        h.fixed_view_mut::<3, 3>(0, 0).copy_from(&b_cross);

        let r = Matrix3::identity() * self.noise.magnetometer_sigma.powi(2);
        self.apply_kalman_update(&z, &h, &r);
    }

    fn apply_kalman_update(&mut self, z: &Vector3<f64>, h: &DMatrix<f64>, r: &Matrix3<f64>) {
        // Use DMatrix arithmetic. H is 3x6, R is 3x3, P is 6x6.
        // S = H P H^T + R  (3x3)
        // K = P H^T S^-1    (6x3)
        // Joseph update: P+ = (I - K H) P (I - K H)^T + K R K^T (6x6).
        let h_dyn: DMatrix<f64> = h.clone();
        let p_dyn: DMatrix<f64> =
            DMatrix::from_row_slice(6, 6, self.estimate.covariance.as_slice());
        let r3: DMatrix<f64> = dmatrix_from_matrix3(r);

        let ht = h_dyn.transpose();
        let s: DMatrix<f64> = h_dyn.clone() * p_dyn.clone() * ht.clone() + r3.clone();
        let s_inv: DMatrix<f64> = s
            .clone()
            .try_inverse()
            .unwrap_or_else(|| DMatrix::identity(3, 3));

        let k: DMatrix<f64> = p_dyn.clone() * ht * s_inv.clone();
        let z_dyn = DVector::from_row_slice(&[z.x, z.y, z.z]);
        let dx: DVector<f64> = k.clone() * z_dyn;

        let dx_top = Vector3::new(dx[0], dx[1], dx[2]);
        let dx_bot = Vector3::new(dx[3], dx[4], dx[5]);

        // Apply attitude error reset.
        let mut dq = Quaternion::new(1.0, dx_top.x * 0.5, dx_top.y * 0.5, dx_top.z * 0.5);
        dq = dq.normalize();
        let q_new = self.estimate.attitude.quaternion() * dq;
        self.estimate.attitude = UnitQuaternion::from_quaternion(q_new.normalize());
        self.estimate.gyro_bias += dx_bot;

        // Joseph-form covariance update.
        let i6 = DMatrix::identity(6, 6);
        let kh: DMatrix<f64> = k.clone() * h_dyn.clone();
        let i_kh: DMatrix<f64> = i6 - kh.clone();
        let first: DMatrix<f64> = i_kh.clone() * p_dyn.clone();
        let first: DMatrix<f64> = first * i_kh.transpose();
        // K R K^T where K is 6x3, R is 3x3 -> result 6x6.
        let kt = k.transpose();
        let second: DMatrix<f64> = k.clone() * r3.clone();
        let second: DMatrix<f64> = second * kt;
        let p_new: DMatrix<f64> = first + second;
        self.estimate.covariance = Matrix6::from_row_slice(p_new.as_slice());
    }
}

fn skew_symmetric(v: &Vector3<f64>) -> Matrix3<f64> {
    Matrix3::new(0.0, -v.z, v.y, v.z, 0.0, -v.x, -v.y, v.x, 0.0)
}

fn observation_matrix_star_tracker() -> DMatrix<f64> {
    let mut h = DMatrix::zeros(3, 6);
    h.fixed_view_mut::<3, 3>(0, 0)
        .copy_from(&Matrix3::identity());
    h
}

fn dmatrix_from_matrix3(r: &Matrix3<f64>) -> DMatrix<f64> {
    DMatrix::from_row_slice(3, 3, r.as_slice())
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use nalgebra::{Matrix6, UnitQuaternion, Vector3};

    use super::*;

    #[test]
    fn test_ekf_predict_conserves_attitude_norm() {
        let est = AttitudeEstimate {
            attitude: UnitQuaternion::identity(),
            gyro_bias: Vector3::zeros(),
            covariance: Matrix6::identity() * 1e-3,
        };
        let mut ekf = AttitudeEkf::new(est, EkfNoise::default());
        ekf.predict(&Vector3::new(0.01, 0.0, 0.0), Seconds::new(1.0));
        assert_relative_eq!(ekf.estimate().attitude.norm(), 1.0, epsilon = 1e-9);
    }

    #[test]
    fn test_ekf_update_reduces_covariance() {
        let est = AttitudeEstimate {
            attitude: UnitQuaternion::identity(),
            gyro_bias: Vector3::zeros(),
            covariance: Matrix6::identity() * 0.1,
        };
        let mut ekf = AttitudeEkf::new(est, EkfNoise::default());
        let measured = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), 1e-3);
        let p0 = ekf.estimate().covariance[(0, 0)];
        ekf.update_star_tracker(&measured);
        let p1 = ekf.estimate().covariance[(0, 0)];
        assert!(p1 < p0, "covariance did not reduce: p0={} p1={}", p0, p1);
    }
}
