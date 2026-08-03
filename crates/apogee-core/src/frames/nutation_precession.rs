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
//!   N = R_x(ε_A + Δε) · R_z(+Δψ) · R_x(-ε_A)  (ERFA/SOFA convention)
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
//! - Hilton, J.L., et al. (2006), "Report of the International Astronomical
//!   Union Division I Working Group on Precession and the Ecliptic",
//!   Celest. Mech. Dyn. Astron. 94, 351-367
//!   https://ui.adsabs.harvard.edu/abs/2006CeMDA..94..351H
//!
//! Frame bias:
//! - IERS Conventions (2010), §5.2.4: Frame bias
//!   https://iers-conventions.obspm.fr/content/tn36.pdf
//!
//! ERFA reference implementation (open-source SOFA fork):
//! - ERFA: https://github.com/liberfa/erfa
//! - The coefficients for the IAU 2000B nutation series, fundamental
//!   arguments, precession angles, and frame bias offsets are taken
//!   directly from the ERFA source files nut00b.c, p06e.c, and bi00.c.

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

/// 360 degrees in arcseconds (for fundamental-argument reduction).
const TURNAS: f64 = 360.0 * 3600.0;

/// Type alias for an IAU 2000B nutation series term.
///
/// Format: (n_l, n_l', n_F, n_D, n_Ω,
///          S, S', C, C', C'', C''')
///
/// - S  = longitude sine coefficient (0.1 μas)
/// - S' = longitude sine t-coefficient (0.1 μas / century)
/// - C  = longitude cosine coefficient (0.1 μas)
/// - C' = longitude cosine t-coefficient (0.1 μas / century)
/// - C'' = obliquity cosine coefficient (0.1 μas)
/// - C'''= obliquity cosine t-coefficient (0.1 μas / century)
type NutTerm = (i32, i32, i32, i32, i32, f64, f64, f64, f64, f64, f64);

// ═══════════════════════════════════════════════
// FRAME BIAS CONSTANTS (IERS Conventions 2010, §5.2.4)
// ═══════════════════════════════════════════════

/// Frame bias ξ₀ (ICRS pole offset in x) in radians.
/// ICRS pole offset from J2000 mean pole: ξ₀ = -0.016617 arcsec.
const XI0: f64 = -0.016617 * ARCS;

/// Frame bias η₀ (ICRS pole offset in y) in radians.
/// η₀ = -0.0068192 arcsec.
const ETA0: f64 = -0.0068192 * ARCS;

/// Frame bias dα₀ (ICRS right ascension offset) in radians.
/// dα₀ = -0.0146 arcsec.
const DA0: f64 = -0.0146 * ARCS;

// ═══════════════════════════════════════════════
// IAU 2000B NUTATION FIXED OFFSETS (ERFA nut00b.c)
// ═══════════════════════════════════════════════

/// Fixed longitude offset in lieu of planetary terms (milliarcsec → rad).
const DPPLAN: f64 = -0.135 * 1e-3 * ARCS;

