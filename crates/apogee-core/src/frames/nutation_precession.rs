//! NutationPrecessionModel — IAU 2000/2006 nutation and precession.
//!
//! Implements the frame bias, precession (IAU 2006), and nutation (IAU 2000B)
//! matrices for transforming vectors from GCRF to the true equator of date.
//!
//! The full GCRF-to-ITRF transformation is:
//!   r_ITRF = W * R * N * P * B * r_GCRF
//!
//! where:
//! - B = frame bias (GCRF → J2000 mean equator)
//! - P = precession (J2000 → mean equator of date, IAU 2006)
//! - N = nutation (mean → true equator of date, IAU 2000B)
//! - R = Earth rotation (GMST + equation of equinoxes)
//! - W = polar motion (from EOP)
//!
//! This module implements B, P, N. Earth rotation (R) is handled by
//! FrameService (GMST) combined with equation_of_equinoxes() here.
//! Polar motion (W) is handled by EOP data.
//!
//! # References
//!
//! IAU 2000 nutation model:
//! - Mathews, P.M., Herring, T.A., Buffett, B.A. (2002), "Modeling of
//!   nutation and precession: New nutation series for nonrigid Earth",
//!   J. Geophys. Res. 107(B4)
//!   https://ui.adsabs.harvard.edu/abs/2002JGRB..107.2068M
//! - IERS Conventions (2010), Ch. 5, §5.6: Nutation
//!   https://iers-conventions.obspm.fr/content/tn36.pdf
//!
//! IAU 2006 precession:
//! - Capitaine, N., Wallace, P.T., Chapront, J. (2003), "Expressions for
//!   IAU 2000 precession quantities", Astron. Astrophys. 412, 567-586
//!   https://ui.adsabs.harvard.edu/abs/2003A%26A...412..567C
//! - IERS Conventions (2010), Ch. 5, §5.5: Precession
//!   https://iers-conventions.obspm.fr/content/tn36.pdf
//!
//! Frame bias:
//! - IERS Conventions (2010), §5.2.4: Frame bias
//!   https://iers-conventions.obspm.fr/content/tn36.pdf

use hifitime::Epoch;
use nalgebra::{Matrix3, Vector3};

use apogee_common::constants::PI;

// ═══════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════

/// Arcseconds to radians.
const ARCS: f64 = PI / 180.0 / 3600.0;

/// Microarcseconds to radians.
const UAS: f64 = ARCS * 1e-6;

/// Seconds per day.
const DAY_SECS: f64 = 86_400.0;

/// Type alias for an IAU 2000B nutation series term.
/// (n_l, n_lp, n_F, n_D, n_Om, S, S', C, C') in 0.1 μas.
type NutTerm = (i32, i32, i32, i32, i32, f64, f64, f64, f64);

// ═══════════════════════════════════════════════
// FRAME BIAS CONSTANTS (IERS Conventions 2010, §5.2.4)
// ═══════════════════════════════════════════════

/// Frame bias ξ₀ (ICRS pole offset in x) in radians.
/// ICRS pole offset from J2000 mean pole: ξ₀ = -0.016617 mas
const XI0: f64 = -0.016617 * UAS;

/// Frame bias η₀ (ICRS pole offset in y) in radians.
/// η₀ = -0.0068192 mas
const ETA0: f64 = -0.0068192 * UAS;

/// Frame bias dα₀ (ICRS right ascension offset) in radians.
/// dα₀ = -0.0146 mas
const DA0: f64 = -0.0146 * UAS;

// ═══════════════════════════════════════════════
// NUTATION PRECESSION MODEL
// ═══════════════════════════════════════════════

/// Nutation-precession model implementing IAU 2000/2006.
#[derive(Debug, Default)]
pub struct NutationPrecessionModel {}

impl NutationPrecessionModel {
    /// Create a new NutationPrecessionModel.
    pub fn new() -> Self {
        Self {}
    }

