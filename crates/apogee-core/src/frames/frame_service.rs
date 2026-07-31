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
//!
//! Earth Rotation Angle (ERA):
//! - IERS Conventions (2010), Eq. (5.15)
//!   https://iers-conventions.obspm.fr/content/tn36.pdf

use hifitime::{Epoch, TimeScale};
use nalgebra::{Matrix3, Vector3};

use super::{EopData, Frame, NutationPrecessionModel};

/// Obliquity of the ecliptic at J2000 epoch (radians).
/// IAU 1976 value: 23°26'21.448" = 23.4392911°
///
/// Reference:
/// - Lieske, J.H., et al. (1977), Astron. Astrophys. 58, 1-16, Eq. (1)
///   https://ui.adsabs.harvard.edu/abs/1977A%26A....58....1L/abstract
const OBLIQUITY_J2000: f64 = 23.4392911_f64.to_radians();

/// 1 arcsecond in radians.
const ARCS: f64 = std::f64::consts::PI / 180.0 / 3600.0;

/// Frame transformation service.
#[derive(Debug, Default)]
pub struct FrameService {
    nutation_precession: NutationPrecessionModel,
    eop: Option<EopData>,
}

impl FrameService {
    pub fn new() -> Self {
        Self {
            nutation_precession: NutationPrecessionModel::new(),
            eop: None,
        }
    }

    /// Create a FrameService with Earth Orientation Parameters for
    /// high-precision ICRF↔ITRF transformations.
    pub fn with_eop(eop: EopData) -> Self {
        Self {
            nutation_precession: NutationPrecessionModel::new(),
            eop: Some(eop),
        }
    }

    /// Get the rotation matrix from one frame to another at a given epoch.
    ///
    /// For epoch-independent pairs (Eci↔EclipticJ2000, Eci↔Icrf) the epoch
    /// is ignored. For Eci↔Ecef the epoch is used to compute GMST and, when
    /// EOP data is available, the polar-motion matrix.
    pub fn rotation_matrix(&self, from: Frame, to: Frame, epoch: Epoch) -> Matrix3<f64> {
        match (from, to) {
            _ if from == to => Matrix3::identity(),
            (Frame::Eci, Frame::EclipticJ2000) => self.eci_to_ecliptic(),
            (Frame::EclipticJ2000, Frame::Eci) => self.eci_to_ecliptic().transpose(),
            (Frame::Eci, Frame::Icrf) => self.icrf_to_eci(epoch).transpose(),
            (Frame::Icrf, Frame::Eci) => self.icrf_to_eci(epoch),
            (Frame::Eci, Frame::Ecef) => self.eci_to_ecef(epoch),
            (Frame::Ecef, Frame::Eci) => self.eci_to_ecef(epoch).transpose(),
            (Frame::Icrf, Frame::EclipticJ2000) => {
                self.rotation_matrix(Frame::Icrf, Frame::Eci, epoch) * self.eci_to_ecliptic()
            }
            (Frame::EclipticJ2000, Frame::Icrf) => {
                self.eci_to_ecliptic().transpose()
                    * self.rotation_matrix(Frame::Eci, Frame::Icrf, epoch)
            }
            (Frame::Icrf, Frame::Ecef) => {
                self.rotation_matrix(Frame::Icrf, Frame::Eci, epoch) * self.eci_to_ecef(epoch)
            }
            (Frame::Ecef, Frame::Icrf) => {
                self.eci_to_ecef(epoch).transpose()
                    * self.rotation_matrix(Frame::Eci, Frame::Icrf, epoch)
            }
            (Frame::EclipticJ2000, Frame::Ecef) => {
                self.rotation_matrix(Frame::EclipticJ2000, Frame::Eci, epoch)
                    * self.eci_to_ecef(epoch)
            }
            (Frame::Ecef, Frame::EclipticJ2000) => {
                self.eci_to_ecef(epoch).transpose() * self.eci_to_ecliptic()
            }
            (Frame::BodyFixed(_), _) | (_, Frame::BodyFixed(_)) => {
                // Body-fixed frames require a planetary rotation model;
                // not implemented in phase 1.1.
                Matrix3::identity()
            }
            _ => {
                // Fallback for any uncovered same-frame pairs (should be
                // unreachable because of the identity arm above).
                Matrix3::identity()
            }
        }
    }

    /// Transform a position vector from one frame to another at an epoch.
    pub fn transform_position(
        &self,
        pos: &Vector3<f64>,
        from: Frame,
        to: Frame,
        epoch: Epoch,
    ) -> Vector3<f64> {
        self.rotation_matrix(from, to, epoch) * pos
    }

