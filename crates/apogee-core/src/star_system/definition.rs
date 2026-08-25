//! Config-driven star-system definitions.
//!
//! A [`SystemDefinition`] describes an arbitrary collection of celestial
//! bodies — real (via presets) or fictional (via config files or seeded
//! random generation). Each body carries its own gravity configuration:
//! point-mass, J2-only, or a full spherical-harmonics coefficient table.
//!
//! The definition is plain data (serde-serializable), decoupled from the ECS.
//! Converting it into a live simulation is [`crate::world::World::add_system`],
//! which spawns one entity per body with a `GravitySource` component that
//! carries the resolved gravity model.
//!
//! # Example
//!
//! ```ignore
//! use apogee_core::star_system::{SystemDefinition, BodyDefinition, GravityConfig};
//!
//! // From a preset:
//! let system = SystemDefinition::earth_moon(epoch);
//!
//! // From JSON config:
//! let system = SystemDefinition::from_json_str(json)?;
//!
//! // Randomly generated (deterministic given the seed):
//! let system = SystemDefinition::random(seed, 6);
//!
//! // Into a live world:
//! world.add_system(&system)?;
//! ```

use crate::gravity::SphericalHarmonics;
use apogee_common::units::{GravitationalParameter, Meters};
use apogee_common::ApogeeResult;
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------
// Gravity configuration
// -----------------------------------------------------------------------

/// Gravity model configuration for a single body.
///
/// `PointMass` is the default. `J2` and `SphericalHarmonics` attach a
/// spherical-harmonics model to the body's `GravitySource` component.
/// `FromFile` loads coefficients from an ICGEM `.gfc`, gzipped EGM2008, or
/// plain `degree order C S` text file at simulation-setup time.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GravityConfig {
    /// Spherically symmetric (point-mass) gravity. The GM value is taken
    /// from the body's `gm` field or the NAIF table.
    #[default]
    PointMass,
    /// J2-only oblateness model.
    J2 {
        /// Fully normalized C_2,0 coefficient (dimensionless).
        c20: f64,
    },
    /// Full spherical-harmonics model with inline coefficients.
    SphericalHarmonics {
        /// Reference (equatorial) radius for non-dimensionalization (m).
        reference_radius: f64,
        /// Maximum degree to evaluate.
        degree: usize,
        /// Maximum order to evaluate.
        order: usize,
        /// Coefficient rows: `(degree, order, C, S)`.
        coefficients: Vec<[f64; 4]>,
    },
    /// Spherical-harmonics model loaded from a coefficient file.
    FromFile {
        /// Path to an ICGEM `.gfc`, gzipped EGM2008, or plain text file.
        path: String,
        /// Maximum degree to load.
        degree: usize,
        /// Maximum order to load.
        order: usize,
    },
}

impl GravityConfig {
    /// Resolve this configuration into a [`SphericalHarmonics`] model for a
    /// body with the given GM, or `None` for point-mass gravity.
    pub fn resolve(&self, gm: GravitationalParameter<f64>) -> Option<SphericalHarmonics> {
        match self {
            Self::PointMass => None,
            Self::J2 { c20 } => {
                let mut sh = SphericalHarmonics::new(2, 0);
                sh.gm = gm;
                sh.c[2][0] = *c20;
                Some(sh)
            }
            Self::SphericalHarmonics {
                reference_radius,
                degree,
                order,
                coefficients,
            } => {
                let mut sh = SphericalHarmonics::new(*degree, *order);
                sh.gm = gm;
                sh.reference_radius = Meters::new(*reference_radius);
                for row in coefficients {
                    let (n, m, c, s) = (row[0] as usize, row[1] as usize, row[2], row[3]);
                    if n <= *degree && m <= *order && m <= n {
                        sh.c[n][m] = c;
                        sh.s[n][m] = s;
                    }
                }
                Some(sh)
            }
            Self::FromFile {
                path,
                degree,
                order,
            } => SphericalHarmonics::load_egm2008(path, *degree, *order).ok(),
        }
    }
}

