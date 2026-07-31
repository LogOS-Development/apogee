//! ClockService tests — time scale conversions.
//!
//! hifitime 4.x handles TDB/TAI/UTC/TT natively. ClockService wraps
//! hifitime and adds UT1-UTC support via EOP data.

use crate::frames::clock::ClockService;
use hifitime::{Epoch, TimeScale};

#[test]
fn test_tai_to_utc_no_leap_second() {
    // TAI is 37 seconds ahead of UTC (as of 2024)
    let svc = ClockService::new();
    // Create a TAI epoch: 2024-01-01T00:00:37 TAI = 2024-01-01T00:00:00 UTC
    let tai = Epoch::from_gregorian(2024, 1, 1, 0, 0, 37, 0, TimeScale::TAI);
    let utc = svc.tai_to_utc(tai);
    let expected = Epoch::from_gregorian_utc(2024, 1, 1, 0, 0, 0, 0);
    assert_eq!(utc, expected);
}

#[test]
fn test_utc_to_tai() {
    let svc = ClockService::new();
    let utc = Epoch::from_gregorian_utc(2024, 1, 1, 0, 0, 0, 0);
    let tai = svc.utc_to_tai(utc);
    // TAI = UTC + 37 seconds
    let expected = Epoch::from_gregorian(2024, 1, 1, 0, 0, 37, 0, TimeScale::TAI);
    assert_eq!(tai, expected);
}

#[test]
fn test_tdb_to_tai_roundtrip() {
    let svc = ClockService::new();
    let tdb = Epoch::from_gregorian(2024, 1, 1, 12, 0, 0, 0, TimeScale::TDB);
    let tai = svc.tdb_to_tai(tdb);
    let tdb_back = svc.tai_to_tdb(tai);
    // Should roundtrip within microsecond precision (TDB-TAI is ~32.184s with small periodic variations)
    let diff = (tdb - tdb_back).abs();
    assert!(
        diff.to_seconds() < 1e-6,
        "TDB roundtrip diff: {:?} (should be < 1μs)",
        diff
    );
}

#[test]
fn test_tt_to_tai() {
    // TT is always TAI + 32.184s
    let svc = ClockService::new();
    let tt = Epoch::from_gregorian(2024, 1, 1, 0, 0, 0, 0, TimeScale::TT);
    let tai = svc.tt_to_tai(tt);
    let diff = (tai.to_time_scale(TimeScale::TT) - tt).abs();
    assert!(diff.to_seconds() < 1e-9);
}

#[test]
fn test_utc_to_tdb() {
    let svc = ClockService::new();
    let utc = Epoch::from_gregorian_utc(2024, 6, 15, 12, 30, 0, 0);
    let tdb = svc.utc_to_tdb(utc);
    // TDB should be very close to TT (within milliseconds, periodic variation)
    let tt = utc.to_time_scale(TimeScale::TT);
    let diff = (tdb.to_time_scale(TimeScale::TT) - tt).abs();
    assert!(
        diff.to_seconds() < 0.002,
        "TDB-TT diff: {:?} (should be < 2ms)",
        diff
    );
}

#[test]
fn test_gps_offset() {
    // GPS time = TAI - 19s
    let svc = ClockService::new();
    let tai = Epoch::from_gregorian_utc(2024, 1, 1, 0, 0, 0, 0).to_time_scale(TimeScale::TAI);
    let gps = svc.tai_to_gps(tai);
    let back = svc.gps_to_tai(gps);
    let diff = (tai - back).abs();
    assert!(diff.to_seconds() < 1e-9);
}

#[test]
fn test_ut1_utc_with_eop() {
    // UT1 = UTC + (UT1-UTC offset from EOP data)
    let eop_data = crate::frames::EopData::parse(
        "  2000   1   1  51544  0.043312  0.377830  0.3557700  0.0001230  -0.0001  -0.0002  -0.0003  -0.0004\n\
         2000   1   2  51545  0.043345  0.377850  0.3557800  0.0001240  -0.0001  -0.0002  -0.0003  -0.0004"
    ).unwrap();
    let svc = ClockService::with_eop(eop_data);
    let utc = Epoch::from_gregorian_utc(2000, 1, 1, 0, 0, 0, 0);
    let ut1 = svc.utc_to_ut1(utc);
    // UT1-UTC = 0.3557700 seconds at MJD 51544
    // UT1 should be 0.35577 seconds ahead of UTC
    let diff = (ut1 - utc).to_seconds();
    assert!(
        diff.abs() < 1.0,
        "UT1-UTC diff: {} (should be ~0.356s)",
        diff
    );
}

#[test]
fn test_ut1_utc_without_eop_returns_utc() {
    // Without EOP data, UT1 ≈ UTC (no correction)
    let svc = ClockService::new();
    let utc = Epoch::from_gregorian_utc(2024, 1, 1, 0, 0, 0, 0);
    let ut1 = svc.utc_to_ut1(utc);
    let diff = (ut1 - utc).to_seconds().abs();
    assert!(
        diff < 1e-9,
        "UT1 without EOP should equal UTC, diff: {}",
        diff
    );
}

#[test]
fn test_leap_second_transition_2016() {
    // 2016-12-31 23:59:60 UTC was a leap second
    let svc = ClockService::new();
    let before = Epoch::from_gregorian_utc(2016, 12, 31, 23, 59, 59, 0);
    let after = Epoch::from_gregorian_utc(2017, 1, 1, 0, 0, 0, 0);
    // TAI-UTC should be 36 before, 37 after
    let tai_before = svc.utc_to_tai(before);
    let tai_after = svc.utc_to_tai(after);
    let elapsed = (tai_after - tai_before).to_seconds();
    // Should be 2 seconds (23:59:59 to 00:00:00 with a leap second = 2 TAI seconds)
    assert!(
        elapsed > 1.5 && elapsed < 2.5,
        "Leap second transition: {}s (should be 2s)",
        elapsed
    );
}
