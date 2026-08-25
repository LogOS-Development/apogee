//! Horizontal Wind Model (HWM).
//!
//! The full Horizontal Wind Model (HWM14) predicts meridional and zonal
//! thermospheric wind velocities as a function of altitude, latitude, local
//! time, season, solar activity, and geomagnetic activity.
//!
//! This module provides the API surface (`WindInput`, `WindOutput`,
//! `HorizontalWindModel`) and a trivial placeholder (`Hwm`) that returns zero
//! wind, allowing drag simulations to depend on a typed model without
//! requiring gfortran. When the `hwm14` feature is enabled the full HWM14
//! model is available via [`Hwm14`], which calls the vendored NRL Fortran
//! implementation through a C-ABI FFI boundary.

use apogee_common::units::{Meters, Radians};
use nalgebra::Vector3;

// ---------------------------------------------------------------------------
// Feature-gated FFI to the vendored HWM14 Fortran model.
// ---------------------------------------------------------------------------
// The Fortran source and C-ABI wrapper live under `vendor/`; the coefficient
// files live under `assets/`. `build.rs` compiles them into a static library
// when the `hwm14` feature is enabled. The Fortran model uses module-level
// mutable state, so all evaluation is serialized behind a global lock.

#[cfg(feature = "hwm14")]
extern "C" {
    fn hwm14_init();
    fn hwm14_evaluate(
        iyd: i32,
        sec: f32,
        alt: f32,
        glat: f32,
        glon: f32,
        stl: f32,
        f107a: f32,
        f107: f32,
        ap2: f32,
        meridional: *mut f32,
        zonal: *mut f32,
    );
}

#[cfg(feature = "hwm14")]
use std::sync::Once;

#[cfg(feature = "hwm14")]
static HWM14_INIT: Once = Once::new();

/// Evaluate the raw HWM14 Fortran routine.
///
/// Inputs match the Fortran routine's units:
/// - `iyd`: year/day as `yyddd`.
/// - `sec`: universal time in seconds.
/// - `alt`: altitude in kilometres.
/// - `glat`, `glon`: geodetic latitude/longitude in degrees.
/// - `stl`: local solar time in hours (unused by HWM14; pass -1).
/// - `ap2`: current 3-hour Ap index.
#[cfg(feature = "hwm14")]
#[allow(clippy::too_many_arguments)]
fn hwm14_ffi_evaluate(
    iyd: i32,
    sec: f64,
    alt: f64,
    glat: f64,
    glon: f64,
    stl: f64,
    f107a: f64,
    f107: f64,
    ap2: f64,
) -> (f64, f64) {
    use std::sync::Mutex;

    static EVAL_LOCK: Mutex<()> = Mutex::new(());

    HWM14_INIT.call_once(|| {
        let dir = write_hwm14_data_files();
        std::env::set_var("HWMPATH", &dir);
        unsafe { hwm14_init() };
    });

    let mut meridional: f32 = 0.0;
    let mut zonal: f32 = 0.0;

    let _guard = EVAL_LOCK.lock().unwrap();
    unsafe {
        hwm14_evaluate(
            iyd,
            sec as f32,
            alt as f32,
            glat as f32,
            glon as f32,
            stl as f32,
            f107a as f32,
            f107 as f32,
            ap2 as f32,
            &mut meridional,
            &mut zonal,
        );
    }

    (f64::from(meridional), f64::from(zonal))
}

#[cfg(feature = "hwm14")]
fn write_hwm14_data_files() -> std::path::PathBuf {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    const HWM123114_BIN: &[u8] = include_bytes!("../../external/hwm14/assets/hwm123114.bin");
    const DWM07B104I_DAT: &[u8] = include_bytes!("../../external/hwm14/assets/dwm07b104i.dat");
    const GD2QD_DAT: &[u8] = include_bytes!("../../external/hwm14/assets/gd2qd.dat");

    let base = std::env::temp_dir().join("apogee-hwm14-assets");
    fs::create_dir_all(&base).expect("failed to create HWM14 data directory");

    fn write_if_changed(path: &Path, data: &[u8]) {
        let current = fs::read(path).unwrap_or_default();
        if current != data {
            let mut file = fs::File::create(path)
                .unwrap_or_else(|e| panic!("failed to create {}: {}", path.display(), e));
            file.write_all(data)
                .unwrap_or_else(|e| panic!("failed to write {}: {}", path.display(), e));
        }
    }

    write_if_changed(&base.join("hwm123114.bin"), HWM123114_BIN);
    write_if_changed(&base.join("dwm07b104i.dat"), DWM07B104I_DAT);
    write_if_changed(&base.join("gd2qd.dat"), GD2QD_DAT);

    base
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Geodetic location and conditions for wind evaluation.
#[derive(Debug, Clone, Copy)]
pub struct WindInput {
    /// Altitude above the ellipsoid, metres.
    pub altitude_m: Meters<f64>,
    /// Geodetic latitude, radians.
    pub latitude_rad: Radians<f64>,
    /// Geodetic longitude, radians.
    pub longitude_rad: Radians<f64>,
    /// Local apparent solar time, hours.
    pub local_solar_time_hours: f64,
    /// Day of year (1..=366).
    pub day_of_year: u16,
    /// Daily F10.7 solar flux, sfu.
    pub f107: f64,
    /// Daily Ap geomagnetic index.
    pub ap: f64,
}

/// Wind velocity output, m/s, in the local East/North/Up frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindOutput {
    /// Eastward component, m/s.
    pub east_mps: f64,
    /// Northward component, m/s.
    pub north_mps: f64,
    /// Upward component, m/s.
    pub up_mps: f64,
}

