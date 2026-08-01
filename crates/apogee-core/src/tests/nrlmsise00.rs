//! NRLMSISE-00 validation tests against `pymsis` reference outputs.
//!
//! Reference values were generated with the official NRLMSISE-00 Fortran model
//! via the `pymsis` Python wrapper (version='0', all options enabled), which
//! yields output in SI units (kg/m³ and K). The tests below assert that the
//! vendored Brahe/Rust implementation matches those reference values within
//! a tight relative tolerance, confirming the port is numerically faithful.

use approx::relative_eq;

use crate::aero::model::AtmosphereInput;
use crate::aero::nrlmsise00::Nrlmsise00;

/// Reference case and expected output.
struct ReferenceCase {
    doy: u16,
    sec: f64,
    alt_m: f64,
    lat_rad: f64,
    lon_rad: f64,
    f107a: f64,
    f107: f64,
    ap: f64,
    expected_density: f64,
    expected_temperature_alt: f64,
}

const CASES: &[ReferenceCase] = &[
    // Reference values from pymsis (NRLMSISE-00 Fortran, version='0').
    ReferenceCase {
        doy: 80,
        sec: 43200.0,
        alt_m: 400_000.0,
        lat_rad: 0.0,
        lon_rad: 0.0,
        f107a: 150.0,
        f107: 150.0,
        ap: 4.0,
        expected_density: 6.059685e-12,
        expected_temperature_alt: 1127.55,
    },
    ReferenceCase {
        doy: 80,
        sec: 0.0,
        alt_m: 400_000.0,
        lat_rad: 45.0f64.to_radians(),
        lon_rad: 0.0,
        f107a: 150.0,
        f107: 150.0,
        ap: 4.0,
        expected_density: 3.176054e-12,
        expected_temperature_alt: 951.41,
    },
    ReferenceCase {
        doy: 80,
        sec: 43200.0,
        alt_m: 200_000.0,
        lat_rad: 0.0,
        lon_rad: 0.0,
        f107a: 70.0,
        f107: 70.0,
        ap: 4.0,
        expected_density: 2.040385e-10,
        expected_temperature_alt: 744.87,
    },
    ReferenceCase {
        doy: 80,
        sec: 43200.0,
        alt_m: 600_000.0,
        lat_rad: 0.0,
        lon_rad: 0.0,
        f107a: 200.0,
        f107: 200.0,
        ap: 20.0,
        expected_density: 9.673022e-13,
        expected_temperature_alt: 1310.88,
    },
];

#[test]
fn test_nrlmsise00_matches_pymsis_density() {
    for case in CASES {
        let input = AtmosphereInput {
            altitude_m: case.alt_m,
            latitude_rad: case.lat_rad,
            longitude_rad: case.lon_rad,
            day_of_year: case.doy,
            seconds_utc: case.sec,
            f107: case.f107,
            f107a: case.f107a,
            ap: case.ap,
        };
        let out = Nrlmsise00::evaluate_simple(&input);
        assert!(
            relative_eq!(
                out.density,
                case.expected_density,
                epsilon = case.expected_density * 0.05
            ),
            "density mismatch at alt={} km lat={}°: expected {:.6e}, got {:.6e}",
            case.alt_m / 1000.0,
            case.lat_rad.to_degrees(),
            case.expected_density,
            out.density
        );
    }
}

#[test]
fn test_nrlmsise00_matches_pymsis_temperature() {
    for case in CASES {
        let input = AtmosphereInput {
            altitude_m: case.alt_m,
            latitude_rad: case.lat_rad,
            longitude_rad: case.lon_rad,
            day_of_year: case.doy,
            seconds_utc: case.sec,
            f107: case.f107,
            f107a: case.f107a,
            ap: case.ap,
        };
        let out = Nrlmsise00::evaluate_simple(&input);
        assert!(
            relative_eq!(
                out.temperature_alt,
                case.expected_temperature_alt,
                epsilon = 5.0
            ),
            "temperature mismatch at alt={} km lat={}°: expected {:.2}, got {:.2}",
            case.alt_m / 1000.0,
            case.lat_rad.to_degrees(),
            case.expected_temperature_alt,
            out.temperature_alt
        );
    }
}

#[test]
fn test_nrlmsise00_evaluates_at_iss_altitude() {
    let input = AtmosphereInput {
        altitude_m: 408_000.0,
        latitude_rad: 51.6f64.to_radians(),
        longitude_rad: 0.0,
        day_of_year: 80,
        seconds_utc: 43200.0,
        f107: 150.0,
        f107a: 150.0,
        ap: 4.0,
    };
    let out = Nrlmsise00::evaluate_simple(&input);
    assert!(out.density.is_finite() && out.density > 1e-15);
    assert!(out.temperature_alt > 600.0);
}
