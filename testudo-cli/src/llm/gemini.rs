// @anchor infra:cli:llm:gemini
// @tags api

//! Google Gemini provider — GenerateContent API.
//!
//! Uses the v1beta API with API key as query parameter (simplest auth).
//! System instructions are sent as top-level `systemInstruction`, not as a message.

use crate::config::LlmConfig;
use crate::llm::client::LlmClient;
use crate::llm::types::{LlmError, LlmMessage, LlmResponse, LlmToolCall};
use async_trait::async_trait;

const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

// ── Message translation ────────────────────────────────────────

/// Convert LlmMessages into Gemini contents + optional system instruction.
/// Returns (contents, system_instruction_parts).
pub fn to_gemini_request(
    messages: &[LlmMessage],
) -> (Vec<serde_json::Value>, Option<serde_json::Value>) {
    let mut contents: Vec<serde_json::Value> = Vec::new();
    let mut system_instruction: Option<serde_json::Value> = None;

    for msg in messages {
        match msg.role.as_str() {
            "system" => {
                system_instruction = Some(serde_json::json!({
                    "parts": [{"text": msg.content}],
                }));
            }
            "tool" => {
                contents.push(serde_json::json!({
                    "role": "tool",
                    "parts": [{
                        "functionResponse": {
                            "name": msg.name,
                            "response": {"result": msg.content},
                        },
                    }],
                }));
            }
            "assistant" if msg.tool_calls.is_some() => {
                let mut parts: Vec<serde_json::Value> = Vec::new();
                if let Some(ref text) = msg.content {
                    if !text.is_empty() {
                        parts.push(serde_json::json!({"text": text}));
                    }
                }
                for tc in msg.tool_calls.as_ref().unwrap() {
                    parts.push(serde_json::json!({
                        "functionCall": {
                            "name": tc.name,
                            "args": tc.arguments,
                        },
                    }));
                }
                contents.push(serde_json::json!({
                    "role": "model",
                    "parts": parts,
                }));
            }
            _ => {
                // user or assistant without tool calls
                let role = if msg.role == "assistant" { "model" } else { "user" };
                contents.push(serde_json::json!({
                    "role": role,
                    "parts": [{"text": msg.content}],
                }));
            }
        }
    }

    (contents, system_instruction)
}

// ── Tool conversion ────────────────────────────────────────────

/// Convert internal tool definitions to Gemini functionDeclarations format.
pub fn to_gemini_tools(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t["name"],
                "description": t["description"],
                "parameters": t["parameters"],
            })
        })
        .collect()
}

// ── Response parsing ───────────────────────────────────────────

/// Parse a Gemini API response JSON into an LlmResponse.
pub fn parse_gemini_response(data: &serde_json::Value) -> Result<LlmResponse, LlmError> {
    let candidates = data["candidates"].as_array().ok_or_else(|| {
        LlmError::Deserialize("Gemini response missing 'candidates' array".into())
    })?;

    if candidates.is_empty() {
        return Err(LlmError::Deserialize(
            "Gemini response has empty candidates".into(),
        ));
    }

    let candidate = &candidates[0];
    let finish_reason = candidate["finishReason"]
        .as_str()
        .unwrap_or("STOP")
        .to_string();

    let empty_vec = vec![];
    let parts = candidate["content"]["parts"].as_array().unwrap_or(&empty_vec);
    let mut text_content: Option<String> = None;
    let mut tool_calls: Vec<LlmToolCall> = Vec::new();

    for part in parts {
        if let Some(text) = part["text"].as_str() {
            // Concatenate multiple text parts
            match text_content.as_mut() {
                Some(existing) => existing.push_str(text),
                None => text_content = Some(text.to_string()),
            }
        }
        if let Some(fc) = part.get("functionCall") {
            tool_calls.push(LlmToolCall {
                id: format!("gemini_call_{}", tool_calls.len()),
                name: fc["name"].as_str().unwrap_or("").to_string(),
                arguments: fc["args"].clone(),
            });
        }
    }

    Ok(LlmResponse {
        content: text_content,
        tool_calls,
        finish_reason,
    })
}

// ── URL construction ───────────────────────────────────────────

/// Build the Gemini API URL for the given model and API key.
pub fn gemini_url(model: &str, api_key: &str) -> String {
    format!(
        "{}/{}:generateContent?key={}",
        GEMINI_BASE, model, api_key
    )
}

// ── Gemini client ──────────────────────────────────────────────

