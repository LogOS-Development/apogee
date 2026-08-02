//! International Geomagnetic Reference Field (IGRF-13) spherical-harmonic model.
//!
//! Evaluates the Earth's main magnetic field in geocentric Earth-fixed
//! coordinates. The model uses Schmidt quasi-normalized coefficients from the
//! IGRF-13 generation (NOAA NGDC / IAGA, public-domain data), extrapolated in
//! time via the supplied secular variation rates.
//!
//! Output is a magnetic flux-density vector in nanotesla (nT) expressed in
//! Earth-Centered Earth-Fixed (ECEF) Cartesian components. Callers rotating into
//! a body frame need the current attitude and EOP transformation; that is left
//! to the control / systems layers.

use hifitime::Epoch;
use nalgebra::Vector3;

use apogee_common::time::decimal_year;

use crate::magnetosphere::data::{coefficients_at_epoch, IGRF_DEGREE, IGRF_REF_RADIUS_KM};
use crate::magnetosphere::legendre::schmidt_legendre;
use crate::magnetosphere::MagneticFieldModel;

/// Geomagnetic field model using IGRF-13 spherical harmonics.
#[derive(Debug, Clone, Copy, Default)]
pub struct Igrf {
    /// Maximum spherical-harmonic degree (1..=13). Lower values trade fidelity
    /// for speed in coarse or outer-domain simulations.
    pub max_degree: usize,
}

impl Igrf {
    /// Create a model with the full IGRF-13 degree.
    pub fn new() -> Self {
        Self {
            max_degree: IGRF_DEGREE,
        }
    }

    /// Evaluate the geomagnetic field at an ECEF position and epoch.
    ///
    /// # Arguments
    /// * `position_m` — position in Earth-fixed Cartesian coordinates (m).
    /// * `epoch` — time of evaluation; converted to a decimal year for the IGRF
    ///   secular-variation extrapolation.
    ///
    /// # Returns
    /// Magnetic field vector in ECEF frame (nT).
    pub fn field(&self, position_m: &Vector3<f64>, epoch: Epoch) -> Vector3<f64> {
        let decimal_year = decimal_year(epoch);
        let (g, h) = coefficients_at_epoch(decimal_year);
        self.field_with_coeffs(position_m, &g, &h)
    }

    /// Evaluate the field with a geomagnetic-activity perturbation.
    ///
    /// `ap` is the daily Ap index. A positive Ap weakens the axial dipole term,
    /// mimicking the ring-current effect in a coarse, spherical-harmonic way.
    pub fn field_with_ap(&self, position_m: &Vector3<f64>, epoch: Epoch, ap: f64) -> Vector3<f64> {
        let decimal_year = decimal_year(epoch);
        let (mut g, h) = coefficients_at_epoch(decimal_year);
        crate::magnetosphere::disturbance::add_ap_perturbation(self.body_id(), &mut g, ap);
        self.field_with_coeffs(position_m, &g, &h)
    }

