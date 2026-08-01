//! Military volunteer assignment system.

use apogee_common::ApogeeResult;
use apogee_social::actor::{Actor, PlayerId};
use apogee_social::role::{Assigner, Role, RoleAssignment, RoleAuthority, RoleDuration};

/// A request from a player to volunteer for military service.
#[derive(Debug, Clone, Default)]
pub struct VolunteerRequest {
    pub player_id: PlayerId,
    pub actor_id: u64,
    pub polity: u64,
}

/// Assigns volunteers to military units.
#[derive(Debug, Clone, Default)]
pub struct MilitaryVolunteerSystem;

impl MilitaryVolunteerSystem {
    pub fn new() -> Self {
        Self
    }

    /// Assign a player actor to an available military unit.
    pub fn assign(
        &self,
        actor: &mut Actor,
        _request: &VolunteerRequest,
    ) -> ApogeeResult<()> {
        // TODO: find unit, assign role, switch actor mind to Hybrid soldier,
        // notify squad leader.
        actor.current_roles.push(apogee_social::actor::RoleAssignmentStub);
        let _assignment = RoleAssignment {
            role: Role::Soldier,
            context: Default::default(),
            authority: RoleAuthority::None,
            assigned_by: Assigner::System,
            duration: RoleDuration::Tenure,
        };
        Ok(())
    }
}
