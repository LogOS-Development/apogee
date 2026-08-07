//! Leap second table parser tests.
//!
//! IERS leap second file format:
//!   Comment lines start with #
//!   Data line: NNNNN NNNNN N NNNNN NNN NNN NNN NNN NNN NNN NNN
//!   Columns: year month day MJD TAI-UTC
//! Example data from naif0012.txt or tai-utc.dat

use crate::frames::leap_seconds::*;

#[test]
fn test_parses_single_entry() {
    let data = "# Leap second table\n\
               #  1972 JAN  1 =JD 2440578.5  TAI-UTC=  10.0       JAN  1 (TAI-UTC=10.0)\n\
               + 1972  1  1 41317 10";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.len(), 1);
}

#[test]
fn test_parses_multiple_entries() {
    let data = "+ 1972  1  1 41317 10\n\
               + 1972  7  1 41499 11\n\
               + 1973  1  1 41683 12";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.len(), 3);
}

#[test]
fn test_skips_comment_lines() {
    let data = "# comment line\n\
               # another comment\n\
               + 1972  1  1 41317 10";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.len(), 1);
}

#[test]
fn test_skips_blank_lines() {
    let data = "\n\
               + 1972  1  1 41317 10\n\n";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.len(), 1);
}

#[test]
fn test_parses_tai_utc_offset() {
    let data = "+ 1972  1  1 41317 10";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.entries()[0].tai_utc, 10);
}

#[test]
fn test_parses_year_month_day() {
    let data = "+ 1972  7  1 41499 11";
    let table = LeapSecondTable::parse(data).unwrap();
    let e = &table.entries()[0];
    assert_eq!(e.year, 1972);
    assert_eq!(e.month, 7);
    assert_eq!(e.day, 1);
}

#[test]
fn test_parses_mjd() {
    let data = "+ 1972  1  1 41317 10";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.entries()[0].mjd, 41317);
}

#[test]
fn test_rejects_missing_tai_utc() {
    let data = "+ 1972  1  1 41317";
    assert!(LeapSecondTable::parse(data).is_err());
}

#[test]
fn test_rejects_non_numeric_year() {
    let data = "+ XXXX  1  1 41317 10";
    assert!(LeapSecondTable::parse(data).is_err());
}

#[test]
fn test_lookup_tai_utc_at_epoch_before_first() {
    let data = "+ 1972  1  1 41317 10";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.tai_utc_at_mjd(40000), 0); // before 1972, no offset
}

#[test]
fn test_lookup_tai_utc_at_exact_entry() {
    let data = "+ 1972  1  1 41317 10\n\
               + 1972  7  1 41499 11";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.tai_utc_at_mjd(41317), 10);
    assert_eq!(table.tai_utc_at_mjd(41499), 11);
}

#[test]
fn test_lookup_tai_utc_between_entries() {
    let data = "+ 1972  1  1 41317 10\n\
               + 1972  7  1 41499 11";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.tai_utc_at_mjd(41400), 10);
}

#[test]
fn test_lookup_tai_utc_after_last() {
    let data = "+ 1972  1  1 41317 10\n\
               + 1972  7  1 41499 11";
    let table = LeapSecondTable::parse(data).unwrap();
    assert_eq!(table.tai_utc_at_mjd(60000), 11);
}