pub struct GeminiClient {
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl GeminiClient {
    pub fn new(config: &LlmConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmClient for GeminiClient {
    async fn send_message(
        &self,
        messages: &[LlmMessage],
        tools: &[serde_json::Value],
    ) -> Result<LlmResponse, LlmError> {
        let (contents, system_instruction) = to_gemini_request(messages);

        let mut body = serde_json::json!({
            "contents": contents,
        });

        if let Some(si) = system_instruction {
            body["systemInstruction"] = si;
        }

        if !tools.is_empty() {
            body["tools"] = serde_json::json!([{
                "functionDeclarations": to_gemini_tools(tools),
            }]);
        }

        let url = gemini_url(&self.model, &self.api_key);

        let resp = self
            .http
            .post(&url)
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
            LlmError::Deserialize(format!("Failed to parse Gemini response: {}", e))
        })?;

        parse_gemini_response(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{LlmMessage, LlmToolCall};

    // ── Message translation ────────────────────────────────────

    #[test]
    fn to_gemini_contents_user_message() {
        let msgs = vec![LlmMessage {
            role: "user".into(),
            content: Some("What is BTC price?".into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];
        let (contents, system) = to_gemini_request(&msgs);
        assert!(system.is_none());
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
        let parts = contents[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["text"], "What is BTC price?");
    }

    #[test]
    fn to_gemini_contents_system_becomes_system_instruction() {
        let msgs = vec![
            LlmMessage {
                role: "system".into(),
                content: Some("You are a trader.".into()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            LlmMessage {
                role: "user".into(),
                content: Some("Hi".into()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];
        let (contents, system) = to_gemini_request(&msgs);
        assert!(system.is_some());
        assert_eq!(system.unwrap()["parts"][0]["text"], "You are a trader.");
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
    }

    #[test]
    fn to_gemini_contents_model_with_tool_calls() {
        let msgs = vec![LlmMessage {
            role: "assistant".into(),
            content: Some("Let me check.".into()),
            tool_calls: Some(vec![LlmToolCall {
                id: "call_001".into(),
                name: "fetch_klines".into(),
                arguments: serde_json::json!({"symbol": "BTC_USDT"}),
            }]),
            tool_call_id: None,
            name: None,
        }];
        let (contents, _) = to_gemini_request(&msgs);
        assert_eq!(contents[0]["role"], "model");
        let parts = contents[0]["parts"].as_array().unwrap();
        assert!(parts.len() >= 2);
        let fc = parts.iter().find(|p| p.get("functionCall").is_some()).unwrap();
        assert_eq!(fc["functionCall"]["name"], "fetch_klines");
        assert_eq!(fc["functionCall"]["args"]["symbol"], "BTC_USDT");
    }

    #[test]
    fn to_gemini_contents_tool_result() {
        let msgs = vec![LlmMessage {
            role: "tool".into(),
            content: Some("{\"close\": 97000}".into()),
            tool_calls: None,
            tool_call_id: Some("call_001".into()),
            name: Some("fetch_klines".into()),
        }];
        let (contents, _) = to_gemini_request(&msgs);
        assert_eq!(contents[0]["role"], "tool");
        let parts = contents[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        let fr = &parts[0]["functionResponse"];
        assert_eq!(fr["name"], "fetch_klines");
        assert_eq!(fr["response"]["result"], "{\"close\": 97000}");
    }

    // ── Tool conversion ────────────────────────────────────────

    #[test]
    fn to_gemini_tools_converts_correctly() {
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
        let result = to_gemini_tools(&tools);
        assert_eq!(result.len(), 1);
        let fd = &result[0];
        assert_eq!(fd["name"], "fetch_klines");
        assert_eq!(fd["description"], "Get candlestick data");
        assert_eq!(fd["parameters"]["type"], "object");
    }

    #[test]
    fn to_gemini_tools_empty_input() {
        let result = to_gemini_tools(&[]);
        assert!(result.is_empty());
    }

    // ── Response parsing ───────────────────────────────────────

    #[test]
    fn parse_gemini_response_text_only() {
        let json = serde_json::json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "BTC is at 97000"}
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        let resp = parse_gemini_response(&json).unwrap();
        assert_eq!(resp.content, Some("BTC is at 97000".into()));
        assert!(resp.tool_calls.is_empty());
        assert_eq!(resp.finish_reason, "STOP");
    }

    #[test]
    fn parse_gemini_response_with_tool_call() {
        let json = serde_json::json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "Let me fetch data."},
                        {
                            "functionCall": {
                                "name": "fetch_klines",
                                "args": {"symbol": "BTC_USDT"}
                            }
                        }
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        let resp = parse_gemini_response(&json).unwrap();
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "fetch_klines");
        assert_eq!(resp.tool_calls[0].arguments["symbol"], "BTC_USDT");
        assert_eq!(resp.content, Some("Let me fetch data.".into()));
    }

    #[test]
    fn parse_gemini_response_no_candidates_is_error() {
        let json = serde_json::json!({});
        assert!(parse_gemini_response(&json).is_err());
    }

    // ── URL construction ───────────────────────────────────────

    #[test]
    fn gemini_url_includes_model_and_key() {
        let url = gemini_url("gemini-2.5-flash", "test-key-123");
        assert!(url.contains("gemini-2.5-flash"));
        assert!(url.contains("generateContent"));
        assert!(url.contains("key=test-key-123"));
        assert!(url.starts_with("https://generativelanguage.googleapis.com"));
    }
}
