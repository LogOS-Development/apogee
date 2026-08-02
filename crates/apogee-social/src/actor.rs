use apogee_common::ApogeeResult;

/// Unique identifier for an actor (player or NPC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ActorId(pub u64);

/// Unique identifier for a player account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PlayerId(pub u64);

/// An Actor is the unified abstraction for every entity that can act in the
/// world. The body and role systems are independent of whether the mind is
/// human, LLM, scripted, or hybrid.
#[derive(Debug, Clone, Default)]
pub struct Actor {
    pub id: ActorId,
    pub mind: MindType,
    pub current_roles: Vec<RoleAssignmentStub>,
    pub capabilities: ActorCapabilities,
    pub identity: ActorIdentity,
}

/// Cognitive control mode for an Actor.
#[derive(Debug, Clone, Default)]
pub enum MindType {
    #[default]
    Scripted,
    Human {
        player_id: PlayerId,
    },
    Llm {
        agent_id: u64,
    },
    Hybrid {
        player_override: Option<PlayerId>,
        agent_id: u64,
    },
}

/// Stub for role assignment; full definition lives in `role.rs`.
#[derive(Debug, Clone, Default)]
pub struct RoleAssignmentStub;

/// What an actor is capable of doing.
#[derive(Debug, Clone, Default)]
pub struct ActorCapabilities {
    pub can_fly_spacecraft: bool,
    pub can_lead_units: bool,
    pub can_govern: bool,
    pub can_research: bool,
}

/// Persistent identity (name, background, etc.).
#[derive(Debug, Clone, Default)]
pub struct ActorIdentity {
    pub display_name: String,
}

/// System that transfers control between mind types.
#[derive(Debug, Clone, Default)]
pub struct ActorControlSystem;

impl ActorControlSystem {
    pub fn new() -> Self {
        Self
    }

    /// Suspend the current mind and hand control to a player.
    pub fn player_takeover(&self, _actor: &mut Actor, _player: PlayerId) -> ApogeeResult<()> {
        // TODO: implement mind suspension + handoff
        Ok(())
    }

    /// Release player control and resume LLM/scripted mind.
    pub fn player_disconnect(&self, _actor: &mut Actor, _player: PlayerId) -> ApogeeResult<()> {
        // TODO: implement context summarization + resume
        Ok(())
    }
}
