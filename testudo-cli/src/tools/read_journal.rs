// @anchor infra:cli:tools:read_journal
// @tags api

//! read_journal tool — fetch recent trading journal summary.

use crate::tools::types::ToolDef;

pub fn tool_def() -> ToolDef {
    ToolDef {
        name: "read_journal".into(),
        description:
            "Read your recent trading journal summary. Shows win rate, P&L, top trades, \
             and setup breakdowns. Use this at the start of each iteration to understand \
             recent performance before making new decisions.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "timeframe": {
                    "type": "string",
                    "enum": ["7d", "30d", "90d", "all"],
                    "description": "Time period to summarize"
                }
            },
            "required": []
        }),
    }
}
