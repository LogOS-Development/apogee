//! Interaction: doors, switches, crafting stations.

use apogee_common::ApogeeResult;

/// An interactive entity in the world.
#[derive(Debug, Clone, Default)]
pub struct Interactable {
    pub id: u64,
    pub label: String,
}

/// Interaction system.
#[derive(Debug, Clone, Default)]
pub struct InteractionSystem;

impl InteractionSystem {
    pub fn new() -> Self {
        Self
    }

    /// Trigger an interaction.
    pub fn interact(&self,
        _actor_id: u64,
        _target: &Interactable,
    ) -> ApogeeResult<()> {
        // TODO: dispatch to door, switch, crafting station, etc.
        Ok(())
    }
}
