//! Contextual organization system: groups and relations.

use std::collections::HashMap;

/// Unique identifier for a group / polity / organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GroupId(pub u64);

/// A node in the organization graph.
#[derive(Debug, Clone, Default)]
pub struct GroupNode {
    pub id: GroupId,
    pub name: String,
    pub members: Vec<u64>,
    pub relations: HashMap<GroupId, GroupRelation>,
}

/// Relationship classification between groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupRelation {
    #[default]
    Neutral,
    Ally,
    TradePartner,
    Rival,
    AtWar,
    Vassal,
    Suzerain,
}

/// Scope within which a group or role is active.
#[derive(Debug, Clone, Default)]
pub struct ContextScope;

/// Temporal context for a group or role.
#[derive(Debug, Clone, Default)]
pub struct TemporalContext;

/// Domain of a contextual group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextDomain {
    #[default]
    Political,
    Economic,
    Military,
    Religious,
    Social,
}

/// Context that defines when and where a group exists.
#[derive(Debug, Clone, Default)]
pub struct GroupContext {
    pub domain: ContextDomain,
    pub scope: ContextScope,
    pub temporal: TemporalContext,
}
