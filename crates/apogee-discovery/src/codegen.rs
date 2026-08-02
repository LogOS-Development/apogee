//! Code generation from LLM service specs to Rust DSL.

/// A generated service specification.
#[derive(Debug, Clone, Default)]
pub struct GeneratedService {
    pub name: String,
    pub source: String,
}

/// Code generator stub.
#[derive(Debug, Clone, Default)]
pub struct CodeGenerator;

impl CodeGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Generate Rust source from a natural-language service spec.
    pub fn generate(&self, _spec: &str) -> GeneratedService {
        // TODO: invoke LLM, parse spec, emit Rust DSL
        GeneratedService::default()
    }
}
