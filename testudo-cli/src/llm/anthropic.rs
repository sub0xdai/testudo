// @anchor infra:cli:llm:anthropic
// @tags api

//! Anthropic Messages API client.

use crate::config::LlmConfig;
use crate::llm::types::{LlmError, LlmMessage, LlmResponse, LlmToolCall};
use async_trait::async_trait;
use crate::llm::client::LlmClient;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl AnthropicClient {
    pub fn new(config: &LlmConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            http: reqwest::Client::new(),
        }
    }

    fn to_anthropic_message(msg: &LlmMessage) -> serde_json::Value {
        match msg.role.as_str() {
            "tool" => serde_json::json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": msg.tool_call_id,
                    "content": msg.content,
                }],
            }),
            "assistant" if msg.tool_calls.is_some() => {
                let tool_uses: Vec<serde_json::Value> = msg
                    .tool_calls
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|tc| {
                        serde_json::json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.arguments,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "role": "assistant",
                    "content": tool_uses,
                })
            }
            _ => serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }),
        }
    }

    fn to_anthropic_tools(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t["name"],
                    "description": t["description"],
                    "input_schema": t["parameters"],
                })
            })
            .collect()
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn send_message(
        &self,
        messages: &[LlmMessage],
        tools: &[serde_json::Value],
    ) -> Result<LlmResponse, LlmError> {
        let system_msg = messages.iter().find(|m| m.role == "system");
        let non_system: Vec<&LlmMessage> =
            messages.iter().filter(|m| m.role != "system").collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": non_system.iter().map(|m| Self::to_anthropic_message(m)).collect::<Vec<_>>(),
        });

        if let Some(sys) = system_msg {
            if let Some(ref content) = sys.content {
                body["system"] = serde_json::json!(content);
            }
        }

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(Self::to_anthropic_tools(tools));
        }

        let resp = self
            .http
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
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
            LlmError::Deserialize(format!("Failed to parse Anthropic response: {}", e))
        })?;

        let stop_reason = data["stop_reason"].as_str().unwrap_or("stop").to_string();
        let mut tool_calls = Vec::new();
        let mut text_content = None;

        if let Some(blocks) = data["content"].as_array() {
            for block in blocks {
                match block["type"].as_str() {
                    Some("tool_use") => {
                        tool_calls.push(LlmToolCall {
                            id: block["id"].as_str().unwrap_or("").to_string(),
                            name: block["name"].as_str().unwrap_or("").to_string(),
                            arguments: block["input"].clone(),
                        });
                    }
                    Some("text") => {
                        text_content = block["text"].as_str().map(String::from);
                    }
                    _ => {}
                }
            }
        }

        Ok(LlmResponse {
            content: text_content,
            tool_calls,
            finish_reason: stop_reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_tool_conversion_formats_correctly() {
        let tools = vec![serde_json::json!({
            "name": "fetch_klines",
            "description": "Get candlestick data",
            "parameters": {
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"}
                },
                "required": ["symbol"]
            }
        })];

        let result = AnthropicClient::to_anthropic_tools(&tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "fetch_klines");
        assert!(result[0]["input_schema"].is_object());
    }

    #[test]
    fn anthropic_message_user_role() {
        let msg = LlmMessage {
            role: "user".into(),
            content: Some("Hello".into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        let result = AnthropicClient::to_anthropic_message(&msg);
        assert_eq!(result["role"], "user");
        assert_eq!(result["content"], "Hello");
    }

    #[test]
    fn anthropic_message_tool_result() {
        let msg = LlmMessage {
            role: "tool".into(),
            content: Some("result data".into()),
            tool_calls: None,
            tool_call_id: Some("toolu_001".into()),
            name: None,
        };
        let result = AnthropicClient::to_anthropic_message(&msg);
        assert_eq!(result["role"], "user");
        let content = &result["content"].as_array().unwrap()[0];
        assert_eq!(content["type"], "tool_result");
        assert_eq!(content["tool_use_id"], "toolu_001");
    }
}
