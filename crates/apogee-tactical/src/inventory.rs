//! Inventory: items, slots, equipment.

use std::collections::HashMap;

/// An item definition.
#[derive(Debug, Clone, Default)]
pub struct ItemDefinition {
    pub id: String,
    pub name: String,
    pub mass: f64,
    pub volume: f64,
}

/// An inventory with typed slots.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub slots: HashMap<SlotType, Vec<ItemDefinition>>,
    pub max_volume: f64,
}

/// Slot type for items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SlotType {
    #[default]
    Hand,
    Belt,
    Backpack,
    Pocket,
    SuitMount,
    WeaponSling,
    Helmet,
    Armor,
}
