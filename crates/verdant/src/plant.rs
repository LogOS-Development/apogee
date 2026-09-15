//! GenericPlant trait — behavior shared by all plant species.
//!
//! Implementation is config-driven: the same code runs for every species,
//! with behavioral differences emerging from SpeciesConfig parameters.
//! There are no per-species Rust structs — a ponderosa pine and a lodgepole
//! pine are both PlantMember instances with different SpeciesConfig data.

use crate::member::Member;
use crate::species::{PostFireRegen, SpeciesConfig};

/// A seed dispersed by a reproductive plant.
#[derive(Clone, Debug)]
pub struct Seed {
    pub species_id: usize,
    pub origin: crate::member::WorldPos,
    pub distance: f32,
    pub bearing: f32, // radians, 0 = east
}

/// Environmental conditions at a location, used to drive plant behavior.
#[derive(Clone, Copy, Debug)]
pub struct Environment {
    /// Available light fraction (1.0 = full sun, 0.0 = full shade).
    pub available_light: f32,
    /// Soil moisture fraction (0 = dry, 1 = field capacity).
    pub soil_moisture: f32,
    /// Temperature in degrees Celsius.
    pub temperature: f32,
}

impl Environment {
    pub fn ideal() -> Self {
        Self {
            available_light: 1.0,
            soil_moisture: 0.5,
            temperature: 20.0,
        }
    }
}

/// Fuel contribution from a plant, for coupling to the fire model.
#[derive(Clone, Copy, Debug)]
pub struct FuelContribution {
    /// Anderson fuel category (1-13).
    pub category: u8,
    /// Fuel moisture content fraction.
    pub moisture: f32,
    /// Biomass available as fuel (kg/m^2).
    pub load: f32,
}

/// Trait defining plant behavior. All species share one implementation;
/// the SpeciesConfig drives the differences.
pub trait GenericPlant {
    /// Grow biomass for one timestep, given local environment.
    fn grow(&mut self, species: &SpeciesConfig, env: &Environment, dt: f32);

    /// Produce seeds if reproductive.
    fn reproduce(&self, species: &SpeciesConfig) -> Vec<Seed>;

    /// Whether this plant has died.
    fn is_dead(&self, species: &SpeciesConfig) -> bool;

    /// Apply fire effects. Severity is 0-1 (0 = no fire, 1 = crown fire).
    fn respond_to_fire(&mut self, species: &SpeciesConfig, severity: f32);

    /// Fuel model contribution for fire coupling.
    fn fuel_contribution(&self, species: &SpeciesConfig, moisture: f32) -> FuelContribution;

    /// Shade contribution (0 = no shade, 1 = full shade).
    fn shade_contribution(&self) -> f32;
}

impl GenericPlant for Member {
    fn grow(&mut self, species: &SpeciesConfig, env: &Environment, dt: f32) {
        if self.is_dead(species) {
            return;
        }

        // Relative age as fraction of longevity.
        let age_frac = self.age / species.longevity as f32;

        // Logistic growth: rises according to growth_curve parameter.
        // At age_frac = growth_curve, growth rate is half maximum.
        let growth_factor = 1.0 / (1.0 + (-10.0 * (age_frac - species.growth_curve)).exp());

        // Light limitation: shade-intolerant species suffer more in low light.
        let light_limit = env
            .available_light
            .max(1.0 - species.shade_tolerance_frac() * (1.0 - env.available_light));

        // Moisture limitation: growth reduced when soil is dry.
        let moisture_limit = env.soil_moisture.clamp(0.0, 1.0);

        // Temperature stress: optimal around 15-25C, reduced outside.
        let temp_opt = 1.0 - ((env.temperature - 20.0) / 30.0).abs().min(1.0);

        // Biomass increment toward maximum.
        let remaining = (species.max_biomass - self.biomass).max(0.0);
        let growth_rate = growth_factor * light_limit * moisture_limit * temp_opt;
        self.biomass += remaining * growth_rate * dt * 0.1; // tuned scaling
        self.biomass = self.biomass.min(species.max_biomass);

        // Health declines with age (senescence) and environmental stress.
        let mortality_factor = 1.0 / (1.0 + (-10.0 * (age_frac - species.mortality_curve)).exp());
        let stress = (1.0 - moisture_limit) * 0.5 + (1.0 - temp_opt) * 0.3;
        self.health -= mortality_factor * stress * dt * 0.01;
        self.health = self.health.max(0.0);
    }

