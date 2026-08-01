//! Spherical harmonics gravity engine.
//!
//! Implements fully normalized associated Legendre functions and evaluates
//! the gravitational acceleration from a set of Stokes coefficients (C/S)
//! up to a configured degree and order.

use apogee_common::{constants::GM_EARTH, constants::R_EARTH_EQ, ApogeeError, ApogeeResult};
use nalgebra::Vector3;
use std::io::{BufRead, BufReader, Read};

/// Stokes coefficient set for spherical harmonics gravity.
#[derive(Debug, Clone)]
pub struct SphericalHarmonics {
    pub degree: usize,
    pub order: usize,
    /// Fully normalized cosine coefficients C[n][m].
    pub c: Vec<Vec<f64>>,
    /// Fully normalized sine coefficients S[n][m].
    pub s: Vec<Vec<f64>>,
    /// Reference radius used to non-dimensionalize the coefficients (m).
    pub reference_radius: f64,
    /// Gravitational parameter paired with these coefficients (m^3/s^2).
    pub gm: f64,
}

impl Default for SphericalHarmonics {
    fn default() -> Self {
        Self {
            degree: 0,
            order: 0,
            c: vec![],
            s: vec![],
            reference_radius: R_EARTH_EQ,
            gm: GM_EARTH,
        }
    }
}

impl SphericalHarmonics {
    /// Create an empty model with the given maximum degree and order.
    pub fn new(degree: usize, order: usize) -> Self {
        let mut c = Vec::with_capacity(degree + 1);
        let mut s = Vec::with_capacity(degree + 1);
        for n in 0..=degree {
            let m_max = n.min(order);
            c.push(vec![0.0; m_max + 1]);
            s.push(vec![0.0; m_max + 1]);
        }
        Self {
            degree,
            order,
            c,
            s,
            reference_radius: R_EARTH_EQ,
            gm: GM_EARTH,
        }
    }