// -----------------------------------------------------------------------
// Body definition
// -----------------------------------------------------------------------

/// The role of a body within the system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BodyRole {
    /// Central star (e.g. the Sun).
    #[default]
    Star,
    /// Body whose motion is defined by the definition, not integrated.
    Central,
    /// Body integrated by `step_world` under N-body gravity.
    Planet,
    /// Natural satellite of a planet.
    Moon,
    /// Minor body (asteroid, comet, dwarf planet).
    Minor,
}

/// Static (config-time) description of one celestial body.
///
/// All state is explicit — no hidden NAIF lookups. `gm` is required for any
/// body that contributes gravity; `mass` is required for dynamic bodies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BodyDefinition {
    /// Human-readable name.
    pub name: String,
    /// NAIF-style ID, if the body has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naif_id: Option<i32>,
    /// Role within the system.
    #[serde(default)]
    pub role: BodyRole,
    /// Inertial position (m).
    pub position: [f64; 3],
    /// Inertial velocity (m/s).
    #[serde(default)]
    pub velocity: [f64; 3],
    /// Gravitational parameter GM (m³/s²).
    pub gm: f64,
    /// Mass (kg). Required for dynamic bodies; derived from GM if omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass: Option<f64>,
    /// Equatorial reference radius (m). Used by SH gravity models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// Gravity model for this body.
    #[serde(default)]
    pub gravity: GravityConfig,
}

impl BodyDefinition {
    /// Build a point-mass body.
    pub fn point_mass(name: impl Into<String>, gm: f64, position: [f64; 3]) -> Self {
        Self {
            name: name.into(),
            naif_id: None,
            role: BodyRole::Planet,
            position,
            velocity: [0.0; 3],
            gm,
            mass: None,
            radius: None,
            gravity: GravityConfig::PointMass,
        }
    }

    /// Build a J2 body.
    pub fn j2(
        name: impl Into<String>,
        gm: f64,
        reference_radius: f64,
        c20: f64,
        position: [f64; 3],
    ) -> Self {
        Self {
            name: name.into(),
            naif_id: None,
            role: BodyRole::Planet,
            position,
            velocity: [0.0; 3],
            gm,
            mass: None,
            radius: Some(reference_radius),
            gravity: GravityConfig::J2 { c20 },
        }
    }

    /// Set the NAIF ID.
    pub fn with_naif_id(mut self, naif_id: i32) -> Self {
        self.naif_id = Some(naif_id);
        self
    }

    /// Set the velocity.
    pub fn with_velocity(mut self, velocity: [f64; 3]) -> Self {
        self.velocity = velocity;
        self
    }

    /// Set the mass.
    pub fn with_mass(mut self, mass: f64) -> Self {
        self.mass = Some(mass);
        self
    }

    /// Set the role.
    pub fn with_role(mut self, role: BodyRole) -> Self {
        self.role = role;
        self
    }
}

// -----------------------------------------------------------------------
// System definition
// -----------------------------------------------------------------------

/// A complete, self-contained description of a star system.
///
/// Bodies are stored in insertion order. Central bodies (`Star`, `Central`)
/// are kinematic (not integrated); `Planet`, `Moon`, and `Minor` bodies are
/// dynamic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemDefinition {
    /// System name.
    pub name: String,
    /// Bodies in insertion order.
    pub bodies: Vec<BodyDefinition>,
}

