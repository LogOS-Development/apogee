//! Jacchia-Bowman 2008 (JB2008) thermospheric density model.
//!
//! Ported from the reference Python/Numba implementation in
//! `pyatmos` (itself a translation of the Bruce R. Bowman 2008 Fortran
//! source). The model computes total mass density and local/exospheric
//! temperature for altitudes from 90 km to 2500 km, given:
//!
//! - modified Julian date (MJD) and day-of-year fraction
//! - Sun right-ascension / declination (radians)
//! - Satellite right-ascension, geocentric latitude, and altitude (km)
//! - Space-weather indices: F10, F10B, S10, S10B, M10, M10B, Y10, Y10B
//! - Dst-derived temperature correction DTCVAL
//!
//! This module covers the **core numerical algorithm**. Loading the
//! space-weather indices and computing Sun/sidereal geometry are handled
//! by companion modules (`crate::aero::space_weather` and the frame/time
//! services). For the initial port the public API accepts the raw model
//! inputs directly.
//!
//! Reference:
//! - Bowman, Bruce R., et al. "A New Empirical Thermospheric Density Model
//!   JB2008 Using New Solar and Geomagnetic Indices", AIAA/AAS 2008,
//!   COSPAR CIRA 2008 Model.
//! - `pyatmos` JB2008 implementation:
//!   https://github.com/geospace-code/pyatmos

use apogee_common::units::{Density, Kelvins};

use crate::aero::model::{AtmosphereInput, AtmosphereModel, AtmosphereOutput, SpeciesDensities};

/// Input bundle for the full JB2008 algorithm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JacchiaBowmanInput {
    /// Modified Julian Date (JD − 2_400_000.5).
    pub mjd: f64,
    /// Day of year as a fractional day (1.0 = Jan 1 00:00).
    pub yday: f64,
    /// Sun right-ascension and declination, radians.
    pub sun: (f64, f64),
    /// Satellite right-ascension (rad), geocentric latitude (rad), altitude (km).
    pub sat: (f64, f64, f64),
    /// Daily F10.7 flux (1-day lagged).
    pub f10: f64,
    /// 81-day centered F10.7 average.
    pub f10b: f64,
    /// Daily EUV S10 index (1-day lagged), scaled to F10.
    pub s10: f64,
    /// 81-day centered S10 average.
    pub s10b: f64,
    /// Daily MG2 M10 index (2-day lagged), scaled to F10.
    pub m10: f64,
    /// 81-day centered M10 average.
    pub m10b: f64,
    /// Daily solar X-ray/Lyα Y10 index (5-day lagged), scaled to F10.
    pub y10: f64,
    /// 81-day centered Y10 average.
    pub y10b: f64,
    /// Dst-derived temperature correction, K.
    pub dstdtc: f64,
}

/// Jacchia-Bowman 2008 model.
#[derive(Debug, Clone, Copy, Default)]
pub struct JacchiaBowman;

impl JacchiaBowman {
    /// Evaluate the full JB2008 density/temperature model.
    pub fn evaluate(input: &JacchiaBowmanInput) -> AtmosphereOutput {
        let (temp, rho) = jb2008_core(input);

        AtmosphereOutput {
            density: Density::new(rho),
            temperature: Kelvins::new(temp[0]),
            temperature_alt: Kelvins::new(temp[1]),
            number_densities: SpeciesDensities::default(),
        }
    }

    /// Convenience wrapper that ignores JB2008-specific indices and returns an
    /// approximate density from F10.7/Ap-only inputs.
    ///
    /// This is a stopgap for callers that only have `AtmosphereInput`; full
    /// integration requires deriving S10/M10/Y10/DTCVAL from space-weather
    /// data.
    pub fn evaluate_approx(input: &AtmosphereInput) -> AtmosphereOutput {
        let altitude_km = input.altitude_m.into_value() / 1000.0;
        let h = 7.0 + 0.02 * (input.f107 - 150.0);
        let h = h.max(5.0);
        let rho = 1.225 * (-altitude_km / h).exp();
        let t = 200.0 + 4.0 * altitude_km + 0.5 * (input.f107 - 70.0);

        AtmosphereOutput {
            density: Density::new(rho),
            temperature: Kelvins::new(t),
            temperature_alt: Kelvins::new(t),
            number_densities: SpeciesDensities::default(),
        }
    }
}

