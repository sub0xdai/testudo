//! Position Types for Binance Position Synchronization
//!
//! This module defines types for tracking Binance positions,
//! comparing shadow vs live positions, and handling sync errors.

// @anchor exchange:common_utils:position_types
// @tags infra

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Position side for Binance positions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PositionSide {
    /// Long position (spot: holding asset)
    Long,
    /// Short position (futures only)
    Short,
}

/// A position on Binance exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinancePosition {
    /// Symbol in Binance format (e.g., "BTCUSDT")
    pub symbol: String,
    /// Position side
    pub side: PositionSide,
    /// Position quantity
    pub quantity: Decimal,
    /// Average entry price
    pub entry_price: Decimal,
    /// Unrealized P&L
    pub unrealized_pnl: Decimal,
    /// Position timestamp (milliseconds)
    pub timestamp: i64,
}

impl BinancePosition {
    /// Create a new Binance position
    pub fn new(
        symbol: String,
        side: PositionSide,
        quantity: Decimal,
        entry_price: Decimal,
    ) -> Self {
        Self {
            symbol,
            side,
            quantity,
            entry_price,
            unrealized_pnl: Decimal::ZERO,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Update unrealized P&L based on mark price
    pub fn update_pnl(&mut self, mark_price: Decimal) {
        self.unrealized_pnl = match self.side {
            PositionSide::Long => (mark_price - self.entry_price) * self.quantity,
            PositionSide::Short => (self.entry_price - mark_price) * self.quantity,
        };
    }

    /// Check if this is an empty/closed position
    pub fn is_empty(&self) -> bool {
        self.quantity.is_zero()
    }
}

/// Result of comparing shadow and Binance positions
#[derive(Debug, Clone, Default)]
pub struct PositionDiff {
    /// Positions in shadow but not on Binance (orphaned shadow positions)
    pub shadow_only: Vec<ShadowPositionInfo>,
    /// Positions on Binance but not in shadow (orphaned Binance positions)
    pub binance_only: Vec<BinancePosition>,
    /// Positions with quantity mismatch
    pub quantity_mismatch: Vec<QuantityMismatch>,
    /// Positions that match perfectly
    pub matched: Vec<MatchedPosition>,
}

impl PositionDiff {
    /// Create an empty diff (all positions match)
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if positions are in sync (no discrepancies)
    pub fn is_synced(&self) -> bool {
        self.shadow_only.is_empty()
            && self.binance_only.is_empty()
            && self.quantity_mismatch.is_empty()
    }

    /// Check if there are any discrepancies
    pub fn has_discrepancies(&self) -> bool {
        !self.is_synced()
    }

    /// Count total discrepancies
    pub fn discrepancy_count(&self) -> usize {
        self.shadow_only.len() + self.binance_only.len() + self.quantity_mismatch.len()
    }
}

/// Shadow position info for comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowPositionInfo {
    /// Symbol in internal format (e.g., "BTC_USDC")
    pub symbol: String,
    /// Position side
    pub side: PositionSide,
    /// Position quantity
    pub quantity: Decimal,
    /// Entry price
    pub entry_price: Decimal,
}

/// A position that exists in both shadow and Binance with matching quantities
#[derive(Debug, Clone)]
pub struct MatchedPosition {
    /// Symbol in internal format
    pub symbol: String,
    /// Position side
    pub side: PositionSide,
    /// Quantity (same in both)
    pub quantity: Decimal,
    /// Shadow entry price
    pub shadow_entry_price: Decimal,
    /// Binance entry price
    pub binance_entry_price: Decimal,
}

/// A position with quantity mismatch between shadow and Binance
#[derive(Debug, Clone)]
pub struct QuantityMismatch {
    /// Symbol in internal format
    pub symbol: String,
    /// Position side
    pub side: PositionSide,
    /// Shadow quantity
    pub shadow_qty: Decimal,
    /// Binance quantity
    pub binance_qty: Decimal,
    /// Quantity difference (shadow - binance)
    pub difference: Decimal,
}

impl QuantityMismatch {
    /// Create a new quantity mismatch
    pub fn new(
        symbol: String,
        side: PositionSide,
        shadow_qty: Decimal,
        binance_qty: Decimal,
    ) -> Self {
        Self {
            symbol,
            side,
            shadow_qty,
            binance_qty,
            difference: shadow_qty - binance_qty,
        }
    }

