//! Member — an instance of a species positioned in space with mutable state.
//!
//! A member can represent a single plant (Individual) or a population
//! at one location (Density). Both share the same interface; the
//! representation determines how state is interpreted and aggregated.

use crate::species::SpeciesConfig;

/// Position in world space (meters, local Cartesian).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPos {
    pub x: f32,
    pub y: f32,
}

impl WorldPos {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Distance to another position.
    pub fn distance_to(&self, other: &Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// How a member represents the plants at its location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Representation {
    /// One individual plant.
    Individual,
    /// N plants of the same species at this location.
    Density { count: u32 },
}

/// An instance of a species in space, with mutable state.
///
/// The same logical population can be tracked as individuals or
/// as a density. Both share the same struct — the `representation`
/// field controls how `biomass` and `age` are interpreted:
/// - Individual: biomass is this plant's biomass, age is this plant's age.
/// - Density: biomass is per-plant mean, age is mean age, count is population.
#[derive(Clone, Debug)]
pub struct Member {
    /// Index into the species table.
    pub species_id: usize,
    /// Position in world space.
    pub position: WorldPos,
    /// Age in years.
    pub age: f32,
    /// Aboveground biomass per plant (kg/m^2).
    pub biomass: f32,
    /// Health fraction (0 = dead, 1 = fully healthy).
    pub health: f32,
    /// Whether this is an individual or a density.
    pub representation: Representation,
}

impl Member {
    /// Create a new individual plant member.
    pub fn individual(species_id: usize, position: WorldPos) -> Self {
        Self {
            species_id,
            position,
            age: 0.0,
            biomass: 0.1,
            health: 1.0,
            representation: Representation::Individual,
        }
    }

    /// Create a new density member representing `count` plants.
    pub fn density(species_id: usize, position: WorldPos, count: u32) -> Self {
        Self {
            species_id,
            position,
            age: 0.0,
            biomass: 0.1,
            health: 1.0,
            representation: Representation::Density { count },
        }
    }

    /// Advance age by `years`.
    pub fn age_by(&mut self, years: f32) {
        self.age += years;
    }

    /// Whether this member is old enough to produce seed.
    pub fn is_reproductive(&self, species: &SpeciesConfig) -> bool {
        self.age >= species.sexual_maturity as f32
    }

    /// Whether this member has died (age exceeded longevity or health reached zero).
    pub fn is_dead(&self, species: &SpeciesConfig) -> bool {
        self.age >= species.longevity as f32 || self.health <= 0.0
    }

    /// Number of individual plants this member represents.
    pub fn plant_count(&self) -> u32 {
        match self.representation {
            Representation::Individual => 1,
            Representation::Density { count } => count,
        }
    }

    /// Total biomass across all plants this member represents (kg/m^2).
    pub fn total_biomass(&self) -> f32 {
        self.biomass * self.plant_count() as f32
    }

    /// Biomass density contribution to the cell (kg/m^2).
    /// For an individual, this is just its biomass.
    /// For a density, this is per-plant biomass times count, scaled
    /// by the area the density occupies (assumed 1 m^2 for now).
    pub fn biomass_density(&self) -> f32 {
        self.total_biomass()
    }

    /// Demote an individual to a density of 1 (first step toward aggregation).
    pub fn to_density(&self) -> Self {
        Self {
            representation: Representation::Density { count: 1 },
            ..*self
        }
    }

    /// Promote a density to an individual (takes the first plant).
    /// Only valid when count == 1.
    pub fn to_individual(&self) -> Self {
        Self {
            representation: Representation::Individual,
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::SpeciesConfig;

    fn test_species() -> SpeciesConfig {
        SpeciesConfig::from_toml_str(
            r#"
name = "test_pine"
longevity = 200
sexual_maturity = 10
shade_tolerance = 2
fire_tolerance = 3
effective_seed_distance = 50.0
max_seed_distance = 500.0
max_biomass = 5.0
"#,
        )
        .unwrap()
    }

    #[test]
    fn individual_starts_at_age_zero() {
        let m = Member::individual(0, WorldPos::new(10.0, 20.0));
        assert_eq!(m.age, 0.0);
        assert!(m.biomass > 0.0);
        assert_eq!(m.plant_count(), 1);
        assert_eq!(m.representation, Representation::Individual);
    }

    #[test]
    fn density_tracks_count() {
        let m = Member::density(0, WorldPos::new(5.0, 5.0), 50);
        assert_eq!(m.plant_count(), 50);
        assert_eq!(m.representation, Representation::Density { count: 50 });
    }

    #[test]
    fn total_biomass_scales_with_count() {
        let mut m = Member::density(0, WorldPos::new(0.0, 0.0), 10);
        m.biomass = 3.0;
        assert!((m.total_biomass() - 30.0).abs() < 1e-6);
    }

    #[test]
    fn age_advances() {
        let mut m = Member::individual(0, WorldPos::new(0.0, 0.0));
        m.age_by(5.0);
        assert!((m.age - 5.0).abs() < 1e-6);
    }

    #[test]
    fn reproductive_after_maturity() {
        let spp = test_species();
        let mut m = Member::individual(0, WorldPos::new(0.0, 0.0));
        assert!(!m.is_reproductive(&spp));
        m.age = 10.0;
        assert!(m.is_reproductive(&spp));
    }

    #[test]
    fn dead_at_longevity() {
        let spp = test_species();
        let mut m = Member::individual(0, WorldPos::new(0.0, 0.0));
        m.age = 200.0;
        assert!(m.is_dead(&spp));
    }

    #[test]
    fn dead_when_health_zero() {
        let spp = test_species();
        let mut m = Member::individual(0, WorldPos::new(0.0, 0.0));
        m.health = 0.0;
        assert!(m.is_dead(&spp));
    }

    #[test]
    fn distance_between_positions() {
        let a = WorldPos::new(0.0, 0.0);
        let b = WorldPos::new(3.0, 4.0);
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn convert_individual_to_density() {
        let m = Member::individual(0, WorldPos::new(1.0, 2.0));
        let d = m.to_density();
        assert_eq!(d.representation, Representation::Density { count: 1 });
        assert_eq!(d.plant_count(), 1);
        assert!((d.total_biomass() - m.total_biomass()).abs() < 1e-6);
    }

    #[test]
    fn convert_density_to_individual() {
        let m = Member::density(0, WorldPos::new(1.0, 2.0), 1);
        let i = m.to_individual();
        assert_eq!(i.representation, Representation::Individual);
        assert_eq!(i.plant_count(), 1);
    }
}
