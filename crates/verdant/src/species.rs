//! Species definitions for the Verdant vegetation model.
//!
//! A species is a config-loaded template (never instantiated directly).
//! Members — individual plants or densities — are instances of a species,
//! positioned in space, with mutable state that changes over time.
//!
//! Config format is TOML. See `config/species/` for examples.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Post-fire regeneration strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PostFireRegen {
    /// Killed by fire, no regeneration mechanism.
    #[default]
    None,
    /// Survives fire by resprouting from roots or lignotuber.
    Resprout,
    /// Killed by fire but seeds released from serotinous cones.
    Serotinous,
    /// Survives via seed bank in soil or dispersal from unburned areas.
    InSeed,
}

/// Life-history traits and growth parameters for a plant species.
///
/// Follows LANDIS-II conventions (Scheller & Mladenoff, 2004) with
/// config-driven parameterization. All plants of a species share one
/// config; behavioral differences emerge from parameter values, not
/// from separate code paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeciesConfig {
    /// Unique species identifier (e.g., "ponderosa_pine").
    pub name: String,
    /// Maximum lifespan in years.
    pub longevity: u32,
    /// Age at first seed production (years).
    pub sexual_maturity: u32,
    /// Shade tolerance class (1 = intolerant, 5 = very tolerant).
    #[serde(default = "default_tolerance")]
    pub shade_tolerance: u8,
    /// Fire tolerance class (1 = very sensitive, 5 = resistant).
    #[serde(default = "default_tolerance")]
    pub fire_tolerance: u8,
    /// Post-fire regeneration strategy.
    #[serde(default)]
    pub post_fire_regen: PostFireRegen,
    /// Effective seed dispersal distance — most seeds land within
    /// this radius (meters).
    pub effective_seed_distance: f32,
    /// Maximum seed dispersal distance (meters).
    pub max_seed_distance: f32,
    /// Maximum aboveground biomass (kg/m^2).
    pub max_biomass: f32,
    /// Growth curve shape parameter (0-1).
    /// Controls how quickly biomass approaches maximum.
    #[serde(default = "default_growth_curve")]
    pub growth_curve: f32,
    /// Mortality curve shape parameter (0-1).
    /// Controls when senescence begins relative to longevity.
    #[serde(default = "default_mortality_curve")]
    pub mortality_curve: f32,
}

fn default_tolerance() -> u8 {
    3
}
fn default_growth_curve() -> f32 {
    0.3
}
fn default_mortality_curve() -> f32 {
    0.85
}

impl SpeciesConfig {
    /// Load a species config from a TOML file.
    pub fn from_toml_file(path: &Path) -> Result<Self, SpeciesConfigError> {
        let text =
            std::fs::read_to_string(path).map_err(|e| SpeciesConfigError::Io(e.to_string()))?;
        Self::from_toml_str(&text)
    }