    fn reproduce(&self, species: &SpeciesConfig) -> Vec<Seed> {
        if !self.is_reproductive(species) {
            return Vec::new();
        }

        // Number of seeds scales with biomass and health.
        let seed_count = (self.biomass * self.health * 10.0) as usize;
        let mut seeds = Vec::with_capacity(seed_count);

        for i in 0..seed_count {
            // Distance: negative exponential — most seeds land close.
            let r = -((i as f32 + 1.0) / seed_count as f32).ln() * species.effective_seed_distance;
            let distance = r.min(species.max_seed_distance);
            let bearing = (i as f32 * 2.39996) % std::f32::consts::TAU; // golden angle
            seeds.push(Seed {
                species_id: self.species_id,
                origin: self.position,
                distance,
                bearing,
            });
        }
        seeds
    }

    fn is_dead(&self, species: &SpeciesConfig) -> bool {
        self.is_dead(species)
    }

    fn respond_to_fire(&mut self, species: &SpeciesConfig, severity: f32) {
        // Fire tolerance determines survival.
        // severity is 0-1; fire_tolerance_frac is 0-1.
        // If severity > fire_tolerance, the plant dies.
        let survives = severity < species.fire_tolerance_frac();

        if !survives {
            match species.post_fire_regen {
                PostFireRegen::None => {
                    self.health = 0.0;
                    self.biomass = 0.0;
                }
                PostFireRegen::Resprout => {
                    // Survives at reduced biomass, regrows from roots.
                    self.biomass *= 0.1;
                    self.health *= 0.5;
                }
                PostFireRegen::Serotinous | PostFireRegen::InSeed => {
                    // Killed but will regenerate from seed next cycle.
                    self.health = 0.0;
                    self.biomass = 0.0;
                }
            }
        } else {
            // Survives fire with some damage.
            let damage = severity * (1.0 - species.fire_tolerance_frac());
            self.health -= damage * 0.5;
            self.biomass *= 1.0 - damage * 0.3;
            self.health = self.health.max(0.0);
        }
    }

    fn fuel_contribution(&self, _species: &SpeciesConfig, moisture: f32) -> FuelContribution {
        // Derive Anderson fuel category from biomass.
        // Low biomass -> grass/shrub (1-6), high biomass -> timber (8-10).
        let category = if self.biomass < 0.5 {
            1 // short grass
        } else if self.biomass < 2.0 {
            3 // tall grass
        } else if self.biomass < 4.0 {
            5 // brush
        } else if self.biomass < 6.0 {
            8 // closed timber litter
        } else {
            10 // timber (litter + understory)
        };

        FuelContribution {
            category,
            moisture,
            load: self.total_biomass(),
        }
    }