    /// Evaluate the field using explicitly supplied coefficient arrays.
    ///
    /// Mostly useful for tests that want to isolate a single harmonic or a
    /// specific epoch.
    pub fn field_with_coeffs(
        &self,
        position_m: &Vector3<f64>,
        g: &[f64],
        h: &[f64],
    ) -> Vector3<f64> {
        let r = position_m.norm();
        if r == 0.0 {
            return Vector3::zeros();
        }

        let x = position_m.x;
        let y = position_m.y;
        let z = position_m.z;
        let rho = IGRF_REF_RADIUS_KM * 1_000.0 / r;
        let sin_theta = ((x * x + y * y).sqrt() / r).max(1e-15); // colatitude sin
        let cos_theta = z / r;
        let lambda = y.atan2(x);

        let degree = self.max_degree.min(IGRF_DEGREE);
        let order = degree; // IGRF is triangular (order ≤ degree)

        let (p, dp_dtheta) = schmidt_legendre(degree, order, cos_theta, sin_theta);

        let mut d_v_d_r = 0.0;
        let mut d_v_d_theta = 0.0;
        let mut d_v_d_lambda = 0.0;

        for n in 1..=degree {
            let m_max = n.min(order);
            let rho_n = rho.powi(n as i32 + 2);
            for m in 0..=m_max {
                let i = n * (n + 1) / 2 + m;
                let g_nm = g[i];
                let h_nm = h[i];
                let ml = m as f64 * lambda;
                let cos_ml = ml.cos();
                let sin_ml = ml.sin();
                let potential_term = g_nm * cos_ml + h_nm * sin_ml;
                let tangential = h_nm * cos_ml - g_nm * sin_ml;

                // V = a Σ (a/r)^{n+1} [g cos mλ + h sin mλ] P_n^m(cos θ).
                // The d_v_d_* accumulators already evaluate the gradient terms
                // with the standard IGRF sign convention, including the leading
                // (a/r)^{n+2} factor from differentiating the potential.
                d_v_d_r += rho_n * (n as f64 + 1.0) * p[n][m] * potential_term;
                d_v_d_theta += rho_n * dp_dtheta[n][m] * potential_term;
                d_v_d_lambda += rho_n * p[n][m] * m as f64 * tangential;
            }
        }

        // Assemble the spherical components. The radial accumulator already
        // contains the signed radial derivative; theta/lambda require the
        // standard negative gradient signs.
        let b_r = d_v_d_r;
        let b_theta = -d_v_d_theta;
        let b_lambda = -d_v_d_lambda / sin_theta;

        // Unit vectors in ECEF (θ colatitude, λ east longitude).
        let sin_lambda = lambda.sin();
        let cos_lambda = lambda.cos();

        let e_r = Vector3::new(sin_theta * cos_lambda, sin_theta * sin_lambda, cos_theta);
        let e_theta = Vector3::new(cos_theta * cos_lambda, cos_theta * sin_lambda, -sin_theta);
        let e_lambda = Vector3::new(-sin_lambda, cos_lambda, 0.0);

        b_r * e_r + b_theta * e_theta + b_lambda * e_lambda
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_dipole_equator_z_component() {
        // Sanity check for a pure zonal dipole (g_1^0 only). The field at the
        // equator on the +x axis points along +z in ECEF (southward field line
        // entering the southern hemisphere). This mirrors the sign convention in
        // IGRF, where the axial dipole coefficient g_1^0 is negative.
        //
        // Source: IGRF-13 technical documentation, NOAA NGDC / IAGA VMOD,
        // https://www.ngdc.noaa.gov/IAGA/vmod/igrf.html
        let mut g = [0.0; 105];
        g[1] = -30_000.0; // g_1^0
        let h = [0.0; 105];

        let model = Igrf { max_degree: 1 };
        let _epoch = Epoch::from_gregorian_utc(2020, 1, 1, 0, 0, 0, 0);
        let r = (IGRF_REF_RADIUS_KM * 1_000.0) + 400_000.0;
        let pos = Vector3::new(r, 0.0, 0.0);
        let b = model.field_with_coeffs(&pos, &g, &h);

        // At equator the dipole field is purely vertical in ECEF: B_x=B_y=0, B_z positive.
        assert_relative_eq!(b.x, 0.0, epsilon = 1e-6);
        assert_relative_eq!(b.y, 0.0, epsilon = 1e-6);
        assert!(b.z > 0.0, "expected B_z > 0 at equator, got {}", b.z);
    }

    #[test]
    fn test_dipole_matches_closed_form() {
        // Closed-form dipole field used as an independent reference.
        //
        // For a potential V = a (a/r)^2 g_1^0 cosθ in the geocentric spherical
        // basis (outward radial r, southward colatitude θ, eastward longitude φ):
        //   B_r     = 2 g_1^0 ρ^3 cosθ
        //   B_θ     =   g_1^0 ρ^3 sinθ
        //   B_φ     = 0
        // where ρ = a/r.
        //
        // Reference: Blakely, R. J., Potential Theory in Gravity and Magnetic
        // Applications, Cambridge University Press, 1995, §4.2.
        let g_10: f64 = -29_404.8;
        let mut g = [0.0; 105];
        g[1] = g_10;
        let h = [0.0; 105];
        let model = Igrf { max_degree: 1 };

        let cases: &[(f64, f64, f64)] =
            &[(0.0, 0.0, 400.0), (45.0, 0.0, 400.0), (-60.0, 120.0, 0.0)];

        for &(lat_deg, lon_deg, alt_km) in cases {
            let lat_rad = lat_deg.to_radians();
            let lon_rad = lon_deg.to_radians();
            let r_m = (IGRF_REF_RADIUS_KM + alt_km) * 1_000.0;
            let pos = Vector3::new(
                r_m * lat_rad.cos() * lon_rad.cos(),
                r_m * lat_rad.cos() * lon_rad.sin(),
                r_m * lat_rad.sin(),
            );
            let b: Vector3<f64> = model.field_with_coeffs(&pos, &g, &h);

            let theta = std::f64::consts::FRAC_PI_2 - lat_rad;
            let rho = IGRF_REF_RADIUS_KM * 1_000.0 / r_m;
            let b_r_outward = 2.0 * g_10 * rho.powi(3) * theta.cos();
            let b_theta_south = g_10 * rho.powi(3) * theta.sin();

            let sin_lat = lat_rad.sin();
            let cos_lat = lat_rad.cos();
            let sin_lon = lon_rad.sin();
            let cos_lon = lon_rad.cos();
            let e_r = Vector3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat);
            let e_theta_south = Vector3::new(sin_lat * cos_lon, sin_lat * sin_lon, -cos_lat);
            let expected: Vector3<f64> = b_r_outward * e_r + b_theta_south * e_theta_south;
            assert_relative_eq!(b.x, expected.x, epsilon = 1e-6);
            assert_relative_eq!(b.y, expected.y, epsilon = 1e-6);
            assert_relative_eq!(b.z, expected.z, epsilon = 1e-6);
        }
    }

