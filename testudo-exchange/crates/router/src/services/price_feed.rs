//! Price Feed Service
//!
//! Background service that polls live Binance ticker data for symbols with
//! open orders and feeds prices to the Shadow Engine's `process_price_update()`.
//!
//! # Architecture (008-shadow-fill-engine)
//!
//! ```text
//! Binance REST API (existing BinanceDataService)
//!     │ get_ticker() every 2 seconds
//!     │
//! ┌───▼──────────────────────────┐
//! │ PriceFeedService (this)       │
//! │ - Queries active symbols      │
//! │ - Polls tickers               │
//! │ - Calls process_price_update()│
//! └───┬──────────────────────────┘
//!     │
//! ┌───▼──────────────────────────┐
//! │ ShadowEngine (existing)       │
//! │ - process_price_update()      │
//! │ - Fill matching (3-phase RCW) │
//! │ - Auto SL/TP creation         │
//! └──────────────────────────────┘
//! ```

// @anchor exchange:router:price_feed
// @tags api

use crate::services::trade_manager::service::TradeManagerService;
use common_utils::services::binance_data::BinanceDataService;
use engine::EngineHandle;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// A price tick broadcast to subscribers (e.g., TradeManagerService).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTick {
    pub symbol: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub high: Decimal,
    pub low: Decimal,
}

/// Background service that feeds live prices to the Shadow Engine.
///
/// Polls Binance ticker data for symbols with open orders and calls
/// `process_price_update()` to trigger order fills.
///
/// Also broadcasts `PriceTick` events for subscribers like TradeManagerService.
pub struct PriceFeedService {
    engine_handle: EngineHandle,
    binance: BinanceDataService,
    poll_interval: Duration,
    price_tx: broadcast::Sender<PriceTick>,
    /// FR-2 (016): Optional live trade manager for polling live-only symbols.
    live_trade_manager: Option<Arc<TradeManagerService>>,
}

impl PriceFeedService {
    /// Create a new PriceFeedService.
    ///
    /// # Arguments
    /// * `engine_handle` - Handle to the ShadowEngine actor
    /// * `binance` - Binance data service for fetching tickers
    /// * `poll_interval` - How often to poll for price updates
    pub fn new(
        engine_handle: EngineHandle,
        binance: BinanceDataService,
        poll_interval: Duration,
    ) -> Self {
        let (price_tx, _) = broadcast::channel(1024);
        Self {
            engine_handle,
            binance,
            poll_interval,
            price_tx,
            live_trade_manager: None,
        }
    }

    /// Create with default 2-second poll interval.
    pub fn with_defaults(engine_handle: EngineHandle) -> Self {
        Self::new(engine_handle, BinanceDataService::new(), Duration::from_secs(2))
    }

    /// FR-2 (016): Set live trade manager for polling symbols with live-only positions.
    pub fn with_live_trade_manager(mut self, tm: Arc<TradeManagerService>) -> Self {
        self.live_trade_manager = Some(tm);
        self
    }

    /// Subscribe to price tick broadcasts.
    pub fn subscribe(&self) -> broadcast::Receiver<PriceTick> {
        self.price_tx.subscribe()
    }

    /// Run the price feed loop until cancellation.
    ///
    /// Polls active symbols, fetches tickers, and calls `process_price_update()`.
    /// Exits gracefully when the cancellation token is triggered.
    pub async fn run(&self, shutdown: CancellationToken) {
        tracing::info!(
            "PriceFeedService started (poll_interval={}ms)",
            self.poll_interval.as_millis()
        );

        loop {
            self.tick().await;
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("PriceFeedService shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.poll_interval) => {}
            }
        }
    }

