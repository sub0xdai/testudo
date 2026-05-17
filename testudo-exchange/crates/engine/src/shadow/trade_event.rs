//! Trade event types for the append-only audit log (019f).
//!
//! Every state transition in the trading lifecycle is captured as a `TradeEvent`.
//! Events are emitted by the `EngineActor` via `try_send()` (non-blocking) and
//! persisted by the `TradeEventWriter` in the router crate.

use serde::Serialize;
use uuid::Uuid;

/// A trade event for the append-only audit log.
#[derive(Debug, Clone, Serialize)]
pub struct TradeEvent {
    pub event_type: TradeEventType,
    pub group_id: Option<Uuid>,
    pub user_id: Uuid,
    pub symbol: Option<String>,
    pub payload: serde_json::Value,
}

/// Discriminant for trade event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeEventType {
    TradeCreated,
    EntryPlaced,
    EntryFilled,
    StopLossPlaced,
    StopLossFilled,
    TakeProfitPlaced,
    TakeProfitFilled,
    OrderCancelled,
    GroupStatusChanged,
    BreakEvenTriggered,
    StopLossAmended,
    ReconciliationAction,
    PlacementTimeout,
    /// CON-01: Emitted by FillDetector when a trade closes (SL or TP fill).
    /// Payload carries full journal data for atomic co-write in TradeEventWriter.
    TradeClosed,
}

impl TradeEventType {
    /// Returns the string representation for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TradeCreated => "trade_created",
            Self::EntryPlaced => "entry_placed",
            Self::EntryFilled => "entry_filled",
            Self::StopLossPlaced => "stop_loss_placed",
            Self::StopLossFilled => "stop_loss_filled",
            Self::TakeProfitPlaced => "take_profit_placed",
            Self::TakeProfitFilled => "take_profit_filled",
            Self::OrderCancelled => "order_cancelled",
            Self::GroupStatusChanged => "group_status_changed",
            Self::BreakEvenTriggered => "break_even_triggered",
            Self::StopLossAmended => "stop_loss_amended",
            Self::ReconciliationAction => "reconciliation_action",
            Self::PlacementTimeout => "placement_timeout",
            Self::TradeClosed => "trade_closed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_as_str_roundtrip() {
        let types = vec![
            (TradeEventType::TradeCreated, "trade_created"),
            (TradeEventType::EntryFilled, "entry_filled"),
            (TradeEventType::StopLossFilled, "stop_loss_filled"),
            (TradeEventType::TakeProfitFilled, "take_profit_filled"),
            (TradeEventType::OrderCancelled, "order_cancelled"),
            (TradeEventType::GroupStatusChanged, "group_status_changed"),
            (TradeEventType::BreakEvenTriggered, "break_even_triggered"),
            (TradeEventType::PlacementTimeout, "placement_timeout"),
            (TradeEventType::TradeClosed, "trade_closed"),
        ];
        for (variant, expected) in types {
            assert_eq!(variant.as_str(), expected);
        }
    }

    #[test]
    fn test_trade_event_serialization() {
        let event = TradeEvent {
            event_type: TradeEventType::EntryFilled,
            group_id: Some(Uuid::nil()),
            user_id: Uuid::nil(),
            symbol: Some("BTC_USDT".to_string()),
            payload: serde_json::json!({"fill_price": "50000"}),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event_type"], "entry_filled");
        assert_eq!(json["symbol"], "BTC_USDT");
    }
}
