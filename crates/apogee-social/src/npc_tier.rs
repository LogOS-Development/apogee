//! NPC tier system and population aggregates.

use std::collections::HashMap;

/// Cognition / persistence tier for an NPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NpcTier {
    /// Full LLM, named, persistent.
    Named,
    /// Hybrid LLM/scripted, semi-persistent.
    SemiNamed,
    /// Scripted aggregate, crystallized only when needed.
    #[default]
    Background,
}

/// Aggregate population for swarm simulation of background NPCs.
#[derive(Debug, Clone, Default)]
pub struct PopulationAggregate {
    pub total: u64,
    pub available_workers: u64,
    pub available_soldiers: u64,
    pub morale: f64,
    pub skill_distribution: HashMap<String, f64>,
    pub crystallized: u32,
}

impl PopulationAggregate {
    /// Promote `count` individuals from the aggregate into individual NPC
    /// entities.
    pub fn crystallize(&mut self, count: u32) -> Vec<u64> {
        let n = count.min(self.total as u32);
        self.total -= n as u64;
        self.available_workers -= n as u64;
        self.available_soldiers -= n as u64;
        self.crystallized += n;
        // TODO: spawn individual entities and return their IDs
        (0..n).map(|i| i as u64).collect()
    }

    /// Return individual NPCs to the aggregate.
    pub fn absorb(&mut self, npcs: Vec<u64>) {
        let count = npcs.len() as u64;
        self.total += count;
        self.crystallized -= count as u32;
        // TODO: despawn individual entities
    }
}
