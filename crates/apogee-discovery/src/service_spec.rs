//! Service specification: typed inputs/outputs from LLM.

/// Specification for a dynamic service.
#[derive(Debug, Clone, Default)]
pub struct ServiceSpec {
    pub name: String,
    pub description: String,
    pub input_schema: String,
    pub output_schema: String,
}
