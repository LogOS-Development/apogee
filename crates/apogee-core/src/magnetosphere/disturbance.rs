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
//!
//! # Multi-body design
//!
//! Disturbance models are body-specific: Earth has a ring-current /
//! magnetopause model, while other bodies have different physics (e.g. Mercury's
//! weak induction, Jupiter's magnetodisc). The `add_ap_perturbation` function
//! takes a NAIF body ID so callers can opt in only for supported bodies.
//! A future `MagneticDisturbanceModel` trait (#TODO: task 77) will let us swap
//! implementations per body with a common configuration interface.
//!
//! # Sources
//! * Ap index: NOAA SWPC, <https://www.swpc.noaa.gov/products/station-k-and-indices>
//! * Ring-current proxy: roughly proportional to the Dst index; see
//!   Sugiura, M. (1965), "Hourly values of equatorial Dst for the IGY".

use apogee_common::NaifId;

/// NAIF body ID for Earth. Only Earth is supported by this placeholder; other
/// bodies silently receive no perturbation.
const EARTH_NAIF_ID: NaifId = 399;

/// Empirical scaling from Ap to an equivalent ring-current perturbation of
/// the dipole coefficient (nT). Negative because increased activity weakens
/// the main-field dipole at low latitudes.
const AP_TO_G10_PERTURBATION_NT: f64 = -2.0;

/// Evaluate the IGRF-13 main field plus an Ap-driven degree-1 disturbance.
///
/// `ap` is the daily geomagnetic activity index. `g_main` are the
/// IGRF coefficients at the requested epoch. The perturbation is added only to
/// the axial dipole term `g_1^0`, keeping the implementation simple and the
/// spherical-harmonic structure intact.
///
/// `h_main` and position dependence are intentionally omitted from the
/// placeholder; they will be handled by the body-specific
/// `MagneticDisturbanceModel` trait tracked in issue #77.
pub(crate) fn add_ap_perturbation(body_id: NaifId, g_main: &mut [f64], ap: f64) {
    if body_id != EARTH_NAIF_ID {
        return;
    }

    // g_1^0 is stored at triangular index n*(n+1)/2 + m = 1 for n=1, m=0.
    g_main[1] += ap * AP_TO_G10_PERTURBATION_NT;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_receives_ap_perturbation() {
        let mut g = [0.0; 4];
        add_ap_perturbation(EARTH_NAIF_ID, &mut g, 10.0);
        assert_eq!(g[1], 10.0 * AP_TO_G10_PERTURBATION_NT);
    }

    #[test]
    fn non_earth_body_is_unperturbed() {
        let mut g = [0.0; 4];
        add_ap_perturbation(499, &mut g, 10.0);
        assert_eq!(g[1], 0.0);
    }
}
