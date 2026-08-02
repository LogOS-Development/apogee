//! Discovery evaluation and service generation via LLM.

/// Evaluates whether a milestone should trigger a new simulation service.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryEvaluator;

impl DiscoveryEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Given a milestone description, decide if a discovery triggers.
    pub fn evaluate(&self, _milestone: &str) -> bool {
        // TODO: build prompt, query LLM, parse yes/no
        false
    }
}
