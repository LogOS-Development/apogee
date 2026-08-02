//! Milestone detection and discovery triggers.

/// A milestone event that may trigger a discovery.
#[derive(Debug, Clone, Default)]
pub struct MilestoneEvent {
    pub id: u64,
    pub description: String,
}

/// Evaluates milestones and decides whether to trigger discoveries.
#[derive(Debug, Clone, Default)]
pub struct MilestoneEvaluator;

impl MilestoneEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Evaluate a milestone and return any triggered discovery IDs.
    pub fn evaluate(&self, _milestone: &MilestoneEvent) -> Vec<u64> {
        // TODO: invoke LLM to evaluate whether milestone warrants a discovery
        Vec::new()
    }
}
