//! Shared body/GM tables for the DE441 n-body validations.
//!
//! Used by both the full-kernel suite (`nbody_validation`, behind the
//! `de441` feature) and the committed-fixture pipeline test
//! (`de441_fixture_validation`, runs in every PR).

/// Bodies under test: (NAIF ID, display name).
///
/// All 11 major bodies of the solar system, integrated as a true n-body
/// system: the Earth-Moon barycenter motion EMERGES from Earth and Moon
/// mutual gravity rather than being imposed.
pub(crate) const BODIES: &[(i32, &str)] = &[
    (10, "Sun"),
    (1, "Mercury barycenter"),
    (2, "Venus barycenter"),
    // EMB (3) is deliberately absent: Earth (399) and Moon (301) are
    // integrated individually, so their barycenter motion emerges from
    // the dynamics. Including all three would double-count the mass.
    (4, "Mars barycenter"),
    (5, "Jupiter barycenter"),
    (6, "Saturn barycenter"),
    (7, "Uranus barycenter"),
    (8, "Neptune barycenter"),
    (9, "Pluto barycenter"),
    (399, "Earth"),
    (301, "Moon"),
];

/// GM values for the bodies (m³/s²), DE441-consistent.
///
/// For barycenters the GM is the total of the system (e.g. Jupiter system
/// = planet + moons), matching how DE441 integrates them. Earth and Moon
/// carry their individual GMs.
#[allow(clippy::excessive_precision)] // GM values quoted from DE441 documentation
pub(crate) const GM: &[(i32, f64)] = &[
    (10, 1.32712440041279419e20),
    (1, 2.203186e13),
    (2, 3.2485859200000005e14),
    (4, 4.282837581600000e13),
    (5, 1.267127648000002e17), // Jupiter system
    (6, 3.793118700000000e16), // Saturn system
    (7, 5.793951322000000e15), // Uranus system
    (8, 6.836527100580000e15), // Neptune system
    (9, 9.755000000000000e11), // Pluto system
    (399, 3.986004354360000e14),
    (301, 4.902800066000000e12),
];

/// GM lookup for a NAIF ID; panics on unknown IDs (the table and the
/// body list must stay in sync).
pub(crate) fn gm_of(naif_id: i32) -> f64 {
    GM.iter()
        .find(|(id, _)| *id == naif_id)
        .map(|(_, gm)| *gm)
        .unwrap_or_else(|| panic!("no GM table entry for NAIF ID {naif_id}"))
}
