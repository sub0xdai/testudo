// @anchor infra:cli:llm:stream
// @tags api

//! SSE/streaming token parser for LLM responses.
//!
//! Full streaming integration (token → TUI pane) is deferred to CLI-05.
//! This module provides the parsing primitives.

use crate::llm::types::{LlmError, LlmResponse, LlmToolCall};
use futures_util::StreamExt;

/// Parse an Anthropic SSE stream into a complete LlmResponse.
///
/// Accumulates text content and tool_use blocks from streamed events.
/// Called by the AnthropicClient streaming path.
pub async fn parse_anthropic_stream(
    mut stream: impl StreamExt<Item = Result<Vec<u8>, reqwest::Error>> + Unpin,
) -> Result<LlmResponse, LlmError> {
    let mut content = String::new();
    let mut tool_calls: Vec<LlmToolCall> = Vec::new();
    let mut finish_reason = String::from("stop");
    let mut tool_use_blocks: std::collections::HashMap<String, (String, String, serde_json::Value)> =
        std::collections::HashMap::new();

    while let Some(chunk) = stream.next().await {
        let data = chunk.map_err(|e| LlmError::Network(e.to_string()))?;
        let text = String::from_utf8_lossy(&data);

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || !line.starts_with("data: ") {
                continue;
            }

            let data = &line["data: ".len()..];

            if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                match event["type"].as_str() {
                    Some("content_block_delta") => {
                        if let Some(delta) = event["delta"].as_object() {
                            match delta.get("type").and_then(|t| t.as_str()) {
                                Some("text_delta") => {
                                    if let Some(text) = delta["text"].as_str() {
                                        content.push_str(text);
                                    }
                                }
                                Some("input_json_delta") => {
                                    if let Some(partial) = delta["partial_json"].as_str() {
                                        let index = event["index"].as_u64().unwrap_or(0);
                                        let entry = tool_use_blocks
                                            .entry(index.to_string())
                                            .or_insert_with(|| {
                                                (String::new(), String::new(), serde_json::Value::Null)
                                            });
                                        entry.2 = serde_json::json!(partial);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Some("content_block_start") => {
                        if let Some(block) = event["content_block"].as_object() {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                let index = event["index"].as_u64().unwrap_or(0);
                                let entry = tool_use_blocks
                                    .entry(index.to_string())
                                    .or_insert_with(|| {
                                        (String::new(), String::new(), serde_json::Value::Null)
                                    });
                                entry.0 = block["id"].as_str().unwrap_or("").to_string();
                                entry.1 = block["name"].as_str().unwrap_or("").to_string();
                            }
                        }
                    }
                    Some("message_delta") => {
                        if let Some(stop) = event["delta"]["stop_reason"].as_str() {
                            finish_reason = stop.to_string();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Convert accumulated tool_use blocks
    for (_, (id, name, input)) in tool_use_blocks {
        if !id.is_empty() && !name.is_empty() {
            tool_calls.push(LlmToolCall {
                id,
                name,
                arguments: input,
            });
        }
    }

    Ok(LlmResponse {
        content: if content.is_empty() { None } else { Some(content) },
        tool_calls,
        finish_reason,
    })
}