impl WindOutput {
    /// Wind vector in the local ENU frame.
    pub fn enu(&self) -> Vector3<f64> {
        Vector3::new(self.east_mps, self.north_mps, self.up_mps)
    }
}

/// Trait for empirical horizontal wind models.
pub trait HorizontalWindModel: Send + Sync {
    /// Evaluate the wind at the given location and activity conditions.
    fn evaluate(&self, input: &WindInput) -> WindOutput;
}

/// HWM placeholder model that returns zero wind.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hwm;

impl Hwm {
    /// Evaluate the placeholder model.
    pub fn evaluate(_input: &WindInput) -> WindOutput {
        WindOutput::default()
    }
}

impl HorizontalWindModel for Hwm {
    fn evaluate(&self, input: &WindInput) -> WindOutput {
        Self::evaluate(input)
    }
}

/// HWM14 empirical horizontal wind model (Fortran via FFI).
///
/// Available only when the `hwm14` feature is enabled. The model coefficient
/// files are vendored under `assets/` and extracted to a temporary directory
/// on first use.
#[cfg(feature = "hwm14")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Hwm14;

#[cfg(feature = "hwm14")]
impl Hwm14 {
    /// Evaluate HWM14 for the given input.
    pub fn evaluate(input: &WindInput) -> WindOutput {
        let iyd = two_digit_year_and_doy(input.day_of_year);
        let sec = input.local_solar_time_hours * 3600.0; // HWM14 expects UT seconds; using LST as approximation
        let (meridional, zonal) = hwm14_ffi_evaluate(
            iyd,
            sec,
            input.altitude_m.into_value() / 1000.0,
            input.latitude_rad.into_value().to_degrees(),
            input.longitude_rad.into_value().to_degrees(),
            input.local_solar_time_hours,
            -1.0,
            -1.0,
            input.ap,
        );
        WindOutput {
            east_mps: zonal,
            north_mps: meridional,
            up_mps: 0.0,
        }
    }
}

#[cfg(feature = "hwm14")]
impl HorizontalWindModel for Hwm14 {
    fn evaluate(&self, input: &WindInput) -> WindOutput {
        Self::evaluate(input)
    }
}

#[cfg(feature = "hwm14")]
fn two_digit_year_and_doy(doy: u16) -> i32 {
    // HWM14's iyd is yyddd. The model is not sensitive to the year for
    // climatological winds, so we use a neutral reference year (1993).
    let year = 93;
    year * 1000 + i32::from(doy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_returns_zero() {
        let input = WindInput {
            altitude_m: Meters::new(400_000.0),
            latitude_rad: Radians::new(0.0),
            longitude_rad: Radians::new(0.0),
            local_solar_time_hours: 12.0,
            day_of_year: 80,
            f107: 150.0,
            ap: 4.0,
        };
        let out = Hwm::evaluate(&input);
        assert_eq!(out.east_mps, 0.0);
        assert_eq!(out.north_mps, 0.0);
        assert_eq!(out.up_mps, 0.0);
    }

    #[cfg(feature = "hwm14")]
    #[test]
    #[ignore = "requires gfortran and the vendored HWM14 coefficient files; run with -- --ignored or -- --include-ignored"]
    fn test_hwm14_evaluates_reference_case() {
        // Reference case from pyhwm2014 example: 1993 DOY 323, 12 UT,
        // 300 km, lat -11.95, lon -76.77, ap=35.
        let input = WindInput {
            altitude_m: Meters::new(300_000.0),
            latitude_rad: Radians::new((-11.95_f64).to_radians()),
            longitude_rad: Radians::new((-76.77_f64).to_radians()),
            local_solar_time_hours: 12.0,
            day_of_year: 323,
            f107: -1.0,
            ap: 35.0,
        };
        let out = Hwm14::evaluate(&input);
        assert!(out.east_mps.is_finite() && out.east_mps.abs() < 1000.0);
        assert!(out.north_mps.is_finite() && out.north_mps.abs() < 1000.0);
    }
}
