//! LLM gateway abstraction: Ollama local / cloud fallback.

use apogee_common::ApogeeResult;

/// Gateway configuration.
#[derive(Debug, Clone, Default)]
pub struct GatewayConfig {
    pub ollama_url: String,
    pub fallback_enabled: bool,
}

/// LLM gateway stub.
pub struct LlmGateway {
    pub config: GatewayConfig,
}

impl LlmGateway {
    pub fn new(config: GatewayConfig) -> Self {
        Self { config }
    }

    /// Send a prompt and return the raw response.
    pub fn query(&self,
        _prompt: &str,
    ) -> ApogeeResult<String> {
        // TODO: implement Ollama HTTP client + cloud fallback
        Ok(String::new())
    }
}
