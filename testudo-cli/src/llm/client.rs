// @anchor infra:cli:llm:client
// @tags api

//! LLM provider abstraction trait and factory.

use crate::config::LlmConfig;
use crate::llm::anthropic::AnthropicClient;
use crate::llm::openai::OpenAiClient;
use crate::llm::types::{LlmError, LlmMessage, LlmResponse};
use async_trait::async_trait;

/// Common interface for all LLM providers.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send messages with optional tool definitions, get a complete response.
    async fn send_message(
        &self,
        messages: &[LlmMessage],
        tools: &[serde_json::Value],
    ) -> Result<LlmResponse, LlmError>;
}

/// Providers that use the OpenAI-compatible client.
const OPENAI_COMPATIBLE: &[&str] = &[
    "openai", "deepseek", "groq", "together", "xai",
    "mistral", "openrouter", "qwen", "ollama",
];

/// Create an LLM client for the configured provider.
pub fn create_client(config: &LlmConfig) -> Box<dyn LlmClient> {
    match config.provider.as_str() {
        "anthropic" => Box::new(AnthropicClient::new(config)),
        p if OPENAI_COMPATIBLE.contains(&p) => Box::new(OpenAiClient::new(config)),
        other => panic!(
            "Unknown LLM provider: {}. Supported: anthropic, {}",
            other,
            OPENAI_COMPATIBLE.join(", ")
        ),
    }
}
