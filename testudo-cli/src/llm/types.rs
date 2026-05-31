// @anchor infra:cli:llm:types
// @tags api

//! Shared types for LLM provider abstraction.

use serde::{Deserialize, Serialize};

/// A message in a conversation with an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A tool call the LLM wants to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Response from an LLM provider.
#[derive(Debug, Clone, Serialize)]
pub struct LlmResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<LlmToolCall>,
    pub finish_reason: String,
}

/// Result of executing a tool call.
#[derive(Debug, Clone)]
pub struct LlmToolResult {
    pub call_id: String,
    pub name: String,
    pub content: String,
}

/// Token usage from an LLM response.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LlmUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Errors from LLM provider operations.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Failed to deserialize response: {0}")]
    Deserialize(String),

    #[error("Provider error ({0}): {1}")]
    ProviderError(u16, String),

    #[error("Rate limited — retry after {0}s")]
    RateLimited(u64),

    #[error("Network error: {0}")]
    Network(String),
}

impl From<reqwest::Error> for LlmError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            LlmError::Network("Request timed out".into())
        } else if e.is_connect() {
            LlmError::Network(format!("Connection refused: {}", e))
        } else {
            LlmError::Network(e.to_string())
        }
    }
}