    /// Load EGM2008 coefficients from a tide-free .gz or plain text file.
    ///
    /// The expected format is whitespace-separated lines:
    ///   degree order C S
    /// Lines outside the requested `degree`/`order` are ignored.
    pub fn load_egm2008(path: &str, degree: usize, order: usize) -> ApogeeResult<Self> {
        let file = std::fs::File::open(path)
            .map_err(|e| ApogeeError::Gravity(format!("failed to open EGM2008 file: {e}")))?;

        let reader: Box<dyn Read> = if path.ends_with(".gz") {
            Box::new(flate2::read::GzDecoder::new(file))
        } else {
            Box::new(file)
        };

        let mut model = Self::new(degree, order);
        for line in BufReader::new(reader).lines() {
            let line = line
                .map_err(|e| ApogeeError::Gravity(format!("failed to read EGM2008 line: {e}")))?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }
            let n: usize = parts[0]
                .parse()
                .map_err(|e| ApogeeError::Gravity(format!("invalid degree: {e}")))?;
            let m: usize = parts[1]
                .parse()
                .map_err(|e| ApogeeError::Gravity(format!("invalid order: {e}")))?;
            if n == 0 || n > degree || m > n.min(order) {
                continue;
            }
            let c_nm: f64 = parts[2]
                .parse()
                .map_err(|e| ApogeeError::Gravity(format!("invalid C coefficient: {e}")))?;
            let s_nm: f64 = parts[3]
                .parse()
                .map_err(|e| ApogeeError::Gravity(format!("invalid S coefficient: {e}")))?;
            model.c[n][m] = c_nm;
            model.s[n][m] = s_nm;
        }
        Ok(model)
    }

    /// Gravitational acceleration due to spherical harmonics at an inertial
    /// position, using the full configured degree and order.
    ///
    /// For this phase the position is assumed to already be in the body-fixed
    /// frame (no EOP rotation). The acceleration is returned in the same frame.
    pub fn acceleration(&self, position: &Vector3<f64>) -> ApogeeResult<Vector3<f64>> {
        self.acceleration_with_degree(position, self.degree, self.order)
    }

    /// Compute acceleration with a runtime-selected degree/order truncation.
    ///
    /// This is useful for adaptive harmonic selection: high-degree terms can be
    /// skipped at high altitudes where their contribution falls below a
    /// tolerance.
    pub fn acceleration_with_degree(
        &self,
        position: &Vector3<f64>,
        max_degree: usize,
        max_order: usize,
    ) -> ApogeeResult<Vector3<f64>> {
        let r = position.norm();
        if r == 0.0 {
            return Err(ApogeeError::Gravity(
                "singularity at origin in spherical harmonics".into(),
            ));
        }

        let effective_degree = max_degree.min(self.degree);
        let effective_order = max_order.min(self.order);

        let x = position.x;
        let y = position.y;
        let z = position.z;
        let rho = self.reference_radius / r;
        let sin_phi = z / r;
        let cos_phi = ((x * x + y * y).sqrt() / r).max(1e-15);
        let lambda = y.atan2(x);

        // Compute fully normalized associated Legendre P_nm(sin phi) and
        // their derivative with respect to phi.
        let (p, dp_dphi) = normalized_legendre(effective_degree, effective_order, sin_phi, cos_phi);

        // Spherical-coordinate acceleration components.
        let mut d_u_d_r = 0.0;
        let mut d_u_d_phi = 0.0;
        let mut d_u_d_lambda = 0.0;

        for n in 2..=effective_degree {
            let m_max = n.min(effective_order);
            let rho_n = rho.powi(n as i32);
            for m in 0..=m_max {
                let c = self.c[n][m];
                let s = self.s[n][m];
                let ml = m as f64 * lambda;
                let cos_ml = ml.cos();
                let sin_ml = ml.sin();
                let geopotential = c * cos_ml + s * sin_ml;
                let tangential = s * cos_ml - c * sin_ml;

                d_u_d_r += rho_n * (n as f64 + 1.0) * p[n][m] * geopotential;
                d_u_d_phi += rho_n * dp_dphi[n][m] * geopotential;
                d_u_d_lambda += rho_n * p[n][m] * m as f64 * tangential;
            }
        }

        // Potential U = -(GM/r) * (1 + sum ...). Acceleration = -∇U, so
        // outward-positive spherical components are the negatives of the
        // partial derivatives computed above.
        let gm_over_r2 = self.gm / (r * r);
        let a_r = -gm_over_r2 * (1.0 + d_u_d_r);
        let a_phi = gm_over_r2 * d_u_d_phi;
        let a_lambda = gm_over_r2 * d_u_d_lambda / cos_phi;

        // Convert spherical perturbation acceleration to Cartesian.
        let sin_lambda = lambda.sin();
        let cos_lambda = lambda.cos();

        let ax = a_r * cos_phi * cos_lambda - a_phi * sin_phi * cos_lambda - a_lambda * sin_lambda;
        let ay = a_r * cos_phi * sin_lambda - a_phi * sin_phi * sin_lambda + a_lambda * cos_lambda;
        let az = a_r * sin_phi + a_phi * cos_phi;

        Ok(Vector3::new(ax, ay, az))
    }

    /// Select an effective degree and order for a given radius ratio.
    ///
    /// `radius_ratio` is `reference_radius / distance_from_body_center`. High
    /// degrees are kept only while their nominal amplitude `ratio^(n+1)` is
    /// above `tolerance`. Order is clamped to the selected degree.
    ///
    /// Returns `(effective_degree, effective_order)` with order ≤ degree.
    pub fn select_degree_order(&self, radius_ratio: f64, tolerance: f64) -> (usize, usize) {
        if radius_ratio <= 0.0 || radius_ratio >= 1.0 || tolerance <= 0.0 {
            return (self.degree, self.order);
        }

        // Amplitude of degree-n term scales roughly as ratio^(n+1).
        // Solve ratio^(n+1) < tolerance for n.
        let log_ratio = radius_ratio.ln();
        let log_tol = tolerance.ln();
        let n_float = (log_tol / log_ratio) - 1.0;
        let selected_degree = (n_float.floor() as usize).clamp(2, self.degree);
        let selected_order = selected_degree.min(self.order);
        (selected_degree, selected_order)
    }
}

/// sqrt(3), used by the normalized Legendre recurrence.
const SQRT_3: f64 = 1.7320508075688772;

