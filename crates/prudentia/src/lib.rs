//! Prudentia - Exchange Integration Adapters
//!
//! This crate provides wisdom in external communications through exchange adapters
//! with failover, circuit breaker patterns, and comprehensive error handling.
//! Primary focus on Binance integration with extensible architecture for additional exchanges.
//!
//! ## Supported Exchanges
//!
//! - **Binance**: Primary exchange with full feature support
//! - **Bybit**: Secondary exchange (future implementation)
//! - **Coinbase Pro**: Tertiary exchange (future implementation)
//!
//! ## Architecture Patterns
//!
//! - **Circuit Breaker**: Automatic failover when exchange becomes unresponsive
//! - **Rate Limiting**: Respect exchange API limits with intelligent backoff
//! - **Connection Pooling**: Efficient connection management for high throughput
//! - **Failover Chain**: Automatic fallback to secondary exchanges
//!
//! ## Roman Military Principle: Prudentia
//!
//! Risk-aware decision making in external communications. Every exchange interaction
//! is monitored, validated, and prepared for failure with systematic recovery procedures.

pub mod adapters;
pub mod binance;
pub mod circuit_breaker;
pub mod rate_limiter;
pub mod failover;
pub mod types;
pub mod websocket;

pub use adapters::{ExchangeAdapter, ExchangeAdapterTrait};
pub use binance::BinanceAdapter;
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerState};
pub use rate_limiter::ExchangeRateLimiter;
pub use failover::{FailoverManager, ExchangeFailoverConfig};
pub use types::{
    ExchangeOrder, OrderStatus, OrderType, TimeInForce,
    MarketData, OrderBook, Trade, Balance, ExchangeInfo
};

use disciplina::{AccountEquity, PositionSize, PricePoint};
use rust_decimal::Decimal;
use std::time::Duration;
use thiserror::Error;

/// Prudentia exchange integration errors
#[derive(Debug, Error, Clone)]
pub enum PrudentiaError {
    #[error("Exchange connection failed: {exchange} - {reason}")]
    ConnectionFailure { exchange: String, reason: String },
    
    #[error("Rate limit exceeded: {exchange} - retry after {retry_after:?}")]
    RateLimitExceeded { exchange: String, retry_after: Duration },
    
    #[error("Order placement failed: {reason}")]
    OrderPlacementFailure { reason: String },
    
    #[error("Market data unavailable: {symbol} from {exchange}")]
    MarketDataUnavailable { symbol: String, exchange: String },
    
    #[error("Authentication failed: {exchange} - check API credentials")]
    AuthenticationFailure { exchange: String },
    
    #[error("Circuit breaker open: {exchange} - exchanges unavailable")]
    CircuitBreakerOpen { exchange: String },
    
    #[error("Insufficient balance: required={required}, available={available}")]
    InsufficientBalance { required: Decimal, available: Decimal },
    
    #[error("Exchange API error: {code} - {message}")]
    ApiError { code: i32, message: String },
    
    #[error("WebSocket connection lost: {exchange}")]
    WebSocketDisconnected { exchange: String },
}

/// Result type for all Prudentia operations
pub type Result<T> = std::result::Result<T, PrudentiaError>;

/// Core trait for exchange adapter implementations
#[async_trait::async_trait]
pub trait ExchangeAdapterTrait {
    /// Get current market data for a trading pair
    async fn get_market_data(&self, symbol: &str) -> Result<MarketData>;
    
    /// Place a new order on the exchange
    async fn place_order(&self, order: &ExchangeOrder) -> Result<String>;
    
    /// Get order status by order ID
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus>;
    
    /// Cancel an existing order
    async fn cancel_order(&self, order_id: &str) -> Result<bool>;
    
    /// Get account balance for a specific asset
    async fn get_balance(&self, asset: &str) -> Result<Balance>;
    
    /// Get exchange information and trading rules
    async fn get_exchange_info(&self) -> Result<ExchangeInfo>;
    
    /// Test connectivity to exchange
    async fn ping(&self) -> Result<Duration>;
    
    /// Get exchange name identifier
    fn exchange_name(&self) -> &'static str;
}

/// Configuration for exchange adapter
#[derive(Debug, Clone)]
pub struct ExchangeConfig {
    /// Exchange name (binance, bybit, coinbase, etc.)
    pub name: String,
    
    /// API endpoint base URL
    pub base_url: String,
    
    /// WebSocket endpoint URL
    pub websocket_url: String,
    
    /// API credentials
    pub api_key: String,
    pub api_secret: String,
    
    /// Rate limiting configuration
    pub max_requests_per_minute: u32,
    pub max_order_requests_per_second: u32,
    
    /// Connection timeouts
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    
    /// Circuit breaker settings
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
}

/// Market data subscription configuration
#[derive(Debug, Clone)]
pub struct MarketDataConfig {
    /// Trading pairs to subscribe to
    pub symbols: Vec<String>,
    
    /// Update frequency preference
    pub update_frequency: Duration,
    
    /// Order book depth level
    pub book_depth: u32,
    
    /// Enable trade stream
    pub trade_stream: bool,
    
    /// Enable 24hr ticker data
    pub ticker_stream: bool,
}

/// Exchange adapter factory for creating configured adapters
pub struct ExchangeAdapterFactory;

impl ExchangeAdapterFactory {
    /// Create a new exchange adapter based on configuration
    pub fn create_adapter(config: ExchangeConfig) -> Result<Box<dyn ExchangeAdapterTrait + Send + Sync>> {
        match config.name.to_lowercase().as_str() {
            "binance" => {
                let adapter = BinanceAdapter::new(config)?;
                Ok(Box::new(adapter))
            },
            _ => Err(PrudentiaError::ConnectionFailure {
                exchange: config.name,
                reason: "Unsupported exchange".to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exchange_config_creation() {
        let config = ExchangeConfig {
            name: "binance".to_string(),
            base_url: "https://api.binance.com".to_string(),
            websocket_url: "wss://stream.binance.com:9443".to_string(),
            api_key: "test_key".to_string(),
            api_secret: "test_secret".to_string(),
            max_requests_per_minute: 1200,
            max_order_requests_per_second: 10,
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            failure_threshold: 5,
            recovery_timeout: Duration::from_mins(1),
        };
        
        assert_eq!(config.name, "binance");
        assert_eq!(config.max_requests_per_minute, 1200);
    }
    
    #[test] 
    fn test_market_data_config_defaults() {
        let config = MarketDataConfig {
            symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
            update_frequency: Duration::from_millis(100),
            book_depth: 20,
            trade_stream: true,
            ticker_stream: true,
        };
        
        assert_eq!(config.symbols.len(), 2);
        assert!(config.trade_stream);
    }
}