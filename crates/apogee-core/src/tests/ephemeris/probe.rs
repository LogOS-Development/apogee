//! SPK Type 3 multi-record evaluation tests.
//!
//! These tests exercise the Chebyshev position+velocity evaluator across
//! multiple data records and epochs, using a synthetic DAF/SPK Type 3 kernel
//! built in-memory. This replaces the previous DE441-based probe tests that
//! required downloading a 3.3 GB kernel file and reading it entirely into
//! memory.
//!
//! The synthetic fixture uses a circular orbit model:
//!   p(t) = [R cos(ωt), R sin(ωt), 0]
//!   v(t) = [-Rω sin(ωt), Rω cos(ωt), 0]
//!
//! where R = 1e5 km and ω = 2π / period. This exercises:
//!   - Multi-record Chebyshev interpolation (the kernel spans multiple
//!     time intervals, each fit independently)
//!   - Position and velocity evaluation at various epochs
//!   - Segment lookup by target ID and epoch
//!   - Boundary epochs (start, mid, end of each record)

use crate::ephemeris::kernel::tests::build_type3_fixture;
use crate::ephemeris::Kernel;

const TARGET_ID: i32 = 4; // Mars barycenter (same NAIF ID as the old DE441 tests)
const START_ET: f64 = 0.0;
const END_ET: f64 = 86400.0 * 30.0; // 30 days
const RECORD_COUNT: i32 = 30; // One record per day

const RADIUS_KM: f64 = 1.0e5; // 100,000 km
const OMEGA: f64 = 2.0 * std::f64::consts::PI / (86400.0 * 10.0); // 10-day period

/// Position function: circular orbit in the XY plane (km).
fn position(t: f64) -> [f64; 3] {
    let angle = OMEGA * t;
    [RADIUS_KM * angle.cos(), RADIUS_KM * angle.sin(), 0.0]
}

/// Velocity function: derivative of the circular orbit (km/s).
fn velocity(t: f64) -> [f64; 3] {
    let angle = OMEGA * t;
    [
        -RADIUS_KM * OMEGA * angle.sin(),
        RADIUS_KM * OMEGA * angle.cos(),
        0.0,
    ]
}

/// Build a synthetic SPK Type 3 kernel for testing.
fn build_test_kernel() -> Vec<u8> {
    build_type3_fixture(
        TARGET_ID,
        START_ET,
        END_ET,
        RECORD_COUNT,
        position,
        velocity,
    )
}

#[test]
fn type3_evaluates_state_at_midpoint() {
    let bytes = build_test_kernel();
    let kernel = Kernel::from_bytes(&bytes).unwrap();

    let mid = (START_ET + END_ET) * 0.5;
    let state = kernel.state_at(TARGET_ID, mid).unwrap();

    let expected_pos = position(mid);
    let expected_vel = velocity(mid);

    // state_at_type3 returns position in km and velocity in km/s.
    let pos_err = (state.position
        - nalgebra::Vector3::new(expected_pos[0], expected_pos[1], expected_pos[2]))
    .norm();
    let vel_err = (state.velocity
        - nalgebra::Vector3::new(expected_vel[0], expected_vel[1], expected_vel[2]))
    .norm();

    // Chebyshev fit of a sinusoid over 1-day records should be accurate to
    // sub-meter in position and sub-mm/s in velocity.
    assert!(pos_err < 1.0e-3, "position error too large: {pos_err} km");
    assert!(vel_err < 1.0e-6, "velocity error too large: {vel_err} km/s");
}

#[test]
fn type3_evaluates_state_at_multiple_epochs() {
    let bytes = build_test_kernel();
    let kernel = Kernel::from_bytes(&bytes).unwrap();

    // Evaluate at 10 epochs spanning the full 30-day coverage window.
    // Clamp to just inside END_ET to avoid the exact end-of-segment edge
    // case in the record index lookup.
    let n = 10;
    for i in 0..n {
        let t = START_ET + (END_ET - START_ET) * (i as f64 / (n - 1) as f64);
        let t = t.min(END_ET - 1e-6);
        let state = kernel.state_at(TARGET_ID, t).unwrap();

        let expected_pos = position(t);
        let expected_vel = velocity(t);

        let pos_err = (state.position
            - nalgebra::Vector3::new(expected_pos[0], expected_pos[1], expected_pos[2]))
        .norm();
        let vel_err = (state.velocity
            - nalgebra::Vector3::new(expected_vel[0], expected_vel[1], expected_vel[2]))
        .norm();

        assert!(
            pos_err < 1.0e-3,
            "epoch {t}: position error too large: {pos_err} km"
        );
        assert!(
            vel_err < 1.0e-6,
            "epoch {t}: velocity error too large: {vel_err} km/s"
        );
    }
}

#[test]
fn type3_evaluates_state_at_record_boundaries() {
    let bytes = build_test_kernel();
    let kernel = Kernel::from_bytes(&bytes).unwrap();

    // Evaluate at the boundaries of each 1-day record. These are the epochs
    // where the Chebyshev series transitions between records, which is the
    // most error-prone point in the interpolation.
    let interval_length = (END_ET - START_ET) / RECORD_COUNT as f64;
    for rec in 0..=RECORD_COUNT {
        let t = START_ET + rec as f64 * interval_length;
        // Clamp the last point to just inside the end epoch, since the
        // final boundary is exclusive in the segment lookup.
        let t = t.min(END_ET - 1e-6);

        let state = kernel.state_at(TARGET_ID, t).unwrap();

        let expected_pos = position(t);
        let expected_vel = velocity(t);

        let pos_err = (state.position
            - nalgebra::Vector3::new(expected_pos[0], expected_pos[1], expected_pos[2]))
        .norm();
        let vel_err = (state.velocity
            - nalgebra::Vector3::new(expected_vel[0], expected_vel[1], expected_vel[2]))
        .norm();

        assert!(
            pos_err < 1.0e-3,
            "record {rec} boundary t={t}: position error too large: {pos_err} km"
        );
        assert!(
            vel_err < 1.0e-6,
            "record {rec} boundary t={t}: velocity error too large: {vel_err} km/s"
        );
    }
}

#[test]
fn type3_rejects_epoch_outside_coverage() {
    let bytes = build_test_kernel();
    let kernel = Kernel::from_bytes(&bytes).unwrap();

    assert!(kernel.state_at(TARGET_ID, START_ET - 1.0).is_err());
    assert!(kernel.state_at(TARGET_ID, END_ET + 1.0).is_err());
    assert!(kernel
        .state_at(TARGET_ID + 1, (START_ET + END_ET) * 0.5)
        .is_err());
}

#[test]
fn type3_segment_summary_is_correct() {
    let bytes = build_test_kernel();
    let kernel = Kernel::from_bytes(&bytes).unwrap();

    assert_eq!(kernel.segments().len(), 1);
    let seg = &kernel.segments()[0];
    assert_eq!(seg.target_id, TARGET_ID);
    assert_eq!(seg.spk_type, 3);
    assert!((seg.start_et - START_ET).abs() < 1e-9);
    assert!((seg.end_et - END_ET).abs() < 1e-9);
}
