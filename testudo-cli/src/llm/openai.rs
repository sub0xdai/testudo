// @anchor infra:cli:llm:openai
// @tags api

//! OpenAI-compatible Chat Completions API client.
//!
//! Covers: OpenAI, DeepSeek, Groq, Together, xAI, Mistral, OpenRouter,
//! Qwen, Ollama, and any custom endpoint that speaks the OpenAI protocol.

use crate::config::LlmConfig;
use crate::llm::client::LlmClient;
use crate::llm::types::{LlmError, LlmMessage, LlmResponse, LlmToolCall};
use async_trait::async_trait;
use std::collections::HashMap;

// ── Provider defaults ──────────────────────────────────────────

/// Returns (base_url, default_model) for a known OpenAI-compatible provider.
/// base_url always ends with /v1 so chat completions path is simply appended.
pub fn provider_defaults(provider: &str) -> Option<(String, String)> {
    let map: HashMap<&str, (&str, &str)> = HashMap::from([
        ("openai",     ("https://api.openai.com/v1",               "gpt-4o")),
        ("deepseek",   ("https://api.deepseek.com/v1",             "deepseek-chat")),
        ("groq",       ("https://api.groq.com/openai/v1",          "llama-3.3-70b-versatile")),
        ("together",   ("https://api.together.xyz/v1",             "meta-llama/Llama-3.3-70B-Instruct-Turbo")),
        ("xai",        ("https://api.x.ai/v1",                     "grok-2")),
        ("mistral",    ("https://api.mistral.ai/v1",               "mistral-large-latest")),
        ("openrouter", ("https://openrouter.ai/api/v1",            "anthropic/claude-sonnet-4")),
        ("qwen",       ("https://dashscope.aliyuncs.com/compatible-mode/v1", "qwen-max")),
        ("ollama",     ("http://localhost:11434/v1",               "llama3")),
    ]);
    map.get(provider).map(|(url, model)| (url.to_string(), model.to_string()))
}

// ── Message translation ────────────────────────────────────────

/// Convert a generic LlmMessage to an OpenAI-format JSON message.
pub fn to_openai_message(msg: &LlmMessage) -> serde_json::Value {
    match msg.role.as_str() {
        "tool" => {
            let mut m = serde_json::json!({
                "role": "tool",
                "tool_call_id": msg.tool_call_id,
                "content": msg.content,
            });
            if let Some(ref name) = msg.name {
                m["name"] = serde_json::json!(name);
            }
            m
        }
        "assistant" if msg.tool_calls.is_some() => {
            let tool_calls: Vec<serde_json::Value> = msg
                .tool_calls
                .as_ref()
                .unwrap()
                .iter()
                .map(|tc| {
                    serde_json::json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": tc.arguments.to_string(),
                        },
                    })
                })
                .collect();
            let mut m = serde_json::json!({
                "role": "assistant",
                "content": msg.content,
                "tool_calls": tool_calls,
            });
            // OpenAI requires null content when tool_calls are present
            if msg.content.is_none() {
                m["content"] = serde_json::Value::Null;
            }
            m
        }
        _ => serde_json::json!({
            "role": msg.role,
            "content": msg.content,
        }),
    }
}

// ── Tool conversion ────────────────────────────────────────────

/// Convert internal tool definitions to OpenAI function-calling format.
pub fn to_openai_tools(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t["name"],
                    "description": t["description"],
                    "parameters": t["parameters"],
                },
            })
        })
        .collect()
}

// ── Auth header ────────────────────────────────────────────────

/// Returns (header_name, header_value) for a given provider.
/// Most providers use `Authorization: Bearer <key>`. Mistral uses `x-api-key`.
pub fn auth_header_for(provider: &str, api_key: &str) -> (String, String) {
    match provider {
        "mistral" => ("x-api-key".to_string(), api_key.to_string()),
        _ => ("authorization".to_string(), format!("Bearer {}", api_key)),
    }
}