    /// Check if shadow has more than Binance
    pub fn shadow_exceeds(&self) -> bool {
        self.difference > Decimal::ZERO
    }

    /// Check if Binance has more than shadow
    pub fn binance_exceeds(&self) -> bool {
        self.difference < Decimal::ZERO
    }
}

/// Errors that can occur during position synchronization
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Exchange unavailable")]
    ExchangeUnavailable,

    #[error("Request timeout")]
    Timeout,
}

/// Result of a sync operation
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Position differences found
    pub diff: PositionDiff,
    /// Timestamp of sync
    pub timestamp: i64,
    /// Whether sync was successful
    pub success: bool,
    /// Error message if any
    pub error_message: Option<String>,
}

impl SyncResult {
    /// Create a successful sync result
    pub fn success(diff: PositionDiff) -> Self {
        Self {
            diff,
            timestamp: chrono::Utc::now().timestamp_millis(),
            success: true,
            error_message: None,
        }
    }

    /// Create a failed sync result
    pub fn failure(error: &str) -> Self {
        Self {
            diff: PositionDiff::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            success: false,
            error_message: Some(error.to_string()),
        }
    }
}

/// Action to reconcile position discrepancies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileAction {
    /// Update shadow to match Binance
    UpdateShadow,
    /// Alert user to manually resolve
    AlertUser,
    /// Close orphaned position on Binance (requires user confirmation)
    CloseOrphaned,
}

/// Binance account balance info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceBalance {
    /// Asset symbol (e.g., "BTC", "USDT")
    pub asset: String,
    /// Free (available) balance
    pub free: Decimal,
    /// Locked balance (in orders)
    pub locked: Decimal,
}

