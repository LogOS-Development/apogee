//! Autonomous military apparatus.

use apogee_social::group::{ContextScope, GroupId};

/// A self-organizing military under a polity.
#[derive(Debug, Clone, Default)]
pub struct MilitaryApparatus {
    pub polity: GroupId,
    pub command_structure: CommandStructure,
    pub standing_forces: Vec<u64>,
    pub reserve_forces: Vec<u64>,
    pub deployments: Vec<Deployment>,
    pub supply_lines: Vec<SupplyLine>,
    pub doctrine: MilitaryDoctrine,
}

/// Command hierarchy for a military apparatus.
#[derive(Debug, Clone, Default)]
pub struct CommandStructure {
    pub commander_in_chief: u64,
    pub general_staff: Vec<u64>,
    pub field_commanders: Vec<u64>,
    pub nco_corps: Vec<u64>,
}

/// A deployed military force with an objective.
#[derive(Debug, Clone, Default)]
pub struct Deployment {
    pub id: u64,
    pub forces: Vec<u64>,
    pub objective: DeploymentObjective,
    pub location: ContextScope,
    pub supply_status: SupplyStatus,
}

/// Objective assigned to a deployment.
#[derive(Debug, Clone, Default)]
pub enum DeploymentObjective {
    #[default]
    Defend,
    Attack,
    Reconnoiter,
    Escort,
    Siege,
}

/// Status of supplies for a deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SupplyStatus {
    #[default]
    Green,
    Amber,
    Red,
}

/// A logistical supply line.
#[derive(Debug, Clone, Default)]
pub struct SupplyLine {
    pub id: u64,
    pub from: u64,
    pub to: u64,
    pub capacity: f64,
}

/// Strategic orientation of a military doctrine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StrategicOrientation {
    #[default]
    Defensive,
    Offensive,
    Deterrent,
    Guerrilla,
    NavalPower,
    SpaceSuperiority,
}

/// Composition preference of a military doctrine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ForceComposition {
    #[default]
    Balanced,
    HeavyInfantry,
    Mobile,
    Ranged,
    Naval,
    Space,
}

/// Doctrine that shapes autonomous military decisions.
#[derive(Debug, Clone, Default)]
pub struct MilitaryDoctrine {
    pub orientation: StrategicOrientation,
    pub composition: ForceComposition,
    pub escalation_threshold: f64,
}

/// Military decision loop driven by an LLM commander.
#[derive(Debug, Clone, Default)]
pub struct MilitaryDecisionCycle;

impl MilitaryDecisionCycle {
    pub fn new() -> Self {
        Self
    }

    /// Run one military decision cycle.
    pub fn run(&self,
        _apparatus: &mut MilitaryApparatus,
    ) {
        // TODO: build prompt, query LLM, parse orders, dispatch units
    }
}
