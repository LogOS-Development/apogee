//! Geomagnetic disturbance field driven by solar / geomagnetic activity.
//!
//! This is a deliberately simple placeholder for a full magnetospheric model
//! (e.g. Tsyganenko or external IGRF). It superposes a degree-1 perturbation
//! on the IGRF main field that scales with the geomagnetic Ap index, analogous
//! to how atmosphere models such as Jacchia-Bowman scale density with F10.7
//! and Ap.
//!
//! A realistic implementation would couple to Dst, solar-wind pressure, and
//! the interplanetary magnetic field; the placeholder covers the API surface
//! and gives a deterministic first-order correction.

use nalgebra::Vector3;

use crate::magnetosphere::data::IGRF_REF_RADIUS_KM;

/// Empirical scaling from Ap to an equivalent ring-current perturbation of
/// the dipole coefficient (nT). Negative because increased activity weakens
/// the main-field dipole at low latitudes.
const AP_TO_G10_PERTURBATION_NT: f64 = -2.0;

/// Evaluate the IGRF-13 main field plus an Ap-driven degree-1 disturbance.
///
/// `ap` is the daily geomagnetic activity index. `g_main`/`h_main` are the
/// IGRF coefficients at the requested epoch. The perturbation is added only to
/// the axial dipole term `g_1^0`, keeping the implementation simple and the
/// spherical-harmonic structure intact.
pub(crate) fn add_ap_perturbation(
    g_main: &mut [f64],
    h_main: &mut [f64],
    _position_m: &Vector3<f64>,
    ap: f64,
) {
    // h coefficients are unchanged by an axisymmetric ring-current model.
    let _ = h_main;
    let _ = _position_m;
    let _ = IGRF_REF_RADIUS_KM;

    // g_1^0 is stored at triangular index n*(n+1)/2 + m = 1 for n=1, m=0.
    g_main[1] += ap * AP_TO_G10_PERTURBATION_NT;
}
