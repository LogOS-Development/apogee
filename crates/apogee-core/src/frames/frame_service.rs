//! FrameService — reference frame transformations.
//!
//! Implements rotation matrices between ICRF, ECI (J2000), ECEF, and ECLIPJ2000.
//! Uses nalgebra Matrix3/Vector3 for 3D rotations.
//!
//! Key rotations:
//! - ECI ↔ ECLIPJ2000: rotation about x-axis by obliquity of ecliptic
//! - ECI ↔ ICRF: near-identity (ICRF and ECI J2000 differ by <0.1 arcsecond)
//! - ECI ↔ ECEF: rotation about z-axis by Greenwich Sidereal Time (GMST)
//!
//! # References
//!
//! Obliquity of the ecliptic (IAU 1976):
//! - Lieske, J.H., et al. (1977), "Expressions for the Precession Quantities
//!   Based upon the IAU (1976) System of Astronomical Constants",
//!   Astron. Astrophys. 58, 1-16
//!   https://ui.adsabs.harvard.edu/abs/1977A%26A....58....1L/abstract
//!
//! GMST and sidereal time:
//! - Vallado, D.A. (2013), "Fundamentals of Astrodynamics and Applications",
//!   4th ed., Microcosm Press, §3.4: Sidereal Time, pp. 183-192
//!   https://microcosmpress.com/publishing/fundamentals-of-astrodynamics-and-applications-fourth-edition/
//! - The Astronomical Almanac (2024), USNO/UKHO, §B: Time and Sidereal Time
//!   https://aa.usno.navy.mil/publications/
//!
//! Frame transformations (general):
//! - IERS Conventions (2010), Petit & Luzum, IERS Technical Note 36, Ch. 5:
//!   "Transformation Between the ICRF and ITRF"
//!   https://iers-conventions.obspm.fr/content/tn36.pdf
//! - Seidelmann, P.K. & Urban, S.E. (Eds.) (2013), "Explanatory Supplement to the
//!   Astronomical Almanac", 3rd ed., University Science Books, Ch. 3
//!   https://aa.usno.navy.mil/publications/exp_supp
//!
//! ICRF realization:
//! - Fey, A.L., et al. (2009), "The Second Realization of the International
//!   Celestial Reference Frame by Very Long Baseline Interferometry",
//!   IERS Technical Note 35
//!   https://ui.adsabs.harvard.edu/abs/2009ITN....35.....F

use nalgebra::{Matrix3, Vector3};

use super::Frame;

/// Obliquity of the ecliptic at J2000 epoch (radians).
/// IAU 1976 value: 23°26'21.448" = 23.4392911°
///
/// Reference:
/// - Lieske, J.H., et al. (1977), Astron. Astrophys. 58, 1-16, Eq. (1)
///   https://ui.adsabs.harvard.edu/abs/1977A%26A....58....1L/abstract
const OBLIQUITY_J2000: f64 = 23.4392911_f64.to_radians();

/// Frame transformation service.
#[derive(Debug, Default)]
pub struct FrameService {}

impl FrameService {
    pub fn new() -> Self {
        Self {}
    }

