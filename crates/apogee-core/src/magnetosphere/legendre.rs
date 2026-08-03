//! Schmidt quasi-normalized associated Legendre functions for geomagnetism.
//!
//! This is a separate implementation from the gravity module because geomagnetic
//! models use Schmidt quasi-normalization, whereas gravity uses full
//! normalization.
//!
//! # Sources
//! * The recurrence relations follow the standard formulation used in IGRF
//!   implementations. See: Lowes, F. J. & Winch, D. E. (1991), "Differentiation of
//!   associated Legendre functions," and the NOAA NGDC IGRF source code.
//! * Normalization convention: Schmidt quasi-normalized, as defined in IAGA
//!   VMOD technical note 6, <https://www.ngdc.noaa.gov/IAGA/vmod/>.

/// Compute Schmidt quasi-normalized associated Legendre functions `P_n^m(x)`
/// and their derivatives `dP/dθ` for `x = cos θ` (θ = colatitude).
///
/// Returns triangular arrays indexed as `p[n][m]` for 0 ≤ n ≤ degree,
/// 0 ≤ m ≤ min(n, order).
///
/// # Sources
/// * Recurrence relations adapted from the IGRF reference implementation
///   (NOAA NGDC / IAGA VMOD, Fortran/Python, public domain).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn schmidt_legendre(
    degree: usize,
    order: usize,
    cos_theta: f64,
    sin_theta: f64,
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
    p[1][0] = cos_theta;
    dp[1][0] = -sin_theta;
    if order >= 1 {
        p[1][1] = sin_theta;
        dp[1][1] = cos_theta;
    }
    if degree == 1 {
        return (p, dp);
    }

    // Diagonal recurrence.
    let m_diag_max = degree.min(order);
    for m in 2..=m_diag_max {
        let factor = ((2 * m - 1) as f64 / (2 * m) as f64).sqrt();
        p[m][m] = factor * sin_theta * p[m - 1][m - 1];
        dp[m][m] = factor * (sin_theta * dp[m - 1][m - 1] + cos_theta * p[m - 1][m - 1]);
    }

    // Zonal and tesseral terms.
    for n in 2..=degree {
        let m_max = n.min(order);

        // m = 0 recurrence.
        let a0 = (2 * n - 1) as f64 / n as f64;
        let b0 = (n - 1) as f64 / n as f64;
        p[n][0] = a0 * cos_theta * p[n - 1][0] - b0 * p[n - 2][0];
        dp[n][0] = a0 * (cos_theta * dp[n - 1][0] - sin_theta * p[n - 1][0]) - b0 * dp[n - 2][0];

        for m in 1..=m_max {
            if m == n {
                continue; // diagonal already computed
            }
            if m == n - 1 {
                // First sub-diagonal.
                let a = (2 * n - 1) as f64 / (((2 * n) * (n - 1)) as f64).sqrt();
                p[n][m] = a * cos_theta * p[n - 1][m];
                dp[n][m] = a * (cos_theta * dp[n - 1][m] - sin_theta * p[n - 1][m]);
                continue;
            }
            let nm = ((n - m) * (n + m)) as f64;
            let a = (2 * n - 1) as f64 / nm.sqrt();
            let nm1 = ((n - m - 1) * (n + m - 1)) as f64;
            let b = nm1.sqrt() / nm.sqrt();

            p[n][m] = a * cos_theta * p[n - 1][m] - b * p[n - 2][m];
            dp[n][m] = a * (cos_theta * dp[n - 1][m] - sin_theta * p[n - 1][m]) - b * dp[n - 2][m];
        }
    }

    (p, dp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn degree_zero_is_constant() {
        let (p, dp) = schmidt_legendre(0, 0, 0.5, (1.0 - 0.5_f64.powi(2)).sqrt());
        assert_relative_eq!(p[0][0], 1.0, epsilon = 1e-12);
        assert_relative_eq!(dp[0][0], 0.0, epsilon = 1e-12);
    }

    #[test]
    fn degree_one_matches_geometry() {
        let cos_theta = 0.6;
        let sin_theta = f64::sqrt(1.0 - cos_theta * cos_theta);
        let (p, dp) = schmidt_legendre(1, 1, cos_theta, sin_theta);
        // P_1^0 = cosθ, dP/dθ = -sinθ.
        assert_relative_eq!(p[1][0], cos_theta, epsilon = 1e-12);
        assert_relative_eq!(dp[1][0], -sin_theta, epsilon = 1e-12);
        // P_1^1 = sinθ, dP/dθ = cosθ (Schmidt normalization).
        assert_relative_eq!(p[1][1], sin_theta, epsilon = 1e-12);
        assert_relative_eq!(dp[1][1], cos_theta, epsilon = 1e-12);
    }

    #[test]
    fn associated_legendre_recurrence_is_consistent() {
        // Compare a degree-2 zonal value against the closed form
        // P_2(cosθ) = (3 cos²θ - 1) / 2.
        let cos_theta = 0.4;
        let sin_theta = f64::sqrt(1.0 - cos_theta * cos_theta);
        let (p, _dp) = schmidt_legendre(2, 0, cos_theta, sin_theta);
        let expected = 1.5 * cos_theta * cos_theta - 0.5;
        assert_relative_eq!(p[2][0], expected, epsilon = 1e-12);
    }
}
