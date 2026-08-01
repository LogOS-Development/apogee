//! Polity autonomy: LLM ruler directives, goals, and projects.

use std::collections::HashMap;

use crate::group::GroupId;

/// A strategic directive for a polity, produced by the LLM ruler.
#[derive(Debug, Clone, Default)]
pub struct PolityDirective {
    pub polity: GroupId,
    pub current_goals: Vec<PolityGoal>,
    pub active_projects: Vec<PolityProject>,
    pub resource_allocation: ResourceBudget,
    pub domestic_priorities: Vec<String>,
}

/// A high-level goal for a polity.
#[derive(Debug, Clone, Default)]
pub struct PolityGoal {
    pub id: u64,
    pub description: String,
    pub priority: f64,
    pub status: GoalStatus,
}

/// Status of a polity goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoalStatus {
    #[default]
    Proposed,
    Approved,
    InProgress,
    Blocked,
    Completed,
    Abandoned,
}

/// A concrete project under a polity directive.
#[derive(Debug, Clone, Default)]
pub struct PolityProject {
    pub id: u64,
    pub goal_id: u64,
    pub project_type: ProjectType,
    pub description: String,
    pub workforce_assigned: u32,
}

/// Classification of a polity project.
#[derive(Debug, Clone, Default)]
pub enum ProjectType {
    #[default]
    Infrastructure,
    MilitaryCampaign,
    Research,
    Colonization,
    TradeExpedition,
    DiplomaticMission,
    PublicWorks,
    SpaceProgram,
}

/// Resource budget for allocation decisions.
#[derive(Debug, Clone, Default)]
pub struct ResourceBudget {
    pub treasury: f64,
    pub currency: String,
    pub stocks: HashMap<String, f64>,
}

/// Async decision cycle for an LLM ruler.
#[derive(Debug, Clone, Default)]
pub struct PolityDecisionCycle;

impl PolityDecisionCycle {
    pub fn new() -> Self {
        Self
    }

    /// Run one decision cycle.
    pub fn run(&self,
        _directive: &mut PolityDirective,
    ) {
        // TODO: build prompt, query LLM, parse decisions, update directive
    }
}
