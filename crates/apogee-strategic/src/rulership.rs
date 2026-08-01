//! Rulership claim paths and installation.

use apogee_common::ApogeeResult;
use apogee_social::actor::{Actor, PlayerId};
use apogee_social::group::GroupId;

/// Ways a player can become ruler of a polity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClaimMethod {
    #[default]
    Election,
    Succession,
    Conquest,
    Revolution,
    CorporateTakeover,
    Founding,
}

/// Installs a player as ruler of a polity.
#[derive(Debug, Clone, Default)]
pub struct RulershipInstaller;

impl RulershipInstaller {
    pub fn new() -> Self {
        Self
    }

    /// Attempt to install the given actor as ruler using the specified claim
    /// method.
    pub fn install(
        &self,
        _polity: GroupId,
        _actor: &mut Actor,
        _player: PlayerId,
        _method: ClaimMethod,
    ) -> ApogeeResult<()> {
        // TODO: validate claim method, switch actor to Hybrid ruler mind,
        // broadcast to polity, schedule loyalty reassessment.
        Ok(())
    }
}