    fn shade_contribution(&self) -> f32 {
        // Shade scales with canopy biomass.
        // 10 kg/m^2 => full shade (tunable).
        (self.biomass_density() / 10.0).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::member::WorldPos;

    fn test_species() -> SpeciesConfig {
        SpeciesConfig::from_toml_str(
            r#"
name = "test_pine"
longevity = 200
sexual_maturity = 10
shade_tolerance = 2
fire_tolerance = 3
post_fire_regen = "InSeed"
effective_seed_distance = 50.0
max_seed_distance = 500.0
max_biomass = 5.0
"#,
        )
        .unwrap()
    }

    fn shade_tolerant_species() -> SpeciesConfig {
        SpeciesConfig::from_toml_str(
            r#"
name = "shade_tree"
longevity = 300
sexual_maturity = 20
shade_tolerance = 5
fire_tolerance = 1
effective_seed_distance = 30.0
max_seed_distance = 200.0
max_biomass = 10.0
"#,
        )
        .unwrap()
    }

    #[test]
    fn biomass_increases_with_growth() {
        let spp = test_species();
        let mut plant = Member::individual(0, WorldPos::new(0.0, 0.0));
        let env = Environment::ideal();
        let initial = plant.biomass;
        plant.grow(&spp, &env, 1.0);
        assert!(plant.biomass > initial, "biomass should increase");
    }

    #[test]
    fn growth_limited_by_shade() {
        let spp = test_species(); // shade tolerance 2 (intolerant)
        let mut plant = Member::individual(0, WorldPos::new(0.0, 0.0));
        let dark_env = Environment {
            available_light: 0.1,
            soil_moisture: 0.5,
            temperature: 20.0,
        };
        let bright_env = Environment::ideal();
        let mut shaded = plant.clone();
        shaded.grow(&spp, &dark_env, 1.0);
        plant.grow(&spp, &bright_env, 1.0);
        assert!(
            plant.biomass > shaded.biomass,
            "plant in bright light should grow more than in shade"
        );
    }

    #[test]
    fn shade_tolerant_species_grows_in_dark() {
        let spp = shade_tolerant_species(); // shade tolerance 5
        let mut plant = Member::individual(0, WorldPos::new(0.0, 0.0));
        let dark_env = Environment {
            available_light: 0.1,
            soil_moisture: 0.5,
            temperature: 20.0,
        };
        let initial = plant.biomass;
        plant.grow(&spp, &dark_env, 1.0);
        assert!(
            plant.biomass > initial,
            "shade-tolerant species should still grow in low light"
        );
    }

    #[test]
    fn no_reproduction_before_maturity() {
        let spp = test_species();
        let plant = Member::individual(0, WorldPos::new(0.0, 0.0));
        assert!(plant.reproduce(&spp).is_empty());
    }

    #[test]
    fn reproduction_after_maturity() {
        let spp = test_species();
        let mut plant = Member::individual(0, WorldPos::new(0.0, 0.0));
        plant.age = 15.0;
        plant.biomass = 3.0;
        let seeds = plant.reproduce(&spp);
        assert!(!seeds.is_empty(), "mature plant should produce seeds");
        for seed in &seeds {
            assert!(seed.distance <= spp.max_seed_distance);
        }
    }

    #[test]
    fn fire_kills_intolerant_at_high_severity() {
        let spp = test_species(); // fire tolerance 3 (0.6 frac)
        let mut plant = Member::individual(0, WorldPos::new(0.0, 0.0));
        plant.respond_to_fire(&spp, 0.8); // severity > tolerance
        assert!(
            plant.health <= 0.0,
            "high-severity fire should kill fire-intolerant plant"
        );
    }

    #[test]
    fn fire_tolerant_survives_low_severity() {
        let spp = test_species(); // fire tolerance 3 (0.6 frac)
        let mut plant = Member::individual(0, WorldPos::new(0.0, 0.0));
        plant.health = 1.0;
        plant.biomass = 3.0;
        plant.respond_to_fire(&spp, 0.3); // severity < tolerance
        assert!(
            plant.health > 0.0,
            "low-severity fire should not kill fire-tolerant plant"
        );
        assert!(plant.biomass > 0.0);
    }

    #[test]
    fn resprout_survives_fire() {
        let toml_str = r#"
name = "oak"
longevity = 300
sexual_maturity = 20
shade_tolerance = 4
fire_tolerance = 2
post_fire_regen = "Resprout"
effective_seed_distance = 30.0
max_seed_distance = 200.0
max_biomass = 10.0
"#;
        let spp = SpeciesConfig::from_toml_str(toml_str).unwrap();
        let mut plant = Member::individual(0, WorldPos::new(0.0, 0.0));
        plant.biomass = 5.0;
        plant.health = 1.0;
        plant.respond_to_fire(&spp, 0.9); // high severity, but resprout
        assert!(plant.health > 0.0, "resprout species should survive fire");
        assert!(
            plant.biomass > 0.0,
            "resprout species should retain some biomass"
        );
    }

    #[test]
    fn fuel_category_increases_with_biomass() {
        let spp = test_species();
        let mut low = Member::individual(0, WorldPos::new(0.0, 0.0));
        low.biomass = 0.3;
        let mut high = Member::individual(0, WorldPos::new(0.0, 0.0));
        high.biomass = 7.0;
        let low_fuel = low.fuel_contribution(&spp, 0.1);
        let high_fuel = high.fuel_contribution(&spp, 0.1);
        assert!(high_fuel.category > low_fuel.category);
        assert!(high_fuel.load > low_fuel.load);
    }

    #[test]
    fn shade_scales_with_biomass() {
        let mut small = Member::individual(0, WorldPos::new(0.0, 0.0));
        small.biomass = 1.0;
        let mut large = Member::individual(0, WorldPos::new(0.0, 0.0));
        large.biomass = 8.0;
        assert!(large.shade_contribution() > small.shade_contribution());
    }

    #[test]
    fn density_member_shade_scales_with_count() {
        let mut single = Member::density(0, WorldPos::new(0.0, 0.0), 1);
        single.biomass = 3.0;
        let mut dense = Member::density(0, WorldPos::new(0.0, 0.0), 10);
        dense.biomass = 3.0;
        assert!(dense.shade_contribution() > single.shade_contribution());
    }
}
