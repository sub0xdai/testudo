//! CCXT-Compatible Types
//!
//! This module defines types that align with the official CCXT library patterns,
//! providing a standardized interface for cryptocurrency exchange integration.
//!
//! # CCXT Patterns
//!
//! The types follow the official CCXT JavaScript/TypeScript library conventions:
//! - Unified method signatures across all exchanges
//! - Standardized error codes and messages
//! - Consistent parameter and response formats
//! - Symbol normalization across exchange-specific formats

// @anchor exchange:common_utils:ccxt_types
// @tags infra

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

/// CCXT-style exchange interface following official patterns
///
/// This trait matches the official CCXT Exchange class structure with standardized
/// method signatures that work consistently across all exchange implementations.
#[async_trait]
pub trait CCXTExchange: Send + Sync {
    /// Create an order on the exchange
    ///
    /// # Parameters
    ///
    /// * `symbol` - Trading pair symbol (e.g., "BTC/USDT")
    /// * `order_type` - Order type ("market", "limit", "stop_loss", etc.)
    /// * `side` - Order side ("buy" or "sell")
    /// * `amount` - Order quantity in base currency
    /// * `price` - Price per unit (None for market orders)
    /// * `params` - Additional exchange-specific parameters
    ///
    /// # Returns
    ///
    /// CCXTOrderResponse containing order details and status
    async fn create_order(
        &self,
        symbol: &str,
        order_type: &str,
        side: &str,
        amount: f64,
        price: Option<f64>,
        params: Value,
    ) -> Result<CCXTOrderResponse, CCXTError>;

    /// Cancel an existing order
    async fn cancel_order(&self, id: &str, symbol: &str) -> Result<CCXTOrderResponse, CCXTError>;

    /// Fetch order details by ID
    async fn fetch_order(&self, id: &str, symbol: &str) -> Result<CCXTOrderResponse, CCXTError>;

    /// Fetch account balance
    async fn fetch_balance(&self) -> Result<CCXTBalance, CCXTError>;

    /// Fetch order book (market depth)
    async fn fetch_order_book(
        &self,
        symbol: &str,
        limit: Option<i32>,
    ) -> Result<CCXTOrderBook, CCXTError>;

    /// Get exchange identifier
    fn get_id(&self) -> &str;

    /// Check if exchange is operational
    async fn health_check(&self) -> Result<(), CCXTError>;

    /// Load exchange markets/symbols
    async fn load_markets(&self) -> Result<HashMap<String, CCXTMarket>, CCXTError>;
}

/// CCXT-compatible error types following official error taxonomy
#[derive(Debug, Error)]
pub enum CCXTError {
    #[error("Exchange error: {message}")]
    ExchangeError { message: String },

    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid order: {message}")]
    InvalidOrder { message: String },

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Authentication error: {message}")]
    AuthenticationError { message: String },

    #[error("Order not found: {order_id}")]
    OrderNotFound { order_id: String },

    #[error("Symbol not found: {symbol}")]
    SymbolNotFound { symbol: String },

    #[error("Exchange not available")]
    ExchangeNotAvailable,

    #[error("Request timeout")]
    RequestTimeout,

    #[error("Bad symbol: {symbol}")]
    BadSymbol { symbol: String },

    #[error("Bad request: {message}")]
    BadRequest { message: String },

    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },
}

/// CCXT-style order response following official structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTOrderResponse {
    /// Exchange-assigned order ID
    pub id: String,
    /// Client-assigned order ID (if any)
    pub client_order_id: Option<String>,
    /// Order status ("open", "closed", "canceled", "expired", "rejected")
    pub status: String,
    /// Trading symbol
    pub symbol: String,
    /// Order type
    #[serde(rename = "type")]
    pub order_type: String,
    /// Order side
    pub side: String,
    /// Original order amount
    pub amount: Decimal,
    /// Filled amount
    pub filled: Decimal,
    /// Remaining amount
    pub remaining: Decimal,
    /// Average fill price
    pub average: Option<Decimal>,
    /// Order price (for limit orders)
    pub price: Option<Decimal>,
    /// Stop price (for stop orders)
    pub stop_price: Option<Decimal>,
    /// Order timestamp
    pub timestamp: i64,
    /// Last update timestamp
    pub last_trade_timestamp: Option<i64>,
    /// Order fee information
    pub fee: Option<CCXTFee>,
    /// Exchange-specific info
    pub info: Value,
}

/// CCXT-style balance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTBalance {
    /// Balance by currency
    pub balances: HashMap<String, CCXTCurrencyBalance>,
    /// Total balance info
    pub info: Value,
    /// Balance timestamp
    pub timestamp: i64,
}

