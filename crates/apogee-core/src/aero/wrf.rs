//! WRF physics schemes via FFI.
//!
//! Wraps selected WRF v4.8.0 Fortran physics parameterizations with
//! `iso_c_binding` C-ABI interfaces. Currently supports:
//!
//! - Kessler microphysics — warm-rain single-moment scheme
//!
//! Compiled behind the `wrf` Cargo feature. Requires gfortran.
//!
//! # References
//! WRF Model v4.8.0: https://github.com/wrf-model/WRF
//! Kessler, E. (1969). On the Distribution and Continuity of Water
//! Substance in Atmospheric Circulations. Meteorological Monographs,
//! Vol. 10, No. 32.

#![cfg(feature = "wrf")]

use std::sync::Once;

static INIT: Once = Once::new();

/// WRF standard thermodynamic constants.
pub mod constants {
    pub const SVP1: f32 = 0.6112;
    pub const SVP2: f32 = 17.67;
    pub const SVP3: f32 = 29.65;
    pub const SVPT0: f32 = 273.15;
    pub const EP2: f32 = 0.622;
    pub const XLV: f32 = 2.5e6;
    pub const CP: f32 = 1004.0;
    pub const RHOWATER: f32 = 1000.0;
}

extern "C" {
    fn wrf_kessler_column(
        nk: i32,
        t: *const f32,
        qv: *const f32,
        qc: *const f32,
        qr: *const f32,
        rho: *const f32,
        pii: *const f32,
        z: *const f32,
        dz8w: *const f32,
        dt: f32,
        xlv: f32,
        cp: f32,
        ep2: f32,
        svp1: f32,
        svp2: f32,
        svp3: f32,
        svpt0: f32,
        rhowater: f32,
        t_out: *mut f32,
        qv_out: *mut f32,
        qc_out: *mut f32,
        qr_out: *mut f32,
        rainnc: *mut f32,
        rainncv: *mut f32,
    );
}

/// Input state for a single atmospheric column.
#[derive(Debug, Clone)]
pub struct KesslerInput {
    /// Potential temperature at each level (K).
    pub theta: Vec<f32>,
    /// Water vapor mixing ratio (kg/kg).
    pub qv: Vec<f32>,
    /// Cloud water mixing ratio (kg/kg).
    pub qc: Vec<f32>,
    /// Rain water mixing ratio (kg/kg).
    pub qr: Vec<f32>,
    /// Air density (kg/m^3).
    pub rho: Vec<f32>,
    /// Exner function (dimensionless).
    pub pii: Vec<f32>,
    /// Height of each level (m).
    pub z: Vec<f32>,
    /// Layer thickness (m).
    pub dz8w: Vec<f32>,
    /// Timestep (s).
    pub dt: f32,
}

/// Output state after Kessler microphysics.
#[derive(Debug, Clone)]
pub struct KesslerOutput {
    /// Updated potential temperature (K).
    pub theta: Vec<f32>,
    /// Updated water vapor mixing ratio (kg/kg).
    pub qv: Vec<f32>,
    /// Updated cloud water mixing ratio (kg/kg).
    pub qc: Vec<f32>,
    /// Updated rain water mixing ratio (kg/kg).
    pub qr: Vec<f32>,
    /// Accumulated precipitation at surface (mm).
    pub rain_accumulated: f32,
    /// Precipitation rate this step (mm).
    pub rain_rate: f32,
}

/// Run Kessler warm-rain microphysics on a single vertical column.
///
/// All input vectors must have the same length. The scheme handles
/// condensation, autoconversion, accretion, evaporation, and
/// sedimentation of rain.
pub fn kessler(input: &KesslerInput) -> KesslerOutput {
    INIT.call_once(|| {});

    let nk = input.theta.len() as i32;
    assert_eq!(input.qv.len(), nk as usize);
    assert_eq!(input.qc.len(), nk as usize);
    assert_eq!(input.qr.len(), nk as usize);
    assert_eq!(input.rho.len(), nk as usize);
    assert_eq!(input.pii.len(), nk as usize);
    assert_eq!(input.z.len(), nk as usize);
    assert_eq!(input.dz8w.len(), nk as usize);

    let mut theta_out = vec![0.0f32; nk as usize];
    let mut qv_out = vec![0.0f32; nk as usize];
    let mut qc_out = vec![0.0f32; nk as usize];
    let mut qr_out = vec![0.0f32; nk as usize];
    let mut rainnc = 0.0f32;
    let mut rainncv = 0.0f32;

    unsafe {
        wrf_kessler_column(
            nk,
            input.theta.as_ptr(),
            input.qv.as_ptr(),
            input.qc.as_ptr(),
            input.qr.as_ptr(),
            input.rho.as_ptr(),
            input.pii.as_ptr(),
            input.z.as_ptr(),
            input.dz8w.as_ptr(),
            input.dt,
            constants::XLV,
            constants::CP,
            constants::EP2,
            constants::SVP1,
            constants::SVP2,
            constants::SVP3,
            constants::SVPT0,
            constants::RHOWATER,
            theta_out.as_mut_ptr(),
            qv_out.as_mut_ptr(),
            qc_out.as_mut_ptr(),
            qr_out.as_mut_ptr(),
            &mut rainnc,
            &mut rainncv,
        );
    }

    KesslerOutput {
        theta: theta_out,
        qv: qv_out,
        qc: qc_out,
        qr: qr_out,
        rain_accumulated: rainnc,
        rain_rate: rainncv,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kessler_runs_and_modifies_state() {
        let nk = 5;
        let input = KesslerInput {
            theta: vec![300.0, 295.0, 290.0, 285.0, 280.0],
            qv: vec![0.015, 0.012, 0.010, 0.008, 0.006],
            qc: vec![0.0005, 0.001, 0.0008, 0.0003, 0.0],
            qr: vec![0.0, 0.0002, 0.0005, 0.0001, 0.0],
            rho: vec![1.0, 1.05, 1.1, 1.15, 1.2],
            pii: vec![1.0, 0.98, 0.96, 0.94, 0.92],
            z: vec![100.0, 300.0, 600.0, 1000.0, 1500.0],
            dz8w: vec![200.0, 200.0, 300.0, 400.0, 500.0],
            dt: 10.0,
        };

        let output = kessler(&input);

        assert_eq!(output.theta.len(), nk);
        assert_eq!(output.qv.len(), nk);

        // Temperature should change (condensation/evaporation heating)
        let temp_changed = output
            .theta
            .iter()
            .zip(input.theta.iter())
            .any(|(o, i)| (o - i).abs() > 1e-4);
        assert!(temp_changed, "theta should change after microphysics");

        // Water vapor should change
        let qv_changed = output
            .qv
            .iter()
            .zip(input.qv.iter())
            .any(|(o, i)| (o - i).abs() > 1e-6);
        assert!(qv_changed, "qv should change after microphysics");
    }

    #[test]
    fn kessler_dry_air_no_rain() {
        let input = KesslerInput {
            theta: vec![300.0, 290.0, 280.0],
            qv: vec![0.001, 0.001, 0.001],
            qc: vec![0.0, 0.0, 0.0],
            qr: vec![0.0, 0.0, 0.0],
            rho: vec![1.0, 1.1, 1.2],
            pii: vec![1.0, 0.97, 0.93],
            z: vec![100.0, 500.0, 1000.0],
            dz8w: vec![400.0, 400.0, 500.0],
            dt: 1.0,
        };

        let output = kessler(&input);

        // No cloud water, no rain → rain accumulation should be zero
        assert_eq!(output.rain_accumulated, 0.0);
    }
}
