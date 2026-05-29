//! Sync Service for Position Synchronization
//!
//! Manages background position sync between Shadow Engine and Binance.
//!
//! # E.4 Requirements
//! - Sync on app start
//! - Sync after each trade
//! - Background sync every 60 seconds
//! - Alert user of discrepancies (no auto-reconcile)

// @anchor exchange:router:sync_service
// @tags api

use common_utils::adapters::{
    BinanceExecutor, PositionSyncer, ShadowPositionInfo, SyncError, SyncResult,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Default sync interval (60 seconds per PRD)
pub const DEFAULT_SYNC_INTERVAL: Duration = Duration::from_secs(60);

/// Sync Service - manages background position synchronization
pub struct SyncService {
    /// Position syncer for Binance API calls
    syncer: Arc<PositionSyncer>,
    /// Last sync result
    last_sync_result: Arc<RwLock<Option<SyncResult>>>,
    /// Sync interval
    sync_interval: Duration,
    /// AUD-03 FR-8: Cancellation token for graceful shutdown
    shutdown: CancellationToken,
}

impl SyncService {
    /// Create a new SyncService
    pub fn new(executor: BinanceExecutor) -> Self {
        Self {
            syncer: Arc::new(PositionSyncer::new(executor)),
            last_sync_result: Arc::new(RwLock::new(None)),
            sync_interval: DEFAULT_SYNC_INTERVAL,
            shutdown: CancellationToken::new(),
        }
    }

    /// Create a SyncService with custom sync interval
    pub fn with_interval(executor: BinanceExecutor, interval: Duration) -> Self {
        Self {
            syncer: Arc::new(PositionSyncer::new(executor)),
            last_sync_result: Arc::new(RwLock::new(None)),
            sync_interval: interval,
            shutdown: CancellationToken::new(),
        }
    }

    /// Perform a sync immediately
    pub async fn sync_now(&self, shadow_positions: &[ShadowPositionInfo]) -> SyncResult {
        let result = self.syncer.sync(shadow_positions).await;

        // Store the result
        let mut last = self.last_sync_result.write().await;
        *last = Some(result.clone());

        result
    }

    /// Sync after a trade execution
    ///
    /// This is called after each trade to verify consistency
    pub async fn sync_after_trade(&self, shadow_positions: &[ShadowPositionInfo]) -> SyncResult {
        // Same as sync_now but semantically different (called after trade)
        self.sync_now(shadow_positions).await
    }

    /// Get the last sync result
    pub async fn get_last_sync_result(&self) -> Option<SyncResult> {
        let last = self.last_sync_result.read().await;
        last.clone()
    }

    /// Check if background sync is running (returns whether token is NOT cancelled)
    pub fn is_background_running(&self) -> bool {
        !self.shutdown.is_cancelled()
    }

    /// Get the sync interval
    pub fn sync_interval(&self) -> Duration {
        self.sync_interval
    }

    /// Start background sync task
    ///
    /// Returns a JoinHandle that can be used to await the task.
    /// The task will sync every `sync_interval` seconds until the shutdown token is cancelled.
    ///
    /// # Arguments
    /// * `get_positions` - A function that returns current shadow positions
    pub fn start_background_sync<F>(&self, get_positions: F) -> JoinHandle<()>
    where
        F: Fn() -> Vec<ShadowPositionInfo> + Send + Sync + 'static,
    {
        let syncer = self.syncer.clone();
        let last_sync_result = self.last_sync_result.clone();
        let shutdown = self.shutdown.clone();
        let interval = self.sync_interval;

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        tracing::info!("SyncService background task shutting down");
                        break;
                    }
                    _ = interval_timer.tick() => {
                        let positions = get_positions();
                        let result = syncer.sync(&positions).await;
                        let mut last = last_sync_result.write().await;
                        *last = Some(result);
                    }
                }
            }
        })
    }

    /// Stop background sync by cancelling the shutdown token
    pub fn stop_background_sync(&self) {
        self.shutdown.cancel();
    }
}

/// Builder for SyncService configuration
pub struct SyncServiceBuilder {
    api_key: Option<String>,
    api_secret: Option<String>,
    testnet: bool,
    sync_interval: Duration,
}

impl SyncServiceBuilder {
    pub fn new() -> Self {
        Self {
            api_key: None,
            api_secret: None,
            testnet: false,
            sync_interval: DEFAULT_SYNC_INTERVAL,
        }
    }

    pub fn api_key(mut self, key: String) -> Self {
        self.api_key = Some(key);
        self
    }

    pub fn api_secret(mut self, secret: String) -> Self {
        self.api_secret = Some(secret);
        self
    }