    /// Transform a velocity vector from one frame to another at an epoch.
    ///
    /// For pure rotations this is the same as a position transform. For
    /// ECI↔ECEF the caller is responsible for adding the Earth-rotation
    /// cross-term when full velocity transformation is required.
    pub fn transform_velocity(
        &self,
        vel: &Vector3<f64>,
        from: Frame,
        to: Frame,
        epoch: Epoch,
    ) -> Vector3<f64> {
        self.rotation_matrix(from, to, epoch) * vel
    }

    /// Rotation matrix from ICRF to the J2000 mean equator/equinox (ECI).
    ///
    /// ICRF and the J2000 mean equator/equinox are related by the IAU 2000
    /// frame bias only (a sub-arcsecond rotation). Nutation and precession
    /// are not included because ECI is a mean-of-date frame fixed at J2000.
    fn icrf_to_eci(&self, _epoch: Epoch) -> Matrix3<f64> {
        self.nutation_precession.frame_bias_matrix().transpose()
    }

    /// Rotation matrix from ECI (J2000) to ECEF at the given epoch.
    ///
    /// This is the bias-precession-nutation matrix (BPN) followed by the
    /// Earth-rotation matrix R_z(-ERA) and the polar-motion matrix W:
    ///
    ///   ECEF = W · R_z(-ERA) · N · P · B · ECI
    ///
    /// When EOP data is unavailable the polar-motion matrix is the identity.
    fn eci_to_ecef(&self, epoch: Epoch) -> Matrix3<f64> {
        let bpn = self.nutation_precession.gcrf_to_tod_matrix(epoch);
        let era = self.earth_rotation_angle(epoch);
        let c = era.cos();
        let s = era.sin();
        // R_z(-ERA)
        let r = Matrix3::new(c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0);
        let earth_rot = r * bpn;

        if let Some(eop) = &self.eop {
            let mjd_utc = epoch.to_time_scale(TimeScale::UTC).to_mjd_utc_days();
            if let Some(entry) = eop.at_mjd(mjd_utc) {
                let w = polar_motion_matrix(entry.x_pole, entry.y_pole);
                return w * earth_rot;
            }
        }

        earth_rot
    }

    /// Earth Rotation Angle (ERA) in radians from UT1 at the given epoch.
    ///
    /// # References
    ///
    /// - IERS Conventions (2010), Eq. (5.15)
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf
    pub fn earth_rotation_angle(&self, epoch: Epoch) -> f64 {
        let jd_ut1 = self.jd_ut1(epoch);
        let d = jd_ut1 - 2_451_545.0;
        let fraction = 0.779_057_273_264_0 + 1.002_737_811_911_354_6 * d;
        let era = (fraction % 1.0) * std::f64::consts::TAU;
        if era < 0.0 {
            era + std::f64::consts::TAU
        } else {
            era
        }
    }

    fn jd_ut1(&self, epoch: Epoch) -> f64 {
        // Use the TT-scale JD as the continuous time proxy, then apply the
        // UT1-UTC correction from EOP. This aligns the J2000.0 reference
        // (JD 2451545.0) correctly across time scales.
        let jd_tt = epoch.to_jde_tt_days();
        let mjd_utc = epoch.to_time_scale(TimeScale::UTC).to_mjd_utc_days();
        let ut1_utc = self
            .eop
            .as_ref()
            .and_then(|eop| eop.at_mjd(mjd_utc))
            .map(|e| e.ut1_utc)
            .unwrap_or(0.0);
        jd_tt + ut1_utc / 86_400.0
    }

    /// Rotation matrix from ECI (J2000 equatorial) to ECLIPJ2000.
    fn eci_to_ecliptic(&self) -> Matrix3<f64> {
        let c = OBLIQUITY_J2000.cos();
        let s = OBLIQUITY_J2000.sin();
        // R_x(-ε) = [[1, 0, 0], [0, cos(ε), -sin(ε)], [0, sin(ε), cos(ε)]]
        Matrix3::new(1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c)
    }
}

/// Polar motion matrix W = R_y(-x_pole) R_x(-y_pole).
///
/// x_pole and y_pole are in arcseconds. Returns the small rotation from
/// the terrestrial intermediate frame to ITRF/ECEF.
fn polar_motion_matrix(x_pole: f64, y_pole: f64) -> Matrix3<f64> {
    let x = x_pole * ARCS;
    let y = y_pole * ARCS;
    let cx = x.cos();
    let sx = x.sin();
    let cy = y.cos();
    let sy = y.sin();

    // R_y(-x) R_x(-y) =
    // [[ cx, sx*sy, sx*cy ],
    //  [ 0,   cy,    -sy  ],
    //  [-sx,  cx*sy, cx*cy ]]
    Matrix3::new(cx, sx * sy, sx * cy, 0.0, cy, -sy, -sx, cx * sy, cx * cy)
}
