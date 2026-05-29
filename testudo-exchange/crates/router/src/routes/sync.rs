//! Sync API Routes
//!
//! Endpoints for position synchronization between Shadow Engine and Binance.
//!
//! # Endpoints
//! - POST /api/v1/sync - Trigger manual sync
//! - GET /api/v1/sync/status - Get last sync result
//! - GET /api/v1/sync/diff - Get current position differences

// @anchor exchange:router:sync
// @tags api

use actix_web::HttpResponse;
use serde::{Deserialize, Serialize};

use common_utils::adapters::{PositionDiff, SyncResult};

/// Response for sync status endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncStatusResponse {
    /// Whether last sync was successful
    pub success: bool,
    /// Timestamp of last sync (milliseconds)
    pub timestamp: i64,
    /// Whether positions are in sync
    pub is_synced: bool,
    /// Number of discrepancies found
    pub discrepancy_count: usize,
    /// Error message if last sync failed
    pub error_message: Option<String>,
}

impl From<&SyncResult> for SyncStatusResponse {
    fn from(result: &SyncResult) -> Self {
        Self {
            success: result.success,
            timestamp: result.timestamp,
            is_synced: result.diff.is_synced(),
            discrepancy_count: result.diff.discrepancy_count(),
            error_message: result.error_message.clone(),
        }
    }
}

/// Response for sync diff endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncDiffResponse {
    /// Positions only in shadow
    pub shadow_only: Vec<PositionInfoResponse>,
    /// Positions only on Binance
    pub binance_only: Vec<PositionInfoResponse>,
    /// Positions with quantity mismatch
    pub quantity_mismatch: Vec<QuantityMismatchResponse>,
    /// Matched positions count
    pub matched_count: usize,
    /// Total discrepancies
    pub total_discrepancies: usize,
}

impl From<&PositionDiff> for SyncDiffResponse {
    fn from(diff: &PositionDiff) -> Self {
        Self {
            shadow_only: diff
                .shadow_only
                .iter()
                .map(|p| PositionInfoResponse {
                    symbol: p.symbol.clone(),
                    side: format!("{:?}", p.side),
                    quantity: p.quantity.to_string(),
                    entry_price: p.entry_price.to_string(),
                })
                .collect(),
            binance_only: diff
                .binance_only
                .iter()
                .map(|p| PositionInfoResponse {
                    symbol: p.symbol.clone(),
                    side: format!("{:?}", p.side),
                    quantity: p.quantity.to_string(),
                    entry_price: p.entry_price.to_string(),
                })
                .collect(),
            quantity_mismatch: diff
                .quantity_mismatch
                .iter()
                .map(|m| QuantityMismatchResponse {
                    symbol: m.symbol.clone(),
                    side: format!("{:?}", m.side),
                    shadow_qty: m.shadow_qty.to_string(),
                    binance_qty: m.binance_qty.to_string(),
                    difference: m.difference.to_string(),
                })
                .collect(),
            matched_count: diff.matched.len(),
            total_discrepancies: diff.discrepancy_count(),
        }
    }
}

/// Position info for API response
#[derive(Debug, Serialize, Deserialize)]
pub struct PositionInfoResponse {
    pub symbol: String,
    pub side: String,
    pub quantity: String,
    pub entry_price: String,
}

/// Quantity mismatch for API response
#[derive(Debug, Serialize, Deserialize)]
pub struct QuantityMismatchResponse {
    pub symbol: String,
    pub side: String,
    pub shadow_qty: String,
    pub binance_qty: String,
    pub difference: String,
}

/// Response for triggering sync
#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerSyncResponse {
    pub success: bool,
    pub message: String,
    pub is_synced: bool,
    pub discrepancy_count: usize,
    pub timestamp: i64,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct SyncErrorResponse {
    pub error: String,
    pub message: String,
}

