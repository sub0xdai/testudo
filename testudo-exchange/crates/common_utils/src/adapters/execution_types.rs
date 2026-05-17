//! Execution Types for Binance Order Execution
//!
//! This module defines types for validated orders, execution results,
//! and error handling for live order execution on Binance.
//!
//! ## Draw-to-Trade Types (006-execution-latency)
//!
//! - [`TradeIntent`]: User's drawing intent with entry, stop, equity
//! - [`SignedOrder`]: Ready-for-CEX order with calculated size
//! - [`CexGateway`]: Trait for exchange dispatch (real or mock)

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use thiserror::Error;

/// Order side for execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecutionOrderSide {
    Buy,
    Sell,
}

/// Order type for execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecutionOrderType {
    Market,
    Limit,
}

/// Time in force for limit orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecutionTimeInForce {
    /// Good 'til canceled
    #[default]
    Gtc,
    /// Immediate or cancel
    Ioc,
    /// Fill or kill
    Fok,
}

/// A validated order ready for execution on Binance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedOrder {
    /// Symbol in Binance format (e.g., "BTCUSDT")
    pub symbol: String,
    /// Order side
    pub side: ExecutionOrderSide,
    /// Order type
    pub order_type: ExecutionOrderType,
    /// Quantity to trade
    pub quantity: Decimal,
    /// Price (required for LIMIT orders)
    pub price: Option<Decimal>,
    /// Time in force
    pub time_in_force: ExecutionTimeInForce,
    /// Client order ID (optional)
    pub client_order_id: Option<String>,
}

impl ValidatedOrder {
    /// Create a new market order
    pub fn market(symbol: String, side: ExecutionOrderSide, quantity: Decimal) -> Self {
        Self {
            symbol,
            side,
            order_type: ExecutionOrderType::Market,
            quantity,
            price: None,
            time_in_force: ExecutionTimeInForce::Gtc,
            client_order_id: None,
        }
    }

    /// Create a new limit order
    pub fn limit(
        symbol: String,
        side: ExecutionOrderSide,
        quantity: Decimal,
        price: Decimal,
    ) -> Self {
        Self {
            symbol,
            side,
            order_type: ExecutionOrderType::Limit,
            quantity,
            price: Some(price),
            time_in_force: ExecutionTimeInForce::Gtc,
            client_order_id: None,
        }
    }

    /// Set client order ID
    pub fn with_client_order_id(mut self, client_order_id: String) -> Self {
        self.client_order_id = Some(client_order_id);
        self
    }

    /// Set time in force
    pub fn with_time_in_force(mut self, tif: ExecutionTimeInForce) -> Self {
        self.time_in_force = tif;
        self
    }
}

/// Order status from Binance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BinanceOrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    PendingCancel,
    Rejected,
    Expired,
}

/// Result of a successful Binance order execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceOrderResult {
    /// Exchange-assigned order ID
    pub order_id: String,
    /// Client-assigned order ID
    pub client_order_id: String,
    /// Order status
    pub status: BinanceOrderStatus,
    /// Filled quantity
    pub filled_qty: Decimal,
    /// Average fill price
    pub avg_price: Decimal,
    /// Execution timestamp (milliseconds)
    pub timestamp: i64,
    /// Original order symbol
    pub symbol: String,
    /// Order side
    pub side: ExecutionOrderSide,
    /// Original quantity
    pub original_qty: Decimal,
}

/// Errors that can occur during order execution
#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
    },

    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Order rejected: code={code}, message={message}")]
    OrderRejected { code: i32, message: String },

    #[error("Invalid order: {0}")]
    InvalidOrder(String),

    #[error("Exchange unavailable")]
    ExchangeUnavailable,

    #[error("Request timeout")]
    Timeout,
}

/// Execution mode for the trading system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExecutionMode {
    /// Paper trading - no real orders
    #[default]
    Shadow,
    /// Live trading - real orders on exchange
    Live,
}

// =============================================================================
// Draw-to-Trade Types (006-execution-latency)
// =============================================================================

/// User's trade intent from drawing on the chart (FR-2)
///
/// Represents the visual "Draw-to-Trade" action where a user draws
/// entry/stop levels on the chart. This payload is normalized and
/// used to calculate position size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeIntent {
    /// Trading symbol in internal format (e.g., "BTC/USDT")
    pub symbol: String,
    /// Entry price where the trade will be executed
    pub entry: Decimal,
    /// Stop loss price for risk calculation
    pub stop: Decimal,
    /// User's account equity in quote currency
    pub account_equity: Decimal,
    /// Risk percentage as decimal (e.g., 0.02 for 2%)
    pub risk_pct: Decimal,
}

