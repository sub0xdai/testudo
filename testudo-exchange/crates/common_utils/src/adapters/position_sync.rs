//! Position Syncer for Binance Position Synchronization
//!
//! This module provides functionality to sync shadow positions with Binance
//! exchange positions, detect discrepancies, and alert users.
//!
//! # E.4 Requirements
//! - Fetch current positions from Binance
//! - Compare shadow positions with Binance positions
//! - Reconcile positions (alert user of discrepancies - no auto-reconcile)

// @anchor exchange:common_utils:position_sync
// @tags infra

use super::binance_executor::BinanceExecutor;
use super::execution_types::symbol;
use super::position_types::{
    BinanceBalance, BinancePosition, MatchedPosition, PositionDiff, QuantityMismatch,
    ShadowPositionInfo, SyncError, SyncResult,
};
use rust_decimal::Decimal;
use serde::Deserialize;

/// Position Syncer - syncs shadow positions with Binance positions
pub struct PositionSyncer {
    /// Binance executor for API calls
    #[allow(dead_code)]
    executor: BinanceExecutor,
}

impl PositionSyncer {
    /// Create a new PositionSyncer with a BinanceExecutor
    pub fn new(executor: BinanceExecutor) -> Self {
        Self { executor }
    }

    /// Fetch current positions from Binance
    ///
    /// For spot trading, positions are derived from account balances.
    /// Only returns non-zero balances.
    pub async fn fetch_binance_positions(&self) -> Result<Vec<BinancePosition>, SyncError> {
        #[cfg(feature = "real-api")]
        {
            self.fetch_real_positions().await
        }

        #[cfg(not(feature = "real-api"))]
        {
            self.fetch_mock_positions().await
        }
    }

    /// Fetch account balances from Binance
    pub async fn fetch_binance_balances(&self) -> Result<Vec<BinanceBalance>, SyncError> {
        #[cfg(feature = "real-api")]
        {
            self.fetch_real_balances().await
        }

        #[cfg(not(feature = "real-api"))]
        {
            self.fetch_mock_balances().await
        }
    }

    /// Compare shadow positions with Binance positions
    ///
    /// Returns a PositionDiff showing:
    /// - Positions only in shadow (orphaned shadow positions)
    /// - Positions only on Binance (orphaned exchange positions)
    /// - Positions with quantity mismatches
    /// - Matched positions
    pub fn compare_positions(
        &self,
        shadow: &[ShadowPositionInfo],
        binance: &[BinancePosition],
    ) -> PositionDiff {
        let mut diff = PositionDiff::new();

        // Convert shadow to a lookup map (symbol -> position)
        let mut shadow_map: std::collections::HashMap<String, &ShadowPositionInfo> =
            std::collections::HashMap::new();
        for pos in shadow {
            // Convert internal symbol to Binance format for comparison
            let binance_symbol = symbol::to_binance(&pos.symbol);
            shadow_map.insert(binance_symbol, pos);
        }

        // Track which shadow positions we've matched
        let mut matched_shadow_symbols: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Check each Binance position
        for binance_pos in binance {
            if let Some(shadow_pos) = shadow_map.get(&binance_pos.symbol) {
                matched_shadow_symbols.insert(binance_pos.symbol.clone());

                // Check if quantities match
                if shadow_pos.quantity == binance_pos.quantity
                    && shadow_pos.side == binance_pos.side
                {
                    // Perfect match
                    diff.matched.push(MatchedPosition {
                        symbol: shadow_pos.symbol.clone(),
                        side: shadow_pos.side,
                        quantity: shadow_pos.quantity,
                        shadow_entry_price: shadow_pos.entry_price,
                        binance_entry_price: binance_pos.entry_price,
                    });
                } else {
                    // Quantity or side mismatch
                    diff.quantity_mismatch.push(QuantityMismatch::new(
                        shadow_pos.symbol.clone(),
                        shadow_pos.side,
                        shadow_pos.quantity,
                        binance_pos.quantity,
                    ));
                }
            } else {
                // Position on Binance but not in shadow
                diff.binance_only.push(binance_pos.clone());
            }
        }

        // Find shadow positions not on Binance
        for pos in shadow {
            let binance_symbol = symbol::to_binance(&pos.symbol);
            if !matched_shadow_symbols.contains(&binance_symbol) {
                diff.shadow_only.push(pos.clone());
            }
        }

        diff
    }

    /// Perform a full sync and return the result
    ///
    /// This fetches Binance positions, compares them with shadow positions,
    /// and returns a SyncResult with the diff.
    pub async fn sync(&self, shadow_positions: &[ShadowPositionInfo]) -> SyncResult {
        match self.fetch_binance_positions().await {
            Ok(binance_positions) => {
                let diff = self.compare_positions(shadow_positions, &binance_positions);
                SyncResult::success(diff)
            }
            Err(e) => SyncResult::failure(&e.to_string()),
        }
    }

