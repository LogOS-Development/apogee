#[cfg(test)]
#[allow(clippy::module_inception, clippy::type_complexity)]
mod artemis2_validation {
    use crate::ephemeris::kernel::{BodyState, Kernel, SolarSystemState};
    use crate::gravity::point_mass::PointMassGravity;
    use crate::integrator::{Integrator, Rk4, StateVector};
    use crate::tests::helpers::point_mass_derivative;
    use nalgebra::Vector3;

    /// Path to the Artemis 2 SPK fixture.
    const ARTEMIS2_BSP: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/artemis2.bsp"
    );

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
    #[ignore = "requires tests/fixtures/artemis2.bsp; run scripts/fetch_data.sh to obtain it"]
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
        let mut integrator = Rk4::new(30.0); // 30 s fixed step

        let mut state = StateVector {
            position: sc_initial.position,
            velocity: sc_initial.velocity,
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };

        let derivative_fn = |s: &StateVector| point_mass_derivative(s, &celestial, &gravity);
        let result = integrator.step(&mut state, &derivative_fn, duration_s);
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
}
