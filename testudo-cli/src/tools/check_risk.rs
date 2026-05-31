// @anchor infra:cli:tools:check_risk
// @tags api

//! check_risk tool — retrieve current risk configuration and limits.

use crate::tools::types::ToolDef;

pub fn tool_def() -> ToolDef {
    ToolDef {
        name: "check_risk".into(),
        description:
            "Check your current risk configuration. Returns position size limits, \
             max leverage, drawdown limits, and stop loss requirements. Always check \
             risk before submitting signals to ensure compliance.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}
