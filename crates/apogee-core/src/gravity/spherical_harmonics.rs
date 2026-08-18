//! Spherical harmonics gravity engine.
//!
//! Implements fully normalized associated Legendre functions and evaluates
//! the gravitational acceleration from a set of Stokes coefficients (C/S)
//! up to a configured degree and order.

use apogee_common::units::AccelerationVector;
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

    /// Create a J2-only spherical harmonics model for Earth.
    ///
    /// Convenience constructor that sets the C_2,0 coefficient to the EGM2008
    /// tide-free fully normalized value. All other coefficients are zero.
    /// The model uses Earth's GM and equatorial radius as defaults.
    ///
    /// Unnormalized J2 = -sqrt(5) * C_2,0 ≈ 1.08263e-3.
    pub fn j2_only() -> Self {
        let mut model = Self::new(2, 0);
        // EGM2008 tide-free fully normalized C_2,0.
        model.c[2][0] = -0.484165143790815e-03;
        model
    }

    /// Load EGM2008 coefficients from a tide-free .gz, ICGEM .gfc,
    /// or plain text file.
    ///
    /// Accepted coefficient line formats:
    ///   `degree order C S`
    ///   `gfc degree order C S [sigma_C] [sigma_S]`
    ///
    /// ICGEM `.gfc` files with a header section are handled automatically:
    /// lines before `end_of_head` are scanned for `earth_gravity_constant`
    /// and `radius` metadata, then skipped. Fortran double-precision
    /// notation (`1.0d0`, `1.0d-03`) in coefficient values is converted
    /// to standard `e` notation before parsing.
    ///
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
        let mut past_header = false;

        for line in BufReader::new(reader).lines() {
            let line = line
                .map_err(|e| ApogeeError::Gravity(format!("failed to read EGM2008 line: {e}")))?;
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // ICGEM .gfc header: scan for metadata, then skip until end_of_head.
            if !past_header {
                if trimmed.starts_with("end_of_head") {
                    past_header = true;
                    continue;
                }
                // Extract header metadata.
                if let Some(val) = Self::parse_header_value(trimmed, "earth_gravity_constant") {
                    if let Ok(gm) = parse_fortran_f64(val) {
                        model.gm = gm;
                    }
                }
                if let Some(val) = Self::parse_header_value(trimmed, "radius") {
                    if let Ok(r) = parse_fortran_f64(val) {
                        model.reference_radius = r;
                    }
                }
                // If we haven't hit end_of_head, skip this line (it's header).
                // But allow plain-format files (no header) to proceed: if the
                // line looks like a coefficient line, process it.
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let is_coeff_line = if parts.len() >= 5 && parts[0].eq_ignore_ascii_case("gfc") {
                    parts[1].parse::<usize>().is_ok()
                } else if parts.len() >= 4 {
                    parts[0].parse::<usize>().is_ok()
                } else {
                    false
                };
                if !is_coeff_line {
                    continue;
                }
                // Falls through to coefficient parsing below.
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            // Allow both plain "degree order C S" and ICGEM "gfc degree order C S ...".
            let (n, m, c_nm, s_nm) = if parts.len() >= 5 && parts[0].eq_ignore_ascii_case("gfc") {
                let n: usize = parts[1]
                    .parse()
                    .map_err(|e| ApogeeError::Gravity(format!("invalid degree: {e}")))?;
                let m: usize = parts[2]
                    .parse()
                    .map_err(|e| ApogeeError::Gravity(format!("invalid order: {e}")))?;
                let c_nm: f64 = parse_fortran_f64(parts[3])
                    .map_err(|e| ApogeeError::Gravity(format!("invalid C coefficient: {e}")))?;
                let s_nm: f64 = parse_fortran_f64(parts[4])
                    .map_err(|e| ApogeeError::Gravity(format!("invalid S coefficient: {e}")))?;
                (n, m, c_nm, s_nm)
            } else {
                let n: usize = parts[0]
                    .parse()
                    .map_err(|e| ApogeeError::Gravity(format!("invalid degree: {e}")))?;
                let m: usize = parts[1]
                    .parse()
                    .map_err(|e| ApogeeError::Gravity(format!("invalid order: {e}")))?;
                let c_nm: f64 = parse_fortran_f64(parts[2])
                    .map_err(|e| ApogeeError::Gravity(format!("invalid C coefficient: {e}")))?;
                let s_nm: f64 = parse_fortran_f64(parts[3])
                    .map_err(|e| ApogeeError::Gravity(format!("invalid S coefficient: {e}")))?;
                (n, m, c_nm, s_nm)
            };
            if n == 0 || n > degree || m > n.min(order) {
                continue;
            }
            model.c[n][m] = c_nm;
            model.s[n][m] = s_nm;
        }
        Ok(model)
    }

    /// Extract the value portion of a `key value` header line from an ICGEM
    /// `.gfc` file. Returns `None` if the line doesn't start with `key`.
    fn parse_header_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(key) {
            let val = rest.trim_start();
            // Value extends to the first whitespace.
            let end = val.find(char::is_whitespace).unwrap_or(val.len());
            Some(&val[..end])
        } else {
            None
        }
    }

    /// Gravitational acceleration due to spherical harmonics at an inertial
    /// position, using the full configured degree and order.
    ///
    /// For this phase the position is assumed to already be in the body-fixed
    /// frame (no EOP rotation). The acceleration is returned in the same frame,
    /// wrapped in [`AccelerationVector`] so the m/s² unit tag is visible at the
    /// public API surface.
    pub fn acceleration(&self, position: &Vector3<f64>) -> ApogeeResult<AccelerationVector> {
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
    ) -> ApogeeResult<AccelerationVector> {
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

        Ok(AccelerationVector::new(Vector3::new(ax, ay, az)))
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

/// Parse an f64 from a string that may use Fortran double-precision notation
/// (`d0`, `d-03`, `d+05`) in addition to standard Rust notation (`e0`, `e-03`).
///
/// The Fortran `d` exponent marker is replaced with `e` before delegating to
/// `f64::from_str`. This handles values like `1.0d0`, `-0.484...d-03`,
/// `0.398...E+15`, and standard `1.5e-9`.
fn parse_fortran_f64(s: &str) -> Result<f64, std::num::ParseFloatError> {
    // Fast path: if there's no 'd' or 'D', parse directly.
    if !s.contains(['d', 'D']) {
        return s.parse::<f64>();
    }
    // Replace Fortran d/D exponent marker with e.
    let fixed: String = s
        .char_indices()
        .map(|(i, c)| {
            if c == 'd' || c == 'D' {
                // Only replace if preceded by a digit (it's an exponent marker,
                // not part of a hex string or identifier).
                if i > 0 && s.as_bytes()[i - 1].is_ascii_digit() {
                    return 'e';
                }
            }
            c
        })
        .collect();
    fixed.parse::<f64>()
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
    use std::path::PathBuf;

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

        let a_sh = *model.acceleration(&pos).unwrap().raw();
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

        let a_sh = *model.acceleration(&pos).unwrap().raw();
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
        assert_relative_eq!(a.raw().x, expected, epsilon = 1e-9);
        assert_relative_eq!(a.raw().y, 0.0, epsilon = 1e-15);
        assert_relative_eq!(a.raw().z, 0.0, epsilon = 1e-15);
    }

    #[test]
    fn test_singularity_at_origin() {
        let model = SphericalHarmonics::new(2, 0);
        assert!(model.acceleration(&Vector3::zeros()).is_err());
    }

    #[test]
    fn test_tesseral_c22_longitude_rotation() {
        // C_2,2 produces an acceleration pattern that rotates with the input
        // longitude. A 90-degree rotation around z should rotate the acceleration
        // by 90 degrees.
        let mut model = SphericalHarmonics::new(2, 2);
        model.c[2][2] = 1.0e-6;
        let r = R_EARTH_EQ + 400_000.0;

        let pos_a = Vector3::new(r, 0.0, 0.0); // λ = 0
        let pos_b = Vector3::new(0.0, r, 0.0); // λ = π/2

        let a_a = model.acceleration(&pos_a).unwrap();
        let a_b = model.acceleration(&pos_b).unwrap();

        // The central gravity is the same at both equatorial points; the C_2,2
        // perturbation is longitude-dependent. Because of the cos(2λ) behavior,
        // the perturbations at λ=0 and λ=π/2 are equal and opposite in the
        // radial-inward direction, so the sum of the inward components equals
        // twice the central acceleration.
        let central = -GM_EARTH / r.powi(2);
        assert_relative_eq!(a_a.raw().x + a_b.raw().y, 2.0 * central, epsilon = 1e-5);
        assert_relative_eq!(a_a.raw().z, 0.0, epsilon = 1e-12);
        assert_relative_eq!(a_b.raw().z, 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_load_egm2008_parses_coefficients() {
        // Synthetic EGM2008-style content.
        let content = "2 0 -4.84169317386951e-04 0.0\n\
                       2 1 -1.86987640000000e-10 1.19528012000000e-09\n\
                       2 2 2.43926074800000e-06 -1.40027358800000e-06\n\
                       3 0 9.57194713000000e-07 0.0\n";
        let dir = std::env::temp_dir().join("apogee_egm2008_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_egm2008.txt");
        std::fs::write(&path, content).unwrap();

        let model = SphericalHarmonics::load_egm2008(path.to_str().unwrap(), 3, 2).unwrap();
        assert_relative_eq!(model.c[2][0], -4.84169317386951e-04, epsilon = 1e-15);
        assert_relative_eq!(model.s[2][2], -1.400273588e-06, epsilon = 1e-15);
        assert_relative_eq!(model.c[3][0], 9.57194713e-07, epsilon = 1e-15);
        // Order 2 line for degree 3 should be stored.
        assert_relative_eq!(model.c[3][2], 0.0, epsilon = 1e-15);
    }

    #[test]
    fn test_higher_degree_perturbation_is_smooth() {
        // Adding a small degree-4 zonal term changes the radial acceleration
        // continuously; the difference should scale like (Re/r)^4.
        let mut model2 = SphericalHarmonics::new(2, 0);
        let mut model4 = SphericalHarmonics::new(4, 0);
        model2.c[2][0] = -0.484169317386951e-03;
        model4.c[2][0] = model2.c[2][0];
        model4.c[4][0] = -5.79905313000000e-07;

        let pos = Vector3::new(0.0, 0.0, R_EARTH_EQ + 400_000.0);
        let a2 = model2.acceleration(&pos).unwrap();
        let a4 = model4.acceleration(&pos).unwrap();

        // Difference is radial and orders of magnitude smaller than central.
        let diff = (a4.raw() - a2.raw()).norm();
        assert!(
            diff > 1e-12 && diff < 1e-2,
            "unexpected J4-scale difference: {diff}"
        );
        assert_relative_eq!((a4.raw() - a2.raw()).x, 0.0, epsilon = 1e-15);
        assert_relative_eq!((a4.raw() - a2.raw()).y, 0.0, epsilon = 1e-15);
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

    // ---- ICGEM .gfc format tests ----

    /// ICGEM .gfc file with a realistic header, `end_of_head` marker,
    /// and Fortran `d0` scientific notation in coefficient values.
    const SAMPLE_GFC: &str = "\
product_type                gravity_field\n\
modelname                   EGM2008\n\
earth_gravity_constant      0.3986004415E+15\n\
radius                      0.63781363E+07\n\
max_degree                  2190\n\
errors                      calibrated\n\
norm                        fully_normalized\n\
tide_system                 tide_free\n\
\n\
key     L    M             C                       S                    sigma C             sigma S\n\
end_of_head ============================================================================================\n\
gfc     0    0    1.0d0                    0.0d0                    0.0d0               0.0d0\n\
gfc     2    0   -0.484165143790815e-03    0.000000000000000e+00    0.7481239490e-11    0.0000000000e-00\n\
gfc     2    1   -0.206615509074176e-09    0.138441389137979e-08    0.7063781502e-11    0.7348347201e-11\n\
gfc     2    2    0.243938357328313e-05   -0.140027370385934e-05    0.7230231722e-11    0.7425816951e-11\n\
gfc     3    0    0.957161207093473e-06    0.000000000000000e+00    0.5731430751e-11    0.0000000000e-00\n\
";

    fn write_temp_gfc(content: &str, filename: &str) -> std::path::PathBuf {
        // Use a unique per-test directory to avoid race conditions when tests
        // run in parallel. The directory name includes the test thread name
        // (via thread id) to ensure isolation.
        let thread_id = format!("{:?}", std::thread::current().id());
        let dir = std::env::temp_dir()
            .join("apogee_egm2008_gfc_test")
            .join(thread_id);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(filename);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_gfc_skips_header_and_parses_coefficients() {
        let path = write_temp_gfc(SAMPLE_GFC, "sample.gfc");
        let model = SphericalHarmonics::load_egm2008(path.to_str().unwrap(), 3, 3).unwrap();

        // C_2,0 from the GFC file (note: differs slightly from the synthetic
        // test value used elsewhere — this is the actual EGM2008 tide-free value).
        assert_relative_eq!(model.c[2][0], -0.484165143790815e-03, epsilon = 1e-18);
        assert_relative_eq!(model.s[2][1], 0.138441389137979e-08, epsilon = 1e-18);
        assert_relative_eq!(model.c[2][2], 0.243938357328313e-05, epsilon = 1e-18);
        assert_relative_eq!(model.s[2][2], -0.140027370385934e-05, epsilon = 1e-18);
        assert_relative_eq!(model.c[3][0], 0.957161207093473e-06, epsilon = 1e-18);
    }

    #[test]
    fn test_gfc_parses_fortran_d0_notation() {
        // 1.0d0 should parse as 1.0; -0.484...d-03 should parse as -0.484...e-03.
        let path = write_temp_gfc(SAMPLE_GFC, "fortran.gfc");
        let model = SphericalHarmonics::load_egm2008(path.to_str().unwrap(), 2, 2).unwrap();

        // The gfc 0 0 line has 1.0d0 — degree 0 is skipped in storage, but
        // the parser must not choke on the d0 notation.
        // Verify C_2,0 which uses e-03 notation (not d0) — but also test
        // a pure d0 file.
        assert_relative_eq!(model.c[2][0], -0.484165143790815e-03, epsilon = 1e-18);

        // Pure Fortran d-notation file.
        let fortran_content = "\
end_of_head =====\n\
gfc 2 0 -4.84165143790815d-04 0.0d0\n\
gfc 2 1 -2.06615509074176d-10 1.38441389137979d-09\n\
gfc 2 2 2.43938357328313d-06 -1.40027370385934d-06\n";
        let path2 = write_temp_gfc(fortran_content, "pure_fortran.gfc");
        let model2 = SphericalHarmonics::load_egm2008(path2.to_str().unwrap(), 2, 2).unwrap();
        assert_relative_eq!(model2.c[2][0], -4.84165143790815e-04, epsilon = 1e-18);
        assert_relative_eq!(model2.s[2][1], 1.38441389137979e-09, epsilon = 1e-18);
        assert_relative_eq!(model2.c[2][2], 2.43938357328313e-06, epsilon = 1e-18);
        assert_relative_eq!(model2.s[2][2], -1.40027370385934e-06, epsilon = 1e-18);
    }

    #[test]
    fn test_gfc_extracts_header_metadata() {
        let path = write_temp_gfc(SAMPLE_GFC, "metadata.gfc");
        let model = SphericalHarmonics::load_egm2008(path.to_str().unwrap(), 2, 2).unwrap();

        // GM and reference radius should be parsed from the header.
        assert_relative_eq!(model.gm, 0.3986004415e15, epsilon = 1.0);
        assert_relative_eq!(model.reference_radius, 0.63781363e7, epsilon = 1e-3);
    }

    #[test]
    fn test_gfc_without_end_of_head_still_parses() {
        // A plain-text file without a header section should still work
        // (backward compatibility with the original synthetic format).
        let content = "2 0 -4.84169317386951e-04 0.0\n\
                       2 2 2.43926074800000e-06 -1.40027358800000e-06\n";
        let path = write_temp_gfc(content, "plain.txt");
        let model = SphericalHarmonics::load_egm2008(path.to_str().unwrap(), 2, 2).unwrap();
        assert_relative_eq!(model.c[2][0], -4.84169317386951e-04, epsilon = 1e-15);
        assert_relative_eq!(model.s[2][2], -1.400273588e-06, epsilon = 1e-15);
    }

    #[test]
    fn test_truncated_fixture_loads_and_validates() {
        // Load the vendored truncated fixture (degree 70) if present.
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("egm2008_to70.gfc");
        if !fixture_path.exists() {
            // Fixture not generated yet; skip.
            return;
        }
        let model =
            SphericalHarmonics::load_egm2008(fixture_path.to_str().unwrap(), 70, 70).unwrap();
        assert_eq!(model.degree, 70);
        assert_eq!(model.order, 70);

        // C_2,0 should match the known EGM2008 tide-free value.
        assert_relative_eq!(model.c[2][0], -0.484165143790815e-03, epsilon = 1e-15);

        // C_2,2 and S_2,2 should match known values.
        assert_relative_eq!(model.c[2][2], 0.243938357328313e-05, epsilon = 1e-15);
        assert_relative_eq!(model.s[2][2], -0.140027370385934e-05, epsilon = 1e-15);

        // GM and radius from header.
        assert_relative_eq!(model.gm, 0.3986004415e15, epsilon = 1.0);
        assert_relative_eq!(model.reference_radius, 0.63781363e7, epsilon = 1e-3);

        // Acceleration at a LEO position should be dominantly radial.
        let pos = Vector3::new(6_778_137.0, 0.0, 0.0);
        let a = model.acceleration(&pos).unwrap();
        let radial = a.raw().x;
        let tangential = (a.raw().y.powi(2) + a.raw().z.powi(2)).sqrt();
        assert!(
            radial.abs() > 100.0 * tangential.abs(),
            "radial component should dominate: radial={radial}, tangential={tangential}"
        );
    }

    #[test]
    fn test_real_egm2008_loads_if_present() {
        // End-to-end test: load the actual EGM2008 file downloaded by
        // scripts/fetch_data.sh. Skips gracefully if the file is absent
        // (e.g. in fast CI without data cache).
        let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data")
            .join("gravity");
        let path = data_dir.join("EGM2008_2190_TideFree.gfc");
        if !path.exists() {
            return;
        }

        // Load to degree 70 (fast enough for CI; full 2190 would be slow).
        let model = SphericalHarmonics::load_egm2008(path.to_str().unwrap(), 70, 70).unwrap();
        assert_eq!(model.degree, 70);
        assert_eq!(model.order, 70);

        // Known EGM2008 tide-free values.
        assert_relative_eq!(model.c[2][0], -0.484165143790815e-03, epsilon = 1e-15);
        assert_relative_eq!(model.c[2][2], 0.243938357328313e-05, epsilon = 1e-15);
        assert_relative_eq!(model.s[2][2], -0.140027370385934e-05, epsilon = 1e-15);

        // Header metadata.
        assert_relative_eq!(model.gm, 0.3986004415e15, epsilon = 1.0);
        assert_relative_eq!(model.reference_radius, 0.63781363e7, epsilon = 1e-3);
    }
}
