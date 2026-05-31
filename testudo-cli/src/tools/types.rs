// @anchor infra:cli:tools:types
// @tags api

//! Tool definition and result types for LLM function calling.

/// Definition of a tool exposed to the LLM.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool's parameters.
    pub parameters: serde_json::Value,
}

/// Result of executing a tool.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub content: String,
}

/// Errors from tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Missing required argument: {0}")]
    MissingArg(String),

    #[error("Tool execution failed: {0}")]
    Execution(String),

    #[error("API error: {0}")]
    Api(String),
}

impl From<crate::api::types::ApiError> for ToolError {
    fn from(e: crate::api::types::ApiError) -> Self {
        ToolError::Api(e.to_string())
    }
}