    /// Get the rotation matrix from one frame to another.
    pub fn rotation_matrix(&self, from: Frame, to: Frame) -> Matrix3<f64> {
        match (from, to) {
            (Frame::Eci, Frame::EclipticJ2000) => self.eci_to_ecliptic(),
            (Frame::EclipticJ2000, Frame::Eci) => self.eci_to_ecliptic().transpose(),
            (Frame::Eci, Frame::Icrf) => Matrix3::identity(),
            (Frame::Icrf, Frame::Eci) => Matrix3::identity(),
            (Frame::Eci, Frame::Ecef) => self.eci_to_ecef(),
            (Frame::Ecef, Frame::Eci) => self.eci_to_ecef().transpose(),
            (Frame::Icrf, Frame::EclipticJ2000) => {
                self.rotation_matrix(Frame::Icrf, Frame::Eci) * self.eci_to_ecliptic()
            }
            (Frame::EclipticJ2000, Frame::Icrf) => {
                self.eci_to_ecliptic().transpose() * self.rotation_matrix(Frame::Eci, Frame::Icrf)
            }
            (Frame::Icrf, Frame::Ecef) => {
                self.rotation_matrix(Frame::Icrf, Frame::Eci) * self.eci_to_ecef()
            }
            (Frame::Ecef, Frame::Icrf) => {
                self.eci_to_ecef().transpose() * self.rotation_matrix(Frame::Eci, Frame::Icrf)
            }
            (Frame::EclipticJ2000, Frame::Ecef) => {
                self.rotation_matrix(Frame::EclipticJ2000, Frame::Eci) * self.eci_to_ecef()
            }
            (Frame::Ecef, Frame::EclipticJ2000) => {
                self.eci_to_ecef().transpose() * self.eci_to_ecliptic()
            }
            _ => Matrix3::identity(),
        }
    }

    /// Transform a position vector from one frame to another.
    pub fn transform_position(&self, pos: &Vector3<f64>, from: Frame, to: Frame) -> Vector3<f64> {
        self.rotation_matrix(from, to) * pos
    }

    /// Transform a velocity vector from one frame to another.
    /// For pure rotations (no time-varying component), this is the same as
    /// transforming a position. For ECI↔ECEF, the full transform includes a
    /// velocity cross-term from Earth rotation, but for the static rotation
    /// matrix case, it's just R * v.
    pub fn transform_velocity(&self, vel: &Vector3<f64>, from: Frame, to: Frame) -> Vector3<f64> {
        self.rotation_matrix(from, to) * vel
    }

    /// Rotation matrix from ECI (J2000 equatorial) to ECLIPJ2000.
    /// Rotation about the x-axis by negative obliquity (tilting the equator
    /// down to the ecliptic plane).
    ///
    /// Reference:
    /// - Murray, C.D. & Dermott, S.F. (1999), "Solar System Dynamics",
    ///   Cambridge Univ. Press, §1.2: Eq. (1.13)
    ///   https://doi.org/10.1017/CBO9781139174817
    /// - Lieske, J.H., et al. (1977), Astron. Astrophys. 58, 1-16
    ///   https://ui.adsabs.harvard.edu/abs/1977A%26A....58....1L/abstract
    fn eci_to_ecliptic(&self) -> Matrix3<f64> {
        let c = OBLIQUITY_J2000.cos();
        let s = OBLIQUITY_J2000.sin();
        // R_x(-ε) = [[1, 0, 0], [0, cos(ε), -sin(ε)], [0, sin(ε), cos(ε)]]
        Matrix3::new(1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c)
    }

    /// Rotation matrix from ECI (J2000) to ECEF.
    /// Rotation about the z-axis by the Greenwich Mean Sidereal Time (GMST).
    /// At J2000.0 epoch (2000-01-01 12:00 TT), GMST ≈ 280.460618°.
    /// For the static case (no epoch parameter yet), use J2000 GMST.
    ///
    /// Reference:
    /// - Vallado, D.A. (2013), "Fundamentals of Astrodynamics and Applications",
    ///   4th ed., Microcosm Press, §3.4, Eq. (3-42): GMST at 0h UT1
    ///   https://microcosmpress.com/publishing/fundamentals-of-astrodynamics-and-applications-fourth-edition/
    /// - IERS Conventions (2010), §5.4: Earth rotation angle
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf (p. 45)
    fn eci_to_ecef(&self) -> Matrix3<f64> {
        // GMST at J2000.0 in radians
        let gmst = 280.4606183744_f64.to_radians();
        let c = gmst.cos();
        let s = gmst.sin();
        // R_z(θ) = [[cos(θ), sin(θ), 0], [-sin(θ), cos(θ), 0], [0, 0, 1]]
        Matrix3::new(c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0)
    }
}
