#[cfg(test)]
#[allow(clippy::module_inception, clippy::type_complexity)]
mod apollo15_validation {
    use crate::gravity::point_mass::PointMassGravity;
    use crate::integrator::{Integrator, Rk4, StateVector};
    use crate::tests::helpers::point_mass_derivative;
    use apogee_common::units::Seconds;
    use nalgebra::Vector3;

    /// Apollo 15 reference trajectory fixture. Generated with spiceypy from
    /// JPL's public `apollo15-1.bsp` SPK Type 1 kernel.
    const APOLLO15_CSV: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/apollo15_reference.csv"
    );

    /// Load the reference trajectory as a list of (et_s, position_km, velocity_km_s).
    fn load_reference() -> Option<Vec<(f64, Vector3<f64>, Vector3<f64>)>> {
        let path = std::path::Path::new(APOLLO15_CSV);
        if !path.exists() {
            return None;
        }
        let contents = std::fs::read_to_string(path).ok()?;
        let mut out = Vec::new();
        for line in contents.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 7 {
                continue;
            }
            let et: f64 = parts[0].parse().ok()?;
            let x: f64 = parts[1].parse().ok()?;
            let y: f64 = parts[2].parse().ok()?;
            let z: f64 = parts[3].parse().ok()?;
            let vx: f64 = parts[4].parse().ok()?;
            let vy: f64 = parts[5].parse().ok()?;
            let vz: f64 = parts[6].parse().ok()?;
            out.push((
                et,
                Vector3::new(x, y, z) * 1_000.0,
                Vector3::new(vx, vy, vz) * 1_000.0,
            ));
        }
        Some(out)
    }

    /// Build a Moon-centered celestial model with Moon as the origin.
    fn moon_only_system() -> crate::ephemeris::kernel::SolarSystemState {
        crate::ephemeris::kernel::SolarSystemState {
            states: vec![crate::ephemeris::kernel::BodyState {
                naif_id: 301,
                position: Vector3::zeros(),
                velocity: Vector3::zeros(),
            }],
        }
    }

    #[test]
    #[ignore = "requires tests/fixtures/apollo15_reference.csv (within apogee-core); generated from apollo15-1.bsp"]
    fn test_apollo15_lunar_orbit_vs_reference() {
        let reference = load_reference().expect("Apollo 15 reference CSV fixture");
        assert!(
            reference.len() >= 2,
            "reference trajectory must contain at least two states"
        );

        let gravity = PointMassGravity {};
        let celestial = moon_only_system();
        let mut integrator = Rk4::new(Seconds::new(10.0)); // 10 s fixed step

        let (et0, pos0, vel0) = reference[0];
        let mut state = StateVector {
            position: pos0,
            velocity: vel0,
            attitude: nalgebra::Quaternion::identity(),
            angular_velocity: nalgebra::Vector3::zeros(),
        };

        let derivative_fn = |s: &StateVector| point_mass_derivative(s, &celestial, &gravity);

        // Propagate from the first reference state to the last one.
        let (et1, pos_ref, vel_ref) = reference.last().unwrap();
        let duration_s = et1 - et0;

        let result = integrator.step(&mut state, &derivative_fn, Seconds::new(duration_s));
        assert!(result.accepted, "integrator did not accept step");

        let position_error_km = (state.position - pos_ref).norm() / 1_000.0;
        let velocity_error_ms = (state.velocity - vel_ref).norm();

        println!(
            "Apollo 15 propagation: et0={et0:.3} -> et1={et1:.3} (duration {duration_s:.0} s)"
        );
        println!("position error: {position_error_km:.3} km");
        println!("velocity error: {velocity_error_ms:.4} m/s");

        // Apollo 15 lunar orbit: ~1 hour propagation around the Moon with
        // Moon-only gravity should stay within a few km of the reference.
        assert!(
            position_error_km < 5.0,
            "Apollo 15 position error too large: {position_error_km:.2} km"
        );
        assert!(
            velocity_error_ms < 3.0,
            "Apollo 15 velocity error too large: {velocity_error_ms:.4} m/s"
        );
    }
}