impl AtmosphereModel for JacchiaBowman {
    fn evaluate(&self, input: &AtmosphereInput) -> AtmosphereOutput {
        Self::evaluate_approx(input)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Core JB2008 algorithm
// ═══════════════════════════════════════════════════════════════════════════

/// Molecular weights in order: N2, O2, O, Ar, He, H (kg/kmol).
const AMW: [f64; 6] = [28.0134, 31.9988, 15.9994, 39.9480, 4.0026, 1.00797];

/// Sea-level volume fractions: N2, O2, Ar, He.
const FRAC: [f64; 4] = [0.7811, 0.20955, 9.34e-3, 1.289e-5];

/// Universal gas constant (J / K / kmol).
const RSTAR: f64 = 8314.32;

/// Avogadro's number (molecules / kmol).
const AVOGAD: f64 = 6.02257e26;

/// Newton-Cotes 5-point quadrature weights, scaled by 2/45.
const WT: [f64; 5] = [
    14.0 / 45.0,
    64.0 / 45.0,
    24.0 / 45.0,
    64.0 / 45.0,
    14.0 / 45.0,
];

/// High-altitude density correction coefficients.
const CHT: [f64; 4] = [0.22, -0.002, 0.00115, -0.00000211];

fn jb2008_core(input: &JacchiaBowmanInput) -> ([f64; 2], f64) {
    let (sun_ra, sun_dec) = input.sun;
    let (sat_ra, sat_lat, sat_alt) = input.sat;
    let mjd = input.mjd;
    let yday = input.yday;
    let zht = sat_alt;

    // Equation (14): base exospheric temperature.
    let mut fn_ = (input.f10b / 240.0).powf(0.25);
    if fn_ > 1.0 {
        fn_ = 1.0;
    }
    let fsb = input.f10b * fn_ + input.s10b * (1.0 - fn_);
    let tsubc = 392.4
        + 3.227 * fsb
        + 0.298 * (input.f10 - input.f10b)
        + 2.259 * (input.s10 - input.s10b)
        + 0.312 * (input.m10 - input.m10b)
        + 0.178 * (input.y10 - input.y10b);

    // Equations (15)–(17): local solar-time / latitude correction.
    let eta = (sat_lat - sun_dec).abs() / 2.0;
    let theta = (sat_lat + sun_dec).abs() / 2.0;
    let h = sat_ra - sun_ra;
    let tau = h - 0.64577182 + 0.10471976 * (h + 0.75049158).sin();
    let c = eta.cos().powf(2.5);
    let s = theta.sin().powf(2.5);
    let df = s + (c - s) * (0.5 * tau).cos().abs().powf(3.0);
    let tubl = tsubc * (1.0 + 0.31 * df);

    // Local solar time (hours) and dTc correction.
    let glst = h + std::f64::consts::PI;
    let glsthr = ((glst / DEGRAD) * (24.0 / 360.0)).rem_euclid(24.0);
    let dtclst = dtsub(input.f10, glsthr, sat_lat, zht);

    let mut temp = [0.0; 2];
    temp[0] = tubl + input.dstdtc;
    let tinf = temp[0] + dtclst;

    // Equation (9) and (11).
    let tsubx = 444.3807 + 0.02385 * tinf - 392.8292 * (-0.0021357 * tinf).exp();
    let gsubx = 0.054285714 * (tsubx - 183.0);
    let tc = [
        tsubx,
        gsubx,
        (tinf - tsubx) / PIOV2,
        gsubx / ((tinf - tsubx) / PIOV2),
    ];

    // Equation (5): integrate from 90 km to min(altitude, 105 km).
    let z1 = 90.0;
    let z2 = zht.min(105.0);
    let al = (z2 / z1).ln();
    let r1 = 0.01;
    let n = (al / r1).floor() as usize + 1;
    let zr = (al / n as f64).exp();

    let ambar1 = xambar(z1);
    let tloc1 = xlocal(z1, &tc);
    let mut zend = z1;
    let mut sum2 = 0.0;
    let mut ain = ambar1 * xgrav(z1) / tloc1;
    let mut ambar2 = ambar1;
    let mut tloc2 = tloc1;

    for _ in 0..n {
        let z = zend;
        zend = zr * z;
        let dz = 0.25 * (zend - z);
        let mut sum1 = WT[0] * ain;
        let mut zz = z;
        #[allow(clippy::needless_range_loop)]
        for j in 1..5 {
            zz += dz;
            ambar2 = xambar(zz);
            tloc2 = xlocal(zz, &tc);
            let gravl = xgrav(zz);
            ain = ambar2 * gravl / tloc2;
            sum1 += WT[j] * ain;
        }
        sum2 += dz * sum1;
    }

    let fact1 = 1e3 / RSTAR;
    let mut rho = 3.46e-6 * ambar2 * tloc1 * (-fact1 * sum2).exp() / ambar1 / tloc2;

    // Equation (2)–(4): number densities at the lower boundary.
    let anm = AVOGAD * rho;
    let an = anm / ambar2;
    let fact2 = anm / 28.96;
    let mut aln = [0.0; 6];
    aln[0] = (FRAC[0] * fact2).ln();
    aln[3] = (FRAC[2] * fact2).ln();
    aln[4] = (FRAC[3] * fact2).ln();
    aln[1] = (fact2 * (1.0 + FRAC[1]) - an).ln();
    aln[2] = (2.0 * (an - fact2)).ln();

    if (0.0..=105.0).contains(&zht) {
        temp[1] = tloc2;
        aln[5] = aln[4] - 25.0;
    } else {
        // Equation (6): integrate from current altitude up to min(altitude, 500 km).
        // Thermal diffusion coefficients ALPHA are zero except for He (-0.38).
        let alpha = [0.0, 0.0, 0.0, 0.0, -0.38, 0.0];
        let r2 = 0.025;
        let z3 = zht.min(500.0);
        let al = (z3 / zend).ln();
        let n = (al / r2).floor() as usize + 1;
        let zr = (al / n as f64).exp();
        let mut sum2b = 0.0;
        ain = xgrav(zend) / tloc2;
        let mut tloc3 = tloc2;
        let mut tloc4 = tloc2;

        for _ in 0..n {
            let z = zend;
            zend = zr * z;
            let dz = 0.25 * (zend - z);
            let mut sum1 = WT[0] * ain;
            let mut zz = z;
            #[allow(clippy::needless_range_loop)]
            #[allow(clippy::needless_range_loop)]
            for j in 1..5 {
                zz += dz;
                tloc3 = xlocal(zz, &tc);
                let gravl = xgrav(zz);
                ain = gravl / tloc3;
                sum1 += WT[j] * ain;
            }
            sum2b += dz * sum1;
        }

        // Integrate from 500 km up to altitude if above 500 km.
        let z4 = zht.max(500.0);
        let al = (z4 / zend).ln();
        let r = if zht > 500.0 { 0.075 } else { r2 };
        let n = (al / r).floor() as usize + 1;
        let zr = (al / n as f64).exp();
        let mut sum3 = 0.0;

        for _ in 0..n {
            let z = zend;
            zend = zr * z;
            let dz = 0.25 * (zend - z);
            let mut sum1 = WT[0] * ain;
            let mut zz = z;
            #[allow(clippy::needless_range_loop)]
            #[allow(clippy::needless_range_loop)]
            for j in 1..5 {
                zz += dz;
                tloc4 = xlocal(zz, &tc);
                let gravl = xgrav(zz);
                ain = gravl / tloc4;
                sum1 += WT[j] * ain;
            }
            sum3 += dz * sum1;
        }

        let (_t500, temp_alt, altr, fact2b, hsign) = if zht <= 500.0 {
            (tloc4, tloc3, (tloc3 / tloc2).ln(), fact1 * sum2b, 1.0)
        } else {
            (
                tloc3,
                tloc4,
                (tloc4 / tloc2).ln(),
                fact1 * (sum2b + sum3),
                -1.0,
            )
        };
        temp[1] = temp_alt;

        #[allow(clippy::needless_range_loop)]
        for i in 0..5 {
            aln[i] -= (1.0 + alpha[i]) * altr + fact2b * AMW[i];
        }

        let al10t5 = tinf.log10();
        let alnh5 = (5.5 * al10t5 - 39.4) * al10t5 + 73.13;
        let al10 = 10.0_f64.ln();
        aln[5] = al10 * (alnh5 + 6.0) + hsign * ((tloc4 / tloc3).ln() + fact1 * sum3 * AMW[5]);
    }

    // Equation (24): seasonal-latitudinal variation.
    let trash = (mjd - 36204.0) / 365.2422;
    let capphi = trash.fract();
    let pi = std::f64::consts::PI;
    let twopi = 2.0 * pi;
    let dlrsl = 0.02
        * (zht - 90.0)
        * (-0.045 * (zht - 90.0)).exp()
        * sat_lat.signum()
        * (twopi * capphi + 1.72).sin()
        * sat_lat.sin().powi(2);

    // Equation (23): semiannual variation.
    let mut dlr_sa = 0.0;
    if zht < 2000.0 {
        let (fzz, _gtz, dlrsa) = semian08(yday, zht, input.f10b, input.s10b, input.m10b);
        if fzz >= 0.0 {
            dlr_sa = dlrsa;
        }
    }

    let al10 = 10.0_f64.ln();
    let dlr = al10 * (dlrsl + dlr_sa);
    #[allow(clippy::needless_range_loop)]
    for i in 0..6 {
        aln[i] += dlr;
    }

    let an: Vec<f64> = aln.iter().map(|x| x.exp()).collect();
    let sumnm: f64 = an.iter().zip(AMW.iter()).map(|(a, m)| a * m).sum();
    rho = sumnm / AVOGAD;

    // High-altitude exospheric density correction, Equation (??).
    let mut fex = 1.0;
    if (1000.0..1500.0).contains(&zht) {
        let zeta = (zht - 1000.0) * 0.002;
        let zeta2 = zeta * zeta;
        let zeta3 = zeta2 * zeta;
        let f15c = CHT[0] + CHT[1] * input.f10b + (CHT[2] + CHT[3] * input.f10b) * 1500.0;
        let f15c_zeta = (CHT[2] + CHT[3] * input.f10b) * 500.0;
        let fex2 = 3.0 * f15c - f15c_zeta - 3.0;
        let fex3 = f15c_zeta - 2.0 * f15c + 2.0;
        fex = 1.0 + fex2 * zeta2 + fex3 * zeta3;
    } else if zht >= 1500.0 {
        fex = CHT[0] + CHT[1] * input.f10b + CHT[2] * zht + CHT[3] * input.f10b * zht;
    }
    rho *= fex;

    (temp, rho)
}

/// Evaluate Equation (1): mean molecular weight as a function of altitude.
fn xambar(z: f64) -> f64 {
    let c = [
        28.15204, -8.5586e-2, 1.2840e-4, -1.0056e-5, -1.0210e-5, 1.5044e-6, 9.9826e-8,
    ];
    let dz = z - 100.0;
    let mut amb = c[6];
    for i in (0..=5).rev() {
        amb = dz * amb + c[i];
    }
    amb
}

/// Evaluate Equation (8): gravity as a function of altitude (km).
fn xgrav(z: f64) -> f64 {
    9.80665 / (1.0 + z / 6356.766).powi(2)
}

/// Evaluate Equation (10) or (13): local temperature profile.
fn xlocal(z: f64, tc: &[f64; 4]) -> f64 {
    let dz = z - 125.0;
    if dz > 0.0 {
        tc[0] + tc[2] * (tc[3] * dz * (1.0 + 4.5e-6 * dz.powf(2.5))).atan()
    } else {
        let a = -9.8204695e-6 * dz - 7.3039742e-4;
        (a * dz * dz + 1.0) * dz * tc[1] + tc[0]
    }
}

#[allow(dead_code)]
const PIOV2: f64 = std::f64::consts::FRAC_PI_2;
#[allow(dead_code)]
const DEGRAD: f64 = std::f64::consts::PI / 180.0;

/// Compute dTc correction for Jacchia-Bowman model.
#[allow(unused_variables)]
fn dtsub(f10: f64, xlst: f64, xlat: f64, zht: f64) -> f64 {
    // Coefficient arrays B and C from the reference implementation.
    let b = [
        -4.57512297,
        -5.12114909,
        -69.3003609,
        203.716701,
        703.316291,
        -1943.49234,
        1106.51308,
        -174.378996,
        1885.94601,
        -7093.71517,
        9224.54523,
        -3845.08073,
        -6.45841789,
        40.9703319,
        -482.006560,
        1818.70931,
        -2373.89204,
        996.703815,
        36.1416936,
    ];
    let c = [
        -15.5986211,
        -5.12114909,
        -69.3003609,
        203.716701,
        703.316291,
        -1943.49234,
        1106.51308,
        -220.835117,
        1432.56989,
        -3184.81844,
        3289.81513,
        -1353.32119,
        19.9956489,
        -12.7093998,
        21.2825156,
        -2.75555432,
        11.0234982,
        148.881951,
        -751.640284,
        637.876542,
        12.7093998,
        -21.2825156,
        2.75555432,
    ];

    let tx = xlst / 24.0;
    let ycs = xlat.cos();
    let f = (f10 - 100.0) / 100.0;
    let mut dtc = 0.0;

    if (120.0..=200.0).contains(&zht) {
        let h = (zht - 200.0) / 50.0;
        let dtc200 = c[16]
            + c[17] * tx * ycs
            + c[18] * tx * tx * ycs
            + c[19] * tx.powi(3) * ycs
            + c[20] * f * ycs
            + c[21] * tx * f * ycs
            + c[22] * tx * tx * f * ycs;

        let sum = c[0]
            + b[1] * f
            + c[2] * tx * f
            + c[3] * tx * tx * f
            + c[4] * tx.powi(3) * f
            + c[5] * tx.powi(4) * f
            + c[6] * tx.powi(5) * f
            + c[7] * tx * ycs
            + c[8] * tx * tx * ycs
            + c[9] * tx.powi(3) * ycs
            + c[10] * tx.powi(4) * ycs
            + c[11] * tx.powi(5) * ycs
            + c[12] * ycs
            + c[13] * f * ycs
            + c[14] * tx * f * ycs
            + c[15] * tx * tx * f * ycs;
        let dtc200dz = sum;
        let cc = 3.0 * dtc200 - dtc200dz;
        let dd = dtc200 - cc;
        let zp = (zht - 120.0) / 80.0;
        dtc = cc * zp * zp + dd * zp * zp * zp;
    } else if (200.0..=240.0).contains(&zht) {
        let h = (zht - 200.0) / 50.0;
        let sum = c[0] * h
            + b[1] * f * h
            + c[2] * tx * f * h
            + c[3] * tx * tx * f * h
            + c[4] * tx.powi(3) * f * h
            + c[5] * tx.powi(4) * f * h
            + c[6] * tx.powi(5) * f * h
            + c[7] * tx * ycs * h
            + c[8] * tx * tx * ycs * h
            + c[9] * tx.powi(3) * ycs * h
            + c[10] * tx.powi(4) * ycs * h
            + c[11] * tx.powi(5) * ycs * h
            + c[12] * ycs * h
            + c[13] * f * ycs * h
            + c[14] * tx * f * ycs * h
            + c[15] * tx * tx * f * ycs * h
            + c[16]
            + c[17] * tx * ycs
            + c[18] * tx * tx * ycs
            + c[19] * tx.powi(3) * ycs
            + c[20] * f * ycs
            + c[21] * tx * f * ycs
            + c[22] * tx * tx * f * ycs;
        dtc = sum;
    } else if (240.0..=300.0).contains(&zht) {
        let h = 0.8;
        let sum1 = c[0] * h
            + b[1] * f * h
            + c[2] * tx * f * h
            + c[3] * tx * tx * f * h
            + c[4] * tx.powi(3) * f * h
            + c[5] * tx.powi(4) * f * h
            + c[6] * tx.powi(5) * f * h
            + c[7] * tx * ycs * h
            + c[8] * tx * tx * ycs * h
            + c[9] * tx.powi(3) * ycs * h
            + c[10] * tx.powi(4) * ycs * h
            + c[11] * tx.powi(5) * ycs * h
            + c[12] * ycs * h
            + c[13] * f * ycs * h
            + c[14] * tx * f * ycs * h
            + c[15] * tx * tx * f * ycs * h
            + c[16]
            + c[17] * tx * ycs
            + c[18] * tx * tx * ycs
            + c[19] * tx.powi(3) * ycs
            + c[20] * f * ycs
            + c[21] * tx * f * ycs
            + c[22] * tx * tx * f * ycs;
        let aa = sum1;
        let bb = c[0]
            + b[1] * f
            + c[2] * tx * f
            + c[3] * tx * tx * f
            + c[4] * tx.powi(3) * f
            + c[5] * tx.powi(4) * f
            + c[6] * tx.powi(5) * f
            + c[7] * tx * ycs
            + c[8] * tx * tx * ycs
            + c[9] * tx.powi(3) * ycs
            + c[10] * tx.powi(4) * ycs
            + c[11] * tx.powi(5) * ycs
            + c[12] * ycs
            + c[13] * f * ycs
            + c[14] * tx * f * ycs
            + c[15] * tx * tx * f * ycs;
        let h = 3.0;
        let sum2 = b[0]
            + b[1] * f
            + b[2] * tx * f
            + b[3] * tx * tx * f
            + b[4] * tx.powi(3) * f
            + b[5] * tx.powi(4) * f
            + b[6] * tx.powi(5) * f
            + b[7] * tx * ycs
            + b[8] * tx * tx * ycs
            + b[9] * tx.powi(3) * ycs
            + b[10] * tx.powi(4) * ycs
            + b[11] * tx.powi(5) * ycs
            + b[12] * h * ycs
            + b[13] * tx * h * ycs
            + b[14] * tx * tx * h * ycs
            + b[15] * tx.powi(3) * h * ycs
            + b[16] * tx.powi(4) * h * ycs
            + b[17] * tx.powi(5) * h * ycs
            + b[18] * ycs;
        let dtc300 = sum2;
        let dtc300dz = b[12] * ycs
            + b[13] * tx * ycs
            + b[14] * tx * tx * ycs
            + b[15] * tx.powi(3) * ycs
            + b[16] * tx.powi(4) * ycs
            + b[17] * tx.powi(5) * ycs;
        let cc = 3.0 * dtc300 - dtc300dz - 3.0 * aa - 2.0 * bb;
        let dd = dtc300 - aa - bb - cc;
        let zp = (zht - 240.0) / 60.0;
        dtc = aa + bb * zp + cc * zp * zp + dd * zp * zp * zp;
    } else if (300.0..=600.0).contains(&zht) {
        let h = zht / 100.0;
        let sum = b[0]
            + b[1] * f
            + b[2] * tx * f
            + b[3] * tx * tx * f
            + b[4] * tx.powi(3) * f
            + b[5] * tx.powi(4) * f
            + b[6] * tx.powi(5) * f
            + b[7] * tx * ycs
            + b[8] * tx * tx * ycs
            + b[9] * tx.powi(3) * ycs
            + b[10] * tx.powi(4) * ycs
            + b[11] * tx.powi(5) * ycs
            + b[12] * h * ycs
            + b[13] * tx * h * ycs
            + b[14] * tx * tx * h * ycs
            + b[15] * tx.powi(3) * h * ycs
            + b[16] * tx.powi(4) * h * ycs
            + b[17] * tx.powi(5) * h * ycs
            + b[18] * ycs;
        dtc = sum;
    } else if (600.0..=800.0).contains(&zht) {
        let zp = (zht - 600.0) / 100.0;
        let hp = 6.0;
        let aa = b[0]
            + b[1] * f
            + b[2] * tx * f
            + b[3] * tx * tx * f
            + b[4] * tx.powi(3) * f
            + b[5] * tx.powi(4) * f
            + b[6] * tx.powi(5) * f
            + b[7] * tx * ycs
            + b[8] * tx * tx * ycs
            + b[9] * tx.powi(3) * ycs
            + b[10] * tx.powi(4) * ycs
            + b[11] * tx.powi(5) * ycs
            + b[12] * hp * ycs
            + b[13] * tx * hp * ycs
            + b[14] * tx * tx * hp * ycs
            + b[15] * tx.powi(3) * hp * ycs
            + b[16] * tx.powi(4) * hp * ycs
            + b[17] * tx.powi(5) * hp * ycs
            + b[18] * ycs;
        let bb = b[12] * ycs
            + b[13] * tx * ycs
            + b[14] * tx * tx * ycs
            + b[15] * tx.powi(3) * ycs
            + b[16] * tx.powi(4) * ycs
            + b[17] * tx.powi(5) * ycs;
        let cc = -(3.0 * aa + 4.0 * bb) / 4.0;
        let dd = (aa + bb) / 4.0;
        dtc = aa + bb * zp + cc * zp * zp + dd * zp * zp * zp;
    }

    dtc
}

/// Compute semiannual variation (delta log rho).
fn semian08(day: f64, ht: f64, f10b: f64, s10b: f64, m10b: f64) -> (f64, f64, f64) {
    let twopi = 2.0 * std::f64::consts::PI;

    // FZ global model values (1997–2006 fit).
    let fzm = [0.2689, -0.01176, 0.02782, -0.02782, 0.3470e-3];

    // GT global model values (1997–2006 fit).
    let gtm = [
        -0.3633, 0.08506, 0.2401, -0.1897, -0.2554, -0.01790, 0.5650e-3, -0.6407e-3, -0.3418e-2,
        -0.1252e-2,
    ];

    let fsmb = f10b - 0.7 * s10b - 0.04 * m10b;
    let htz = ht / 1e3;
    let fzz = fzm[0]
        + fzm[1] * fsmb
        + fzm[2] * fsmb * htz
        + fzm[3] * fsmb * htz * htz
        + fzm[4] * fsmb * fsmb * htz;

    let fsmb2 = f10b - 0.75 * s10b - 0.37 * m10b;
    let tau = (day - 1.0) / 365.0;
    let sin1p = (twopi * tau).sin();
    let cos1p = (twopi * tau).cos();
    let sin2p = (2.0 * twopi * tau).sin();
    let cos2p = (2.0 * twopi * tau).cos();

    let gtz = gtm[0]
        + gtm[1] * sin1p
        + gtm[2] * cos1p
        + gtm[3] * sin2p
        + gtm[4] * cos2p
        + gtm[5] * fsmb2
        + gtm[6] * fsmb2 * sin1p
        + gtm[7] * fsmb2 * cos1p
        + gtm[8] * fsmb2 * sin2p
        + gtm[9] * fsmb2 * cos2p;

    let fzz_eff = fzz.max(1e-6);
    let drlog = fzz_eff * gtz;

    (fzz, gtz, drlog)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference case: 2014-07-22 22:18:45 UTC, lat 25°, lon 102°, alt 600 km
    /// with approximate Sun position (RA=0, Dec=0) and sampled space-weather
    /// indices. The absolute density will differ from a full geometry solution,
    /// but this test ensures the algorithm runs and returns physically
    /// reasonable values.
    fn reference_input(altitude_km: f64) -> JacchiaBowmanInput {
        JacchiaBowmanInput {
            mjd: 56860.9296875,
            yday: 202.9296875,
            sun: (0.0, 0.0),
            sat: (0.0, 25.0_f64.to_radians(), altitude_km),
            f10: 90.1,
            f10b: 128.4,
            s10: 99.0,
            s10b: 134.2,
            m10: 91.4,
            m10b: 130.3,
            y10: 100.8,
            y10b: 121.9,
            dstdtc: 32.3125,
        }
    }

    #[test]
    fn test_jb2008_runs_for_reference_case() {
        let input = reference_input(600.0);
        let out = JacchiaBowman::evaluate(&input);
        let rho = out.density.into_value();
        assert!(
            rho.is_finite() && rho > 0.0,
            "density should be positive finite"
        );
        assert!(
            out.temperature.into_value() > 500.0,
            "exospheric T should be > 500 K"
        );
        assert!(
            out.temperature_alt.into_value() > 200.0,
            "local T should be > 200 K"
        );
    }

    #[test]
    fn test_density_decreases_with_altitude() {
        let low = JacchiaBowman::evaluate(&reference_input(200.0));
        let high = JacchiaBowman::evaluate(&reference_input(800.0));
        assert!(low.density.into_value() > high.density.into_value());
    }
}
