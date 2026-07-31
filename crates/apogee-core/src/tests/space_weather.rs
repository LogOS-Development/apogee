//! Space weather data loader tests.
//!
//! NOAA SWPC format: CSV with F10.7, Ap, Kp indices by date.

use crate::aero::space_weather::*;

#[test]
fn test_parses_single_entry() {
    let data = "2024-01-01,150.0,145.0,3.0,15\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    assert_eq!(sw.len(), 1);
}

#[test]
fn test_parses_multiple_entries() {
    let data = "2024-01-01,150.0,145.0,3.0,15\n2024-01-02,152.0,146.0,2.0,12\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    assert_eq!(sw.len(), 2);
}

#[test]
fn test_skips_header_line() {
    let data = "date,f10.7,f10.7a,ap,kp\n2024-01-01,150.0,145.0,3.0,15\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    assert_eq!(sw.len(), 1);
}

#[test]
fn test_skips_blank_lines() {
    let data = "\n2024-01-01,150.0,145.0,3.0,15\n\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    assert_eq!(sw.len(), 1);
}

#[test]
fn test_parses_f107() {
    let data = "2024-01-01,150.0,145.0,3.0,15\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    assert!((sw.entries()[0].f107 - 150.0).abs() < 1e-9);
}

#[test]
fn test_parses_f107a() {
    let data = "2024-01-01,150.0,145.0,3.0,15\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    assert!((sw.entries()[0].f107a - 145.0).abs() < 1e-9);
}

#[test]
fn test_parses_ap() {
    let data = "2024-01-01,150.0,145.0,3.0,15\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    assert!((sw.entries()[0].ap - 3.0).abs() < 1e-9);
}

#[test]
fn test_parses_kp() {
    let data = "2024-01-01,150.0,145.0,3.0,15\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    assert!((sw.entries()[0].kp - 15.0).abs() < 1e-9);
}

#[test]
fn test_rejects_bad_date() {
    let data = "not-a-date,150.0,145.0,3.0,15\n";
    assert!(SpaceWeatherData::parse(data).is_err());
}

#[test]
fn test_rejects_missing_field() {
    let data = "2024-01-01,150.0,145.0\n";
    assert!(SpaceWeatherData::parse(data).is_err());
}

#[test]
fn test_lookup_by_date() {
    let data = "2024-01-01,150.0,145.0,3.0,15\n2024-01-02,152.0,146.0,2.0,12\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    let entry = sw.at_date(2024, 1, 2).unwrap();
    assert!((entry.f107 - 152.0).abs() < 1e-9);
}

#[test]
fn test_lookup_missing_date_returns_none() {
    let data = "2024-01-01,150.0,145.0,3.0,15\n";
    let sw = SpaceWeatherData::parse(data).unwrap();
    assert!(sw.at_date(2024, 1, 2).is_none());
}
