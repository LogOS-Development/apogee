//! Prompt templates and builders.

/// Builds prompts for the LLM gateway.
#[derive(Debug, Clone, Default)]
pub struct PromptBuilder {
    pub system_prompt: String,
}

impl PromptBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the system prompt.
    pub fn system(mut self, text: impl Into<String>) -> Self {
        self.system_prompt = text.into();
        self
    }

    /// Build the final prompt string.
    pub fn build(&self, user_prompt: &str) -> String {
        format!("{}\n\n{}", self.system_prompt, user_prompt)
    }
}
