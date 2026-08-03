//! Data integrity tests — load fixture files through parsers.
//!
//! These tests verify that the parsers work with real file I/O,
//! not just in-memory strings.

use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("data")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
}

#[test]
fn test_real_iers_leap_second_dat_loads() {
    // End-to-end test: load the actual IERS Bulletin C leap second file
    // downloaded by `scripts/fetch_data.sh` and verify the parser handles the
    // real format. Checks the first entry (1972-01-01, 10 s), the current
    // last entry (37 s since 2017-01-01), and lookup at known MJDs.
    let path = data_dir().join("time").join("Leap_Second.dat");
    if !path.exists() {
        // Data file not fetched yet; skip gracefully in CI.
        return;
    }
    let table = crate::frames::LeapSecondTable::load(&path).unwrap();
    assert!(table.len() > 20);
    assert_eq!(table.entries()[0].tai_utc, 10); // 1972-01-01
    assert_eq!(table.entries().last().unwrap().tai_utc, 37); // current

    // Known historical offsets (from IERS Bulletin C).
    assert_eq!(table.tai_utc_at_mjd(41_317), 10); // 1972-01-01
    assert_eq!(table.tai_utc_at_mjd(57_754), 37); // 2017-01-01 onwards
}

#[test]
fn test_fixture_tle_parses() {
    let path = fixtures_dir().join("iss_tle.txt");
    let content = std::fs::read_to_string(&path).unwrap();
    let tle = crate::tle::Tle::parse(&content).unwrap();
    assert_eq!(tle.satellite_number, 25544);
    assert!(tle.verify_line1_checksum());
    assert!(tle.verify_line2_checksum());
}

#[test]
fn test_fixture_leap_seconds_parses() {
    let path = fixtures_dir().join("leap_seconds.dat");
    let content = std::fs::read_to_string(&path).unwrap();
    let table = crate::frames::LeapSecondTable::parse(&content).unwrap();
    assert!(table.len() > 20);
    // First entry: 1972-01-01, TAI-UTC = 10
    assert_eq!(table.entries()[0].tai_utc, 10);
    // Last entry should have TAI-UTC = 37
    assert_eq!(table.entries().last().unwrap().tai_utc, 37);
}

#[test]
fn test_fixture_eop_parses() {
    let path = fixtures_dir().join("eop_c04_sample.txt");
    let content = std::fs::read_to_string(&path).unwrap();
    let eop = crate::frames::EopData::parse(&content).unwrap();
    assert_eq!(eop.len(), 3);
    assert!((eop.entries()[0].x_pole - 0.043312).abs() < 1e-9);
}

#[test]
fn test_fixture_space_weather_parses() {
    let path = fixtures_dir().join("space_weather_sample.csv");
    let content = std::fs::read_to_string(&path).unwrap();
    let sw = crate::aero::SpaceWeatherData::parse(&content).unwrap();
    assert_eq!(sw.len(), 3);
    let entry = sw.at_date(2024, 1, 2).unwrap();
    assert!((entry.f107 - 152.0).abs() < 1e-9);
}
