//! Exchange adapter trait for unified exchange integration
//!
//! Provides a common interface for interacting with different cryptocurrency exchanges.
//! All exchange implementations must adhere to this trait for integration with the OODA loop.

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use thiserror::Error;

/// Market data snapshot from exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub symbol: String,
    pub bid_price: Decimal,
    pub ask_price: Decimal,
    pub last_price: Decimal,
    pub volume_24h: Decimal,
    pub timestamp: SystemTime,
}

/// Trade order to be executed on exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeOrder {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,  // None for market orders
    pub stop_price: Option<Decimal>,
    pub client_order_id: String,
}

/// Order side (buy/sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    StopLossLimit,
    TakeProfit,
    TakeProfitLimit,
}

/// Order execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResult {
    pub order_id: String,
    pub client_order_id: String,
    pub symbol: String,
    pub status: OrderStatus,
    pub executed_quantity: Decimal,
    pub executed_price: Decimal,
    pub commission: Decimal,
    pub timestamp: SystemTime,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// Account balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub asset: String,
    pub free: Decimal,
    pub locked: Decimal,
    pub total: Decimal,
}

/// Exchange adapter errors
#[derive(Debug, Error, Clone)]
pub enum ExchangeError {
    #[error("Connection error: {message}")]
    ConnectionError { message: String },
    
    #[error("Authentication failed: {message}")]
    AuthenticationError { message: String },
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Insufficient balance for order")]
    InsufficientBalance,
    
    #[error("Invalid order: {reason}")]
    InvalidOrder { reason: String },
    
    #[error("Order not found: {order_id}")]
    OrderNotFound { order_id: String },
    
    #[error("Market data unavailable for {symbol}")]
    MarketDataUnavailable { symbol: String },
    
    #[error("Exchange error: {message}")]
    ExchangeSpecificError { message: String },
}

/// Unified exchange adapter trait
#[async_trait]
pub trait ExchangeAdapterTrait: Send + Sync {
    /// Get current market data for a symbol
    async fn get_market_data(&self, symbol: &str) -> Result<MarketData, ExchangeError>;
    
    /// Place a trade order on the exchange
    async fn place_order(&self, order: &TradeOrder) -> Result<OrderResult, ExchangeError>;
    
    /// Cancel an existing order
    async fn cancel_order(&self, order_id: &str) -> Result<(), ExchangeError>;
    
    /// Get order status
    async fn get_order_status(&self, order_id: &str) -> Result<OrderResult, ExchangeError>;
    
    /// Get account balance for a specific asset
    async fn get_balance(&self, asset: &str) -> Result<AccountBalance, ExchangeError>;
    
    /// Get all account balances
    async fn get_all_balances(&self) -> Result<Vec<AccountBalance>, ExchangeError>;
    
    /// Check if exchange connection is healthy
    async fn health_check(&self) -> Result<bool, ExchangeError>;
    
    /// Get exchange name for identification
    fn exchange_name(&self) -> &str;
    
    /// Check if a trading pair is supported
    async fn is_symbol_supported(&self, symbol: &str) -> Result<bool, ExchangeError>;
}

/// Legacy placeholder struct (to be removed when migrating to trait implementations)
pub struct ExchangeAdapter;

impl ExchangeAdapter {
    /// Create a new exchange adapter (placeholder)
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExchangeAdapter {
    fn default() -> Self {
        Self::new()
    }
}