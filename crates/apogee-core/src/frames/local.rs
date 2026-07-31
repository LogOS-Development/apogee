//! Local relative frames: LVLH and RIC.
//!
//! These frames are defined by a chief spacecraft state (position and
//! velocity). They are orthonormal right-handed bases attached to the
//! chief's orbit, used for relative-motion analysis, rendezvous, and
//! formation flying.
//!
//! # Frame definitions
//!
//! ## LVLH (Local Vertical / Local Horizontal)
//!
//! Common aerospace convention:
//!
//! - **x (R)**: radial direction, positive away from the Earth (`r̂`).
//! - **z (N)**: orbit normal, positive along the angular-momentum vector
//!   (`ĥ = (r × v) / |r × v|`).
//! - **y (T)**: in-track / tangential, completing the right-handed triad
//!   (`t̂ = n̂ × r̂`).
//!
//! Basis: `[r̂, n̂ × r̂, n̂]`.
//!
//! ## RIC (Radial / In-track / Cross-track)
//!
//! Satellite formation-flying convention:
//!
//! - **x (R)**: radial (`r̂`).
//! - **y (I)**: in-track, along the velocity direction in the orbit plane
//!   (`î = ĉ × r̂`).
//! - **z (C)**: cross-track, along the angular-momentum vector
//!   (`ĉ = (r × v) / |r × v|`).
//!
//! Basis: `[r̂, ĉ × r̂, ĉ]`.
//!
//! LVLH and RIC share the same basis vectors; only the axis names differ.
//!
//! ## Spherical RIC
//!
//! Curvilinear version of RIC used for relative motion in chief-centered
//! angular coordinates. For small separations the rotation matrix from ECI
//! to the local tangent plane is identical to the rectilinear RIC basis;
//! the spherical interpretation applies when converting local Cartesian
//! offsets to along-track / cross-track angles.
//!
//! # References
//!
//! - Alfriend, K.T., Vadali, S.R., Gurfil, P., How, J.P. & Breger, L.S.
//!   (2010), "Spacecraft Formation Flying: Dynamics, Control, and
//!   Navigation", Elsevier, Ch. 2
//!   <https://doi.org/10.1016/B978-0-7506-8533-7.00002-6>
//! - Vallado, D.A. (2013), "Fundamentals of Astrodynamics and
//!   Applications", 4th ed., Microcosm Press, §10.4: Relative Motion
//!   <https://microcosmpress.com/publishing/fundamentals-of-astrodynamics-and-applications-fourth-edition/>
//! - Schweighart, S.A. & Sedwick, R.J. (2002), "High-Fidelity Linearized
//!   J2 Model for Satellite Formation Flight", J. Guid. Control Dyn. 25(6)
//!   <https://doi.org/10.2514/2.4986>
//!
//! LVLH convention:
//! - Wertz, J.R. (Ed.) (1978), "Spacecraft Attitude Determination and
//!   Control", Springer, §12.1
//!   <https://doi.org/10.1007/978-94-009-9907-7>

use nalgebra::{Matrix3, Vector3};

/// Local relative frame attached to a chief spacecraft state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalFrame {
    /// Local Vertical / Local Horizontal (x=radial, y=in-track, z=normal).
    Lvlh,
    /// Radial / In-track / Cross-track (x=radial, y=in-track, z=cross-track).
    Ric,
    /// Spherical RIC: same basis as RIC, but used for curvilinear local
    /// coordinates (radial distance, along-track angle, cross-track angle).
    RicSpherical,
}

/// Compute the orthonormal LVLH/RIC basis from a chief position and velocity.
///
/// Returns the rotation matrix from the inertial frame (ECI) to the local
/// frame. All three variants share the same right-handed basis:
///
///   x = r̂
///   z = ĥ = (r × v) / |r × v|
///   y = z × x
///
/// If `r` or `r × v` is zero, returns the identity matrix as a safe fallback.
pub fn local_frame_matrix(local: LocalFrame, r: &Vector3<f64>, v: &Vector3<f64>) -> Matrix3<f64> {
    let _ = local; // LVLH, RIC, and spherical RIC share the same basis.

    let r_norm = r.norm();
    if r_norm == 0.0 {
        return Matrix3::identity();
    }

    let h = r.cross(v);
    let h_norm = h.norm();
    if h_norm == 0.0 {
        return Matrix3::identity();
    }

    let x = r / r_norm; // radial
    let z = h / h_norm; // normal / cross-track
    let y = z.cross(&x); // in-track

    // Rows of the matrix are the local basis vectors expressed in ECI.
    // ECI vector -> local coordinates = [x·v, y·v, z·v].
    Matrix3::new(x.x, x.y, x.z, y.x, y.y, y.z, z.x, z.y, z.z)
}

