//! Combat: weapons, damage, projectiles.

/// Weapon damage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DamageType {
    #[default]
    Kinetic,
    Explosive,
    Energy,
    Emp,
}

/// A weapon specification.
#[derive(Debug, Clone, Default)]
pub struct WeaponSpec {
    pub damage_type: DamageType,
    pub base_damage: f64,
    pub muzzle_velocity: f64,
    pub fire_rate: f64,
    pub magazine_size: u32,
    pub effective_range: f64,
    pub recoil_impulse: f64,
}