impl SystemDefinition {
    /// Create an empty system.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bodies: Vec::new(),
        }
    }

    /// Add a body (builder style).
    pub fn with_body(mut self, body: BodyDefinition) -> Self {
        self.bodies.push(body);
        self
    }

    /// Look up a body by name.
    pub fn body(&self, name: &str) -> Option<&BodyDefinition> {
        self.bodies.iter().find(|b| b.name == name)
    }

    /// Parse from a JSON string.
    pub fn from_json_str(json: &str) -> ApogeeResult<Self> {
        serde_json::from_str(json)
            .map_err(|e| apogee_common::ApogeeError::Data(format!("invalid system JSON: {e}")))
    }

    /// Serialize to a JSON string.
    pub fn to_json_str(&self) -> ApogeeResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            apogee_common::ApogeeError::Data(format!("failed to serialize system: {e}"))
        })
    }

    /// Load from a JSON file.
    pub fn from_file(path: &str) -> ApogeeResult<Self> {
        let json = std::fs::read_to_string(path).map_err(apogee_common::ApogeeError::Io)?;
        Self::from_json_str(&json)
    }

    /// Write to a JSON file.
    pub fn to_file(&self, path: &str) -> ApogeeResult<()> {
        std::fs::write(path, self.to_json_str()?).map_err(apogee_common::ApogeeError::from)
    }

    /// Look up a body's index by name.
    pub fn body_index(&self, name: &str) -> Option<usize> {
        self.bodies.iter().position(|b| b.name == name)
    }

    /// Deterministically generate a random system from a seed.
    ///
    /// The generator is a PCG32 stream — no external RNG dependency, and the
    /// same seed always produces the same system. The generated system has:
    ///
    /// - One central star at the origin.
    /// - `body_count` planets on circular coplanar orbits with randomized
    ///   radii (0.5–8 AU, sorted ascending), GM drawn log-uniformly between
    ///   10^12 and 10^15 m³/s² (Mars-to-Jupiter scale), and randomized
    ///   orbital phase.
    /// - Planets get J2 gravity with C_2,0 drawn uniformly in
    ///   [-2e-2, 0.0) (oblate, zero-to-strong).
    pub fn random(seed: u64, body_count: usize) -> Self {
        let mut rng = Pcg32::new(seed);

        let mut system = Self::new("random-system");

        // Central star: solar-mass scale, at origin.
        let star_gm = 1.0e20_f64 * rng.uniform(0.5, 2.0);
        system = system.with_body(
            BodyDefinition::point_mass("star", star_gm, [0.0; 3]).with_role(BodyRole::Star),
        );

        // Planets on coplanar circular orbits, ascending radius.
        let mut radii: Vec<f64> = (0..body_count)
            .map(|_| 0.5e11 * rng.uniform(1.0, 16.0))
            .collect();
        radii.sort_by(|a, b| a.partial_cmp(b).expect("NaN radius in random system"));

        for (i, r) in radii.iter().enumerate() {
            let gm = 10.0_f64.powf(rng.uniform(12.0, 15.0));
            let phase = rng.uniform(0.0, std::f64::consts::TAU);
            let v_circ = (star_gm / r).sqrt();

            let position = [r * phase.cos(), r * phase.sin(), 0.0];
            // Circular-orbit velocity (tangential, coplanar).
            let velocity = [-v_circ * phase.sin(), v_circ * phase.cos(), 0.0];

            let radius = 2.0e6 * rng.uniform(0.5, 4.0);
            let c20 = -rng.uniform(0.0, 2.0e-2);

            system = system.with_body(
                BodyDefinition::j2(format!("planet-{i}"), gm, radius, c20, position)
                    .with_velocity(velocity)
                    .with_role(BodyRole::Planet),
            );
        }

        system
    }
}

// -----------------------------------------------------------------------
// Presets
// -----------------------------------------------------------------------

/// Named body presets with values from public reference data.
///
/// Sources:
/// - GM values: JPL "Approximate Positions of the Planets" / NAIF
///   (`gm_de431.tpc`), converted from km³/s² to m³/s².
/// - Equatorial radii: IAU 2015 nominal radii (resolution B3 and prior).
/// - C_2,0: Earth from EGM2008 (tide-free, fully normalized). Mars and the
///   Moon are derived from their dynamical J2 values (Mars J2 = 1.96045e-3,
///   Moon J2 = 2.0327e-4) via the exact relation C_2,0 = -J2/√5, which
///   reproduces Earth's EGM2008 C_2,0 to 9 significant digits.
pub mod presets {
    use super::*;

