# Unified Actor Model

> **Scope:** Human players and LLM/NPC agents share the same simulation
> architecture. The only difference is who provides intent.

## Goal

Make Apogee playable as a persistent civilization MMO where the world remains
alive when players log off. Every role — laborer, soldier, ruler, scientist,
pilot — can be filled by either a human or a cognitive agent. Control must
transfer between them without simulation discontinuity.

## Core Insight

There is no separate "NPC system" and "player system". There is only an
**Actor**.

```rust
pub struct Actor {
    pub id: ActorId,
    pub mind: MindType,
    pub current_roles: Vec<RoleAssignment>,
    pub capabilities: ActorCapabilities,
    pub identity: ActorIdentity,
}
```

## Mind Types

| Mind | Description | Use Case |
|------|-------------|----------|
| `Human` | Player provides intent via input devices. | Human is actively controlling the actor. |
| `Llm` | Full LLM agent makes decisions autonomously. | Named leaders, ministers, generals. |
| `Scripted` | Behavior tree / deterministic AI. | Background laborers, common soldiers, crowds. |
| `Hybrid` | LLM sets goals; scripted AI executes moment-to-moment; human can override. | Squad members, foremen, vehicle crew. |

## Seamless Handoff

When a player takes over an NPC, the previous mind is suspended with a context
summary. When the player disconnects, the LLM resumes with a summary of what
the player did. The simulation never waits for human input.

## Role System

Every social function is a `Role` with `RoleAuthority` and `RoleDuration`:

- Economic: Laborer, Craftsman, Merchant, Foreman
- Governance: Citizen, CouncilMember, Magistrate, Diplomat, Ruler
- Military: Soldier, SquadLeader, Commander, Strategist
- Scientific: Researcher, Professor
- Social: Priest, Artist, Spy
- Space: Pilot, FlightController, StationCommander, Astronaut

## Polity Autonomy

A polity runs itself through an LLM ruler that periodically evaluates state and
issues directives:

- Propose projects
- Reallocate resources
- Appoint officials
- Enact policies (natural language → mechanical effects)
- Declare war / make peace

## Military Apparatus

The military is a self-organizing system. LLM commanders issue orders; players
can volunteer for units and receive orders from LLM squad leaders. Players can
be promoted or demoted based on performance.

## Rulership Claim Paths

A player can become ruler through:

- Election
- Succession
- Conquest
- Revolution
- Corporate takeover
- Founding a new polity

## NPC Tiers

To keep simulation cost sane, NPCs are stratified:

| Tier | Count / Shard | Cognition |
|------|---------------|-----------|
| Named | 50–100 | Full LLM |
| SemiNamed | 500–1,000 | Hybrid LLM/scripted |
| Background | 10,000+ | Aggregate population; crystallize into individual entities only when needed |

## Player Opportunity Board

Vacant roles are surfaced as opportunities players can accept. Critical and
complex roles are filled by LLM agents; simple roles by scripted NPCs; low
priority roles become available quests.

## Files

| File | Responsibility |
|------|----------------|
| `crates/apogee-social/src/actor.rs` | `Actor`, `MindType`, handoff system |
| `crates/apogee-social/src/role.rs` | `Role`, `RoleAssignment`, `RoleAuthority`, `RoleDuration` |
| `crates/apogee-social/src/group.rs` | `GroupNode`, `GroupContext`, `GroupRelation` |
| `crates/apogee-social/src/polity.rs` | `PolityDirective`, `PolityGoal`, `PolityProject` |
| `crates/apogee-social/src/agent.rs` | `LlmAgent`, prompts, cognition runtime |
| `crates/apogee-strategic/src/military.rs` | `MilitaryApparatus`, `CommandStructure`, `Deployment`, `MilitaryDoctrine` |
| `crates/apogee-strategic/src/volunteer.rs` | Military volunteer assignment system |
| `crates/apogee-strategic/src/rulership.rs` | Claim paths and `install_ruler` |
| `crates/apogee-strategic/src/interface.rs` | Ruler command interface types |
| `crates/apogee-social/src/opportunity.rs` | `PlayerOpportunity`, `fill_vacant_roles` |
| `crates/apogee-social/src/npc_tier.rs` | `NpcTier`, `PopulationAggregate`, crystallize/absorb |

## Dependencies

- `apogee-core` for physics/ECS primitives
- `apogee-llm` for LLM gateway
- `apogee-common` for shared IDs and errors

## Exit Criterion

A single Actor entity can be spawned, assigned a Role, controlled by an LLM,
taken over by a player, and returned to LLM control with preserved context.