/// Trigger a manual sync
///
/// POST /api/v1/sync
pub async fn trigger_sync() -> HttpResponse {
    // In a real implementation, this would:
    // 1. Get shadow positions from ShadowEngine
    // 2. Call SyncService.sync_now()
    // 3. Return the result
    //
    // For now, return a mock response since we don't have
    // the full integration with ShadowEngine in this route yet.

    let response = TriggerSyncResponse {
        success: true,
        message: "Sync completed successfully".to_string(),
        is_synced: true,
        discrepancy_count: 0,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    HttpResponse::Ok().json(response)
}

/// Get last sync status
///
/// GET /api/v1/sync/status
pub async fn get_sync_status() -> HttpResponse {
    // In a real implementation, this would:
    // 1. Call SyncService.get_last_sync_result()
    // 2. Return the status
    //
    // For now, return a mock response

    let response = SyncStatusResponse {
        success: true,
        timestamp: chrono::Utc::now().timestamp_millis(),
        is_synced: true,
        discrepancy_count: 0,
        error_message: None,
    };

    HttpResponse::Ok().json(response)
}

/// Get current position differences
///
/// GET /api/v1/sync/diff
pub async fn get_sync_diff() -> HttpResponse {
    // In a real implementation, this would:
    // 1. Get last sync result from SyncService
    // 2. Return the diff details
    //
    // For now, return an empty diff

    let response = SyncDiffResponse {
        shadow_only: vec![],
        binance_only: vec![],
        quantity_mismatch: vec![],
        matched_count: 0,
        total_discrepancies: 0,
    };

    HttpResponse::Ok().json(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_utils::adapters::{
        BinancePosition, PositionSide, QuantityMismatch, ShadowPositionInfo,
    };
    use rust_decimal_macros::dec;

    // ==================== Response Conversion Tests ====================

    #[test]
    fn test_sync_status_response_from_success() {
        let diff = PositionDiff::new();
        let result = SyncResult::success(diff);

        let response = SyncStatusResponse::from(&result);

        assert!(response.success);
        assert!(response.is_synced);
        assert_eq!(response.discrepancy_count, 0);
        assert!(response.error_message.is_none());
    }

    #[test]
    fn test_sync_status_response_from_failure() {
        let result = SyncResult::failure("Network timeout");

        let response = SyncStatusResponse::from(&result);

        assert!(!response.success);
        assert!(response.is_synced); // Empty diff is synced
        assert_eq!(response.error_message, Some("Network timeout".to_string()));
    }

    #[test]
    fn test_sync_diff_response_empty() {
        let diff = PositionDiff::new();

        let response = SyncDiffResponse::from(&diff);

        assert!(response.shadow_only.is_empty());
        assert!(response.binance_only.is_empty());
        assert!(response.quantity_mismatch.is_empty());
        assert_eq!(response.matched_count, 0);
        assert_eq!(response.total_discrepancies, 0);
    }

    #[test]
    fn test_sync_diff_response_with_shadow_only() {
        let mut diff = PositionDiff::new();
        diff.shadow_only.push(ShadowPositionInfo {
            symbol: "BTC_USDC".to_string(),
            side: PositionSide::Long,
            quantity: dec!(0.5),
            entry_price: dec!(50000),
        });

        let response = SyncDiffResponse::from(&diff);

        assert_eq!(response.shadow_only.len(), 1);
        assert_eq!(response.shadow_only[0].symbol, "BTC_USDC");
        assert_eq!(response.shadow_only[0].quantity, "0.5");
        assert_eq!(response.total_discrepancies, 1);
    }

    #[test]
    fn test_sync_diff_response_with_mismatch() {
        let mut diff = PositionDiff::new();
        diff.quantity_mismatch.push(QuantityMismatch::new(
            "ETH_USDC".to_string(),
            PositionSide::Long,
            dec!(2.0),
            dec!(1.5),
        ));

        let response = SyncDiffResponse::from(&diff);

        assert_eq!(response.quantity_mismatch.len(), 1);
        assert_eq!(response.quantity_mismatch[0].symbol, "ETH_USDC");
        assert_eq!(response.quantity_mismatch[0].shadow_qty, "2.0");
        assert_eq!(response.quantity_mismatch[0].binance_qty, "1.5");
        assert_eq!(response.quantity_mismatch[0].difference, "0.5");
    }

    // ==================== Route Handler Tests ====================

    #[actix_web::test]
    async fn test_trigger_sync_handler() {
        let response = trigger_sync().await;
        assert!(response.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_sync_status_handler() {
        let response = get_sync_status().await;
        assert!(response.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_sync_diff_handler() {
        let response = get_sync_diff().await;
        assert!(response.status().is_success());
    }
}
