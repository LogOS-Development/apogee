//! Geomagnetic disturbance field driven by solar / geomagnetic activity.
//!
//! This is a deliberately simple placeholder for a full magnetospheric model
//! (e.g. Tsyganenko or external IGRF). It superposes a perturbation on the
//! IGRF main field that scales with the geomagnetic Ap index, analogous to how
//! atmosphere models such as Jacchia-Bowman scale density with F10.7 and Ap.
//!
//! A realistic implementation would couple to Dst, solar-wind pressure, and the
//! interplanetary magnetic field; the placeholder covers the API surface and
//! gives a deterministic first-order correction.
//!
//! # Multi-body design
//!
//! Disturbance models are body-specific: Earth has a ring-current / magnetopause
//! model, while other bodies have different physics (e.g. Mercury's weak
//! induction, Jupiter's magnetodisc). The `add_ap_perturbation` function takes a
//! NAIF body ID plus the same coefficient/position inputs as the main field so
//! callers can opt in only for supported bodies and future implementations can
//! evolve the signature in place.
//!
//! A future `MagneticDisturbanceModel` trait (issue #77) will let us swap
//! implementations per body with a common configuration interface.
//!
//! # Sources
//! * Ap index: NOAA SWPC, <https://www.swpc.noaa.gov/products/station-k-and-indices>
//! * Ring-current proxy: roughly proportional to the Dst index; see
//!   Sugiura, M. (1965), "Hourly values of equatorial Dst for the IGY".

use apogee_common::NaifId;
use nalgebra::Vector3;

/// NAIF body ID for Earth. Only Earth is supported by this placeholder; other
/// bodies silently receive no perturbation.
const EARTH_NAIF_ID: NaifId = 399;

/// Empirical scaling from Ap to an equivalent ring-current perturbation of
/// the dipole coefficient (nT). Negative because increased activity weakens
/// the main-field dipole at low latitudes.
const AP_TO_G10_PERTURBATION_NT: f64 = -2.0;

/// Evaluate the IGRF-13 main field plus an Ap-driven disturbance.
///
/// `ap` is the daily geomagnetic activity index. `g_main`/`h_main` are the
/// IGRF coefficients at the requested epoch. `position_m` is the geocentric
/// ECEF position in meters.
///
/// The current placeholder is axisymmetric and degree-1 only: it weakens the
/// axial dipole `g_1^0` and leaves `h_main` unchanged. Position dependence is
/// intentionally ignored at this fidelity because a realistic spatial
/// perturbation (ring-current latitude profile, magnetopause compression, etc.)
/// belongs in the body-specific `MagneticDisturbanceModel` trait tracked in
/// issue #77. The signature still accepts these inputs so future body-specific
/// models can be dropped in without changing call sites.
pub(crate) fn add_ap_perturbation(
    body_id: NaifId,
    g_main: &mut [f64],
    h_main: &mut [f64],
    _position_m: &Vector3<f64>,
    ap: f64,
) {
    if body_id != EARTH_NAIF_ID {
        return;
    }

    // h coefficients are unchanged by an axisymmetric ring-current model.
    // Keep the parameter in the signature for future body-specific models.
    let _ = h_main;

    // g_1^0 is stored at triangular index n*(n+1)/2 + m = 1 for n=1, m=0.
    g_main[1] += ap * AP_TO_G10_PERTURBATION_NT;

    // Position-dependent modulation would go here under issue #77, e.g.
    // a latitude-weighted ring-current term.
    let _ = _position_m;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_receives_ap_perturbation() {
        let mut g = [0.0; 4];
        let mut h = [0.0; 4];
        add_ap_perturbation(EARTH_NAIF_ID, &mut g, &mut h, &Vector3::zeros(), 10.0);
        assert_eq!(g[1], 10.0 * AP_TO_G10_PERTURBATION_NT);
        assert_eq!(h[1], 0.0);
    }

    #[test]
    fn non_earth_body_is_unperturbed() {
        let mut g = [0.0; 4];
        let mut h = [0.0; 4];
        add_ap_perturbation(499, &mut g, &mut h, &Vector3::zeros(), 10.0);
        assert_eq!(g[1], 0.0);
        assert_eq!(h[1], 0.0);
    }
}