// ── OpenAI-compatible client ───────────────────────────────────

pub struct OpenAiClient {
    api_key: String,
    model: String,
    base_url: String,
    provider: String,
    http: reqwest::Client,
}

impl OpenAiClient {
    /// Create a new OpenAI-compatible client.
    /// Uses `base_url` from config if set, otherwise the provider's default.
    pub fn new(config: &LlmConfig) -> Self {
        let (default_url, _default_model) = provider_defaults(&config.provider)
            .unwrap_or_else(|| {
                panic!(
                    "No default URL for provider '{}'. Set base_url in config.",
                    config.provider
                )
            });

        let base_url = config
            .base_url
            .clone()
            .unwrap_or(default_url);

        // Strip trailing slashes for consistent URL building
        let base_url = base_url.trim_end_matches('/').to_string();

        Self {
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            base_url,
            provider: config.provider.clone(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn send_message(
        &self,
        messages: &[LlmMessage],
        tools: &[serde_json::Value],
    ) -> Result<LlmResponse, LlmError> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages
                .iter()
                .map(to_openai_message)
                .collect::<Vec<_>>(),
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(to_openai_tools(tools));
        }

        let chat_url = format!("{}/chat/completions", self.base_url);
        let (header_name, header_value) = auth_header_for(&self.provider, &self.api_key);

        let resp = self
            .http
            .post(&chat_url)
            .header(&header_name, &header_value)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(LlmError::RateLimited(60));
            }
            return Err(LlmError::ProviderError(status.as_u16(), err_body));
        }

        let data: serde_json::Value = resp.json().await.map_err(|e| {
            LlmError::Deserialize(format!("Failed to parse OpenAI response: {}", e))
        })?;

