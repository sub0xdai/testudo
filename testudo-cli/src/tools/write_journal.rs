// @anchor infra:cli:tools:write_journal
// @tags api

//! write_journal tool — record a trading journal entry.
//!
//! NOTE: The backend does not yet have a POST /journal/agent/note endpoint.
//! This tool is a placeholder that logs entries locally. Full backend
//! integration requires the write endpoint to be implemented.

use crate::tools::types::ToolDef;

pub fn tool_def() -> ToolDef {
    ToolDef {
        name: "write_journal".into(),
        description:
            "Write a journal entry to record your reasoning, trade thesis, or observations. \
             Use after submitting signals to document pre-trade analysis. Tag entries with \
             strategy name and trade context for later review.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Journal entry content (markdown supported)"
                },
                "tag": {
                    "type": "string",
                    "description": "Tag for categorization, e.g. 'mean-reversion', 'breakout'"
                },
                "trade_group_id": {
                    "type": "string",
                    "description": "Associated trade group UUID (if applicable)"
                }
            },
            "required": ["content"]
        }),
    }
}
