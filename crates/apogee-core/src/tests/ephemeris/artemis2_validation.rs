//! Artemis 2 mission trajectory validation.
//!
//! Uses the public Artemis 2 SPK (`artemis2.bsp`) which contains Earth-centered
//! spacecraft (NAIF -24) states as SPK Type 13 (Hermite interpolation) segments.
//!
//! We validate our integrator by propagating the spacecraft from its initial
//! SPK state and comparing against the SPK reference at multiple epochs across
//! the coverage window.

use crate::ephemeris::kernel::{BodyState, Kernel, SolarSystemState};
use crate::gravity::point_mass::PointMassGravity;
use crate::integrator::{Integrator, Rk4, StateVector};
use crate::tests::helpers::point_mass_derivative;
use apogee_common::units::Seconds;
use nalgebra::Vector3;

/// Path to the Artemis 2 SPK fixture.
const ARTEMIS2_BSP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/artemis2.bsp");

/// Build a point-mass ephemeris from kernel states at a single epoch.
fn build_celestial(kernel: &Kernel, et: f64) -> SolarSystemState {
    let mut states = Vec::new();

    // Earth is the center for the Artemis 2 kernel.
    let earth = kernel.state_at(399, et).unwrap_or_else(|_| BodyState {
        naif_id: 399,
        position: Vector3::zeros(),
        velocity: Vector3::zeros(),
    });
    states.push(earth);

    if let Ok(moon) = kernel.state_at(301, et) {
        states.push(moon);
    }

    if let Ok(sun) = kernel.state_at(10, et) {
        states.push(sun);
    }

    SolarSystemState { states }
}

#[test]
#[ignore = "requires tests/fixtures/artemis2.bsp (within apogee-core); run scripts/fetch_data.sh to obtain it"]
fn test_artemis2_propagation_vs_spk() {
    let kernel = Kernel::load(ARTEMIS2_BSP).expect("load Artemis 2 SPK");

    // Pick an epoch near the start of the first continuous coverage window.
    let et0 = 828_367_170.583;
    let duration_s = 3_600.0; // propagate 1 hour
    let et1 = et0 + duration_s;

    let sc_initial = kernel.state_at(-24, et0).expect("Artemis 2 state at t0");
    let sc_reference = kernel.state_at(-24, et1).expect("Artemis 2 state at t1");

    // Build celestial model at t0. For this test we fix the ephemeris at
    // t0; a full validation would update it during propagation.
    let celestial = build_celestial(&kernel, et0);

    let gravity = PointMassGravity {};
    let mut integrator = Rk4::new(Seconds::new(30.0)); // 30 s fixed step

    let mut state = StateVector {
        position: sc_initial.position,
        velocity: sc_initial.velocity,
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: nalgebra::Vector3::zeros(),
    };

    let derivative_fn = |s: &StateVector| point_mass_derivative(s, &celestial, &gravity);
    let result = integrator.step(&mut state, &derivative_fn, Seconds::new(duration_s));
    assert!(result.accepted);

    let position_error_km = (state.position - sc_reference.position).norm() / 1_000.0;
    let velocity_error_ms = (state.velocity - sc_reference.velocity).norm();

    println!(
        "Artemis 2 propagation: et0={et0} ({}) -> et1={et1} ({})",
        hifitime::Epoch::from_tdb_seconds(et0).to_gregorian_str(hifitime::TimeScale::TDB),
        hifitime::Epoch::from_tdb_seconds(et1).to_gregorian_str(hifitime::TimeScale::TDB)
    );
    println!("position error: {position_error_km:.3} km");
    println!("velocity error: {velocity_error_ms:.4} m/s");

    // With a fixed inertial ephemeris and 30 s RK4 we expect tens to
    // hundreds of km over an hour; this is a sanity-check threshold.
    assert!(
        position_error_km < 500.0,
        "Artemis 2 position error too large: {position_error_km:.2} km"
    );
    assert!(
        velocity_error_ms < 10.0,
        "Artemis 2 velocity error too large: {velocity_error_ms:.4} m/s"
    );
}

