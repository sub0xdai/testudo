// @anchor infra:cli:tools:list_positions
// @tags api

//! list_positions tool — list current open positions.
//!
//! NOTE: The positions endpoint requires an exchange account ID
//! (GET /exchanges/accounts/{id}/positions). This tool returns a
//! placeholder. Full implementation requires account discovery first.

use crate::tools::types::ToolDef;

pub fn tool_def() -> ToolDef {
    ToolDef {
        name: "list_positions".into(),
        description:
            "List your current open positions. Returns symbol, side, entry price, \
             current P&L, and stop loss for each position. Use to understand current \
             exposure before entering new trades.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}
