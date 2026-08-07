//! TLE (Two-Line Element) parser tests.
//!
//! TLE format: two lines of 69 characters each, optionally preceded by a name line.
//! Reference: https://celestrak.org/NORAD/documentation/tle-fmt.php

use crate::tle::Tle;

fn iss_tle() -> &'static str {
    "ISS (ZARYA)\n\
     1 25544U 98067A   24001.50000000  .00016717  00000+0  10270-3 0  9996\n\
     2 25544  51.6400 000.0000 0000001 000.0000 000.0000 15.50000000123455"
}

#[test]
fn test_parses_name_line_when_present() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.name, Some("ISS (ZARYA)".to_string()));
}

#[test]
fn test_parses_without_name_line() {
    let lines = "1 25544U 98067A   24001.50000000  .00016717  00000+0  10270-3 0  9996\n\
                2 25544  51.6400 000.0000 0000001 000.0000 000.0000 15.50000000123455";
    let tle = Tle::parse(lines).unwrap();
    assert_eq!(tle.name, None);
}

#[test]
fn test_parses_satellite_number() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.satellite_number, 25544);
}

#[test]
fn test_parses_classification() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.classification, 'U');
}

#[test]
fn test_parses_international_designator() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.international_designator, "98067A");
}

#[test]
fn test_parses_epoch() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.epoch_year, 2024);
    assert!((tle.epoch_day - 1.5).abs() < 1e-9);
}

#[test]
fn test_parses_mean_motion_derivative() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!((tle.mean_motion_dot - 0.00016717).abs() < 1e-9);
}

#[test]
fn test_parses_mean_motion_ddot() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!((tle.mean_motion_ddot - 0.0).abs() < 1e-9);
}

#[test]
fn test_parses_bstar() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!((tle.bstar - 0.00010270).abs() < 1e-9);
}

#[test]
fn test_parses_ephemeris_type() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.ephemeris_type, 0);
}

#[test]
fn test_parses_element_set_number() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.element_set_number, 999);
}

#[test]
fn test_parses_inclination() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!((tle.inclination - 51.6400).abs() < 1e-9);
}

#[test]
fn test_parses_raan() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!((tle.raan - 0.0).abs() < 1e-9);
}

#[test]
fn test_parses_eccentricity() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!((tle.eccentricity - 0.0000001).abs() < 1e-9);
}

#[test]
fn test_parses_arg_perigee() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!((tle.arg_perigee - 0.0).abs() < 1e-9);
}

#[test]
fn test_parses_mean_anomaly() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!((tle.mean_anomaly - 0.0).abs() < 1e-9);
}

#[test]
fn test_parses_mean_motion() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!((tle.mean_motion - 15.5).abs() < 1e-9);
}

#[test]
fn test_parses_revolution_number() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.revolution_number, 12345);
}

#[test]
fn test_rejects_wrong_line1_start() {
    let bad = "2 25544  51.6400 000.0000 0000001 000.0000 000.0000 15.50000000123455\n\
              1 25544U 98067A   24001.50000000  .00016717  00000+0  10270-3 0  9996";
    assert!(Tle::parse(bad).is_err());
}

#[test]
fn test_rejects_too_short() {
    assert!(Tle::parse("1 25544U").is_err());
}

#[test]
fn test_checksum_line1() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.line1_checksum, 6);
}

#[test]
fn test_checksum_line2() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert_eq!(tle.line2_checksum, 5);
}

#[test]
fn test_computes_line1_checksum() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!(tle.verify_line1_checksum());
}

#[test]
fn test_computes_line2_checksum() {
    let tle = Tle::parse(iss_tle()).unwrap();
    assert!(tle.verify_line2_checksum());
}