    /// Earth GM (m³/s²), GGM03C.
    pub const EARTH_GM: f64 = apogee_common::constants::GM_EARTH;
    /// Earth equatorial radius (m), WGS84.
    pub const EARTH_RADIUS: f64 = apogee_common::constants::R_EARTH_EQ;
    /// Earth fully normalized C_2,0, EGM2008 tide-free.
    pub const EARTH_C20: f64 = -0.484165143790815e-03;
    /// Moon GM (m³/s²).
    pub const MOON_GM: f64 = apogee_common::constants::GM_MOON;
    /// Moon equatorial radius (m), IAU 2015.
    pub const MOON_RADIUS: f64 = 1_737_400.0;
    /// Moon fully normalized C_2,0, derived from the J2 = 2.0327e-4 of the
    /// GL0660B solution via C_2,0 = -J2/√5.
    pub const MOON_C20: f64 = -0.909051075e-04;
    /// Sun GM (m³/s²).
    pub const SUN_GM: f64 = apogee_common::constants::GM_SUN;
    /// Sun equatorial radius (m), IAU 2015.
    pub const SUN_RADIUS: f64 = 6.957e8;
    /// Mars GM (m³/s²).
    pub const MARS_GM: f64 = 4.2828375214e13;
    /// Mars equatorial radius (m), IAU 2015.
    pub const MARS_RADIUS: f64 = 3_389_500.0;
    /// Mars fully normalized C_2,0, derived from J2 = 1.96045e-3 of the
    /// MGS-based JPL solution via C_2,0 = -J2/√5.
    pub const MARS_C20: f64 = -0.876739893e-03;
    /// Mean Earth-Moon distance (m).
    pub const EARTH_MOON_DISTANCE: f64 = 384_400_000.0;

    /// Sun + Earth (J2) + Moon (point-mass), Earth at origin.
    ///
    /// Geocentric inertial frame: Earth at origin, Moon on +x at the mean
    /// distance, Sun at 1 AU on -x. The Sun's gravity is included as a
    /// third-body perturbation. Velocities are zero — this is a static
    /// snapshot for test scenarios.
    pub fn earth_moon() -> SystemDefinition {
        SystemDefinition::new("earth-moon")
            .with_body(
                BodyDefinition::point_mass("Earth", EARTH_GM, [0.0; 3])
                    .with_naif_id(399)
                    .with_role(BodyRole::Central),
            )
            .with_body(
                BodyDefinition::point_mass("Moon", MOON_GM, [EARTH_MOON_DISTANCE, 0.0, 0.0])
                    .with_naif_id(301)
                    .with_role(BodyRole::Moon),
            )
            .with_body(
                BodyDefinition::point_mass(
                    "Sun",
                    SUN_GM,
                    [-apogee_common::constants::AU, 0.0, 0.0],
                )
                .with_naif_id(10)
                .with_role(BodyRole::Star),
            )
    }

    /// Sun + Earth (J2) + Moon (point-mass), with Earth carrying J2 gravity.
    pub fn earth_moon_j2() -> SystemDefinition {
        SystemDefinition::new("earth-moon-j2")
            .with_body(
                BodyDefinition::j2("Earth", EARTH_GM, EARTH_RADIUS, EARTH_C20, [0.0; 3])
                    .with_naif_id(399)
                    .with_role(BodyRole::Central),
            )
            .with_body(
                BodyDefinition::point_mass("Moon", MOON_GM, [EARTH_MOON_DISTANCE, 0.0, 0.0])
                    .with_naif_id(301)
                    .with_role(BodyRole::Moon),
            )
            .with_body(
                BodyDefinition::point_mass(
                    "Sun",
                    SUN_GM,
                    [-apogee_common::constants::AU, 0.0, 0.0],
                )
                .with_naif_id(10)
                .with_role(BodyRole::Star),
            )
    }