/// Individual currency balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTCurrencyBalance {
    /// Available balance for trading
    pub free: Decimal,
    /// Balance locked in orders
    pub used: Decimal,
    /// Total balance (free + used)
    pub total: Decimal,
}

/// CCXT-style order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTOrderBook {
    /// Trading symbol
    pub symbol: String,
    /// Buy orders (bids)
    pub bids: Vec<[Decimal; 2]>, // [price, amount]
    /// Sell orders (asks)
    pub asks: Vec<[Decimal; 2]>, // [price, amount]
    /// Order book timestamp
    pub timestamp: i64,
    /// Order book nonce
    pub nonce: Option<i64>,
}

/// Trading fee information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTFee {
    /// Fee currency
    pub currency: String,
    /// Fee cost
    pub cost: Decimal,
    /// Fee rate used
    pub rate: Option<Decimal>,
}

/// Market information following CCXT structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTMarket {
    /// Market ID on exchange
    pub id: String,
    /// Standardized symbol (e.g., "BTC/USDT")
    pub symbol: String,
    /// Base currency (e.g., "BTC")
    pub base: String,
    /// Quote currency (e.g., "USDT")
    pub quote: String,
    /// Whether market is active
    pub active: bool,
    /// Market type ("spot", "future", "option")
    pub market_type: String,
    /// Minimum order amount
    pub limits: CCXTMarketLimits,
    /// Price and amount precision
    pub precision: CCXTMarketPrecision,
}

/// Market trading limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTMarketLimits {
    /// Amount limits
    pub amount: CCXTLimit,
    /// Price limits
    pub price: CCXTLimit,
    /// Cost limits (amount * price)
    pub cost: CCXTLimit,
}

/// Individual limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTLimit {
    /// Minimum value
    pub min: Option<Decimal>,
    /// Maximum value
    pub max: Option<Decimal>,
}

/// Market precision settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTMarketPrecision {
    /// Amount decimal places
    pub amount: u32,
    /// Price decimal places
    pub price: u32,
}

/// CCXT-style ticker information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CCXTTicker {
    /// Trading symbol
    pub symbol: String,
    /// Current best bid price (from orderbook)
    pub bid: Option<Decimal>,
    /// Current best ask price (from orderbook)
    pub ask: Option<Decimal>,
    /// Last trade price
    pub last: Option<Decimal>,
    /// 24h high price
    pub high: Option<Decimal>,
    /// 24h low price
    pub low: Option<Decimal>,
    /// 24h trading volume
    pub base_volume: Option<Decimal>,
    /// 24h trading volume in quote currency
    pub quote_volume: Option<Decimal>,
    /// 24h price change percentage
    pub percentage: Option<Decimal>,
    /// Ticker timestamp
    pub timestamp: i64,
}

/// Configuration for CCXT exchange instances
#[derive(Debug, Clone)]
pub struct CCXTConfig {
    /// Exchange name
    pub exchange_id: String,
    /// API credentials
    pub credentials: CCXTCredentials,
    /// Sandbox mode
    pub sandbox: bool,
    /// Request timeout in milliseconds
    pub timeout: u64,
    /// Rate limiting configuration
    pub rate_limit: CCXTRateLimit,
    /// Additional options
    pub options: HashMap<String, Value>,
}

/// API credentials for exchange
#[derive(Debug, Clone)]
pub struct CCXTCredentials {
    /// API key
    pub api_key: String,
    /// API secret
    pub secret: String,
    /// Passphrase (for some exchanges)
    pub passphrase: Option<String>,
    /// Sub-account ID (if applicable)
    pub sub_account: Option<String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct CCXTRateLimit {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window in seconds
    pub window_seconds: u64,
    /// Enable rate limiting
    pub enabled: bool,
}

impl Default for CCXTRateLimit {
    fn default() -> Self {
        Self {
            max_requests: 1200,
            window_seconds: 60,
            enabled: true,
        }
    }
}

/// Standard CCXT parameter value type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CCXTValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<CCXTValue>),
    Object(HashMap<String, CCXTValue>),
}

impl From<Value> for CCXTValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => CCXTValue::Null,
            Value::Bool(b) => CCXTValue::Bool(b),
            Value::Number(n) => CCXTValue::Number(n.as_f64().unwrap_or_default()),
            Value::String(s) => CCXTValue::String(s),
            Value::Array(arr) => CCXTValue::Array(arr.into_iter().map(CCXTValue::from).collect()),
            Value::Object(obj) => {
                let map = obj
                    .into_iter()
                    .map(|(k, v)| (k, CCXTValue::from(v)))
                    .collect();
                CCXTValue::Object(map)
            }
        }
    }
}