/// Transform a vector from the inertial frame to a local relative frame.
pub fn to_local_frame(
    local: LocalFrame,
    vec: &Vector3<f64>,
    r: &Vector3<f64>,
    v: &Vector3<f64>,
) -> Vector3<f64> {
    local_frame_matrix(local, r, v) * vec
}

/// Transform a vector from a local relative frame back to the inertial frame.
pub fn from_local_frame(
    local: LocalFrame,
    vec: &Vector3<f64>,
    r: &Vector3<f64>,
    v: &Vector3<f64>,
) -> Vector3<f64> {
    local_frame_matrix(local, r, v).transpose() * vec
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_lvlh_radial_axis_is_normalized_position() {
        let r = Vector3::new(7000.0, 0.0, 0.0);
        let v = Vector3::new(0.0, 7.5, 0.0);
        let m = local_frame_matrix(LocalFrame::Lvlh, &r, &v);
        let local_r = m * r;
        assert_relative_eq!(local_r.x, r.norm(), epsilon = 1e-9);
        assert_relative_eq!(local_r.y, 0.0, epsilon = 1e-9);
        assert_relative_eq!(local_r.z, 0.0, epsilon = 1e-9);
    }

    #[test]
    fn test_lvlh_normal_axis_is_angular_momentum_direction() {
        let r = Vector3::new(7000.0, 0.0, 0.0);
        let v = Vector3::new(0.0, 7.5, 0.0);
        let m = local_frame_matrix(LocalFrame::Lvlh, &r, &v);
        let h = r.cross(&v).normalize();
        let local_h = m * h;
        assert_relative_eq!(local_h.x, 0.0, epsilon = 1e-9);
        assert_relative_eq!(local_h.y, 0.0, epsilon = 1e-9);
        assert_relative_eq!(local_h.z, 1.0, epsilon = 1e-9);
    }

    #[test]
    fn test_lvlh_velocity_is_pure_x_y() {
        let r = Vector3::new(7000.0, 0.0, 0.0);
        let v = Vector3::new(0.0, 7.5, 1.0);
        let m = local_frame_matrix(LocalFrame::Lvlh, &r, &v);
        let local_v = m * v;
        assert_relative_eq!(local_v.z, 0.0, epsilon = 1e-9);
    }

    #[test]
    fn test_ric_and_lvlh_share_same_basis() {
        let r = Vector3::new(7000.0, 1000.0, 500.0);
        let v = Vector3::new(-1.0, 7.5, 0.5);
        let m_lvlh = local_frame_matrix(LocalFrame::Lvlh, &r, &v);
        let m_ric = local_frame_matrix(LocalFrame::Ric, &r, &v);
        let m_sric = local_frame_matrix(LocalFrame::RicSpherical, &r, &v);
        assert_relative_eq!(m_lvlh, m_ric, epsilon = 1e-12);
        assert_relative_eq!(m_lvlh, m_sric, epsilon = 1e-12);
    }

    #[test]
    fn test_local_frame_matrix_is_orthogonal() {
        let r = Vector3::new(7000.0, 1000.0, 500.0);
        let v = Vector3::new(-1.0, 7.5, 0.5);
        let m = local_frame_matrix(LocalFrame::Ric, &r, &v);
        let product = m * m.transpose();
        let identity = Matrix3::identity();
        assert_relative_eq!(product, identity, epsilon = 1e-12);
    }

    #[test]
    fn test_round_trip_local_to_inertial() {
        let r = Vector3::new(7000.0, 1000.0, 500.0);
        let v = Vector3::new(-1.0, 7.5, 0.5);
        let vec = Vector3::new(1.0, 2.0, 3.0);
        let local = to_local_frame(LocalFrame::Ric, &vec, &r, &v);
        let back = from_local_frame(LocalFrame::Ric, &local, &r, &v);
        assert_relative_eq!(back, vec, epsilon = 1e-12);
    }

    #[test]
    fn test_zero_position_fallback_identity() {
        let r = Vector3::zeros();
        let v = Vector3::new(0.0, 7.5, 0.0);
        let m = local_frame_matrix(LocalFrame::Lvlh, &r, &v);
        assert_relative_eq!(m, Matrix3::identity(), epsilon = 1e-12);
    }

    #[test]
    fn test_zero_angular_momentum_fallback_identity() {
        let r = Vector3::new(7000.0, 0.0, 0.0);
        let v = Vector3::new(1.0, 0.0, 0.0); // parallel to r
        let m = local_frame_matrix(LocalFrame::Lvlh, &r, &v);
        assert_relative_eq!(m, Matrix3::identity(), epsilon = 1e-12);
    }
}