/// Compute fully normalized associated Legendre functions P_nm and their
/// derivatives dP/dφ using the forward column recursion.
///
/// Returns triangular arrays indexed as `p[n][m]` for 0 ≤ n ≤ degree,
/// 0 ≤ m ≤ min(n, order).
fn normalized_legendre(
    degree: usize,
    order: usize,
    sin_phi: f64,
    cos_phi: f64,
) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let mut p: Vec<Vec<f64>> = Vec::with_capacity(degree + 1);
    let mut dp: Vec<Vec<f64>> = Vec::with_capacity(degree + 1);

    for n in 0..=degree {
        let m_max = n.min(order);
        p.push(vec![0.0; m_max + 1]);
        dp.push(vec![0.0; m_max + 1]);
    }

    p[0][0] = 1.0;
    dp[0][0] = 0.0;
    if degree == 0 {
        return (p, dp);
    }

    // Degree 1.
    p[1][0] = SQRT_3 * sin_phi;
    dp[1][0] = SQRT_3 * cos_phi;
    if order >= 1 {
        p[1][1] = SQRT_3 * cos_phi;
        dp[1][1] = -SQRT_3 * sin_phi;
    }
    if degree == 1 {
        return (p, dp);
    }

    // Diagonal recurrence.
    let m_diag_max = degree.min(order);
    for m in 2..=m_diag_max {
        let factor = ((2 * m + 1) as f64 / (2 * m) as f64).sqrt();
        p[m][m] = factor * cos_phi * p[m - 1][m - 1];
        dp[m][m] = factor * (cos_phi * dp[m - 1][m - 1] - sin_phi * p[m - 1][m - 1]);
    }

    // Zonal and tesseral terms.
    for n in 2..=degree {
        let m_max = n.min(order);

        // m = 0 recurrence.
        let a0 = ((2 * n + 1) as f64 * (2 * n - 1) as f64).sqrt() / n as f64;
        let b0 = ((n - 1) as f64 / n as f64) * ((2 * n + 1) as f64 / (2 * n - 3) as f64).sqrt();
        p[n][0] = a0 * sin_phi * p[n - 1][0] - b0 * p[n - 2][0];
        dp[n][0] = a0 * (sin_phi * dp[n - 1][0] + cos_phi * p[n - 1][0]) - b0 * dp[n - 2][0];

        for m in 1..=m_max {
            if m == n {
                continue; // diagonal already computed
            }
            if m == n - 1 {
                // First sub-diagonal: P_{n,n-1} = sqrt(2n+1) sin(phi) P_{n-1,n-1}.
                let a = ((2 * n + 1) as f64).sqrt();
                p[n][m] = a * sin_phi * p[n - 1][m];
                dp[n][m] = a * (sin_phi * dp[n - 1][m] + cos_phi * p[n - 1][m]);
                continue;
            }
            let a = ((2 * n + 1) as f64 * (2 * n - 1) as f64 / ((n - m) * (n + m)) as f64).sqrt();
            let b = ((2 * n + 1) as f64 * (n - m - 1) as f64 * (n + m - 1) as f64
                / ((2 * n - 3) as f64 * (n - m) as f64 * (n + m) as f64))
                .sqrt();

            p[n][m] = a * sin_phi * p[n - 1][m] - b * p[n - 2][m];
            dp[n][m] = a * (sin_phi * dp[n - 1][m] + cos_phi * p[n - 1][m]) - b * dp[n - 2][m];
        }
    }

    (p, dp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// Closed-form total gravity (central + J2 perturbation) using the
    /// unnormalized J2 value derived from the fully normalized EGM2008 C_2,0.
    fn j2_closed_form(position: &Vector3<f64>, gm: f64, re: f64, c20: f64) -> Vector3<f64> {
        // Unnormalized J2 = -sqrt(5) * C_2,0.
        let j2 = -5.0_f64.sqrt() * c20;
        let r = position.norm();
        let r_ratio = re / r;

        // Central spherical gravity.
        let a_central = -gm / (r * r) * (position / r);

        // J2 perturbation.
        let factor = -1.5 * j2 * gm / (r * r) * r_ratio * r_ratio;
        let z_r = position.z / r;
        let z_r2 = z_r * z_r;

        let common_xy = 1.0 - 5.0 * z_r2;
        let common_z = 3.0 - 5.0 * z_r2;

        let a_j2 = factor
            * Vector3::new(
                position.x / r * common_xy,
                position.y / r * common_xy,
                position.z / r * common_z,
            );

        a_central + a_j2
    }

    #[test]
    fn test_j2_matches_closed_form_on_equator() {
        let mut model = SphericalHarmonics::new(2, 0);
        // EGM2008 tide-free fully normalized C_2,0.
        model.c[2][0] = -0.484169317386951e-03;
        let pos = Vector3::new(R_EARTH_EQ + 400_000.0, 0.0, 0.0);

        let a_sh = model.acceleration(&pos).unwrap();
        let a_closed = j2_closed_form(&pos, model.gm, model.reference_radius, model.c[2][0]);

        assert_relative_eq!(a_sh.x, a_closed.x, epsilon = 1e-9);
        assert_relative_eq!(a_sh.y, a_closed.y, epsilon = 1e-15);
        assert_relative_eq!(a_sh.z, a_closed.z, epsilon = 1e-15);
    }

    #[test]
    fn test_j2_matches_closed_form_on_z_axis() {
        let mut model = SphericalHarmonics::new(2, 0);
        model.c[2][0] = -0.484169317386951e-03;
        let pos = Vector3::new(0.0, 0.0, R_EARTH_EQ + 400_000.0);

        let a_sh = model.acceleration(&pos).unwrap();
        let a_closed = j2_closed_form(&pos, model.gm, model.reference_radius, model.c[2][0]);

        assert_relative_eq!(a_sh.x, a_closed.x, epsilon = 1e-12);
        assert_relative_eq!(a_sh.y, a_closed.y, epsilon = 1e-12);
        assert_relative_eq!(a_sh.z, a_closed.z, epsilon = 1e-9);
    }

    #[test]
    fn test_spherical_only_matches_central_gravity() {
        let model = SphericalHarmonics::new(1, 0);
        let pos = Vector3::new(R_EARTH_EQ + 400_000.0, 0.0, 0.0);
        let a = model.acceleration(&pos).unwrap();
        let expected = -GM_EARTH / pos.norm_squared();
        assert_relative_eq!(a.x, expected, epsilon = 1e-9);
        assert_relative_eq!(a.y, 0.0, epsilon = 1e-15);
        assert_relative_eq!(a.z, 0.0, epsilon = 1e-15);
    }

    #[test]
    fn test_singularity_at_origin() {
        let model = SphericalHarmonics::new(2, 0);
        assert!(model.acceleration(&Vector3::zeros()).is_err());
    }

    #[test]
    fn test_select_degree_order_reduces_with_altitude() {
        let model = SphericalHarmonics::new(70, 70);

        // LEO: ratio close to 1 -> keep high degree.
        let (deg, ord) = model.select_degree_order(0.95, 1e-9);
        assert!(deg >= 50, "expected high degree at LEO, got {deg}");
        assert_eq!(ord, deg);

        // GEO-like: ratio much smaller -> drop high degrees.
        let (deg, ord) = model.select_degree_order(0.15, 1e-9);
        assert!(deg < 20, "expected low degree at GEO, got {deg}");
        assert_eq!(ord, deg);

        // Deep space: ratio tiny -> keep only a few zonal terms.
        let (deg, ord) = model.select_degree_order(0.02, 1e-9);
        assert!(deg <= 10, "expected low degree in deep space, got {deg}");
        assert_eq!(ord, deg);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn test_degree_70_evaluates_under_50us() {
        let mut model = SphericalHarmonics::new(70, 70);
        for n in 2..=70 {
            let m_max = n.min(70);
            for m in 0..=m_max {
                model.c[n][m] = 1e-6 * ((n + m) as f64).sin();
                model.s[n][m] = 1e-6 * ((n - m) as f64).cos();
            }
        }
        let pos = Vector3::new(6_778_137.0, 0.0, 0.0);

        // Warmup.
        let _ = model.acceleration(&pos).unwrap();

        let iters = 100u64;
        let start = std::time::Instant::now();
        for _ in 0..iters {
            let _ = model.acceleration(&pos).unwrap();
        }
        let elapsed = start.elapsed();
        let per_call_us = elapsed.as_micros() as f64 / iters as f64;
        assert!(
            per_call_us < 50.0,
            "degree-70 spherical harmonics took {per_call_us:.1} us per call, target < 50 us"
        );
    }
}
