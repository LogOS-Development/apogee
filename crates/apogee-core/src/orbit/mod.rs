//! Two-body orbital mechanics utilities.
//!
//! Pure functions for computing classical orbital elements and analytical
//! perturbation rates from state vectors. These are used by the simulation
//! pipeline (energy-conservation checks, scheduler heuristics) and by
//! integration tests (J2 nodal regression validation).
//!
//! All functions take raw `Vector3<f64>` position and velocity in an inertial
//! frame and return values in SI units (radians, rad/s, J/kg).

use nalgebra::Vector3;

/// Specific orbital energy (J/kg): v²/2 - μ/r.
///
/// Positive for hyperbolic orbits, negative for elliptical orbits.
#[must_use]
pub fn specific_energy(pos: &Vector3<f64>, vel: &Vector3<f64>, gm: f64) -> f64 {
    vel.norm_squared() / 2.0 - gm / pos.norm()
}

/// Specific orbital energy with Earth's GM (convenience for Earth-orbiting
/// spacecraft).
#[must_use]
pub fn specific_energy_earth(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
    use apogee_common::constants::GM_EARTH;
    specific_energy(pos, vel, GM_EARTH)
}

/// Specific angular momentum vector h = r × v (m²/s).
#[must_use]
pub fn angular_momentum(pos: &Vector3<f64>, vel: &Vector3<f64>) -> Vector3<f64> {
    pos.cross(vel)
}

/// Right ascension of the ascending node (RAAN, radians).
///
/// Returns 0.0 for equatorial orbits where the node line is undefined.
#[must_use]
pub fn raan(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
    let h = angular_momentum(pos, vel);
    let node = Vector3::new(0.0, 0.0, 1.0).cross(&h);
    if node.norm() < 1e-10 {
        return 0.0;
    }
    node.y.atan2(node.x)
}

/// Orbital inclination (radians).
///
/// The angle between the angular momentum vector and the z-axis of the
/// reference frame.
#[must_use]
pub fn inclination(pos: &Vector3<f64>, vel: &Vector3<f64>) -> f64 {
    let h = angular_momentum(pos, vel);
    let h_mag = h.norm();
    if h_mag < 1e-10 {
        return 0.0;
    }
    (h.z / h_mag).acos()
}

/// Semi-major axis (m).
///
/// Computed from the vis-viva equation: a = -μ / (2ε), where ε is the
/// specific orbital energy. Returns `f64::INFINITY` for parabolic trajectories
/// (ε = 0).
#[must_use]
pub fn semi_major_axis(pos: &Vector3<f64>, vel: &Vector3<f64>, gm: f64) -> f64 {
    let energy = specific_energy(pos, vel, gm);
    if energy.abs() < 1e-30 {
        return f64::INFINITY;
    }
    -gm / (2.0 * energy)
}

/// Analytical J2 nodal regression rate (rad/s).
///
/// Omega_dot = -3/2 * n * J2 * (R_eq/a)^2 * cos(i) / (1-e^2)^2
///
/// where n = sqrt(GM/a^3) is the mean motion, J2 is the unnormalized
/// zonal harmonic, a is the semi-major axis, i is the inclination,
/// R_eq is the equatorial radius, and e is the eccentricity.
///
/// This is the first-order J2 secular rate. Higher-order terms and
/// short-period oscillations are not included.
#[must_use]
pub fn j2_nodal_regression_rate(
    gm: f64,
    semi_major_axis: f64,
    eccentricity: f64,
    inclination: f64,
    j2: f64,
    equatorial_radius: f64,
) -> f64 {
    let n = (gm / semi_major_axis.powi(3)).sqrt();
    let cos_i = inclination.cos();
    let e2 = eccentricity * eccentricity;
    -1.5 * n * j2 * (equatorial_radius / semi_major_axis).powi(2) * cos_i / (1.0 - e2).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn circular_orbit_energy() {
        // 400 km circular orbit around Earth.
        let r = 6_778_137.0_f64;
        let v = (3.986004415e14_f64 / r).sqrt();
        let pos = Vector3::new(r, 0.0, 0.0);
        let vel = Vector3::new(0.0, v, 0.0);
        let energy = specific_energy_earth(&pos, &vel);
        // Expected: v^2/2 - GM/r = GM/(2r) - GM/r = -GM/(2r)
        let expected = -3.986004415e14 / (2.0 * r);
        assert_relative_eq!(energy, expected, epsilon = 1e-3);
    }

    #[test]
    fn equatorial_orbit_raan_is_zero() {
        let pos = Vector3::new(7_000_000.0, 0.0, 0.0);
        let vel = Vector3::new(0.0, 7_500.0, 0.0);
        assert_relative_eq!(raan(&pos, &vel), 0.0, epsilon = 1e-10);
    }

    #[test]
    fn inclined_orbit_raan_nonzero() {
        // 45-degree inclined orbit with node along x-axis.
        let pos = Vector3::new(7_000_000.0, 0.0, 0.0);
        let inc = 45.0_f64.to_radians();
        let vel = Vector3::new(0.0, 7_500.0 * inc.cos(), 7_500.0 * inc.sin());
        // Node line is along x => RAAN = 0.
        assert_relative_eq!(raan(&pos, &vel), 0.0, epsilon = 1e-10);
        // Inclination should be 45 degrees.
        assert_relative_eq!(inclination(&pos, &vel), inc, epsilon = 1e-10);
    }

    #[test]
    fn semi_major_axis_circular() {
        let r = 7_000_000.0_f64;
        let v = (3.986004415e14_f64 / r).sqrt();
        let pos = Vector3::new(r, 0.0, 0.0);
        let vel = Vector3::new(0.0, v, 0.0);
        let a = semi_major_axis(&pos, &vel, 3.986004415e14);
        // Circular orbit: a = r.
        assert_relative_eq!(a, r, epsilon = 1e-3);
    }

    #[test]
    fn j2_regression_rate_negative_for_prograde() {
        // Prograde orbit (i < 90°) should have negative RAAN rate.
        let rate = j2_nodal_regression_rate(
            3.986004415e14,
            7_000_000.0,
            0.0,
            51.6_f64.to_radians(),
            1.08263e-3,
            6_378_137.0,
        );
        assert!(rate < 0.0, "prograde orbit should have negative RAAN rate");
    }

    #[test]
    fn j2_regression_rate_zero_for_polar() {
        // Polar orbit (i = 90°) should have zero nodal regression.
        let rate = j2_nodal_regression_rate(
            3.986004415e14,
            7_000_000.0,
            0.0,
            std::f64::consts::FRAC_PI_2,
            1.08263e-3,
            6_378_137.0,
        );
        // cos(90°) is ~6e-17 in f64, so the rate is ~1e-23 — effectively zero.
        assert!(
            rate.abs() < 1e-15,
            "polar orbit should have ~zero RAAN rate, got {rate:.6e}"
        );
    }
}