    /// Inner solar system: Sun, Mercury, Venus, Earth+Moon, Mars.
    ///
    /// Positions are rough mean states (not ephemeris-accurate) — good for
    /// scenario scaffolding, not for mission design. All planets are
    /// point-mass; Earth additionally carries J2.
    pub fn inner_solar_system() -> SystemDefinition {
        let au = apogee_common::constants::AU;
        let mercury = BodyDefinition::point_mass("Mercury", 2.2032e13, [0.387 * au, 0.0, 0.0])
            .with_naif_id(199)
            .with_role(BodyRole::Planet);
        let venus = BodyDefinition::point_mass("Venus", 3.24858599e14, [0.723 * au, 0.0, 0.0])
            .with_naif_id(299)
            .with_role(BodyRole::Planet);
        let earth = BodyDefinition::j2(
            "Earth",
            EARTH_GM,
            EARTH_RADIUS,
            EARTH_C20,
            [1.0 * au, 0.0, 0.0],
        )
        .with_naif_id(399)
        .with_role(BodyRole::Planet);
        let moon =
            BodyDefinition::point_mass("Moon", MOON_GM, [1.0 * au + EARTH_MOON_DISTANCE, 0.0, 0.0])
                .with_naif_id(301)
                .with_role(BodyRole::Moon);
        let mars = BodyDefinition::j2(
            "Mars",
            MARS_GM,
            MARS_RADIUS,
            MARS_C20,
            [1.524 * au, 0.0, 0.0],
        )
        .with_naif_id(499)
        .with_role(BodyRole::Planet);

        SystemDefinition::new("inner-solar-system")
            .with_body(
                BodyDefinition::point_mass("Sun", SUN_GM, [0.0; 3])
                    .with_naif_id(10)
                    .with_role(BodyRole::Star),
            )
            .with_body(mercury)
            .with_body(venus)
            .with_body(earth)
            .with_body(moon)
            .with_body(mars)
    }

    /// Sun + Earth (J2) only — minimal Earth-orbit scenario.
    pub fn earth_only_j2() -> SystemDefinition {
        SystemDefinition::new("earth-only-j2")
            .with_body(
                BodyDefinition::j2("Earth", EARTH_GM, EARTH_RADIUS, EARTH_C20, [0.0; 3])
                    .with_naif_id(399)
                    .with_role(BodyRole::Central),
            )
            .with_body(
                BodyDefinition::point_mass(
                    "Sun",
                    SUN_GM,
                    [-apogee_common::constants::AU, 0.0, 0.0],
                )
                .with_naif_id(10)
                .with_role(BodyRole::Star),
            )
    }
}

// -----------------------------------------------------------------------
// Seeded RNG (PCG32) — no external dependency
// -----------------------------------------------------------------------