        let finish_reason = data["choices"][0]["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        let msg = &data["choices"][0]["message"];
        let text_content = msg["content"].as_str().map(String::from);

        let mut tool_calls = Vec::new();
        if let Some(calls) = msg["tool_calls"].as_array() {
            for call in calls {
                let args_str = call["function"]["arguments"].as_str().unwrap_or("{}");
                let arguments: serde_json::Value =
                    serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
                tool_calls.push(LlmToolCall {
                    id: call["id"].as_str().unwrap_or("").to_string(),
                    name: call["function"]["name"].as_str().unwrap_or("").to_string(),
                    arguments,
                });
            }
        }

        Ok(LlmResponse {
            content: text_content,
            tool_calls,
            finish_reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{LlmMessage, LlmToolCall};

    // ── Provider defaults ──────────────────────────────────────

    #[test]
    fn provider_defaults_openai() {
        let (url, model) = provider_defaults("openai").unwrap();
        assert_eq!(url, "https://api.openai.com/v1");
        assert_eq!(model, "gpt-4o");
    }

    #[test]
    fn provider_defaults_deepseek() {
        let (url, model) = provider_defaults("deepseek").unwrap();
        assert_eq!(url, "https://api.deepseek.com/v1");
        assert_eq!(model, "deepseek-chat");
    }

    #[test]
    fn provider_defaults_groq() {
        let (url, model) = provider_defaults("groq").unwrap();
        assert_eq!(url, "https://api.groq.com/openai/v1");
        assert_eq!(model, "llama-3.3-70b-versatile");
    }

    #[test]
    fn provider_defaults_ollama() {
        let (url, model) = provider_defaults("ollama").unwrap();
        assert_eq!(url, "http://localhost:11434/v1");
        assert_eq!(model, "llama3");
    }

    #[test]
    fn provider_defaults_unknown_returns_none() {
        assert!(provider_defaults("nonexistent").is_none());
    }

    #[test]
    fn provider_defaults_all_known_providers() {
        let providers = &[
            "openai", "deepseek", "groq", "together", "xai",
            "mistral", "openrouter", "qwen", "ollama",
        ];
        for p in providers {
            let result = provider_defaults(p);
            assert!(result.is_some(), "missing defaults for provider: {}", p);
            let (url, model) = result.unwrap();
            assert!(!url.is_empty(), "empty URL for provider: {}", p);
            assert!(!model.is_empty(), "empty model for provider: {}", p);
            assert!(url.ends_with("/v1"), "URL should end with /v1: {}", url);
        }
    }

    // ── Message translation ────────────────────────────────────

    #[test]
    fn to_openai_message_user_role() {
        let msg = LlmMessage {
            role: "user".into(),
            content: Some("Hello".into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        let result = to_openai_message(&msg);
        assert_eq!(result["role"], "user");
        assert_eq!(result["content"], "Hello");
        assert!(result.get("tool_calls").is_none());
    }

    #[test]
    fn to_openai_message_system_role() {
        let msg = LlmMessage {
            role: "system".into(),
            content: Some("You are a trader.".into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        let result = to_openai_message(&msg);
        assert_eq!(result["role"], "system");
        assert_eq!(result["content"], "You are a trader.");
    }

    #[test]
    fn to_openai_message_assistant_with_tool_calls() {
        let msg = LlmMessage {
            role: "assistant".into(),
            content: Some("Let me check.".into()),
            tool_calls: Some(vec![LlmToolCall {
                id: "call_001".into(),
                name: "fetch_klines".into(),
                arguments: serde_json::json!({"symbol": "BTC_USDT"}),
            }]),
            tool_call_id: None,
            name: None,
        };
        let result = to_openai_message(&msg);
        assert_eq!(result["role"], "assistant");
        assert_eq!(result["content"], "Let me check.");
        let calls = result["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["id"], "call_001");
        assert_eq!(calls[0]["function"]["name"], "fetch_klines");
        assert_eq!(
            calls[0]["function"]["arguments"],
            "{\"symbol\":\"BTC_USDT\"}"
        );
    }

    #[test]
    fn to_openai_message_tool_result() {
        let msg = LlmMessage {
            role: "tool".into(),
            content: Some("{\"close\": 97000}".into()),
            tool_calls: None,
            tool_call_id: Some("call_001".into()),
            name: Some("fetch_klines".into()),
        };
        let result = to_openai_message(&msg);
        assert_eq!(result["role"], "tool");
        assert_eq!(result["tool_call_id"], "call_001");
        assert_eq!(result["content"], "{\"close\": 97000}");
    }

    // ── Tool conversion ────────────────────────────────────────

    #[test]
    fn to_openai_tools_converts_correctly() {
        let tools = vec![serde_json::json!({
            "name": "fetch_klines",
            "description": "Get candlestick data",
            "parameters": {
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Trading pair"}
                },
                "required": ["symbol"]
            }
        })];
        let result = to_openai_tools(&tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "function");
        let func = &result[0]["function"];
        assert_eq!(func["name"], "fetch_klines");
        assert_eq!(func["description"], "Get candlestick data");
        assert_eq!(func["parameters"]["type"], "object");
        assert_eq!(
            func["parameters"]["properties"]["symbol"]["type"],
            "string"
        );
    }

    #[test]
    fn to_openai_tools_empty_input() {
        let result = to_openai_tools(&[]);
        assert!(result.is_empty());
    }

    // ── Auth header ────────────────────────────────────────────

    #[test]
    fn auth_header_mistral_uses_x_api_key() {
        let (name, value) = auth_header_for("mistral", "test-key");
        assert_eq!(name, "x-api-key");
        assert_eq!(value, "test-key");
    }

    #[test]
    fn auth_header_default_uses_bearer() {
        let (name, value) = auth_header_for("openai", "sk-test");
        assert_eq!(name, "authorization");
        assert_eq!(value, "Bearer sk-test");
    }

    #[test]
    fn auth_header_deepseek_uses_bearer() {
        let (name, value) = auth_header_for("deepseek", "sk-test");
        assert_eq!(name, "authorization");
        assert_eq!(value, "Bearer sk-test");
    }
}
