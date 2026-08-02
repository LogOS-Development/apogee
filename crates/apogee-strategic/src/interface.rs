//! Ruler command interface types (strategic map view).

use std::collections::HashMap;

/// Ruler-facing command interface state.
#[derive(Debug, Clone, Default)]
pub struct RulerCommandInterface {
    pub map_mode: MapMode,
    pub treasury: f64,
    pub military_summary: MilitarySummary,
    pub active_projects: Vec<ActiveProjectSummary>,
    pub personnel: HashMap<String, PersonnelSlot>,
}

/// Map visualization mode for the strategic view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapMode {
    #[default]
    Political,
    Terrain,
    Trade,
    Resource,
    Population,
    Military,
}

/// Summary of military forces for the ruler UI.
#[derive(Debug, Clone, Default)]
pub struct MilitarySummary {
    pub armies: u32,
    pub fleets: u32,
    pub pending_offers: u32,
    pub pending_ultimatums: u32,
}

/// Summary of an active project in the ruler UI.
#[derive(Debug, Clone, Default)]
pub struct ActiveProjectSummary {
    pub name: String,
    pub progress_percent: f64,
}

/// A personnel slot that can be filled by a player or LLM agent.
#[derive(Debug, Clone, Default)]
pub struct PersonnelSlot {
    pub title: String,
    pub occupant: Option<String>,
}
