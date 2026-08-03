//! Rust FFI bindings to the NRL Horizontal Wind Model 14 (HWM14).
//!
//! The underlying model is implemented in Fortran (`vendor/hwm14.f90`) and
//! compiled into a static library by `build.rs`. The C-ABI wrapper in
//! `vendor/hwm14_c.f90` exposes two functions:
//!
//! - `hwm14_init()`: loads the model coefficient files.
//! - `hwm14_evaluate(...)`: returns the meridional (northward) and zonal
//!   (eastward) wind components in m/s.
//!
//! Model coefficient files (`*.bin`, `*.dat`) are vendored under `assets/`
//! and written to a temporary directory at runtime so the Fortran code can
//! locate them via the `HWMPATH` environment variable.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};

use once_cell::sync::OnceCell;

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

static INIT: Once = Once::new();
static DATA_DIR: OnceCell<PathBuf> = OnceCell::new();
static EVAL_LOCK: Mutex<()> = Mutex::new(());

const HWM123114_BIN: &[u8] = include_bytes!("../assets/hwm123114.bin");
const DWM07B104I_DAT: &[u8] = include_bytes!("../assets/dwm07b104i.dat");
const GD2QD_DAT: &[u8] = include_bytes!("../assets/gd2qd.dat");

/// HWM14 model handle.
///
/// Construct once and reuse. The first construction (or first call to
/// [`Hwm14::evaluate`]) writes the coefficient files to a temporary directory
/// and initializes the Fortran state. The Fortran model uses module-level
/// mutable state, so all evaluation is serialized behind a global lock.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hwm14;

impl Hwm14 {
    /// Create a new HWM14 model handle.
    pub fn new() -> Self {
        Self::ensure_initialized();
        Self
    }

    /// Evaluate HWM14 for the given location and activity conditions.
    ///
    /// Inputs are in the same units expected by the Fortran routine:
    /// - `iyd`: year/day as `yyddd` (e.g., 1993323 for DOY 323 of 1993).
    /// - `sec`: universal time in seconds.
    /// - `alt`: altitude in kilometres.
    /// - `glat`, `glon`: geodetic latitude/longitude in degrees.
    /// - `stl`: local solar time in hours (unused by HWM14; pass -1).
    /// - `ap2`: current 3-hour Ap index.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate(
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
        Self::ensure_initialized();

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

    fn ensure_initialized() {
        INIT.call_once(|| {
            let dir = write_data_files();
            DATA_DIR
                .set(dir.clone())
                .expect("HWM14 data dir already set");
            env::set_var("HWMPATH", &dir);

            unsafe {
                hwm14_init();
            }
        });
    }
}

fn write_data_files() -> PathBuf {
    let base = env::temp_dir().join("hwm14-sys-assets");
    fs::create_dir_all(&base).expect("failed to create HWM14 data directory");

    write_if_changed(&base.join("hwm123114.bin"), HWM123114_BIN);
    write_if_changed(&base.join("dwm07b104i.dat"), DWM07B104I_DAT);
    write_if_changed(&base.join("gd2qd.dat"), GD2QD_DAT);

    base
}

fn write_if_changed(path: &Path, data: &[u8]) {
    let current = fs::read(path).unwrap_or_default();
    if current != data {
        let mut file = fs::File::create(path)
            .unwrap_or_else(|e| panic!("failed to create {}: {}", path.display(), e));
        file.write_all(data)
            .unwrap_or_else(|e| panic!("failed to write {}: {}", path.display(), e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires gfortran and the vendored HWM14 coefficient files; run with -- --ignored or -- --include-ignored"]
    fn test_hwm14_evaluates_reference_case() {
        // Reference case from pyhwm2014 example: 1993 DOY 323, 12 UT,
        // 300 km, lat -11.95, lon -76.77, ap=35.
        let (meridional, zonal) = Hwm14::evaluate(
            93323,
            12.0 * 3600.0,
            300.0,
            -11.95,
            -76.77,
            -1.0,
            -1.0,
            -1.0,
            35.0,
        );
        assert!(meridional.is_finite() && meridional.abs() < 1000.0);
        assert!(zonal.is_finite() && zonal.abs() < 1000.0);
    }
}
