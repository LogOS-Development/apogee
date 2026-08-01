#[cfg(test)]
mod artemis2_validation {
    use crate::ephemeris::kernel::{BodyState, Kernel, SolarSystemState};
    use crate::gravity::point_mass::PointMassGravity;
    use crate::integrator::{Integrator, Rk4, StateDerivative, StateVector};
    use nalgebra::Vector3;

    /// Path to the Artemis 2 SPK fixture.
    const ARTEMIS2_BSP: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/artemis2.bsp"
    );

    /// Convert seconds past J2000 TDB to a calendar string.
    fn et_to_utc(et: f64) -> String {
        // J2000 epoch is 2000-01-01 12:00:00 TDB.
        // Approximate TDB ≈ UTC for short-duration validation.
        let jd = et / 86400.0 + 2_451_545.0;
        format!("JD {jd}")
    }

    fn point_mass_derivative(
        state: &StateVector,
        celestial: &SolarSystemState,
        gravity: &PointMassGravity,
    ) -> StateDerivative {
        let acc = gravity
            .acceleration(&state.position, celestial)
            .expect("valid point-mass acceleration");
        StateDerivative {
            velocity: state.velocity,
            acceleration: acc,
        }
    }

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

        let gravity = PointMassGravity::default();
        let mut integrator = Rk4::new(30.0); // 30 s fixed step

        let mut state = StateVector {
            position: sc_initial.position,
            velocity: sc_initial.velocity,
        };

        let derivative_fn = |s: &StateVector| point_mass_derivative(s, &celestial, &gravity);
        let result = integrator.step(&mut state, &derivative_fn, duration_s);
        assert!(result.accepted);

        let position_error_km = (state.position - sc_reference.position).norm() / 1_000.0;
        let velocity_error_ms = (state.velocity - sc_reference.velocity).norm();

        println!(
            "Artemis 2 propagation: et0={et0} ({}) -> et1={et1} ({})",
            et_to_utc(et0),
            et_to_utc(et1)
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