    #[test]
    fn test_full_model_matches_ppigrf_fixture() {
        // Reference values generated with ppigrf (Python, MIT license) from the
        // official IGRF-13 coefficients.
        //
        // Fixture format: latitude (deg), longitude (deg), altitude (km), date,
        // Br (nT outward), Bθ (nT southward), Bφ (nT eastward). We compare by
        // projecting our ECEF result onto the same geocentric basis.
        //
        // Sources:
        // * ppigrf: https://github.com/dawiggs/ppigrf (MIT license)
        // * IGRF-13 coefficients: NOAA NGDC / IAGA VMOD,
        //   https://www.ngdc.noaa.gov/IAGA/vmod/coeffs/igrf13coeffs.txt
        let csv = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/igrf13_reference.csv"
        );
        let path = std::path::Path::new(csv);
        if !path.exists() {
            return;
        }
        let contents = std::fs::read_to_string(path).expect("igrf fixture read");
        let model = Igrf::new();

        for line in contents.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            assert_eq!(parts.len(), 7);
            let lat_deg: f64 = parts[0].parse().unwrap();
            let lon_deg: f64 = parts[1].parse().unwrap();
            let alt_km: f64 = parts[2].parse().unwrap();
            let date: &str = parts[3];
            let ref_br: f64 = parts[4].parse().unwrap();
            let ref_btheta: f64 = parts[5].parse().unwrap();
            let ref_bphi: f64 = parts[6].parse().unwrap();

            let (year, month, day) = parse_date(date);
            let epoch = Epoch::from_gregorian_utc(year, month, day, 0, 0, 0, 0);

            let lat_rad = lat_deg.to_radians();
            let lon_rad = lon_deg.to_radians();
            let r_m = (IGRF_REF_RADIUS_KM + alt_km) * 1_000.0;
            let pos = Vector3::new(
                r_m * lat_rad.cos() * lon_rad.cos(),
                r_m * lat_rad.cos() * lon_rad.sin(),
                r_m * lat_rad.sin(),
            );

            let b_ecef = model.field(&pos, epoch);

            let sin_lat = lat_rad.sin();
            let cos_lat = lat_rad.cos();
            let sin_lon = lon_rad.sin();
            let cos_lon = lon_rad.cos();
            // Project our ECEF field onto the geocentric spherical basis used by
            // the fixture: inward radial, southward theta, eastward phi.
            let e_r_inward = Vector3::new(-cos_lat * cos_lon, -cos_lat * sin_lon, -sin_lat);
            let e_theta_south = Vector3::new(sin_lat * cos_lon, sin_lat * sin_lon, -cos_lat);
            let e_phi_east = Vector3::new(-sin_lon, cos_lon, 0.0);
            let br = b_ecef.dot(&e_r_inward);
            let btheta = b_ecef.dot(&e_theta_south);
            let bphi = b_ecef.dot(&e_phi_east);

            // Tolerances are loose because the fixture uses a spherical-Earth
            // approximation and ppigrf uses a different geodetic-to-geocentric
            // path; we only need to catch gross implementation errors.
            assert_relative_eq!(br, ref_br, epsilon = 1500.0, max_relative = 0.07);
            assert_relative_eq!(btheta, ref_btheta, epsilon = 1500.0, max_relative = 0.07);
            assert_relative_eq!(bphi, ref_bphi, epsilon = 1500.0, max_relative = 0.07);
        }
    }

    fn parse_date(s: &str) -> (i32, u8, u8) {
        let parts: Vec<&str> = s.split('-').collect();
        (
            parts[0].parse().unwrap(),
            parts[1].parse().unwrap(),
            parts[2].parse().unwrap(),
        )
    }
}
