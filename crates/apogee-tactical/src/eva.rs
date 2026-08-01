//! EVA / spacewalk systems.

/// EVA capability for an actor.
#[derive(Debug, Clone, Default)]
pub struct EVACapability {
    pub o2_supply: f64,
    pub suit_pressure: f64,
    pub suit_integrity: f64,
    pub thermal_reserve: f64,
    pub thruster_fuel: f64,
}
