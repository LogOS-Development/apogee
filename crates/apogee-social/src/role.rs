//! Role system: every social function is a role that can be filled by human or AI.

use crate::actor::{ActorId, PlayerId};
use crate::group::{GroupContext, GroupId};

/// Authority level attached to a role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoleAuthority {
    #[default]
    None,
    TaskLevel,
    ResourceLevel,
    Strategic,
    Sovereign,
}

/// Duration / tenure of a role assignment.
#[derive(Debug, Clone, Default)]
pub enum RoleDuration {
    Task,
    Shift { hours: f64 },
    Tenure,
    #[default]
    Lifetime,
    Hereditary,
}

/// Who assigned the role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Assigner {
    #[default]
    System,
    Player(PlayerId),
    Actor(ActorId),
}

/// A role that can be filled by a human or an agent.
#[derive(Debug, Clone, Default)]
pub enum Role {
    #[default]
    Laborer,
    Craftsman,
    Merchant,
    Foreman,
    Citizen,
    CouncilMember,
    Magistrate,
    Diplomat,
    Ruler { polity: GroupId },
    Soldier,
    SquadLeader,
    Commander,
    Strategist,
    Researcher,
    Professor,
    Priest,
    Artist,
    Spy,
    Pilot,
    FlightController,
    StationCommander,
    Astronaut,
}

/// A concrete assignment of a role to an actor within a context.
#[derive(Debug, Clone, Default)]
pub struct RoleAssignment {
    pub role: Role,
    pub context: GroupContext,
    pub authority: RoleAuthority,
    pub assigned_by: Assigner,
    pub duration: RoleDuration,
}