impl TradeIntent {
    /// Create a new trade intent
    pub fn new(
        symbol: impl Into<String>,
        entry: Decimal,
        stop: Decimal,
        account_equity: Decimal,
        risk_pct: Decimal,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            entry,
            stop,
            account_equity,
            risk_pct,
        }
    }

    /// Calculate position size using FR-1 formula:
    /// Size = (Account_Equity * risk_pct) / abs(Entry - Stop)
    pub fn calculate_size(&self) -> Decimal {
        let stop_distance = (self.entry - self.stop).abs();
        if stop_distance.is_zero() {
            return Decimal::ZERO;
        }
        (self.account_equity * self.risk_pct) / stop_distance
    }

    /// Determine order side based on entry vs stop relationship
    pub fn side(&self) -> ExecutionOrderSide {
        if self.entry > self.stop {
            ExecutionOrderSide::Buy // Long: stop below entry
        } else {
            ExecutionOrderSide::Sell // Short: stop above entry
        }
    }
}

/// Order ready for CEX dispatch (FR-2)
///
/// The output of the ExecutionService after processing a TradeIntent.
/// Contains all information needed to send to the exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedOrder {
    /// Unique order identifier
    pub order_id: String,
    /// Symbol in exchange format
    pub symbol: String,
    /// Order side (buy/sell)
    pub side: ExecutionOrderSide,
    /// Calculated position size (from FR-1 formula)
    pub quantity: Decimal,
    /// Entry price
    pub price: Decimal,
    /// Stop loss price (for reference)
    pub stop_loss: Decimal,
    /// Order type
    pub order_type: ExecutionOrderType,
    /// Timestamp when order was signed (monotonic, for latency tracking)
    pub signed_at_micros: u128,
}

impl SignedOrder {
    /// Create a new signed order from a trade intent
    pub fn from_intent(intent: &TradeIntent, order_id: String) -> Self {
        use std::time::Instant;

        // Use monotonic time for internal latency tracking
        let signed_at_micros = Instant::now().elapsed().as_micros();

        Self {
            order_id,
            symbol: intent.symbol.clone(),
            side: intent.side(),
            quantity: intent.calculate_size(),
            price: intent.entry,
            stop_loss: intent.stop,
            order_type: ExecutionOrderType::Limit,
            signed_at_micros,
        }
    }
}

/// Trait for CEX gateway dispatch (FR-4)
///
/// Abstracts the exchange interaction for testing without real funds.
/// Implementations can be real (Binance) or mock.
pub trait CexGateway: Send + Sync {
    /// Dispatch a signed order to the exchange
    fn dispatch<'a>(
        &'a self,
        order: &'a SignedOrder,
    ) -> Pin<Box<dyn Future<Output = Result<DispatchResult, ExecutionError>> + Send + 'a>>;

    /// Health check the gateway
    fn health_check(&self) -> bool {
        true
    }
}

/// Result of dispatching an order to the exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult {
    /// Exchange-assigned order ID
    pub exchange_order_id: String,
    /// Status of the order
    pub status: BinanceOrderStatus,
    /// Timestamp from exchange (milliseconds)
    pub exchange_timestamp: i64,
}

/// Mock CEX gateway for testing (FR-4)
///
/// Simulates exchange latency without making real API calls.
#[derive(Debug, Clone, Default)]
pub struct MockCexGateway {
    /// Simulated latency in microseconds (default: 100μs)
    pub simulated_latency_micros: u64,
}

impl MockCexGateway {
    /// Create a new mock gateway with default settings
    pub fn new() -> Self {
        Self {
            simulated_latency_micros: 100,
        }
    }

    /// Create a mock gateway with custom latency
    pub fn with_latency(latency_micros: u64) -> Self {
        Self {
            simulated_latency_micros: latency_micros,
        }
    }
}

impl CexGateway for MockCexGateway {
    fn dispatch<'a>(
        &'a self,
        order: &'a SignedOrder,
    ) -> Pin<Box<dyn Future<Output = Result<DispatchResult, ExecutionError>> + Send + 'a>> {
        Box::pin(async move {
            // Simulate exchange latency
            if self.simulated_latency_micros > 0 {
                tokio::time::sleep(tokio::time::Duration::from_micros(
                    self.simulated_latency_micros,
                ))
                .await;
            }

            Ok(DispatchResult {
                exchange_order_id: format!("MOCK-{}", &order.order_id),
                status: BinanceOrderStatus::New,
                exchange_timestamp: chrono::Utc::now().timestamp_millis(),
            })
        })
    }
}

/// Latency guard error (FR-3)
#[derive(Debug, Error)]
#[error("Latency exceeded threshold: {elapsed_micros}μs > {threshold_micros}μs")]
pub struct LatencyExceededError {
    /// Elapsed time in microseconds
    pub elapsed_micros: u128,
    /// Threshold in microseconds
    pub threshold_micros: u128,
}

/// Symbol normalization utilities
pub mod symbol {
    /// Convert internal symbol format (BTC_USDC) to Binance format (BTCUSDT)
    ///
    /// Internal format: BASE_QUOTE (e.g., BTC_USDC)
    /// Binance format: BASEQUOTE with USDT (e.g., BTCUSDT)
    ///
    /// # Examples
    /// ```
    /// use common_utils::adapters::execution_types::symbol::to_binance;
    /// assert_eq!(to_binance("BTC_USDC"), "BTCUSDT");
    /// assert_eq!(to_binance("ETH_USDC"), "ETHUSDT");
    /// ```
    pub fn to_binance(internal: &str) -> String {
        // Split on underscore: "BTC_USDC" -> ["BTC", "USDC"]
        let parts: Vec<&str> = internal.split('_').collect();
        if parts.len() != 2 {
            return internal.to_string();
        }

        let base = parts[0];
        // Binance uses USDT as the primary stablecoin
        format!("{}USDT", base)
    }