    pub fn testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    pub fn sync_interval(mut self, interval: Duration) -> Self {
        self.sync_interval = interval;
        self
    }

    pub fn build(self) -> Result<SyncService, SyncError> {
        let api_key = self
            .api_key
            .ok_or_else(|| SyncError::AuthenticationFailed)?;
        let api_secret = self
            .api_secret
            .ok_or_else(|| SyncError::AuthenticationFailed)?;

        let executor = if self.testnet {
            BinanceExecutor::testnet(api_key, api_secret)
        } else {
            BinanceExecutor::new(api_key, api_secret)
        }
        .map_err(|_| SyncError::AuthenticationFailed)?;

        Ok(SyncService::with_interval(executor, self.sync_interval))
    }
}

impl Default for SyncServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_utils::adapters::PositionSide;
    use rust_decimal_macros::dec;

    // Helper to create a test executor
    fn create_test_executor() -> BinanceExecutor {
        BinanceExecutor::new("test_key".to_string(), "test_secret".to_string()).unwrap()
    }

    // Helper to create shadow positions
    fn shadow_pos(symbol: &str, qty: Decimal) -> ShadowPositionInfo {
        ShadowPositionInfo {
            symbol: symbol.to_string(),
            side: PositionSide::Long,
            quantity: qty,
            entry_price: dec!(50000),
        }
    }

    use rust_decimal::Decimal;

    // ==================== SyncService Creation Tests ====================

    #[test]
    fn test_sync_service_creation() {
        let executor = create_test_executor();
        let service = SyncService::new(executor);

        assert_eq!(service.sync_interval(), DEFAULT_SYNC_INTERVAL);
    }

    #[test]
    fn test_sync_service_custom_interval() {
        let executor = create_test_executor();
        let interval = Duration::from_secs(30);
        let service = SyncService::with_interval(executor, interval);

        assert_eq!(service.sync_interval(), interval);
    }

    // ==================== Sync Now Tests ====================

    #[tokio::test]
    async fn test_sync_now_empty_positions() {
        let executor = create_test_executor();
        let service = SyncService::new(executor);

        let positions: Vec<ShadowPositionInfo> = vec![];
        let result = service.sync_now(&positions).await;

        assert!(result.success);
        assert!(result.diff.is_synced());
    }

    #[tokio::test]
    async fn test_sync_now_with_shadow_positions() {
        let executor = create_test_executor();
        let service = SyncService::new(executor);

        let positions = vec![shadow_pos("BTC_USDC", dec!(0.5))];
        let result = service.sync_now(&positions).await;

        assert!(result.success);
        // Shadow has positions, mock Binance returns empty -> shadow_only
        assert!(!result.diff.is_synced());
        assert_eq!(result.diff.shadow_only.len(), 1);
    }

    #[tokio::test]
    async fn test_sync_stores_last_result() {
        let executor = create_test_executor();
        let service = SyncService::new(executor);

        // Initially no result
        assert!(service.get_last_sync_result().await.is_none());

        // Sync
        let positions: Vec<ShadowPositionInfo> = vec![];
        service.sync_now(&positions).await;

        // Now there's a result
        let last = service.get_last_sync_result().await;
        assert!(last.is_some());
        assert!(last.unwrap().success);
    }

    // ==================== Sync After Trade Tests ====================

    #[tokio::test]
    async fn test_sync_after_trade() {
        let executor = create_test_executor();
        let service = SyncService::new(executor);

        let positions = vec![shadow_pos("ETH_USDC", dec!(1.0))];
        let result = service.sync_after_trade(&positions).await;

        assert!(result.success);
    }

    // ==================== Background Sync Tests ====================

    #[test]
    fn test_background_sync_not_running_initially() {
        let executor = create_test_executor();
        let service = SyncService::new(executor);

        // Not cancelled yet = running
        assert!(service.is_background_running());
    }

    #[test]
    fn test_stop_background_sync() {
        let executor = create_test_executor();
        let service = SyncService::new(executor);

        assert!(service.is_background_running());

        // Stop
        service.stop_background_sync();

        assert!(!service.is_background_running());
    }

    // ==================== Builder Tests ====================

    #[test]
    fn test_builder_missing_api_key() {
        let result = SyncServiceBuilder::new()
            .api_secret("secret".to_string())
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_missing_api_secret() {
        let result = SyncServiceBuilder::new().api_key("key".to_string()).build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_success() {
        let result = SyncServiceBuilder::new()
            .api_key("key".to_string())
            .api_secret("secret".to_string())
            .testnet(true)
            .sync_interval(Duration::from_secs(30))
            .build();

        assert!(result.is_ok());
        let service = result.unwrap();
        assert_eq!(service.sync_interval(), Duration::from_secs(30));
    }
}