impl From<CCXTValue> for Value {
    fn from(ccxt_value: CCXTValue) -> Self {
        match ccxt_value {
            CCXTValue::Null => Value::Null,
            CCXTValue::Bool(b) => Value::Bool(b),
            CCXTValue::Number(n) => {
                // Use safe conversion that handles None case
                match serde_json::Number::from_f64(n) {
                    Some(num) => Value::Number(num),
                    None => Value::Null,
                }
            }
            CCXTValue::String(s) => Value::String(s),
            CCXTValue::Array(arr) => {
                let vec = arr.into_iter().map(Value::from).collect();
                Value::Array(vec)
            }
            CCXTValue::Object(obj) => {
                let map = obj.into_iter().map(|(k, v)| (k, Value::from(v))).collect();
                Value::Object(map)
            }
        }
    }
}

/// Helper functions for CCXT compatibility
impl CCXTOrderResponse {
    /// Create a new order response in CCXT format
    pub fn new(
        id: String,
        symbol: String,
        order_type: String,
        side: String,
        amount: Decimal,
        price: Option<Decimal>,
    ) -> Self {
        let timestamp = Utc::now().timestamp_millis();

        Self {
            id,
            client_order_id: None,
            status: "open".to_string(),
            symbol,
            order_type,
            side,
            amount,
            filled: Decimal::ZERO,
            remaining: amount,
            average: None,
            price,
            stop_price: None,
            timestamp,
            last_trade_timestamp: None,
            fee: None,
            info: Value::Object(Default::default()),
        }
    }

    /// Check if order is closed (filled or canceled)
    pub fn is_closed(&self) -> bool {
        matches!(
            self.status.as_str(),
            "closed" | "canceled" | "expired" | "rejected"
        )
    }

    /// Check if order is filled
    pub fn is_filled(&self) -> bool {
        self.status == "closed" && self.filled == self.amount
    }

    /// Get fill percentage
    pub fn fill_percentage(&self) -> Decimal {
        if self.amount.is_zero() {
            Decimal::ZERO
        } else {
            (self.filled / self.amount) * Decimal::from(100)
        }
    }
}

impl Default for CCXTBalance {
    fn default() -> Self {
        Self::new()
    }
}

impl CCXTBalance {
    /// Create a new balance response
    pub fn new() -> Self {
        Self {
            balances: HashMap::new(),
            info: Value::Object(Default::default()),
            timestamp: Utc::now().timestamp_millis(),
        }
    }

    /// Add or update a currency balance
    pub fn set_balance(&mut self, currency: &str, free: Decimal, used: Decimal) {
        let total = free + used;
        self.balances.insert(
            currency.to_uppercase(),
            CCXTCurrencyBalance { free, used, total },
        );
    }

    /// Get balance for a specific currency
    pub fn get_balance(&self, currency: &str) -> Option<&CCXTCurrencyBalance> {
        self.balances.get(&currency.to_uppercase())
    }
}

impl CCXTOrderBook {
    /// Create a new order book
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            bids: Vec::new(),
            asks: Vec::new(),
            timestamp: Utc::now().timestamp_millis(),
            nonce: None,
        }
    }

    /// Add a bid to the order book
    pub fn add_bid(&mut self, price: Decimal, amount: Decimal) {
        self.bids.push([price, amount]);
        // Keep bids sorted by price descending
        self.bids.sort_by(|a, b| b[0].cmp(&a[0]));
    }

    /// Add an ask to the order book
    pub fn add_ask(&mut self, price: Decimal, amount: Decimal) {
        self.asks.push([price, amount]);
        // Keep asks sorted by price ascending
        self.asks.sort_by(|a, b| a[0].cmp(&b[0]));
    }

    /// Get best bid (highest buy price)
    pub fn best_bid(&self) -> Option<[Decimal; 2]> {
        self.bids.first().copied()
    }

    /// Get best ask (lowest sell price)
    pub fn best_ask(&self) -> Option<[Decimal; 2]> {
        self.asks.first().copied()
    }

    /// Get bid-ask spread
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask[0] - bid[0]),
            _ => None,
        }
    }
}

