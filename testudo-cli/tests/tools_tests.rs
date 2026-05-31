// @anchor test:cli:tools
// @tags api

use testudo_cli::tools::types::{ToolDef, ToolError, ToolResult};

#[test]
fn tool_def_has_required_fields() {
    let td = ToolDef {
        name: "fetch_klines".into(),
        description: "Get OHLCV data".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {"symbol": {"type": "string"}},
            "required": ["symbol"]
        }),
    };
    assert_eq!(td.name, "fetch_klines");
    assert!(!td.description.is_empty());
    assert!(td.parameters.is_object());
}

#[test]
fn tool_result_holds_content() {
    let result = ToolResult {
        content: "OK: 100 candles".into(),
    };
    assert_eq!(result.content, "OK: 100 candles");
}

#[test]
fn tool_error_missing_arg() {
    let err = ToolError::MissingArg("symbol".into());
    assert!(err.to_string().contains("symbol"));
}

#[test]
fn tool_error_execution() {
    let err = ToolError::Execution("API timeout".into());
    assert!(err.to_string().contains("API timeout"));
}

#[test]
fn all_tools_returns_seven_tools() {
    let tools = testudo_cli::tools::all_tools();
    assert_eq!(tools.len(), 7, "should return exactly 7 tool definitions");
}

#[test]
fn fetch_klines_def_has_valid_schema() {
    let def = testudo_cli::tools::fetch_klines::tool_def();
    assert_eq!(def.name, "fetch_klines");
    assert!(def.parameters["properties"]["symbol"]["type"] == "string");
    assert!(def.parameters["required"].as_array().unwrap().contains(&serde_json::json!("symbol")));
}

#[test]
fn submit_signal_def_requires_reasoning() {
    let def = testudo_cli::tools::submit_signal::tool_def();
    assert_eq!(def.name, "submit_signal");
    let required = def.parameters["required"].as_array().unwrap();
    let fields: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(fields.contains(&"reasoning"), "must require reasoning");
    assert!(fields.contains(&"confidence"), "must require confidence");
}

#[test]
fn read_journal_def_has_correct_name() {
    let def = testudo_cli::tools::read_journal::tool_def();
    assert_eq!(def.name, "read_journal");
}

#[test]
fn write_journal_def_has_correct_name() {
    let def = testudo_cli::tools::write_journal::tool_def();
    assert_eq!(def.name, "write_journal");
}

#[test]
fn list_positions_def_has_correct_name() {
    let def = testudo_cli::tools::list_positions::tool_def();
    assert_eq!(def.name, "list_positions");
}

#[test]
fn check_risk_def_has_correct_name() {
    let def = testudo_cli::tools::check_risk::tool_def();
    assert_eq!(def.name, "check_risk");
}

#[test]
fn check_onboarding_def_has_correct_name() {
    let def = testudo_cli::tools::check_onboarding::tool_def();
    assert_eq!(def.name, "check_onboarding");
}
