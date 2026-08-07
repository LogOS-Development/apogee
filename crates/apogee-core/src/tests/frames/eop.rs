//! EOP C04 parser tests.
//!
//! IERS EOP C04 format (fixed-width columns):
//!   Year Month Day MJD x-pole y-pole UT1-UTC LOD dPsi dEps dX dY
//! Units: arcseconds for x/y pole, seconds for UT1-UTC and LOD

use crate::frames::eop::*;

fn sample_eop() -> &'static str {
    "   2000   1   1   0  51544.00  0.043312  0.377830  0.3557700  0.000000  0.000000  0.000000  0.000000  0.0017230  0.000100  0.000100  0.0000100  0.000050  0.000050  0.000010  0.000010  0.0000010\n\
     2000   1   2   0  51545.00  0.043345  0.377850  0.3557800  0.000000  0.000000  0.000000  0.000000  0.0017240  0.000100  0.000100  0.0000100  0.000050  0.000050  0.000010  0.000010  0.0000010"
}

#[test]
fn test_parses_two_entries() {
    let data = EopData::parse(sample_eop()).unwrap();
    assert_eq!(data.len(), 2);
}

#[test]
fn test_parses_year_month_day() {
    let data = EopData::parse(sample_eop()).unwrap();
    let e = &data.entries()[0];
    assert_eq!(e.year, 2000);
    assert_eq!(e.month, 1);
    assert_eq!(e.day, 1);
}

#[test]
fn test_parses_mjd() {
    let data = EopData::parse(sample_eop()).unwrap();
    assert_eq!(data.entries()[0].mjd, 51544.0);
}

#[test]
fn test_parses_x_pole() {
    let data = EopData::parse(sample_eop()).unwrap();
    assert!((data.entries()[0].x_pole - 0.043312).abs() < 1e-9);
}

#[test]
fn test_parses_y_pole() {
    let data = EopData::parse(sample_eop()).unwrap();
    assert!((data.entries()[0].y_pole - 0.377830).abs() < 1e-9);
}

#[test]
fn test_parses_ut1_utc() {
    let data = EopData::parse(sample_eop()).unwrap();
    assert!((data.entries()[0].ut1_utc - 0.3557700).abs() < 1e-9);
}

#[test]
fn test_parses_lod() {
    let data = EopData::parse(sample_eop()).unwrap();
    assert!((data.entries()[0].lod - 0.0017230).abs() < 1e-9);
}

#[test]
fn test_lookup_at_exact_mjd() {
    let data = EopData::parse(sample_eop()).unwrap();
    let e = data.at_mjd(51544.0).unwrap();
    assert!((e.x_pole - 0.043312).abs() < 1e-9);
}

#[test]
fn test_lookup_between_entries_interpolates() {
    let data = EopData::parse(sample_eop()).unwrap();
    let e = data.at_mjd(51544.5).unwrap();
    // Linear interpolation between 0.043312 and 0.043345 → midpoint ~0.0433285
    assert!((e.x_pole - 0.0433285).abs() < 1e-6);
}

#[test]
fn test_lookup_before_first_fails() {
    let data = EopData::parse(sample_eop()).unwrap();
    assert!(data.at_mjd(50000.0).is_none());
}

#[test]
fn test_lookup_after_last_extrapolates() {
    let data = EopData::parse(sample_eop()).unwrap();
    let e = data.at_mjd(51546.0).unwrap();
    assert!((e.x_pole - 0.043345).abs() < 1e-9);
}

#[test]
fn test_skips_header_lines() {
    let data = "# Header line\n\
               Another header\n\
     2000   1   1   0  51544.00  0.043312  0.377830  0.3557700  0.000000  0.000000  0.000000  0.000000  0.0017230  0.000100  0.000100  0.0000100  0.000050  0.000050  0.000010  0.000010  0.0000010";
    let eop = EopData::parse(data).unwrap();
    assert_eq!(eop.len(), 1);
}
