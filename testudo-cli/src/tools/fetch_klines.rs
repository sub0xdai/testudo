// @anchor infra:cli:tools:fetch_klines
// @tags api

//! fetch_klines tool — retrieve OHLCV candlestick data.

use crate::tools::types::ToolDef;

/// Return the tool definition for fetch_klines.
pub fn tool_def() -> ToolDef {
    ToolDef {
        name: "fetch_klines".into(),
        description:
            "Fetch OHLCV candlestick data for a trading pair. Use this before making any \
             trading decisions to understand recent price action, volatility, and trends.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Trading pair symbol, e.g. 'ETH_USDT'"
                },
                "interval": {
                    "type": "string",
                    "enum": ["1m", "5m", "15m", "1h", "4h", "1d", "1w"],
                    "description": "Candlestick interval"
                },
                "start_time": {
                    "type": "string",
                    "description": "ISO 8601 start timestamp (optional)"
                }
            },
            "required": ["symbol", "interval"]
        }),
    }
}