/// Validate the Artemis 2 spacecraft state at multiple epochs across the SPK
/// coverage window. Each checkpoint re-initialises from the SPK and
/// propagates a short arc, which isolates per-segment integration error.
#[test]
#[ignore = "requires tests/fixtures/artemis2.bsp (within apogee-core); run scripts/fetch_data.sh to obtain it"]
fn test_artemis2_multi_epoch_state_check() {
    let kernel = Kernel::load(ARTEMIS2_BSP).expect("load Artemis 2 SPK");

    // Epochs spanning the first segment coverage window (~20.8 hours).
    // Segment 0 covers et [828367170.6, 828442230.6].
    let et0 = 828_367_170.583;
    let checkpoints: &[f64] = &[
        600.0,    // 10 min
        1_800.0,  // 30 min
        3_600.0,  // 1 hour
        7_200.0,  // 2 hours
        14_400.0, // 4 hours
        36_000.0, // 10 hours
    ];

    let sc_initial = kernel.state_at(-24, et0).expect("Artemis 2 state at t0");
    let celestial = build_celestial(&kernel, et0);
    let gravity = PointMassGravity {};

    for &dt in checkpoints {
        let et_target = et0 + dt;
        let sc_ref = kernel
            .state_at(-24, et_target)
            .unwrap_or_else(|_| panic!("Artemis 2 state at et={et_target:.3}"));

        let mut state = StateVector {
            position: sc_initial.position,
            velocity: sc_initial.velocity,
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };

        let mut integrator = Rk4::new(Seconds::new(30.0));
        let derivative_fn = |s: &StateVector| point_mass_derivative(s, &celestial, &gravity);
        let result = integrator.step(&mut state, &derivative_fn, Seconds::new(dt));
        assert!(result.accepted, "integrator failed at dt={dt}s");

        let pos_err_km = (state.position - sc_ref.position).norm() / 1_000.0;
        let vel_err_ms = (state.velocity - sc_ref.velocity).norm();

        println!("Artemis 2 dt={dt:.0}s: pos_err={pos_err_km:.3} km, vel_err={vel_err_ms:.4} m/s");

        // Error grows with propagation duration due to fixed ephemeris.
        // Threshold scales with dt: short arcs should be tight, longer arcs looser.
        let pos_limit_km = 50.0 + dt * 0.05; // 50 km baseline + 0.05 km/s growth
        let vel_limit_ms = 1.0 + dt * 0.005;

        assert!(
            pos_err_km < pos_limit_km,
            "Artemis 2 position error at dt={dt}s: {pos_err_km:.2} km (limit {pos_limit_km:.1})"
        );
        assert!(
            vel_err_ms < vel_limit_ms,
            "Artemis 2 velocity error at dt={dt}s: {vel_err_ms:.4} m/s (limit {vel_limit_ms:.2})"
        );
    }
}

/// Validate SPK state evaluation directly (no integration): compare
/// spacecraft states read from the kernel at known segment boundaries.
/// This verifies the SPK Type 13 parser is returning consistent values.
#[test]
#[ignore = "requires tests/fixtures/artemis2.bsp (within apogee-core); run scripts/fetch_data.sh to obtain it"]
fn test_artemis2_spk_state_consistency() {
    let kernel = Kernel::load(ARTEMIS2_BSP).expect("load Artemis 2 SPK");

    // The spacecraft should be within the Earth-Moon system throughout.
    // Check that states at several epochs are physically plausible.
    // Use epochs just inside each segment's start epoch (where coverage
    // is guaranteed) rather than hardcoded boundary values, since the
    // actual segment boundaries may have gaps between them.
    let segments = kernel.segments();
    let mut epochs = Vec::new();
    for seg in segments {
        if seg.target_id == -24 {
            // Use the start epoch + 1s to be safely inside the segment.
            epochs.push(seg.start_et + 1.0);
        }
    }
    assert!(!epochs.is_empty(), "no Artemis 2 segments found");

    let mut prev_pos: Option<Vector3<f64>> = None;
    for &et in &epochs {
        let state = match kernel.state_at(-24, et) {
            Ok(s) => s,
            Err(e) => {
                panic!("Artemis 2 state at et={et:.3} failed: {e:?}")
            }
        };

        let pos_km = state.position.norm() / 1_000.0;
        let vel_kms = state.velocity.norm() / 1_000.0;

        // Spacecraft should be between LEO and cislunar distances.
        assert!(
            pos_km > 6_000.0 && pos_km < 500_000.0,
            "Artemis 2 position at et={et:.3} out of expected range: {pos_km:.1} km"
        );
        // Velocity should be sub-escape (~11 km/s) and above a minimal
        // threshold. Some segments start near apogee where velocity is low.
        assert!(
            vel_kms > 0.1 && vel_kms < 12.0,
            "Artemis 2 velocity at et={et:.3} out of expected range: {vel_kms:.3} km/s"
        );

        println!("Artemis 2 et={et:.1}: |r|={pos_km:.1} km, |v|={vel_kms:.4} km/s");

        // Position should change between epochs (spacecraft is moving).
        if let Some(pp) = &prev_pos {
            let displacement = (state.position - pp).norm() / 1_000.0;
            assert!(
                displacement > 1.0,
                "Artemis 2 displacement between epochs too small: {displacement:.3} km"
            );
        }
        prev_pos = Some(state.position);
    }
}
