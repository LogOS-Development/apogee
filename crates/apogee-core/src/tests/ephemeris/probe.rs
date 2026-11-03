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

    let pos_err = (state.position
        - // Fixture positions are km (SPK units); state_at returns SI meters.
        nalgebra::Vector3::new(expected_pos[0], expected_pos[1], expected_pos[2]) * 1_000.0)
        .norm();
    let vel_err = (state.velocity
        - nalgebra::Vector3::new(expected_vel[0], expected_vel[1], expected_vel[2]) * 1_000.0)
        .norm();

    // Chebyshev fit of a sinusoid over 1-day records should be accurate to
    // sub-meter in position and sub-mm/s in velocity.
    assert!(pos_err < 1.0, "position error too large: {pos_err} m");
    assert!(vel_err < 1.0e-3, "velocity error too large: {vel_err} m/s");
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

        // Fixture positions are km (SPK units); state_at returns SI meters.
        let pos_err = (state.position
            - nalgebra::Vector3::new(expected_pos[0], expected_pos[1], expected_pos[2]) * 1_000.0)
            .norm();
        let vel_err = (state.velocity
            - nalgebra::Vector3::new(expected_vel[0], expected_vel[1], expected_vel[2]) * 1_000.0)
            .norm();

        assert!(
            pos_err < 1.0,
            "epoch {t}: position error too large: {pos_err} m"
        );
        assert!(
            vel_err < 1.0e-3,
            "epoch {t}: velocity error too large: {vel_err} m/s"
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

        // Fixture positions are km (SPK units); state_at returns SI meters.
        let pos_err = (state.position
            - nalgebra::Vector3::new(expected_pos[0], expected_pos[1], expected_pos[2]) * 1_000.0)
            .norm();
        let vel_err = (state.velocity
            - nalgebra::Vector3::new(expected_vel[0], expected_vel[1], expected_vel[2]) * 1_000.0)
            .norm();

        assert!(
            pos_err < 1.0,
            "record {rec} boundary t={t}: position error too large: {pos_err} m"
        );
        assert!(
            vel_err < 1.0e-3,
            "record {rec} boundary t={t}: velocity error too large: {vel_err} m/s"
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

/// A type-3 velocity series is evaluated directly, with no interval
/// scaling: SPICE's SPKE03 reads the X'/Y'/Z' coefficient sets with CHBVAL,
/// and SPKW03 defines them as derivatives with respect to time. The record
/// below is correct by construction in that convention — a constant
/// velocity V is the Chebyshev series V·T0 (independent of the record
/// interval), and the linear position series P0 + V·R·s has coefficients
/// P0 (T0) and V·R (T1).
///
/// The interval half-width is 10 s, so a velocity divided by the radius is
/// off by an order of magnitude. Regression test for #205: the reader
/// previously divided the evaluated velocity series by the record
/// half-interval, and the fitted fixture scaled its samples to match,
/// so every round-trip test validated the wrong convention against itself.
#[test]
fn type3_velocity_series_evaluated_without_interval_scaling() {
    use crate::ephemeris::kernel::tests::build_type3_raw_fixture;

    let start_et = 0.0;
    let end_et = 20.0;
    let radius = 10.0; // record half-interval, (end - start) / 2
    let p0 = [1.0e5, -2.0e5, 5.0e4]; // km
    let v0 = [3.0, -4.0, 12.0]; // km/s

    let mut pos = vec![[0.0f64; 3]; 8];
    let mut vel = vec![[0.0f64; 3]; 8];
    pos[0] = p0;
    pos[1] = [v0[0] * radius, v0[1] * radius, v0[2] * radius];
    vel[0] = v0;

    let bytes = build_type3_raw_fixture(TARGET_ID, start_et, end_et, &[(pos, vel)]);
    let kernel = Kernel::from_bytes(&bytes).unwrap();

    for dt in [-5.0, 0.0, 7.5] {
        let et = start_et + radius + dt; // mid + dt
        let state = kernel.state_at(TARGET_ID, et).unwrap();

        let expected_pos = nalgebra::Vector3::new(
            (p0[0] + v0[0] * dt) * 1_000.0,
            (p0[1] + v0[1] * dt) * 1_000.0,
            (p0[2] + v0[2] * dt) * 1_000.0,
        );
        let expected_vel =
            nalgebra::Vector3::new(v0[0] * 1_000.0, v0[1] * 1_000.0, v0[2] * 1_000.0);

        let pos_err = (state.position - expected_pos).norm();
        let vel_err = (state.velocity - expected_vel).norm();
        assert!(
            pos_err < 1.0e-3,
            "position error at t-mid = {dt} s too large: {pos_err} m"
        );
        assert!(
            vel_err < 1.0e-6,
            "velocity error at t-mid = {dt} s too large: {vel_err} m/s"
        );
    }
}
