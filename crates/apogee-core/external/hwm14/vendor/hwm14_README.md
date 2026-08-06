# hwm14 (vendored NRL HWM14 Fortran)

Rust FFI bindings to the NRL Horizontal Wind Model 14 (HWM14).

The original Fortran 90 implementation is vendored under `vendor/hwm14.f90`
and the model coefficient files are vendored under `assets/`. `build.rs`
compiles the Fortran into a static library using `gfortran`, so a working
Fortran compiler is required to build this crate.

## License

- The Fortran model (`vendor/hwm14.f90`) and data files (`assets/*`) are from
  the NRL HWM14 distribution and are public-domain U.S. government work.
- The C-ABI wrapper (`vendor/hwm14_c.f90`) and the Rust bindings are licensed
  under the MIT license.

## Usage

Enable the `hwm14` feature on `apogee-core`:

```toml
[dependencies]
apogee-core = { path = "../apogee-core", features = ["hwm14"] }
```

```rust
use apogee_core::aero::hwm::{Hwm14, HorizontalWindModel, WindInput};

let input = WindInput {
    altitude_m: 300_000.0,
    latitude_rad: (-11.95_f64).to_radians(),
    longitude_rad: (-76.77_f64).to_radians(),
    local_solar_time_hours: 12.0,
    day_of_year: 323,
    f107: -1.0,
    ap: 35.0,
};
let wind = Hwm14.evaluate(&input);
```

At runtime the coefficient files are written from embedded bytes to a
temporary directory and the `HWMPATH` environment variable is set so the
Fortran code can locate them.
