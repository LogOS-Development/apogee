//! Schmidt quasi-normalized associated Legendre functions for geomagnetism.
//!
//! This is a separate implementation from the gravity module because geomagnetic
//! models use Schmidt quasi-normalization, whereas gravity uses full
//! normalization.

/// Compute Schmidt quasi-normalized associated Legendre functions `P_n^m(x)`
/// and their derivatives `dP/dθ` for `x = cos θ` (θ = colatitude).
///
/// Returns triangular arrays indexed as `p[n][m]` for 0 ≤ n ≤ degree,
/// 0 ≤ m ≤ min(n, order).
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