/// Minimal PCG32 generator for deterministic system generation.
///
/// See O'Neill, "PCG: A Family of Simple Fast Space-Efficient Statistically
/// Good Algorithms for Random Number Generation" (2014). Only the 32-bit
/// output variant is implemented; `uniform` is unbiased via rejection
/// sampling.
#[derive(Debug, Clone)]
struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    fn new(seed: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (seed << 1) | 1,
        };
        rng.next_u32();
        rng.state += seed;
        rng.next_u32();
        rng
    }

    fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform sample in `[lo, hi)`.
    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        debug_assert!(hi > lo);
        // Unbiased float generation via rejection on the u32 range.
        let scale = (hi - lo) / u32::MAX as f64;
        lo + self.next_u32() as f64 * scale
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_preserves_system() {
        let system = presets::earth_moon_j2();
        let json = system.to_json_str().unwrap();
        let parsed = SystemDefinition::from_json_str(&json).unwrap();
        assert_eq!(parsed, system);
    }

    #[test]
    fn presets_have_correct_body_count() {
        assert_eq!(presets::earth_moon().bodies.len(), 3);
        assert_eq!(presets::earth_moon_j2().bodies.len(), 3);
        assert_eq!(presets::earth_only_j2().bodies.len(), 2);
        assert_eq!(presets::inner_solar_system().bodies.len(), 6);
    }

    #[test]
    fn preset_earth_has_j2() {
        let system = presets::earth_moon_j2();
        let earth = system.body("Earth").unwrap();
        assert_eq!(
            earth.gravity,
            GravityConfig::J2 {
                c20: presets::EARTH_C20
            }
        );
        // Sanity: EGM2008 C20 is negative and ~1e-3 scale.
        assert!(earth
            .gravity
            .resolve(GravitationalParameter::new(presets::EARTH_GM))
            .is_some());
    }

    #[test]
    fn random_system_is_deterministic() {
        let a = SystemDefinition::random(42, 5);
        let b = SystemDefinition::random(42, 5);
        assert_eq!(a, b);

        let c = SystemDefinition::random(43, 5);
        assert_ne!(a, c);
    }

    #[test]
    fn random_system_structure() {
        let system = SystemDefinition::random(7, 4);
        assert_eq!(system.bodies.len(), 5); // star + 4 planets
        assert_eq!(system.bodies[0].name, "star");
        assert_eq!(system.bodies[0].role, BodyRole::Star);

        // Planets have circular velocities ~ perpendicular to radius.
        for planet in &system.bodies[1..] {
            let r: nalgebra::Vector3<f64> = planet.position.into();
            let v: nalgebra::Vector3<f64> = planet.velocity.into();
            // Coplanar: no z motion or position.
            assert_eq!(r.z, 0.0);
            assert_eq!(v.z, 0.0);
            // Near-circular: |r × v| ≈ |r||v|.
            let cross = r.cross(&v).norm();
            let par = r.norm() * v.norm();
            assert!(
                (cross - par).abs() / par < 1e-12,
                "velocity not tangential: cross={cross}, par={par}"
            );
            // J2 gravity configured.
            assert!(matches!(planet.gravity, GravityConfig::J2 { .. }));
        }
    }

    #[test]
    fn gravity_config_resolve_point_mass_is_none() {
        assert!(GravityConfig::PointMass
            .resolve(GravitationalParameter::new(1.0e14))
            .is_none());
    }

    #[test]
    fn gravity_config_resolve_j2() {
        let sh = GravityConfig::J2 { c20: -1.0e-3 }
            .resolve(GravitationalParameter::new(1.0e14))
            .unwrap();
        assert_eq!(sh.degree, 2);
        assert_eq!(sh.c[2][0], -1.0e-3);
    }

    #[test]
    fn gravity_config_resolve_full_sh() {
        let sh = GravityConfig::SphericalHarmonics {
            reference_radius: 6_378_137.0,
            degree: 2,
            order: 2,
            coefficients: vec![[2.0, 0.0, -4.84e-4, 0.0], [2.0, 2.0, 1.0e-6, -2.0e-6]],
        }
        .resolve(GravitationalParameter::new(1.0e14))
        .unwrap();
        assert_eq!(sh.c[2][0], -4.84e-4);
        assert_eq!(sh.c[2][2], 1.0e-6);
        assert_eq!(sh.s[2][2], -2.0e-6);
    }

    #[test]
    fn file_round_trip() {
        let system = presets::earth_moon();
        let path = std::env::temp_dir().join("apogee_system_test.json");
        system.to_file(path.to_str().unwrap()).unwrap();
        let loaded = SystemDefinition::from_file(path.to_str().unwrap()).unwrap();
        assert_eq!(loaded, system);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn from_json_str_parses_custom_system() {
        let json = r#"{
            "name": "custom",
            "bodies": [
                {
                    "name": "homeworld",
                    "role": "central",
                    "position": [0.0, 0.0, 0.0],
                    "gm": 3.986e14,
                    "gravity": {"type": "j2", "c20": -0.00048}
                },
                {
                    "name": "relay",
                    "role": "minor",
                    "position": [7000000.0, 0.0, 0.0],
                    "gm": 0.0
                }
            ]
        }"#;
        let system = SystemDefinition::from_json_str(json).unwrap();
        assert_eq!(system.name, "custom");
        assert_eq!(system.bodies.len(), 2);
        assert_eq!(
            system.body("homeworld").unwrap().gravity,
            GravityConfig::J2 { c20: -0.00048 }
        );
    }
}