    /// Execute a single tick of the price feed.
    ///
    /// 019d: Uses fire-and-forget `push_price()` — fills are emitted to the
    /// actor's fill event channel and handled by FillDetectorService.
    ///
    /// Separated from `run()` for testability.
    pub async fn tick(&self) {
        // FR-2 (016): Merge symbols from shadow engine AND live trade manager
        let mut symbol_set: HashSet<String> =
            self.engine_handle.get_active_symbols().await.into_iter().collect();

        // Include live-only symbols so pending/filled live trades receive price ticks
        if let Some(ref live_tm) = self.live_trade_manager {
            let live_symbols = live_tm.get_active_symbols().await;
            symbol_set.extend(live_symbols);
        }

        let symbols: Vec<String> = symbol_set.into_iter().collect();

        if symbols.is_empty() {
            return;
        }

        tracing::debug!("PriceFeedService polling {} symbols", symbols.len());

        for symbol in &symbols {
            match self.binance.get_ticker(symbol).await {
                Ok(ticker) => {
                    let bid = ticker.bid.unwrap_or(Decimal::ZERO);
                    let ask = ticker.ask.unwrap_or(Decimal::ZERO);
                    let high = ticker.high.unwrap_or(ask);
                    let low = ticker.low.unwrap_or(bid);

                    // Skip if we got no meaningful price data
                    if bid == Decimal::ZERO && ask == Decimal::ZERO {
                        tracing::warn!("PriceFeedService: no price data for {}", symbol);
                        continue;
                    }

                    // 019d: Fire-and-forget — fills go to actor's fill event channel
                    if let Err(e) = self
                        .engine_handle
                        .push_price(symbol.clone(), bid, ask, high, low)
                        .await
                    {
                        tracing::warn!(
                            "PriceFeedService: push_price failed for {}: {}",
                            symbol,
                            e
                        );
                    }

                    // Broadcast price tick to subscribers (e.g., TradeManagerService)
                    let _ = self.price_tx.send(PriceTick {
                        symbol: symbol.clone(),
                        bid,
                        ask,
                        high,
                        low,
                    });
                }
                Err(e) => {
                    // FR-5: Graceful degradation - log and continue
                    tracing::warn!("PriceFeedService: ticker error for {}: {}", symbol, e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{EngineActor, ShadowEngine, ShadowOrder, ShadowOrderStatus};
    use rust_decimal_macros::dec;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    /// Helper: Create a risk-validated order for testing
    fn validated_order(mut order: ShadowOrder) -> ShadowOrder {
        order.mark_risk_validated();
        order
    }

    #[tokio::test]
    async fn test_price_feed_tick_no_orders() {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        let binance = BinanceDataService::new();
        let service = PriceFeedService::new(handle, binance, Duration::from_secs(2));

        // Should not panic when no orders exist
        service.tick().await;
    }

    #[tokio::test]
    async fn test_price_feed_creation() {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        let service = PriceFeedService::with_defaults(handle);
        assert_eq!(service.poll_interval, Duration::from_secs(2));
    }

    /// Integration test: place an order, simulate price feed tick with mock,
    /// verify order fills. This test uses a mock Binance server.
    #[tokio::test]
    async fn test_price_feed_fills_order_via_mock() {
        // Set up a mock HTTP server that returns ticker data
        let mut mock_server = mockito::Server::new_async().await;
        let mock_url = mock_server.url();

        // Mock the ticker endpoint
        let ticker_mock = mock_server
            .mock("GET", "/fapi/v1/ticker/24hr")
            .match_query(mockito::Matcher::UrlEncoded(
                "symbol".into(),
                "BTCUSDT".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "symbol": "BTCUSDT",
                    "priceChange": "100",
                    "priceChangePercent": "0.2",
                    "weightedAvgPrice": "50000",
                    "lastPrice": "49900",
                    "openPrice": "49800",
                    "highPrice": "50200",
                    "lowPrice": "49800",
                    "volume": "1000",
                    "quoteVolume": "50000000"
                }"#,
            )
            .create_async()
            .await;

        // Mock the depth endpoint (for bid/ask)
        // ask=49900 is below the limit buy at 50000, so it should fill
        let depth_mock = mock_server
            .mock("GET", "/fapi/v1/depth")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("symbol".into(), "BTCUSDT".into()),
                mockito::Matcher::UrlEncoded("limit".into(), "5".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "lastUpdateId": 1,
                    "bids": [["49800", "1.0"]],
                    "asks": [["49900", "1.0"]]
                }"#,
            )
            .create_async()
            .await;

        // Set up engine with an order via EngineHandle
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        // Place a limit buy at 50000 - ask of 49900 should fill this
        let order = validated_order(ShadowOrder::limit_buy(
            user_id,
            "BTC_USDT",
            dec!(0.01),
            dec!(50000),
        ));
        handle.place_order(user_id, order).await.unwrap();

        // Verify order is open
        let open = handle.get_open_orders(user_id).await;
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].status, ShadowOrderStatus::Open);

        // Create service with mock binance — uses the same handle
        let binance = BinanceDataService::with_base_url(&mock_url);
        let service = PriceFeedService::new(handle.clone(), binance, Duration::from_secs(2));

        // Run one tick (fire-and-forget push_price)
        service.tick().await;

        // Give the actor a moment to process the fire-and-forget command
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Verify order was filled
        let open = handle.get_open_orders(user_id).await;
        assert_eq!(
            open.len(),
            0,
            "Order should be filled after price feed tick"
        );

        // Verify mocks were called
        ticker_mock.assert_async().await;
        depth_mock.assert_async().await;
    }
}