/// Fixed obliquity offset in lieu of planetary terms (milliarcsec → rad).
const DEPLAN: f64 = 0.388 * 1e-3 * ARCS;

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
        let j2000 = Epoch::from_gregorian(2000, 1, 1, 12, 0, 0, 0, hifitime::TimeScale::TDB);
        let diff = tdb - j2000;
        let days = diff.to_seconds() / DAY_SECS;
        days / 36525.0
    }

    /// Mean obliquity of the ecliptic (IAU 2006).
    ///
    /// ε_A = ε₀ - 46.836769" * T - 0.0001831" * T² + 0.00200340" * T³
    ///       - 0.000000576" * T⁴ - 0.0000000434" * T⁵
    ///
    /// where ε₀ = 23°26'21.406" = 84381.406"
    ///
    /// Reference:
    /// - IERS Conventions (2010), Ch. 5
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf
    /// - ERFA obl06.c
    ///   https://github.com/liberfa/erfa/blob/master/src/obl06.c
    pub fn mean_obliquity(&self, epoch: Epoch) -> f64 {
        let t = self.centuries_since_j2000(epoch);

        let eps = 84381.406 - 46.836769 * t - 0.0001831 * t * t + 0.00200340 * t.powi(3)
            - 0.000000576 * t.powi(4)
            - 0.0000000434 * t.powi(5);

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
        // The IAU 2000 frame bias is a sub-arcsecond rotation. For the
        // tiny angles ξ₀, η₀, dα₀ the first-order matrix is accurate to
        // better than 1 microarcsecond and matches the ERFA/SOFA frame
        // bias matrix exactly.
        Matrix3::new(1.0, DA0, -XI0, -DA0, 1.0, -ETA0, XI0, ETA0, 1.0)
    }

    /// Precession matrix P (J2000 mean equator → mean equator of date).
    ///
    /// Implements the IAU 2006 precession model using the equinox-based
    /// Euler angles ζ_A, z_A, θ_A:
    ///
    ///   P = R_z(z_A) * R_y(-θ_A) * R_z(ζ_A)
    ///
    /// where R_z and R_y are the standard active rotation matrices.
    ///
    /// Reference:
    /// - Capitaine et al. (2003), Astron. Astrophys. 412, 567-586
    /// - ERFA p06e.c
    ///   https://github.com/liberfa/erfa/blob/master/src/p06e.c
    pub fn precession_matrix(&self, epoch: Epoch) -> Matrix3<f64> {
        let t = self.centuries_since_j2000(epoch);

        // IAU 2006 / P03 precession angles (arcseconds).
        // See ERFA p06e.c for the exact polynomials.
        let zeta_a = 2.650545 + 2306.083227 * t + 0.2988499 * t * t + 0.01801828 * t.powi(3)
            - 0.000005971 * t.powi(4)
            - 0.0000003173 * t.powi(5);

        let z_a = -2.650545 + 2306.077181 * t + 1.0927348 * t * t + 0.01826837 * t.powi(3)
            - 0.000028596 * t.powi(4)
            - 0.0000002904 * t.powi(5);

        let theta_a = 2004.191903 * t
            - 0.4294934 * t * t
            - 0.04182264 * t.powi(3)
            - 0.000007089 * t.powi(4)
            - 0.0000001274 * t.powi(5);

        let zeta = zeta_a * ARCS;
        let z = z_a * ARCS;
        let theta = theta_a * ARCS;

        // Standard active rotation about z by z_A.
        let czz = z.cos();
        let szz = z.sin();
        let rz_z = Matrix3::new(czz, -szz, 0.0, szz, czz, 0.0, 0.0, 0.0, 1.0);

        // Standard active rotation about y by -θ_A.
        let ct = theta.cos();
        let st = theta.sin();
        let ry_theta = Matrix3::new(ct, 0.0, -st, 0.0, 1.0, 0.0, st, 0.0, ct);

        // Standard active rotation about z by ζ_A.
        let cz = zeta.cos();
        let sz = zeta.sin();
        let rz_zeta = Matrix3::new(cz, -sz, 0.0, sz, cz, 0.0, 0.0, 0.0, 1.0);

        // P = R_z(z_A) R_y(-θ_A) R_z(ζ_A)
        rz_z * ry_theta * rz_zeta
    }

    /// Fundamental arguments (Delaunay variables) at the given epoch.
    ///
    /// Returns (l, l', F, D, Ω) in radians:
    /// - l  = mean anomaly of the Moon
    /// - l' = mean anomaly of the Sun
    /// - F  = mean longitude of the Moon minus Ω
    /// - D  = mean elongation of the Moon from the Sun
    /// - Ω  = mean longitude of the Moon's ascending node
    ///
    /// Coefficients and reduction are taken directly from ERFA nut00b.c.
    fn fundamental_arguments(&self, epoch: Epoch) -> [f64; 5] {
        let t = self.centuries_since_j2000(epoch);

        // Mean anomaly of the Moon (arcsec, reduced mod 360°).
        let l_arcsec = 485868.249036 + 1717915923.2178 * t;
        // Mean anomaly of the Sun.
        let lp_arcsec = 1287104.79305 + 129596581.0481 * t;
        // Mean argument of latitude of the Moon.
        let f_arcsec = 335779.526232 + 1739527262.8478 * t;
        // Mean elongation of the Moon from the Sun.
        let d_arcsec = 1072260.70369 + 1602961601.2090 * t;
        // Mean longitude of the ascending node of the Moon.
        let om_arcsec = 450160.398036 - 6962890.5431 * t;

        let reduce = |x: f64| {
            let y = x % TURNAS;
            if y < 0.0 {
                y + TURNAS
            } else {
                y
            }
        };

        [
            reduce(l_arcsec) * ARCS,
            reduce(lp_arcsec) * ARCS,
            reduce(f_arcsec) * ARCS,
            reduce(d_arcsec) * ARCS,
            reduce(om_arcsec) * ARCS,
        ]
    }

    /// IAU 2000B nutation series — 77 luni-solar terms.
    ///
    /// Coefficients are in units of 0.1 μas (microarcseconds × 10⁻¹).
    /// Format for each term:
    ///   (nl, nlp, nF, nD, nOm,
    ///    ps, pst, pc,   // longitude: (S + S'·T)·sin(arg) + C·cos(arg)
    ///    ec, ect, es)   // obliquity:  (C + C'·T)·cos(arg) + S·sin(arg)
    ///
    /// Source: ERFA nut00b.c
    /// https://github.com/liberfa/erfa/blob/master/src/nut00b.c
    const NUTATION_SERIES: [NutTerm; 77] = [
        (
            0,
            0,
            0,
            0,
            1,
            -172064161.0,
            -174666.0,
            33386.0,
            92052331.0,
            9086.0,
            15377.0,
        ),
        (
            0,
            0,
            2,
            -2,
            2,
            -13170906.0,
            -1675.0,
            -13696.0,
            5730336.0,
            -3015.0,
            -4587.0,
        ),
        (
            0, 0, 2, 0, 2, -2276413.0, -234.0, 2796.0, 978459.0, -485.0, 1374.0,
        ),
        (
            0, 0, 0, 0, 2, 2074554.0, 207.0, -698.0, -897492.0, 470.0, -291.0,
        ),
        (
            0, 1, 0, 0, 0, 1475877.0, -3633.0, 11817.0, 73871.0, -184.0, -1924.0,
        ),
        (
            0, 1, 2, -2, 2, -516821.0, 1226.0, -524.0, 224386.0, -677.0, -174.0,
        ),
        (1, 0, 0, 0, 0, 711159.0, 73.0, -872.0, -6750.0, 0.0, 358.0),
        (
            0, 0, 2, 0, 1, -387298.0, -367.0, 380.0, 200728.0, 18.0, 318.0,
        ),
        (
            1, 0, 2, 0, 2, -301461.0, -36.0, 816.0, 129025.0, -63.0, 367.0,
        ),
        (
            0, -1, 2, -2, 2, 215829.0, -494.0, 111.0, -95929.0, 299.0, 132.0,
        ),
        (0, 0, 2, -2, 1, 128227.0, 137.0, 181.0, -68982.0, -9.0, 39.0),
        (-1, 0, 2, 0, 2, 123457.0, 11.0, 19.0, -53311.0, 32.0, -4.0),
        (-1, 0, 0, 2, 0, 156994.0, 10.0, -168.0, -1235.0, 0.0, 82.0),
        (1, 0, 0, 0, 1, 63110.0, 63.0, 27.0, -33228.0, 0.0, -9.0),
        (-1, 0, 0, 0, 1, -57976.0, -63.0, -189.0, 31429.0, 0.0, -75.0),
        (-1, 0, 2, 2, 2, -59641.0, -11.0, 149.0, 25543.0, -11.0, 66.0),
        (1, 0, 2, 0, 1, -51613.0, -42.0, 129.0, 26366.0, 0.0, 78.0),
        (-2, 0, 2, 0, 1, 45893.0, 50.0, 31.0, -24236.0, -10.0, 20.0),
        (0, 0, 0, 2, 0, 63384.0, 11.0, -150.0, -1220.0, 0.0, 29.0),
        (0, 0, 2, 2, 2, -38571.0, -1.0, 158.0, 16452.0, -11.0, 68.0),
        (0, -2, 2, -2, 2, 32481.0, 0.0, 0.0, -13870.0, 0.0, 0.0),
        (-2, 0, 0, 2, 0, -47722.0, 0.0, -18.0, 477.0, 0.0, -25.0),
        (2, 0, 2, 0, 2, -31046.0, -1.0, 131.0, 13238.0, -11.0, 59.0),
        (1, 0, 2, -2, 2, 28593.0, 0.0, -1.0, -12338.0, 10.0, -3.0),
        (-1, 0, 2, 0, 1, 20441.0, 21.0, 10.0, -10758.0, 0.0, -3.0),
        (2, 0, 0, 0, 0, 29243.0, 0.0, -74.0, -609.0, 0.0, 13.0),
        (0, 0, 2, 0, 0, 25887.0, 0.0, -66.0, -550.0, 0.0, 11.0),
        (0, 1, 0, 0, 1, -14053.0, -25.0, 79.0, 8551.0, -2.0, -45.0),
        (-1, 0, 0, 2, 1, 15164.0, 10.0, 11.0, -8001.0, 0.0, -1.0),
        (0, 2, 2, -2, 2, -15794.0, 72.0, -16.0, 6850.0, -42.0, -5.0),
        (0, 0, -2, 2, 0, 21783.0, 0.0, 13.0, -167.0, 0.0, 13.0),
        (1, 0, 0, -2, 1, -12873.0, -10.0, -37.0, 6953.0, 0.0, -14.0),
        (0, -1, 0, 0, 1, -12654.0, 11.0, 63.0, 6415.0, 0.0, 26.0),
        (-1, 0, 2, 2, 1, -10204.0, 0.0, 25.0, 5222.0, 0.0, 15.0),
        (0, 2, 0, 0, 0, 16707.0, -85.0, -10.0, 168.0, -1.0, 10.0),
        (1, 0, 2, 2, 2, -7691.0, 0.0, 44.0, 3268.0, 0.0, 19.0),
        (-2, 0, 2, 0, 0, -11024.0, 0.0, -14.0, 104.0, 0.0, 2.0),
        (0, 1, 2, 0, 2, 7566.0, -21.0, -11.0, -3250.0, 0.0, -5.0),
        (0, 0, 2, 2, 1, -6637.0, -11.0, 25.0, 3353.0, 0.0, 14.0),
        (0, -1, 2, 0, 2, -7141.0, 21.0, 8.0, 3070.0, 0.0, 4.0),
        (0, 0, 0, 2, 1, -6302.0, -11.0, 2.0, 3272.0, 0.0, 4.0),
        (1, 0, 2, -2, 1, 5800.0, 10.0, 2.0, -3045.0, 0.0, -1.0),
        (2, 0, 2, -2, 2, 6443.0, 0.0, -7.0, -2768.0, 0.0, -4.0),
        (-2, 0, 0, 2, 1, -5774.0, -11.0, -15.0, 3041.0, 0.0, -5.0),
        (2, 0, 2, 0, 1, -5350.0, 0.0, 21.0, 2695.0, 0.0, 12.0),
        (0, -1, 2, -2, 1, -4752.0, -11.0, -3.0, 2719.0, 0.0, -3.0),
        (0, 0, 0, -2, 1, -4940.0, -11.0, -21.0, 2720.0, 0.0, -9.0),
        (-1, -1, 0, 2, 0, 7350.0, 0.0, -8.0, -51.0, 0.0, 4.0),
        (2, 0, 0, -2, 1, 4065.0, 0.0, 6.0, -2206.0, 0.0, 1.0),
        (1, 0, 0, 2, 0, 6579.0, 0.0, -24.0, -199.0, 0.0, 2.0),
        (0, 1, 2, -2, 1, 3579.0, 0.0, 5.0, -1900.0, 0.0, 1.0),
        (1, -1, 0, 0, 0, 4725.0, 0.0, -6.0, -41.0, 0.0, 3.0),
        (-2, 0, 2, 0, 2, -3075.0, 0.0, -2.0, 1313.0, 0.0, -1.0),
        (3, 0, 2, 0, 2, -2904.0, 0.0, 15.0, 1233.0, 0.0, 7.0),
        (0, -1, 0, 2, 0, 4348.0, 0.0, -10.0, -81.0, 0.0, 2.0),
        (1, -1, 2, 0, 2, -2878.0, 0.0, 8.0, 1232.0, 0.0, 4.0),
        (0, 0, 0, 1, 0, -4230.0, 0.0, 5.0, -20.0, 0.0, -2.0),
        (-1, -1, 2, 2, 2, -2819.0, 0.0, 7.0, 1207.0, 0.0, 3.0),
        (-1, 0, 2, 0, 0, -4056.0, 0.0, 5.0, 40.0, 0.0, -2.0),
        (0, -1, 2, 2, 2, -2647.0, 0.0, 11.0, 1129.0, 0.0, 5.0),
        (-2, 0, 0, 0, 1, -2294.0, 0.0, -10.0, 1266.0, 0.0, -4.0),
        (1, 1, 2, 0, 2, 2481.0, 0.0, -7.0, -1062.0, 0.0, -3.0),
        (2, 0, 0, 0, 1, 2179.0, 0.0, -2.0, -1129.0, 0.0, -2.0),
        (-1, 1, 0, 1, 0, 3276.0, 0.0, 1.0, -9.0, 0.0, 0.0),
        (1, 1, 0, 0, 0, -3389.0, 0.0, 5.0, 35.0, 0.0, -2.0),
        (1, 0, 2, 0, 0, 3339.0, 0.0, -13.0, -107.0, 0.0, 1.0),
        (-1, 0, 2, -2, 1, -1987.0, 0.0, -6.0, 1073.0, 0.0, -2.0),
        (1, 0, 0, 0, 2, -1981.0, 0.0, 0.0, 854.0, 0.0, 0.0),
        (-1, 0, 0, 1, 0, 4026.0, 0.0, -353.0, -553.0, 0.0, -139.0),
        (0, 0, 2, 1, 2, 1660.0, 0.0, -5.0, -710.0, 0.0, -2.0),
        (-1, 0, 2, 4, 2, -1521.0, 0.0, 9.0, 647.0, 0.0, 4.0),
        (-1, 1, 0, 1, 1, 1314.0, 0.0, 0.0, -700.0, 0.0, 0.0),
        (0, -2, 2, -2, 1, -1283.0, 0.0, 0.0, 672.0, 0.0, 0.0),
        (1, 0, 2, 2, 1, -1331.0, 0.0, 8.0, 663.0, 0.0, 4.0),
        (-2, 0, 2, 2, 2, 1383.0, 0.0, -2.0, -594.0, 0.0, -2.0),
        (-1, 0, 0, 0, 2, 1405.0, 0.0, 4.0, -610.0, 0.0, 2.0),
        (1, 1, 2, -2, 2, 1290.0, 0.0, 0.0, -556.0, 0.0, 0.0),
    ];

    /// Nutation angles (Δψ, Δε) using the IAU 2000B truncated model.
    ///
    /// IAU 2000B is an 80-term truncated version of the IAU 2000A model
    /// (1320 lunisolar + planetary terms), with ~1 mas accuracy. This
    /// implementation uses the 77-term luni-solar series from ERFA plus
    /// the fixed planetary offsets DPPLAN/DEPLAN, matching ERFA's output
    /// to well below 1 arcsecond.
    ///
    /// Reference:
    /// - McCarthy & Luzum (2003), "An Abridged Model of the IAU 2000
    ///   Nutation Model", IERS TN 32
    ///   https://ui.adsabs.harvard.edu/abs/2004ITN....32.....M
    /// - ERFA nut00b.c
    ///   https://github.com/liberfa/erfa/blob/master/src/nut00b.c
    pub fn nutation_angles(&self, epoch: Epoch) -> (f64, f64) {
        let args = self.fundamental_arguments(epoch);
        let l = args[0];
        let lp = args[1];
        let f = args[2];
        let d = args[3];
        let om = args[4];

        let t = self.centuries_since_j2000(epoch);

        // Coefficients are in 0.1 μas. Convert to radians.
        let u2r = UAS * 0.1;

        let mut dp = 0.0_f64; // longitude accumulator in 0.1 μas
        let mut de = 0.0_f64; // obliquity accumulator in 0.1 μas

        for &(nl, nlp, nf, nd, nom, ps, pst, pc, ec, ect, es) in &Self::NUTATION_SERIES {
            let arg =
                nl as f64 * l + nlp as f64 * lp + nf as f64 * f + nd as f64 * d + nom as f64 * om;
            dp += (ps + pst * t) * arg.sin() + pc * arg.cos();
            de += (ec + ect * t) * arg.cos() + es * arg.sin();
        }

        let dpsi = dp * u2r + DPPLAN;
        let deps = de * u2r + DEPLAN;

        (dpsi, deps)
    }

    /// Nutation matrix N (mean equator of date → true equator of date).
    ///
    /// ERFA/SOFA construction (matches `eraNum00b`):
    ///
    ///   N = R_x(ε_A + Δε) · R_z(+Δψ) · R_x(-ε_A)
    ///
    /// where R_x and R_z are the standard active rotation matrices.
    ///
    /// Reference:
    /// - IERS Conventions (2010), §5.6
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf
    /// - ERFA num00b.c / num00a.c
    pub fn nutation_matrix(&self, epoch: Epoch) -> Matrix3<f64> {
        let (dpsi, deps) = self.nutation_angles(epoch);
        let eps_a = self.mean_obliquity(epoch);

        // R_x(-ε_A)
        let ceps = eps_a.cos();
        let seps = eps_a.sin();
        let rx_eps = Matrix3::new(1.0, 0.0, 0.0, 0.0, ceps, seps, 0.0, -seps, ceps);

        // R_z(+Δψ)
        let cdpsi = dpsi.cos();
        let sdpsi = dpsi.sin();
        let rz_dpsi = Matrix3::new(cdpsi, -sdpsi, 0.0, sdpsi, cdpsi, 0.0, 0.0, 0.0, 1.0);

        // R_x(ε_A + Δε)
        let eps_true = eps_a + deps;
        let cet = eps_true.cos();
        let set = eps_true.sin();
        let rx_true = Matrix3::new(1.0, 0.0, 0.0, 0.0, cet, -set, 0.0, set, cet);

        // N = R_x(ε_A + Δε) R_z(Δψ) R_x(-ε_A)
        rx_true * rz_dpsi * rx_eps
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