/// CCXT error code mapping for standardized error handling
impl CCXTError {
    /// Map exchange-specific error codes to CCXT standard errors
    pub fn from_exchange_error(exchange: &str, code: &str, message: &str) -> Self {
        match (exchange, code) {
            // Binance error mappings
            ("binance", "-1021") => CCXTError::RequestTimeout,
            ("binance", "-1003") => CCXTError::RateLimitExceeded,
            ("binance", "-2010") => CCXTError::InsufficientFunds,
            ("binance", "-2011") => CCXTError::OrderNotFound {
                order_id: "unknown".to_string(),
            },

            // Coinbase error mappings
            ("coinbase", "insufficient_funds") => CCXTError::InsufficientFunds,
            ("coinbase", "rate_limit_exceeded") => CCXTError::RateLimitExceeded,
            ("coinbase", "invalid_order") => CCXTError::InvalidOrder {
                message: message.to_string(),
            },

            // Kraken error mappings
            ("kraken", "EGeneral:Invalid arguments") => CCXTError::BadRequest {
                message: message.to_string(),
            },
            ("kraken", "EService:Unavailable") => CCXTError::ExchangeNotAvailable,
            ("kraken", "EAPI:Rate limit exceeded") => CCXTError::RateLimitExceeded,

            // Default mapping
            _ => CCXTError::ExchangeError {
                message: format!("{}: {}", code, message),
            },
        }
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CCXTError::NetworkError { .. }
                | CCXTError::RequestTimeout
                | CCXTError::ExchangeNotAvailable
        )
    }

    /// Check if error is rate limiting
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, CCXTError::RateLimitExceeded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_order_response_creation() {
        let order = CCXTOrderResponse::new(
            "12345".to_string(),
            "BTC/USDT".to_string(),
            "limit".to_string(),
            "buy".to_string(),
            Decimal::from_str("1.0").unwrap(),
            Some(Decimal::from_str("50000.0").unwrap()),
        );

        assert_eq!(order.id, "12345");
        assert_eq!(order.symbol, "BTC/USDT");
        assert_eq!(order.status, "open");
        assert!(!order.is_closed());
        assert!(!order.is_filled());
        assert_eq!(order.fill_percentage(), Decimal::ZERO);
    }

    #[test]
    fn test_balance_operations() {
        let mut balance = CCXTBalance::new();
        balance.set_balance(
            "BTC",
            Decimal::from_str("1.5").unwrap(),
            Decimal::from_str("0.5").unwrap(),
        );

        let btc_balance = balance.get_balance("BTC").unwrap();
        assert_eq!(btc_balance.free, Decimal::from_str("1.5").unwrap());
        assert_eq!(btc_balance.used, Decimal::from_str("0.5").unwrap());
        assert_eq!(btc_balance.total, Decimal::from_str("2.0").unwrap());
    }

    #[test]
    fn test_order_book_operations() {
        let mut book = CCXTOrderBook::new("BTC/USDT".to_string());

        book.add_bid(
            Decimal::from_str("49000").unwrap(),
            Decimal::from_str("0.1").unwrap(),
        );
        book.add_bid(
            Decimal::from_str("49100").unwrap(),
            Decimal::from_str("0.2").unwrap(),
        );
        book.add_ask(
            Decimal::from_str("49200").unwrap(),
            Decimal::from_str("0.15").unwrap(),
        );
        book.add_ask(
            Decimal::from_str("49150").unwrap(),
            Decimal::from_str("0.1").unwrap(),
        );

        let best_bid = book.best_bid().unwrap();
        let best_ask = book.best_ask().unwrap();

        assert_eq!(best_bid[0], Decimal::from_str("49100").unwrap()); // Highest bid
        assert_eq!(best_ask[0], Decimal::from_str("49150").unwrap()); // Lowest ask
        assert_eq!(book.spread().unwrap(), Decimal::from_str("50").unwrap()); // 49150 - 49100
    }

    #[test]
    fn test_error_mapping() {
        let binance_error =
            CCXTError::from_exchange_error("binance", "-1003", "Rate limit exceeded");
        assert!(matches!(binance_error, CCXTError::RateLimitExceeded));
        assert!(binance_error.is_rate_limit());

        let network_error = CCXTError::NetworkError {
            message: "Connection failed".to_string(),
        };
        assert!(network_error.is_retryable());
    }

    #[test]
    fn test_ccxt_value_conversion() {
        let json_value = serde_json::json!({
            "symbol": "BTC/USDT",
            "price": 50000.0,
            "active": true
        });

        let ccxt_value = CCXTValue::from(json_value.clone());
        let converted_back = Value::from(ccxt_value);

        assert_eq!(json_value, converted_back);
    }
}