    /// Convert Binance symbol format (BTCUSDT) to internal format (BTC_USDC)
    ///
    /// Binance format: BASEQUOTE (e.g., BTCUSDT)
    /// Internal format: BASE_QUOTE (e.g., BTC_USDC)
    ///
    /// # Examples
    /// ```
    /// use common_utils::adapters::execution_types::symbol::from_binance;
    /// assert_eq!(from_binance("BTCUSDT"), "BTC_USDC");
    /// assert_eq!(from_binance("ETHUSDT"), "ETH_USDC");
    /// ```
    pub fn from_binance(binance: &str) -> String {
        // Strip USDT suffix and add _USDC
        if let Some(base) = binance.strip_suffix("USDT") {
            format!("{}_USDC", base)
        } else if let Some(base) = binance.strip_suffix("BUSD") {
            format!("{}_USDC", base)
        } else if let Some(base) = binance.strip_suffix("USDC") {
            format!("{}_USDC", base)
        } else {
            binance.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // ==================== Symbol Normalization Tests ====================

    #[test]
    fn test_symbol_to_binance_btc() {
        assert_eq!(symbol::to_binance("BTC_USDC"), "BTCUSDT");
    }

    #[test]
    fn test_symbol_to_binance_eth() {
        assert_eq!(symbol::to_binance("ETH_USDC"), "ETHUSDT");
    }

    #[test]
    fn test_symbol_to_binance_sol() {
        assert_eq!(symbol::to_binance("SOL_USDC"), "SOLUSDT");
    }

    #[test]
    fn test_symbol_from_binance_btc() {
        assert_eq!(symbol::from_binance("BTCUSDT"), "BTC_USDC");
    }

    #[test]
    fn test_symbol_from_binance_eth() {
        assert_eq!(symbol::from_binance("ETHUSDT"), "ETH_USDC");
    }

    #[test]
    fn test_symbol_from_binance_sol() {
        assert_eq!(symbol::from_binance("SOLUSDT"), "SOL_USDC");
    }

    #[test]
    fn test_symbol_roundtrip() {
        let internal = "BTC_USDC";
        let binance = symbol::to_binance(internal);
        let back = symbol::from_binance(&binance);
        assert_eq!(back, internal);
    }

    // ==================== ValidatedOrder Tests ====================

    #[test]
    fn test_validated_order_market_buy() {
        let order = ValidatedOrder::market(
            "BTCUSDT".to_string(),
            ExecutionOrderSide::Buy,
            Decimal::from_str("0.1").unwrap(),
        );

        assert_eq!(order.symbol, "BTCUSDT");
        assert_eq!(order.side, ExecutionOrderSide::Buy);
        assert_eq!(order.order_type, ExecutionOrderType::Market);
        assert_eq!(order.quantity, Decimal::from_str("0.1").unwrap());
        assert!(order.price.is_none());
    }

    #[test]
    fn test_validated_order_limit_sell() {
        let order = ValidatedOrder::limit(
            "ETHUSDT".to_string(),
            ExecutionOrderSide::Sell,
            Decimal::from_str("1.5").unwrap(),
            Decimal::from_str("2500.00").unwrap(),
        );

        assert_eq!(order.symbol, "ETHUSDT");
        assert_eq!(order.side, ExecutionOrderSide::Sell);
        assert_eq!(order.order_type, ExecutionOrderType::Limit);
        assert_eq!(order.quantity, Decimal::from_str("1.5").unwrap());
        assert_eq!(order.price, Some(Decimal::from_str("2500.00").unwrap()));
    }

    #[test]
    fn test_validated_order_with_client_id() {
        let order = ValidatedOrder::market(
            "BTCUSDT".to_string(),
            ExecutionOrderSide::Buy,
            Decimal::from_str("0.1").unwrap(),
        )
        .with_client_order_id("my-order-123".to_string());

        assert_eq!(order.client_order_id, Some("my-order-123".to_string()));
    }

    #[test]
    fn test_validated_order_with_time_in_force() {
        let order = ValidatedOrder::limit(
            "BTCUSDT".to_string(),
            ExecutionOrderSide::Buy,
            Decimal::from_str("0.1").unwrap(),
            Decimal::from_str("50000.00").unwrap(),
        )
        .with_time_in_force(ExecutionTimeInForce::Ioc);

        assert_eq!(order.time_in_force, ExecutionTimeInForce::Ioc);
    }

    // ==================== ExecutionMode Tests ====================

    #[test]
    fn test_execution_mode_default_is_shadow() {
        assert_eq!(ExecutionMode::default(), ExecutionMode::Shadow);
    }
}