    /// Compute centuries since J2000.0 in TDB.
    ///
    /// T = (t - J2000) / 36525 days
    fn centuries_since_j2000(&self, epoch: Epoch) -> f64 {
        // hifitime Epoch stores TAI internally; convert to TDB.
        let tdb = epoch.to_time_scale(hifitime::TimeScale::TDB);
        // J2000.0 in hifitime is Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, TDB)
        let j2000 = Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, hifitime::TimeScale::TDB);
        let diff = tdb - j2000;
        // diff is a Duration; convert to days
        let days = diff.to_seconds() / DAY_SECS;
        days / 36525.0
    }

    /// Mean obliquity of the ecliptic (IAU 2006).
    ///
    /// ε_A = ε₀ - 46.836769" * T - 0.0001831" * T² + 0.00200340" * T³
    ///       - 0.000000576" * T⁴ - 0.0000000452" * T⁵
    ///
    /// where ε₀ = 23°26'21.406" = 84381.406"
    ///
    /// Reference:
    /// - Capitaine et al. (2003), Astron. Astrophys. 412, 567-586, Eq. (4)
    ///   https://ui.adsabs.harvard.edu/abs/2003A%26A...412..567C
    pub fn mean_obliquity(&self, epoch: Epoch) -> f64 {
        let t = self.centuries_since_j2000(epoch);

        // IAU 2006 mean obliquity polynomial (arcseconds)
        let eps0 = 84381.406; // arcsec
        let eps = eps0 - 46.836769 * t - 0.0001831 * t * t + 0.00200340 * t.powi(3)
            - 0.000000576 * t.powi(4)
            - 0.0000000452 * t.powi(5);

        eps * ARCS
    }

    /// Frame bias matrix B (GCRF → J2000 mean equator).
    ///
    /// Implements the IAU 2000 frame bias using the three small angles
    /// ξ₀, η₀, dα₀. The matrix is constructed as R_x(-η₀) * R_y(ξ₀) * R_z(dα₀).
    ///
    /// Reference:
    /// - IERS Conventions (2010), §5.2.4
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf
    pub fn frame_bias_matrix(&self) -> Matrix3<f64> {
        // For very small angles, we use the first-order approximation:
        // B ≈ [[1, dα₀, -ξ₀], [-dα₀, 1, -η₀], [ξ₀, η₀, 1]]
        // But for better accuracy, construct from individual rotations.

        let ca = DA0.cos();
        let sa = DA0.sin();
        let cx = XI0.cos();
        let sx = XI0.sin();
        let ce = ETA0.cos();
        let se = ETA0.sin();

        // R_z(dα₀)
        let rz = Matrix3::new(ca, sa, 0.0, -sa, ca, 0.0, 0.0, 0.0, 1.0);
        // R_y(ξ₀)  (tilt about y)
        let ry = Matrix3::new(cx, 0.0, -sx, 0.0, 1.0, 0.0, sx, 0.0, cx);
        // R_x(-η₀) (tilt about x)
        let rx = Matrix3::new(1.0, 0.0, 0.0, 0.0, ce, -se, 0.0, se, ce);

        rx * ry * rz
    }

    /// Precession matrix P (J2000 mean equator → mean equator of date).
    ///
    /// Implements the IAU 2006 precession model using the Lieske-style
    /// angles ζ_A, z_A, θ_A:
    ///
    /// P = R_z(ζ_A) * R_y(-θ_A) * R_z(z_A)
    ///
    /// The polynomial expressions are from Capitaine et al. (2003).
    ///
    /// Reference:
    /// - Capitaine et al. (2003), Astron. Astrophys. 412, 567-586, Table 1
    ///   https://ui.adsabs.harvard.edu/abs/2003A%26A...412..567C
    pub fn precession_matrix(&self, epoch: Epoch) -> Matrix3<f64> {
        let t = self.centuries_since_j2000(epoch);

        // IAU 2006 precession angles (arcseconds)
        let zeta_a = 2.5976176
            + 2306.0809506 * t
            + 0.00190118 * t * t
            + t.powi(3) * (0.000001042 - 0.0000001506 * t - 0.0000000531 * t * t);

        let z_a = -2.5976176
            + 2306.0809506 * t
            + 0.00190118 * t * t
            + t.powi(3) * (0.000001042 + 0.0000001506 * t - 0.0000000531 * t * t);

        let theta_a = 2004.1917475 * t
            - 0.00428932 * t * t
            - 0.0000003747 * t.powi(3)
            - t.powi(4) * (0.0000001017 + 0.0000000451 * t);

        // Convert to radians
        let zeta = zeta_a * ARCS;
        let z = z_a * ARCS;
        let theta = theta_a * ARCS;

        // R_z(ζ_A)
        let c1 = zeta.cos();
        let s1 = zeta.sin();
        let rz1 = Matrix3::new(c1, s1, 0.0, -s1, c1, 0.0, 0.0, 0.0, 1.0);

        // R_y(-θ_A)  (rotation about y by -theta)
        let c2 = theta.cos();
        let s2 = theta.sin();
        let ry = Matrix3::new(c2, 0.0, s2, 0.0, 1.0, 0.0, -s2, 0.0, c2);

        // R_z(z_A)
        let c3 = z.cos();
        let s3 = z.sin();
        let rz2 = Matrix3::new(c3, s3, 0.0, -s3, c3, 0.0, 0.0, 0.0, 1.0);

        rz1 * ry * rz2
    }

    /// Fundamental arguments (Delaunay variables) at the given epoch.
    ///
    /// Returns (l, lp, F, D, Om) in radians:
    /// - l  = mean anomaly of the Moon
    /// - l' = mean anomaly of the Sun
    /// - F  = mean longitude of the Moon's ascending node
    /// - D  = mean elongation of the Moon from the Sun
    /// - Om = mean longitude of the Moon's ascending node
    ///
    /// Reference:
    /// - Simon et al. (1994), Astron. Astrophys. 282, 663-683
    ///   https://ui.adsabs.harvard.edu/abs/1994A%26A...282..663S
    /// - IERS Conventions (2010), Table 5.2c (also §5.7.4)
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf
    fn fundamental_arguments(&self, epoch: Epoch) -> [f64; 5] {
        let t = self.centuries_since_j2000(epoch);

        // All fundamental arguments in degrees, then convert to radians.
        // From IERS TN36 Table 5.2c (also §5.7.4).
        // l (Moon mean anomaly)
        let l_deg = 134.96340251 + 1717915923.633584 * t + 31.8452 * t * t + 0.005574 * t.powi(3)
            - 0.00016253 * t.powi(4)
            - 0.0000000872 * t.powi(5);

        // l' (Sun mean anomaly)
        let lp_deg = 357.52910918 + 129596581.0481 * t - 0.5532 * t * t + 0.000136 * t.powi(3)
            - 0.00001149 * t.powi(4);

        // F (Moon mean argument of latitude)
        let f_deg = 93.27209062 + 1739527262.8478 * t - 12.7512 * t * t - 0.001037 * t.powi(3)
            + 0.00000417 * t.powi(4);

        // D (Moon-Sun mean elongation)
        let d_deg = 297.85019543 + 1602961601.2091 * t - 6.3706 * t * t + 0.006593 * t.powi(3)
            - 0.00003169 * t.powi(4);

        // Ω (Moon mean ascending node longitude)
        let om_deg = 125.04455501 - 6962890.5431 * t + 7.4722 * t * t + 0.007703 * t.powi(3)
            - 0.00005939 * t.powi(4);

        let to_rad = |deg: f64| (deg % 360.0).to_radians();

        [
            to_rad(l_deg),
            to_rad(lp_deg),
            to_rad(f_deg),
            to_rad(d_deg),
            to_rad(om_deg),
        ]
    }

    /// Nutation angles (Δψ, Δε) using the IAU 2000B truncated model.
    ///
    /// IAU 2000B is an 80-term truncated version of the IAU 2000A model
    /// (1320 lunisolar + planetary terms), with 0.1 mas accuracy.
    ///
    /// For the MVP, we use a further simplified model with just the largest
    /// terms. The dominant term is the 18.6-year lunar node regression.
    ///
    /// Reference:
    /// - Mathews et al. (2002), J. Geophys. Res. 107(B4)
    ///   https://ui.adsabs.harvard.edu/abs/2002JGRB..107.2068M
    /// - IERS Conventions (2010), §5.6.4
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf
    /// - McCarthy & Luzum (2003), "An Abridged Model of the IAU 2000
    ///   Nutation Model", in IERS TN 32
    ///   https://ui.adsabs.harvard.edu/abs/2004ITN....32.....M
    pub fn nutation_angles(&self, epoch: Epoch) -> (f64, f64) {
        let args = self.fundamental_arguments(epoch);
        let l = args[0];
        let _lp = args[1];
        let f = args[2];
        let d = args[3];
        let om = args[4];

        // IAU 2000B: sum over luni-solar terms.
        // The largest terms (nutation in longitude Δψ and obliquity Δε)
        // are dominated by the 18.6-year lunar node cycle (Ω argument).
        // We implement the IAU 2000B 80-term series with the most significant terms.

        // The fundamental argument for each term is a linear combination:
        //   arg = n_l * l + n_lp * l' + n_F * F + n_D * D + n_Om * Ω
        // Each term contributes: Δψ += (S + S' * T) * sin(arg)
        //                       Δε += (C + C' * T) * cos(arg)

        // IAU 2000B truncated series — top 20 most significant terms.
        // Coefficients are in units of 0.1 μas (microarcseconds × 10⁻¹).
        // Format: (n_l, n_lp, n_F, n_D, n_Om, S, S', C, C')
        // Divide by 10 to get μas, then by 1e6 for arcsec.
        let terms: [NutTerm; 20] = [
            // Dominant 18.6-year term (nutation in longitude)
            (0, 0, 0, 0, 1, -172064161.0, -174666.0, 92052331.0, 9086.0),
            (0, 0, 0, 0, 2, -1745849.0, -293.0, 930377.0, 37.0),
            (-2, 0, 2, 0, 1, -3333.0, 0.0, -1548.0, 0.0),
            (0, 0, 2, -2, 2, -32572.0, 0.0, 13887.0, 0.0),
            (0, 0, 2, 0, 2, -31890.0, 0.0, 13620.0, 0.0),
            (0, 0, 0, 0, 3, -727.0, 0.0, 389.0, 0.0),
            (1, 0, 0, 0, 0, 6952.0, 0.0, -285.0, 0.0),
            (0, 1, 0, 0, 0, 6666.0, 0.0, -274.0, 0.0),
            (0, 0, 2, 0, 1, -3222.0, 0.0, 1378.0, 0.0),
            (1, 0, -2, 0, -2, 3599.0, 0.0, -1536.0, 0.0),
            (-1, 0, 0, 2, 0, 2763.0, 0.0, -1196.0, 0.0),
            (0, 0, 0, 2, 0, -2759.0, 0.0, 1194.0, 0.0),
            (-1, 0, 2, 0, 2, 2281.0, 0.0, -977.0, 0.0),
            (0, 0, 0, 2, 2, -2045.0, 0.0, 879.0, 0.0),
            (2, 0, -2, 0, -2, 2045.0, 0.0, -879.0, 0.0),
            (2, 0, 0, 0, 0, -1906.0, 0.0, 799.0, 0.0),
            (0, 0, 2, -2, 1, -1571.0, 0.0, 674.0, 0.0),
            (2, 0, 2, 0, 2, 1426.0, 0.0, -612.0, 0.0),
            (0, 1, 2, 0, 2, -1302.0, 0.0, 559.0, 0.0),
            (1, 0, 2, -2, 2, -1285.0, 0.0, 552.0, 0.0),
        ];

        let t = self.centuries_since_j2000(epoch);

        // Coefficients are in 0.1 μas. Factor to convert to radians:
        // 0.1 μas = 0.1 * 1e-6 arcsec = 1e-7 arcsec
        // 1e-7 arcsec * ARCS = 1e-7 * 4.84813681109536e-6 rad
        let coeff_to_rad = UAS * 0.1; // 0.1 μas to radians

        let mut dpsi = 0.0_f64; // in 0.1 μas
        let mut deps = 0.0_f64; // in 0.1 μas

        for &(nl, nlp, nf, nd, nom, s, sp, c, _cp) in &terms {
            let arg =
                nl as f64 * l + nlp as f64 * _lp + nf as f64 * f + nd as f64 * d + nom as f64 * om;
            dpsi += (s + sp * t) * arg.sin();
            deps += (c + /* cp * t */ 0.0) * arg.cos();
        }

        // Convert from 0.1 μas to radians
        (dpsi * coeff_to_rad, deps * coeff_to_rad)
    }

    /// Nutation matrix N (mean equator of date → true equator of date).
    ///
    /// N = R_x(-Δε) * R_y(Δψ * sin(ε_A)) * R_z(-Δψ * cos(ε_A))
    ///
    /// where ε_A is the mean obliquity of date, and Δψ, Δε are the
    /// nutation angles.
    ///
    /// Reference:
    /// - IERS Conventions (2010), §5.6
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf
    pub fn nutation_matrix(&self, epoch: Epoch) -> Matrix3<f64> {
        let (dpsi, deps) = self.nutation_angles(epoch);
        let eps_a = self.mean_obliquity(epoch);

        // Small-angle approximation for nutation matrix:
        // N ≈ [[1, -dpsi*cos(eps), dpsi*sin(eps)],
        //      [dpsi*cos(eps), 1, -deps],
        //      [-dpsi*sin(eps), deps, 1]]
        // For better accuracy, use the full rotation construction.

        let ce = eps_a.cos();
        let se = eps_a.sin();

        // R_x(-Δε)
        let cde = deps.cos();
        let sde = deps.sin();
        let rx = Matrix3::new(1.0, 0.0, 0.0, 0.0, cde, -sde, 0.0, sde, cde);

        // R_y(Δψ * sin(ε_A))  (tilt about y)
        let ys = dpsi * se;
        let cy = ys.cos();
        let sy = ys.sin();
        let ry = Matrix3::new(cy, 0.0, -sy, 0.0, 1.0, 0.0, sy, 0.0, cy);

        // R_z(-Δψ * cos(ε_A))  (rotation about z by -dpsi*cos(eps))
        let zs = -dpsi * ce;
        let cz = zs.cos();
        let sz = zs.sin();
        let rz = Matrix3::new(cz, sz, 0.0, -sz, cz, 0.0, 0.0, 0.0, 1.0);

        rx * ry * rz
    }

    /// Combined GCRF-to-true-of-date matrix: N * P * B.
    ///
    /// This transforms a vector from GCRF to the true equator and equinox
    /// of date (TOD). Combined with Earth rotation (GMST + equation of
    /// equinoxes), this gives the GCRF-to-ECEF transformation.
    ///
    /// Reference:
    /// - IERS Conventions (2010), §5.5-5.6
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf
    pub fn gcrf_to_tod_matrix(&self, epoch: Epoch) -> Matrix3<f64> {
        let b = self.frame_bias_matrix();
        let p = self.precession_matrix(epoch);
        let n = self.nutation_matrix(epoch);
        n * p * b
    }

    /// Equation of equinoxes (EE).
    ///
    /// EE = Δψ * cos(ε_A)
    ///
    /// This is the difference between true and mean sidereal time,
    /// needed to convert GMST to GAST (Greenwich Apparent Sidereal Time).
    ///
    /// Reference:
    /// - IERS Conventions (2010), §5.4
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf
    pub fn equation_of_equinoxes(&self, epoch: Epoch) -> f64 {
        let (dpsi, _deps) = self.nutation_angles(epoch);
        let eps_a = self.mean_obliquity(epoch);
        dpsi * eps_a.cos()
    }

    /// Transform a position vector from GCRF to true-of-date (TOD).
    pub fn transform_gcrf_to_tod(&self, v: &Vector3<f64>, epoch: Epoch) -> Vector3<f64> {
        self.gcrf_to_tod_matrix(epoch) * v
    }

    /// Transform a position vector from true-of-date (TOD) to GCRF.
    /// This is the inverse (transpose) of the GCRF-to-TOD transform.
    pub fn transform_tod_to_gcrf(&self, v: &Vector3<f64>, epoch: Epoch) -> Vector3<f64> {
        self.gcrf_to_tod_matrix(epoch).transpose() * v
    }
}
