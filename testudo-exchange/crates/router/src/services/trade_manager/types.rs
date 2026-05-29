//! Trade Manager Types
//!
//! Core types for the automated trade management system (EXT-09).
//! ManagedPosition tracks positions with management rules that the
//! trade manager evaluates on each price tick.

// @anchor exchange:router:types
// @tags api

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// State of a managed position through its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionState {
    /// Order placed, waiting for fill
    Pending,
    /// Entry filled, position active
    Filled,
    /// Actively managed (BE/trailing/TP rules running)
    Managing,
    /// Position closed (SL hit, TP hit, or manual close)
    Closed,
}

/// Side of the position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
}

/// Rules governing automated position management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementRules {
    /// Percentage of account to risk (for position sizing)
    pub risk_percent: Decimal,
    /// Move SL to entry when price reaches this % of distance to target
    pub break_even_at: u32,
    /// Leverage multiplier (1-125, Binance Futures max)
    pub leverage: u8,
    /// Optional trailing stop configuration
    pub trailing_stop: Option<TrailingStopRule>,
    /// Optional partial take-profit configuration
    pub partial_tp: Option<PartialTpRule>,
}

/// Trailing stop rule configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailingStopRule {
    /// Whether trailing stop is active
    pub enabled: bool,
    /// Trail distance as percentage of entry-to-target range
    pub distance_percent: u32,
}

/// Partial take-profit rule configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialTpRule {
    /// Whether partial TP is active
    pub enabled: bool,
    /// Percentage of position to close at target
    pub close_percent: u32,
}

/// Exchange order IDs associated with a managed position.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExchangeOrderIds {
    pub entry_order_id: Option<String>,
    pub stop_loss_order_id: Option<String>,
    pub take_profit_order_id: Option<String>,
}

/// A position being managed by the trade manager service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedPosition {
    pub id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub side: PositionSide,
    pub entry_price: Decimal,
    pub stop_price: Decimal,
    pub target_price: Decimal,
    pub quantity: Decimal,
    pub rules: ManagementRules,
    pub state: PositionState,
    /// Whether break-even has been triggered
    pub be_triggered: bool,
    /// Whether partial TP has fired
    pub partial_tp_fired: bool,
    /// Current stop loss price (may differ from original after BE/trailing)
    pub current_stop: Decimal,
    /// Remaining quantity (may differ after partial TP)
    pub remaining_qty: Decimal,
    pub exchange_order_ids: ExchangeOrderIds,
    pub created_at: DateTime<Utc>,
    /// EXT-16 FR-3: Exchange account to route trades to (None = first account).
    #[serde(default)]
    pub exchange_account_id: Option<Uuid>,
    /// RSK-02: Optional user-supplied setup tag captured at Alt+X entry time.
    #[serde(default)]
    pub setup_tag: Option<String>,
}

impl ManagedPosition {
    /// Create a new managed position with default state.
    pub fn new(
        user_id: Uuid,
        symbol: String,
        side: PositionSide,
        entry_price: Decimal,
        stop_price: Decimal,
        target_price: Decimal,
        quantity: Decimal,
        rules: ManagementRules,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            symbol,
            side,
            entry_price,
            stop_price,
            target_price,
            quantity,
            rules,
            state: PositionState::Pending,
            be_triggered: false,
            partial_tp_fired: false,
            current_stop: stop_price,
            remaining_qty: quantity,
            exchange_order_ids: ExchangeOrderIds::default(),
            created_at: Utc::now(),
            exchange_account_id: None,
            setup_tag: None,
        }
    }
}

/// Actions the trade manager can take on a managed position.
#[derive(Debug, Clone, PartialEq)]
pub enum ManagementAction {
    /// Move stop loss to entry price (break-even)
    MoveStopToEntry,
    /// Adjust trailing stop to a new price
    AdjustTrailingStop { new_price: Decimal },
    /// Close a portion of the position
    PartialClose { quantity: Decimal },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_managed_position_creation() {
        let pos = ManagedPosition::new(
            Uuid::new_v4(),
            "BTC_USDT".to_string(),
            PositionSide::Long,
            dec!(50000),
            dec!(49000),
            dec!(52000),
            dec!(0.2),
            ManagementRules {
                risk_percent: dec!(2),
                break_even_at: 50,
                leverage: 1,
                trailing_stop: None,
                partial_tp: None,
            },
        );

        assert_eq!(pos.state, PositionState::Pending);
        assert!(!pos.be_triggered);
        assert!(!pos.partial_tp_fired);
        assert_eq!(pos.current_stop, dec!(49000));
        assert_eq!(pos.remaining_qty, dec!(0.2));
    }

    #[test]
    fn test_position_side_serialization() {
        let long_json = serde_json::to_string(&PositionSide::Long).unwrap();
        assert_eq!(long_json, "\"Long\"");

        let short: PositionSide = serde_json::from_str("\"Short\"").unwrap();
        assert_eq!(short, PositionSide::Short);
    }

    #[test]
    fn test_management_rules_with_all_options() {
        let rules = ManagementRules {
            risk_percent: dec!(1.5),
            break_even_at: 50,
            leverage: 10,
            trailing_stop: Some(TrailingStopRule {
                enabled: true,
                distance_percent: 20,
            }),
            partial_tp: Some(PartialTpRule {
                enabled: true,
                close_percent: 50,
            }),
        };

        let json = serde_json::to_string(&rules).unwrap();
        let deserialized: ManagementRules = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.risk_percent, dec!(1.5));
        assert!(deserialized.trailing_stop.unwrap().enabled);
        assert_eq!(deserialized.partial_tp.unwrap().close_percent, 50);
    }

    #[test]
    fn test_exchange_order_ids_default() {
        let ids = ExchangeOrderIds::default();
        assert!(ids.entry_order_id.is_none());
        assert!(ids.stop_loss_order_id.is_none());
        assert!(ids.take_profit_order_id.is_none());
    }
}
