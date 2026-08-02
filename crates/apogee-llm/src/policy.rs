//! Natural language policy interpretation.

/// Result of interpreting a natural-language policy.
#[derive(Debug, Clone, Default)]
pub struct PolicyInterpretation {
    pub original_text: String,
    pub mechanical_effects: Vec<String>,
}

/// Policy interpreter.
#[derive(Debug, Clone, Default)]
pub struct PolicyInterpreter;

impl PolicyInterpreter {
    pub fn new() -> Self {
        Self
    }

    /// Convert natural language policy text into mechanical effects.
    pub fn interpret(&self, _policy_text: &str) -> PolicyInterpretation {
        // TODO: build prompt, query LLM, parse structured effects
        PolicyInterpretation::default()
    }
}