    // ==================== Mock implementations (for testing without real API) ====================

    #[cfg(not(feature = "real-api"))]
    async fn fetch_mock_positions(&self) -> Result<Vec<BinancePosition>, SyncError> {
        // Return empty positions for mock (no positions by default)
        Ok(Vec::new())
    }

    #[cfg(not(feature = "real-api"))]
    async fn fetch_mock_balances(&self) -> Result<Vec<BinanceBalance>, SyncError> {
        // Return mock balances for testing
        Ok(vec![BinanceBalance {
            asset: "USDT".to_string(),
            free: Decimal::from(10000),
            locked: Decimal::ZERO,
        }])
    }

    // ==================== Real API implementations ====================

    #[cfg(feature = "real-api")]
    async fn fetch_real_positions(&self) -> Result<Vec<BinancePosition>, SyncError> {
        // For spot, we derive "positions" from non-zero balances
        // This would need the authenticated account endpoint
        // GET /api/v3/account with timestamp and signature

        // For now, return empty - actual implementation would use self.executor
        // to make authenticated API calls
        Ok(Vec::new())
    }

    #[cfg(feature = "real-api")]
    async fn fetch_real_balances(&self) -> Result<Vec<BinanceBalance>, SyncError> {
        // GET /api/v3/account
        // Parse balances array
        Ok(Vec::new())
    }
}

/// Binance account API response structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct BinanceAccountResponse {
    balances: Vec<BinanceBalanceResponse>,
    #[serde(default)]
    can_trade: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BinanceBalanceResponse {
    asset: String,
    free: String,
    locked: String,
}

#[cfg(test)]
mod tests {
    use super::super::position_types::PositionSide;
    use super::*;
    use rust_decimal_macros::dec;