impl BinanceBalance {
    /// Get total balance (free + locked)
    pub fn total(&self) -> Decimal {
        self.free + self.locked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ==================== BinancePosition Tests ====================

    #[test]
    fn test_binance_position_creation() {
        let pos = BinancePosition::new(
            "BTCUSDT".to_string(),
            PositionSide::Long,
            dec!(0.5),
            dec!(50000),
        );

        assert_eq!(pos.symbol, "BTCUSDT");
        assert_eq!(pos.side, PositionSide::Long);
        assert_eq!(pos.quantity, dec!(0.5));
        assert_eq!(pos.entry_price, dec!(50000));
        assert_eq!(pos.unrealized_pnl, Decimal::ZERO);
    }

    #[test]
    fn test_binance_position_pnl_long() {
        let mut pos = BinancePosition::new(
            "BTCUSDT".to_string(),
            PositionSide::Long,
            dec!(0.1),
            dec!(50000),
        );

        // Price goes up - profit
        pos.update_pnl(dec!(51000));
        assert_eq!(pos.unrealized_pnl, dec!(100)); // 0.1 * (51000 - 50000)

        // Price goes down - loss
        pos.update_pnl(dec!(49000));
        assert_eq!(pos.unrealized_pnl, dec!(-100)); // 0.1 * (49000 - 50000)
    }

    #[test]
    fn test_binance_position_pnl_short() {
        let mut pos = BinancePosition::new(
            "BTCUSDT".to_string(),
            PositionSide::Short,
            dec!(0.1),
            dec!(50000),
        );

        // Price goes down - profit for short
        pos.update_pnl(dec!(49000));
        assert_eq!(pos.unrealized_pnl, dec!(100)); // 0.1 * (50000 - 49000)

        // Price goes up - loss for short
        pos.update_pnl(dec!(51000));
        assert_eq!(pos.unrealized_pnl, dec!(-100)); // 0.1 * (50000 - 51000)
    }

    #[test]
    fn test_binance_position_is_empty() {
        let pos = BinancePosition::new(
            "BTCUSDT".to_string(),
            PositionSide::Long,
            Decimal::ZERO,
            dec!(50000),
        );
        assert!(pos.is_empty());

        let pos2 = BinancePosition::new(
            "BTCUSDT".to_string(),
            PositionSide::Long,
            dec!(0.1),
            dec!(50000),
        );
        assert!(!pos2.is_empty());
    }

    // ==================== PositionDiff Tests ====================

    #[test]
    fn test_position_diff_empty_is_synced() {
        let diff = PositionDiff::new();
        assert!(diff.is_synced());
        assert!(!diff.has_discrepancies());
        assert_eq!(diff.discrepancy_count(), 0);
    }

    #[test]
    fn test_position_diff_with_shadow_only() {
        let mut diff = PositionDiff::new();
        diff.shadow_only.push(ShadowPositionInfo {
            symbol: "BTC_USDC".to_string(),
            side: PositionSide::Long,
            quantity: dec!(0.1),
            entry_price: dec!(50000),
        });

        assert!(!diff.is_synced());
        assert!(diff.has_discrepancies());
        assert_eq!(diff.discrepancy_count(), 1);
    }

    #[test]
    fn test_position_diff_with_binance_only() {
        let mut diff = PositionDiff::new();
        diff.binance_only.push(BinancePosition::new(
            "BTCUSDT".to_string(),
            PositionSide::Long,
            dec!(0.1),
            dec!(50000),
        ));

        assert!(!diff.is_synced());
        assert!(diff.has_discrepancies());
        assert_eq!(diff.discrepancy_count(), 1);
    }

    #[test]
    fn test_position_diff_with_mismatch() {
        let mut diff = PositionDiff::new();
        diff.quantity_mismatch.push(QuantityMismatch::new(
            "BTC_USDC".to_string(),
            PositionSide::Long,
            dec!(0.2),
            dec!(0.1),
        ));

        assert!(!diff.is_synced());
        assert!(diff.has_discrepancies());
        assert_eq!(diff.discrepancy_count(), 1);
    }

    // ==================== QuantityMismatch Tests ====================

    #[test]
    fn test_quantity_mismatch_shadow_exceeds() {
        let mismatch = QuantityMismatch::new(
            "BTC_USDC".to_string(),
            PositionSide::Long,
            dec!(0.2),
            dec!(0.1),
        );

        assert!(mismatch.shadow_exceeds());
        assert!(!mismatch.binance_exceeds());
        assert_eq!(mismatch.difference, dec!(0.1));
    }

    #[test]
    fn test_quantity_mismatch_binance_exceeds() {
        let mismatch = QuantityMismatch::new(
            "BTC_USDC".to_string(),
            PositionSide::Long,
            dec!(0.1),
            dec!(0.2),
        );

        assert!(!mismatch.shadow_exceeds());
        assert!(mismatch.binance_exceeds());
        assert_eq!(mismatch.difference, dec!(-0.1));
    }

    // ==================== SyncResult Tests ====================

    #[test]
    fn test_sync_result_success() {
        let diff = PositionDiff::new();
        let result = SyncResult::success(diff);

        assert!(result.success);
        assert!(result.error_message.is_none());
        assert!(result.diff.is_synced());
    }

    #[test]
    fn test_sync_result_failure() {
        let result = SyncResult::failure("Network timeout");

        assert!(!result.success);
        assert_eq!(result.error_message, Some("Network timeout".to_string()));
    }

    // ==================== BinanceBalance Tests ====================

    #[test]
    fn test_binance_balance_total() {
        let balance = BinanceBalance {
            asset: "USDT".to_string(),
            free: dec!(1000),
            locked: dec!(500),
        };

        assert_eq!(balance.total(), dec!(1500));
    }
}
