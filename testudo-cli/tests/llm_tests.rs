// @anchor test:cli:llm
// @tags api

use testudo_cli::llm::types::{LlmMessage, LlmResponse, LlmToolCall};

#[test]
fn llm_message_constructs_correctly() {
    let msg = LlmMessage {
        role: "user".into(),
        content: Some("Analyze ETH".into()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    };
    assert_eq!(msg.role, "user");
    assert_eq!(msg.content, Some("Analyze ETH".into()));
}

#[test]
fn llm_tool_call_has_required_fields() {
    let tc = LlmToolCall {
        id: "toolu_001".into(),
        name: "fetch_klines".into(),
        arguments: serde_json::json!({"symbol": "ETH_USDT"}),
    };
    assert_eq!(tc.name, "fetch_klines");
    assert_eq!(tc.arguments["symbol"], "ETH_USDT");
}

#[test]
fn llm_response_with_no_tool_calls() {
    let resp = LlmResponse {
        content: Some("ETH looks bullish".into()),
        tool_calls: vec![],
        finish_reason: "stop".into(),
    };
    assert!(resp.tool_calls.is_empty());
    assert_eq!(resp.content.unwrap(), "ETH looks bullish");
}

#[test]
fn create_client_does_not_panic() {
    let config = testudo_cli::config::LlmConfig {
        provider: "anthropic".into(),
        api_key: "sk-ant-test".into(),
        model: "claude-sonnet-4-20250514".into(),
        base_url: None,
    };
    let _client = testudo_cli::llm::client::create_client(&config);
    // Just verify it doesn't panic
}