    // Helper to create a test executor
    fn create_test_executor() -> BinanceExecutor {
        BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string()).unwrap()
    }

    // Helper to create a shadow position
    fn shadow_pos(
        symbol: &str,
        side: PositionSide,
        qty: Decimal,
        entry: Decimal,
    ) -> ShadowPositionInfo {
        ShadowPositionInfo {
            symbol: symbol.to_string(),
            side,
            quantity: qty,
            entry_price: entry,
        }
    }

    // Helper to create a Binance position
    fn binance_pos(
        symbol: &str,
        side: PositionSide,
        qty: Decimal,
        entry: Decimal,
    ) -> BinancePosition {
        BinancePosition::new(symbol.to_string(), side, qty, entry)
    }

    // ==================== PositionSyncer Creation Tests ====================

    #[test]
    fn test_position_syncer_creation() {
        let executor = create_test_executor();
        let _syncer = PositionSyncer::new(executor);
        // Just verify it compiles and creates
    }

    // ==================== Fetch Positions Tests ====================

    #[tokio::test]
    async fn test_fetch_binance_positions_empty() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        let result = syncer.fetch_binance_positions().await;
        assert!(result.is_ok());

        let positions = result.unwrap();
        // Mock returns empty by default
        assert!(positions.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_binance_balances() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        let result = syncer.fetch_binance_balances().await;
        assert!(result.is_ok());

        let balances = result.unwrap();
        // Mock returns USDT balance
        assert!(!balances.is_empty());
        assert_eq!(balances[0].asset, "USDT");
    }

    // ==================== Compare Positions Tests - Match ====================

    #[test]
    fn test_compare_positions_perfect_match() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        let shadow = vec![shadow_pos(
            "BTC_USDC",
            PositionSide::Long,
            dec!(0.5),
            dec!(50000),
        )];
        let binance = vec![binance_pos(
            "BTCUSDT",
            PositionSide::Long,
            dec!(0.5),
            dec!(50000),
        )];

        let diff = syncer.compare_positions(&shadow, &binance);

        assert!(diff.is_synced());
        assert_eq!(diff.matched.len(), 1);
        assert_eq!(diff.matched[0].symbol, "BTC_USDC");
        assert_eq!(diff.matched[0].quantity, dec!(0.5));
    }

    #[test]
    fn test_compare_positions_multiple_matches() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        let shadow = vec![
            shadow_pos("BTC_USDC", PositionSide::Long, dec!(0.5), dec!(50000)),
            shadow_pos("ETH_USDC", PositionSide::Long, dec!(2.0), dec!(3000)),
        ];
        let binance = vec![
            binance_pos("BTCUSDT", PositionSide::Long, dec!(0.5), dec!(50000)),
            binance_pos("ETHUSDT", PositionSide::Long, dec!(2.0), dec!(3000)),
        ];

        let diff = syncer.compare_positions(&shadow, &binance);

        assert!(diff.is_synced());
        assert_eq!(diff.matched.len(), 2);
    }

    // ==================== Compare Positions Tests - Mismatch ====================

    #[test]
    fn test_compare_positions_quantity_mismatch() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        let shadow = vec![shadow_pos(
            "BTC_USDC",
            PositionSide::Long,
            dec!(0.5),
            dec!(50000),
        )];
        let binance = vec![binance_pos(
            "BTCUSDT",
            PositionSide::Long,
            dec!(0.3),
            dec!(50000),
        )];

        let diff = syncer.compare_positions(&shadow, &binance);

        assert!(!diff.is_synced());
        assert_eq!(diff.quantity_mismatch.len(), 1);
        assert_eq!(diff.quantity_mismatch[0].shadow_qty, dec!(0.5));
        assert_eq!(diff.quantity_mismatch[0].binance_qty, dec!(0.3));
        assert!(diff.quantity_mismatch[0].shadow_exceeds());
    }

    // ==================== Compare Positions Tests - Shadow Only ====================

    #[test]
    fn test_compare_positions_shadow_only() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        let shadow = vec![shadow_pos(
            "BTC_USDC",
            PositionSide::Long,
            dec!(0.5),
            dec!(50000),
        )];
        let binance: Vec<BinancePosition> = vec![];

        let diff = syncer.compare_positions(&shadow, &binance);

        assert!(!diff.is_synced());
        assert_eq!(diff.shadow_only.len(), 1);
        assert_eq!(diff.shadow_only[0].symbol, "BTC_USDC");
    }

    // ==================== Compare Positions Tests - Binance Only ====================

    #[test]
    fn test_compare_positions_binance_only() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        let shadow: Vec<ShadowPositionInfo> = vec![];
        let binance = vec![binance_pos(
            "BTCUSDT",
            PositionSide::Long,
            dec!(0.5),
            dec!(50000),
        )];

        let diff = syncer.compare_positions(&shadow, &binance);

        assert!(!diff.is_synced());
        assert_eq!(diff.binance_only.len(), 1);
        assert_eq!(diff.binance_only[0].symbol, "BTCUSDT");
    }

    // ==================== Compare Positions Tests - Mixed ====================

    #[test]
    fn test_compare_positions_mixed_discrepancies() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        let shadow = vec![
            shadow_pos("BTC_USDC", PositionSide::Long, dec!(0.5), dec!(50000)), // Match
            shadow_pos("ETH_USDC", PositionSide::Long, dec!(2.0), dec!(3000)),  // Shadow only
            shadow_pos("SOL_USDC", PositionSide::Long, dec!(10.0), dec!(100)),  // Mismatch
        ];
        let binance = vec![
            binance_pos("BTCUSDT", PositionSide::Long, dec!(0.5), dec!(50000)), // Match
            binance_pos("SOLUSDT", PositionSide::Long, dec!(8.0), dec!(100)),   // Mismatch
            binance_pos("BNBUSDT", PositionSide::Long, dec!(5.0), dec!(300)),   // Binance only
        ];

        let diff = syncer.compare_positions(&shadow, &binance);

        assert!(!diff.is_synced());
        assert_eq!(diff.matched.len(), 1); // BTC matches
        assert_eq!(diff.shadow_only.len(), 1); // ETH shadow only
        assert_eq!(diff.binance_only.len(), 1); // BNB binance only
        assert_eq!(diff.quantity_mismatch.len(), 1); // SOL mismatch
        assert_eq!(diff.discrepancy_count(), 3);
    }

    // ==================== Sync Tests ====================

    #[tokio::test]
    async fn test_sync_success_empty() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        let shadow: Vec<ShadowPositionInfo> = vec![];
        let result = syncer.sync(&shadow).await;

        assert!(result.success);
        assert!(result.diff.is_synced());
        assert!(result.error_message.is_none());
    }

    #[tokio::test]
    async fn test_sync_detects_shadow_only() {
        let executor = create_test_executor();
        let syncer = PositionSyncer::new(executor);

        // Shadow has a position, but mock Binance returns empty
        let shadow = vec![shadow_pos(
            "BTC_USDC",
            PositionSide::Long,
            dec!(0.5),
            dec!(50000),
        )];
        let result = syncer.sync(&shadow).await;

        assert!(result.success);
        assert!(!result.diff.is_synced());
        assert_eq!(result.diff.shadow_only.len(), 1);
    }
}
