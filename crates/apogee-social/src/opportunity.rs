//! Player opportunity board and role-filling logic.

use apogee_common::ApogeeResult;

/// An opportunity posted for a player to take a role.
#[derive(Debug, Clone, Default)]
pub struct PlayerOpportunity {
    pub id: u64,
    pub description: String,
    pub posted_by: u64,
}

/// Fills vacant roles with appropriate actors.
#[derive(Debug, Clone, Default)]
pub struct RoleFiller;

impl RoleFiller {
    pub fn new() -> Self {
        Self
    }

    /// Fill all currently vacant roles.
    pub fn fill_all(&self) -> ApogeeResult<Vec<PlayerOpportunity>> {
        // TODO: inspect vacant roles, spawn LLM/scripted agents, post player
        // opportunities for low-priority roles.
        Ok(Vec::new())
    }
}
