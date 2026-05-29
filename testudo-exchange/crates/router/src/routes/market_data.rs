//! Market Data Routes
//!
//! Provides live market data from Binance via the BinanceDataService.
//! These endpoints are unauthenticated and serve as the primary source
//! for real-time pricing data in the hybrid trading system.
//!
//! # Endpoints
//! - GET /market-data/ticker?symbol=BTC_USDC
//! - GET /market-data/orderbook?symbol=BTC_USDC&limit=20
//! - GET /market-data/klines?symbol=BTC_USDC&interval=1h&limit=100
//! - GET /market-data/markets

// @anchor exchange:router:market_data
// @tags api

use actix_web::{web, HttpResponse};
use common_utils::columnar::ColumnarOrderBook;
use common_utils::services::BinanceDataService;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// Shared state for market data routes
pub struct MarketDataState {
    pub binance_service: Arc<BinanceDataService>,
}

impl MarketDataState {
    pub fn new() -> Self {
        Self {
            binance_service: Arc::new(BinanceDataService::new()),
        }
    }
}

impl Default for MarketDataState {
    fn default() -> Self {
        Self::new()
    }
}

/// Query params for ticker endpoint
#[derive(Debug, Deserialize)]
pub struct TickerQuery {
    pub symbol: String,
}

/// Query params for orderbook endpoint
#[derive(Debug, Deserialize)]
pub struct OrderbookQuery {
    pub symbol: String,
    pub limit: Option<i32>,
}

/// Query params for klines endpoint
#[derive(Debug, Deserialize)]
pub struct KlinesQuery {
    pub symbol: String,
    pub interval: Option<String>,
    pub limit: Option<i32>,
}

/// Response wrapper for market data
#[derive(Debug, Serialize)]
pub struct MarketDataResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub latency_ms: u64,
}

impl<T> MarketDataResponse<T> {
    pub fn success(data: T, latency_ms: u64) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            latency_ms,
        }
    }

    pub fn error(message: String, latency_ms: u64) -> MarketDataResponse<T> {
        MarketDataResponse {
            success: false,
            data: None,
            error: Some(message),
            latency_ms,
        }
    }
}

/// GET /market-data/ticker?symbol=BTC_USDC
///
/// Fetches 24hr ticker data from Binance.
pub async fn get_ticker(
    query: web::Query<TickerQuery>,
    state: web::Data<MarketDataState>,
) -> HttpResponse {
    let start = Instant::now();
    let symbol = &query.symbol;

    tracing::debug!("Fetching ticker for symbol: {}", symbol);

    match state.binance_service.get_ticker(symbol).await {
        Ok(ticker) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::debug!("Ticker fetched in {}ms", latency);
            HttpResponse::Ok().json(MarketDataResponse::success(ticker, latency))
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::error!("Failed to fetch ticker: {:?}", e);
            HttpResponse::InternalServerError()
                .json(MarketDataResponse::<()>::error(e.to_string(), latency))
        }
    }
}

/// GET /market-data/orderbook?symbol=BTC_USDC&limit=20
///
/// Fetches order book depth from Binance.
pub async fn get_orderbook(
    query: web::Query<OrderbookQuery>,
    state: web::Data<MarketDataState>,
) -> HttpResponse {
    let start = Instant::now();
    let symbol = &query.symbol;
    let limit = query.limit;

    tracing::debug!(
        "Fetching orderbook for symbol: {}, limit: {:?}",
        symbol,
        limit
    );

    match state.binance_service.get_orderbook(symbol, limit).await {
        Ok(orderbook) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::debug!("Orderbook fetched in {}ms", latency);
            HttpResponse::Ok().json(MarketDataResponse::success(orderbook, latency))
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::error!("Failed to fetch orderbook: {:?}", e);
            HttpResponse::InternalServerError()
                .json(MarketDataResponse::<()>::error(e.to_string(), latency))
        }
    }
}

/// GET /v2/market-data/orderbook?symbol=BTC_USDC&limit=20
///
/// Fetches order book depth from Binance in columnar format.
/// Returns ~25% smaller JSON payload compared to row-based format.
pub async fn get_orderbook_columnar(
    query: web::Query<OrderbookQuery>,
    state: web::Data<MarketDataState>,
) -> HttpResponse {
    let start = Instant::now();
    let symbol = &query.symbol;
    let limit = query.limit;

    tracing::debug!(
        "Fetching columnar orderbook for symbol: {}, limit: {:?}",
        symbol,
        limit
    );

    match state.binance_service.get_orderbook(symbol, limit).await {
        Ok(orderbook) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::debug!("Columnar orderbook fetched in {}ms", latency);

            // Convert to columnar format
            let columnar = ColumnarOrderBook::from_ccxt(
                orderbook.symbol,
                &orderbook.bids,
                &orderbook.asks,
                orderbook.timestamp,
                orderbook.nonce,
            );

            HttpResponse::Ok().json(MarketDataResponse::success(columnar, latency))
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::error!("Failed to fetch columnar orderbook: {:?}", e);
            HttpResponse::InternalServerError()
                .json(MarketDataResponse::<()>::error(e.to_string(), latency))
        }
    }
}

/// GET /market-data/klines?symbol=BTC_USDC&interval=1h&limit=100
///
/// Fetches candlestick/kline data from Binance.
pub async fn get_klines(
    query: web::Query<KlinesQuery>,
    state: web::Data<MarketDataState>,
) -> HttpResponse {
    let start = Instant::now();
    let symbol = &query.symbol;
    let interval = query.interval.as_deref().unwrap_or("1h");
    let limit = query.limit;

    tracing::debug!(
        "Fetching klines for symbol: {}, interval: {}, limit: {:?}",
        symbol,
        interval,
        limit
    );

    match state
        .binance_service
        .get_klines(symbol, interval, limit)
        .await
    {
        Ok(candles) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::debug!(
                "Klines fetched in {}ms ({} candles)",
                latency,
                candles.len()
            );
            HttpResponse::Ok().json(MarketDataResponse::success(candles, latency))
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::error!("Failed to fetch klines: {:?}", e);
            HttpResponse::InternalServerError()
                .json(MarketDataResponse::<()>::error(e.to_string(), latency))
        }
    }
}

/// GET /market-data/markets
///
/// Fetches list of supported trading pairs from Binance.
pub async fn get_markets(state: web::Data<MarketDataState>) -> HttpResponse {
    let start = Instant::now();

    tracing::debug!("Fetching available markets");

    match state.binance_service.get_markets().await {
        Ok(markets) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::debug!("Markets fetched in {}ms ({} pairs)", latency, markets.len());
            HttpResponse::Ok().json(MarketDataResponse::success(markets, latency))
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::error!("Failed to fetch markets: {:?}", e);
            HttpResponse::InternalServerError()
                .json(MarketDataResponse::<()>::error(e.to_string(), latency))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_data_state_default() {
        let state = MarketDataState::default();
        assert!(Arc::strong_count(&state.binance_service) == 1);
    }

    #[test]
    fn test_response_success() {
        let response: MarketDataResponse<&str> = MarketDataResponse::success("test", 100);
        assert!(response.success);
        assert_eq!(response.data, Some("test"));
        assert!(response.error.is_none());
        assert_eq!(response.latency_ms, 100);
    }

    #[test]
    fn test_response_error() {
        let response: MarketDataResponse<()> =
            MarketDataResponse::error("test error".to_string(), 50);
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
        assert_eq!(response.latency_ms, 50);
    }
}