    /// Parse a species config from a TOML string.
    pub fn from_toml_str(text: &str) -> Result<Self, SpeciesConfigError> {
        let config: SpeciesConfig =
            toml::from_str(text).map_err(|e| SpeciesConfigError::Parse(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Validate that all trait values are within biological ranges.
    pub fn validate(&self) -> Result<(), SpeciesConfigError> {
        if self.name.is_empty() {
            return Err(SpeciesConfigError::Invalid("name is empty".into()));
        }
        if self.longevity == 0 {
            return Err(SpeciesConfigError::Invalid("longevity must be > 0".into()));
        }
        if self.sexual_maturity == 0 {
            return Err(SpeciesConfigError::Invalid(
                "sexual_maturity must be > 0".into(),
            ));
        }
        if self.sexual_maturity >= self.longevity {
            return Err(SpeciesConfigError::Invalid(
                "sexual_maturity must be < longevity".into(),
            ));
        }
        if !(1..=5).contains(&self.shade_tolerance) {
            return Err(SpeciesConfigError::Invalid(
                "shade_tolerance must be 1-5".into(),
            ));
        }
        if !(1..=5).contains(&self.fire_tolerance) {
            return Err(SpeciesConfigError::Invalid(
                "fire_tolerance must be 1-5".into(),
            ));
        }
        if self.effective_seed_distance < 0.0 {
            return Err(SpeciesConfigError::Invalid(
                "effective_seed_distance must be >= 0".into(),
            ));
        }
        if self.max_seed_distance < self.effective_seed_distance {
            return Err(SpeciesConfigError::Invalid(
                "max_seed_distance must be >= effective_seed_distance".into(),
            ));
        }
        if self.max_biomass <= 0.0 {
            return Err(SpeciesConfigError::Invalid(
                "max_biomass must be > 0".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.growth_curve) {
            return Err(SpeciesConfigError::Invalid(
                "growth_curve must be 0-1".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.mortality_curve) {
            return Err(SpeciesConfigError::Invalid(
                "mortality_curve must be 0-1".into(),
            ));
        }
        Ok(())
    }

    /// Relative shade tolerance as a fraction (0 = intolerant, 1 = very tolerant).
    pub fn shade_tolerance_frac(&self) -> f32 {
        self.shade_tolerance as f32 / 5.0
    }

    /// Relative fire tolerance as a fraction (0 = sensitive, 1 = resistant).
    pub fn fire_tolerance_frac(&self) -> f32 {
        self.fire_tolerance as f32 / 5.0
    }
}

/// Errors that can occur loading or validating species configs.
#[derive(Debug, Clone)]
pub enum SpeciesConfigError {
    Io(String),
    Parse(String),
    Invalid(String),
}

impl std::fmt::Display for SpeciesConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "species config IO error: {msg}"),
            Self::Parse(msg) => write!(f, "species config parse error: {msg}"),
            Self::Invalid(msg) => write!(f, "species config invalid: {msg}"),
        }
    }
}

impl std::error::Error for SpeciesConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    const PONDEROSA_TOML: &str = r#"
name = "ponderosa_pine"
longevity = 350
sexual_maturity = 15
shade_tolerance = 2
fire_tolerance = 3
post_fire_regen = "InSeed"
effective_seed_distance = 50.0
max_seed_distance = 4000.0
max_biomass = 8.5
growth_curve = 0.3
mortality_curve = 0.85
"#;

    #[test]
    fn load_ponderosa_pine() {
        let spp = SpeciesConfig::from_toml_str(PONDEROSA_TOML).unwrap();
        assert_eq!(spp.name, "ponderosa_pine");
        assert_eq!(spp.longevity, 350);
        assert_eq!(spp.sexual_maturity, 15);
        assert_eq!(spp.shade_tolerance, 2);
        assert_eq!(spp.fire_tolerance, 3);
        assert_eq!(spp.post_fire_regen, PostFireRegen::InSeed);
        assert!((spp.effective_seed_distance - 50.0).abs() < 1e-6);
        assert!((spp.max_seed_distance - 4000.0).abs() < 1e-6);
        assert!((spp.max_biomass - 8.5).abs() < 1e-6);
    }

    #[test]
    fn shade_tolerance_clamped_by_validation() {
        let toml_str = r#"
name = "bad_species"
longevity = 100
sexual_maturity = 10
shade_tolerance = 7
fire_tolerance = 3
effective_seed_distance = 10.0
max_seed_distance = 100.0
max_biomass = 5.0
"#;
        let result = SpeciesConfig::from_toml_str(toml_str);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("shade_tolerance"));
    }

    #[test]
    fn maturity_must_be_less_than_longevity() {
        let toml_str = r#"
name = "bad_species"
longevity = 10
sexual_maturity = 10
shade_tolerance = 3
fire_tolerance = 3
effective_seed_distance = 10.0
max_seed_distance = 100.0
max_biomass = 5.0
"#;
        let result = SpeciesConfig::from_toml_str(toml_str);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("sexual_maturity"));
    }

    #[test]
    fn max_seed_must_exceed_effective() {
        let toml_str = r#"
name = "bad_species"
longevity = 100
sexual_maturity = 10
shade_tolerance = 3
fire_tolerance = 3
effective_seed_distance = 100.0
max_seed_distance = 50.0
max_biomass = 5.0
"#;
        let result = SpeciesConfig::from_toml_str(toml_str);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("max_seed_distance"));
    }

    #[test]
    fn defaults_applied_for_omitted_fields() {
        let toml_str = r#"
name = "simple"
longevity = 100
sexual_maturity = 10
effective_seed_distance = 10.0
max_seed_distance = 100.0
max_biomass = 5.0
"#;
        let spp = SpeciesConfig::from_toml_str(toml_str).unwrap();
        assert_eq!(spp.shade_tolerance, 3);
        assert_eq!(spp.fire_tolerance, 3);
        assert_eq!(spp.post_fire_regen, PostFireRegen::None);
        assert!((spp.growth_curve - 0.3).abs() < 1e-6);
        assert!((spp.mortality_curve - 0.85).abs() < 1e-6);
    }

    #[test]
    fn tolerance_fractions() {
        let spp = SpeciesConfig::from_toml_str(PONDEROSA_TOML).unwrap();
        assert!((spp.shade_tolerance_frac() - 0.4).abs() < 1e-6);
        assert!((spp.fire_tolerance_frac() - 0.6).abs() < 1e-6);
    }

    #[test]
    fn post_fire_regen_variants() {
        for regen in [
            PostFireRegen::None,
            PostFireRegen::Resprout,
            PostFireRegen::Serotinous,
            PostFireRegen::InSeed,
        ] {
            let toml_str = format!(
                r#"
name = "test"
longevity = 100
sexual_maturity = 10
post_fire_regen = "{regen:?}"
effective_seed_distance = 10.0
max_seed_distance = 100.0
max_biomass = 5.0
"#
            );
            let spp = SpeciesConfig::from_toml_str(&toml_str).unwrap();
            assert_eq!(spp.post_fire_regen, regen);
        }
    }
}
