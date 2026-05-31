// @anchor infra:cli:tools:submit_signal
// @tags api

//! submit_signal tool — submit a trade signal to the backend.

use crate::tools::types::ToolDef;

pub fn tool_def() -> ToolDef {
    ToolDef {
        name: "submit_signal".into(),
        description:
            "Submit a trade signal. Always include stop_loss, reasoning, and confidence. \
             Start in SHADOW mode to test before going LIVE. Signals are validated by the \
             backend risk engine before execution.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Trading pair, e.g. 'ETH_USDT'"
                },
                "side": {
                    "type": "string",
                    "enum": ["LONG", "SHORT"]
                },
                "entry_price": {
                    "type": "number",
                    "description": "Entry price for the trade"
                },
                "stop_loss": {
                    "type": "number",
                    "description": "Stop loss price"
                },
                "take_profit": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "price": {"type": "number"},
                            "quantity": {"type": "number"}
                        }
                    },
                    "description": "Take profit targets"
                },
                "execution_mode": {
                    "type": "string",
                    "enum": ["SHADOW", "LIVE"],
                    "description": "SHADOW = paper trade, LIVE = real execution"
                },
                "leverage": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 20,
                    "description": "Leverage multiplier"
                },
                "reasoning": {
                    "type": "string",
                    "description": "Why this trade makes sense — thesis, setup, risk/reward"
                },
                "confidence": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Confidence in this signal (0.0–1.0)"
                },
                "source": {
                    "type": "string",
                    "description": "Agent identifier, e.g. 'agent:hermes_v1.2'"
                }
            },
            "required": [
                "symbol", "side", "entry_price", "stop_loss",
                "execution_mode", "reasoning", "confidence", "source"
            ]
        }),
    }
}
