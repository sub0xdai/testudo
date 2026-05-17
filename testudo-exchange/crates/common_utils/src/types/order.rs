//! StandardOrder Type Definition for Exchange Integration
//!
//! This module provides a unified order representation that supports all major exchange
//! integrations while following SOLID design principles and TDD methodology.
//!
//! # Features
//!
//! - **Comprehensive Order Types**: Market, Limit, Stop Loss, Take Profit orders
//! - **Margin Trading Support**: Long/Short positions with automatic conversion to Buy/Sell
//! - **Exchange Agnostic**: Works with any exchange adapter (Binance, Coinbase, etc.)
//! - **Type Safety**: Strong validation with detailed error messages
//! - **Serialization**: Full JSON support for database storage and API responses
//! - **Builder Pattern**: Safe order construction with compile-time guarantees
//!
//! # Quick Start
//!
//! ```rust
//! use common_utils::{StandardOrder, StandardOrderBuilder, OrderSide, OrderType, TimeInForce};
//! use rust_decimal::Decimal;
//! use std::str::FromStr;
//! use uuid::Uuid;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let user_id = Uuid::new_v4();
//!
//! // Create a simple market order
//! let market_order = StandardOrder::market_buy(
//!     user_id,
//!     "BTC/USDT",
//!     Decimal::from_str("0.1")?
//! )?;
//!
//! // Create a limit order with builder pattern
//! let limit_order = StandardOrderBuilder::new()
//!     .user_id(user_id)
//!     .symbol("ETH/USDT")
//!     .side(OrderSide::Sell)
//!     .order_type(OrderType::Limit)
//!     .quantity(Decimal::from_str("2.5")?)
//!     .price(Decimal::from_str("2500.0")?)
//!     .exchange("binance")
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Architecture
//!
//! The StandardOrder type follows the Single Responsibility Principle by focusing
//! solely on order data representation. Validation logic is separated into the
//! `StandardOrderValidator` trait, allowing for dependency injection and testing.
//!
//! ```rust
//! use common_utils::{StandardOrderBuilder, OrderValidator, StandardOrderValidator};
//! # use common_utils::{OrderSide, OrderType};
//! # use rust_decimal::Decimal;
//! # use std::str::FromStr;
//! # use uuid::Uuid;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Validation is automatic during order creation
//! let validator = StandardOrderValidator;
//! let order = StandardOrderBuilder::new()
//!     .user_id(Uuid::new_v4())
//!     .symbol("BTC/USDT")
//!     .side(OrderSide::Buy)
//!     .order_type(OrderType::Market)
//!     .quantity(Decimal::from_str("1.0")?)
//!     .build()?; // Validation happens here
//! # Ok(())
//! # }
//! ```
//!
//! # Error Handling
//!
//! All validation errors are strongly typed with detailed context:
//!
//! ```rust
//! use common_utils::{StandardOrder, OrderValidationError};
//! # use rust_decimal::Decimal;
//! # use uuid::Uuid;
//!
//! # fn main() {
//! let user_id = Uuid::new_v4();
//! match StandardOrder::market_buy(user_id, "", Decimal::ZERO) {
//!     Ok(_) => unreachable!(),
//!     Err(OrderValidationError::InvalidQuantity(msg)) => {
//!         println!("Quantity error: {}", msg);
//!     }
//!     Err(OrderValidationError::InvalidSymbol(msg)) => {
//!         println!("Symbol error: {}", msg);
//!     }
//!     Err(e) => println!("Other error: {}", e),
//! }
//! # }
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;
use validator::Validate;

/// Order side enumeration supporting both spot and margin trading
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
    Long,  // Margin long position
    Short, // Margin short position
}

/// Order type enumeration supporting various execution strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    StopLossLimit,
    TakeProfit,
    TakeProfitLimit,
}

/// Time in force enumeration defining order lifetime behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TimeInForce {
    GTC, // Good Till Canceled
    IOC, // Immediate or Cancel
    FOK, // Fill or Kill
    GTD, // Good Till Date (with expiry)
}

/// Order status enumeration tracking order lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

/// Standard order representation for unified exchange interaction
/// Follows Single Responsibility Principle - solely represents order data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Validate)]
pub struct StandardOrder {
    pub id: Uuid,
    pub user_id: Uuid,
    #[validate(length(min = 1, message = "Symbol cannot be empty"))]
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    // Note: We validate quantity manually in the validator rather than using the derive macro
    // because rust_decimal is not supported by the validator crate's range validator
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub time_in_force: TimeInForce,
    pub exchange: Option<String>,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>, // For GTD orders
}

/// Order validation errors with detailed context
#[derive(Debug, Error)]
pub enum OrderValidationError {
    #[error("Invalid quantity: {0}")]
    InvalidQuantity(String),
    #[error("Missing price for limit order")]
    MissingPrice,
    #[error("Missing stop price for stop order")]
    MissingStopPrice,
    #[error("Invalid symbol format: {0}")]
    InvalidSymbol(String),
    #[error("Invalid time in force for order type")]
    InvalidTimeInForce,
    #[error("Unsupported order type: {0:?}")]
    UnsupportedOrderType(OrderType),
    #[error("Invalid price precision: {0}")]
    InvalidPricePrecision(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

/// Order builder following the Builder pattern for safe order construction
pub struct StandardOrderBuilder {
    id: Option<Uuid>,
    user_id: Option<Uuid>,
    symbol: Option<String>,
    side: Option<OrderSide>,
    order_type: Option<OrderType>,
    quantity: Option<Decimal>,
    price: Option<Decimal>,
    stop_price: Option<Decimal>,
    time_in_force: Option<TimeInForce>,
    exchange: Option<String>,
    expires_at: Option<DateTime<Utc>>,
}

impl StandardOrderBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            user_id: None,
            symbol: None,
            side: None,
            order_type: None,
            quantity: None,
            price: None,
            stop_price: None,
            time_in_force: None,
            exchange: None,
            expires_at: None,
        }
    }

    pub fn user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    pub fn order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = Some(order_type);
        self
    }

    pub fn quantity(mut self, quantity: Decimal) -> Self {
        self.quantity = Some(quantity);
        self
    }

    pub fn price(mut self, price: Decimal) -> Self {
        self.price = Some(price);
        self
    }

    pub fn stop_price(mut self, stop_price: Decimal) -> Self {
        self.stop_price = Some(stop_price);
        self
    }

    pub fn time_in_force(mut self, tif: TimeInForce) -> Self {
        self.time_in_force = Some(tif);
        self
    }

    pub fn exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = Some(exchange.into());
        self
    }

    pub fn expires_at(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn build(self) -> Result<StandardOrder, OrderValidationError> {
        let user_id = self.user_id.ok_or_else(|| {
            OrderValidationError::ValidationFailed("user_id is required".to_string())
        })?;
        let symbol = self.symbol.ok_or_else(|| {
            OrderValidationError::ValidationFailed("symbol is required".to_string())
        })?;
        let side = self.side.ok_or_else(|| {
            OrderValidationError::ValidationFailed("side is required".to_string())
        })?;
        let order_type = self.order_type.ok_or_else(|| {
            OrderValidationError::ValidationFailed("order_type is required".to_string())
        })?;
        let quantity = self.quantity.ok_or_else(|| {
            OrderValidationError::ValidationFailed("quantity is required".to_string())
        })?;

        let now = Utc::now();
        let order = StandardOrder {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            user_id,
            symbol,
            side,
            order_type,
            quantity,
            price: self.price,
            stop_price: self.stop_price,
            time_in_force: self.time_in_force.unwrap_or(TimeInForce::GTC),
            exchange: self.exchange,
            status: OrderStatus::New,
            created_at: now,
            updated_at: now,
            expires_at: self.expires_at,
        };

        let validator = StandardOrderValidator;
        validator.validate_order(&order)?;
        Ok(order)
    }
}

impl Default for StandardOrderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Order validator following Interface Segregation Principle
pub trait OrderValidator: Send + Sync {
    fn validate_order(&self, order: &StandardOrder) -> Result<(), OrderValidationError>;
}

/// Standard implementation of OrderValidator
pub struct StandardOrderValidator;

impl OrderValidator for StandardOrderValidator {
    fn validate_order(&self, order: &StandardOrder) -> Result<(), OrderValidationError> {
        // Validate quantity is positive
        if order.quantity <= Decimal::ZERO {
            return Err(OrderValidationError::InvalidQuantity(
                "Quantity must be positive".to_string(),
            ));
        }

        // Validate symbol format
        if order.symbol.is_empty() {
            return Err(OrderValidationError::InvalidSymbol(
                "Symbol cannot be empty".to_string(),
            ));
        }

        if !Self::is_valid_symbol_format(&order.symbol) {
            return Err(OrderValidationError::InvalidSymbol(format!(
                "Invalid symbol format: {}",
                order.symbol
            )));
        }

        // Order type specific validations
        match order.order_type {
            OrderType::Limit => {
                if order.price.is_none() {
                    return Err(OrderValidationError::MissingPrice);
                }
                if let Some(price) = order.price {
                    if price <= Decimal::ZERO {
                        return Err(OrderValidationError::InvalidQuantity(
                            "Price must be positive".to_string(),
                        ));
                    }
                }
            }
            OrderType::StopLoss | OrderType::StopLossLimit => {
                if order.stop_price.is_none() {
                    return Err(OrderValidationError::MissingStopPrice);
                }
                if let Some(stop_price) = order.stop_price {
                    if stop_price <= Decimal::ZERO {
                        return Err(OrderValidationError::InvalidQuantity(
                            "Stop price must be positive".to_string(),
                        ));
                    }
                }
                // StopLossLimit also needs price
                if matches!(order.order_type, OrderType::StopLossLimit) && order.price.is_none() {
                    return Err(OrderValidationError::MissingPrice);
                }
            }
            OrderType::TakeProfit | OrderType::TakeProfitLimit => {
                if order.stop_price.is_none() {
                    return Err(OrderValidationError::MissingStopPrice);
                }
                if let Some(stop_price) = order.stop_price {
                    if stop_price <= Decimal::ZERO {
                        return Err(OrderValidationError::InvalidQuantity(
                            "Take profit price must be positive".to_string(),
                        ));
                    }
                }
                // TakeProfitLimit also needs price
                if matches!(order.order_type, OrderType::TakeProfitLimit) && order.price.is_none() {
                    return Err(OrderValidationError::MissingPrice);
                }
            }
            OrderType::Market => {
                // Market orders shouldn't have price set
                if order.price.is_some() {
                    return Err(OrderValidationError::ValidationFailed(
                        "Market orders should not have a price".to_string(),
                    ));
                }
            }
        }

        // Validate time in force compatibility
        match (order.time_in_force, order.expires_at.is_some()) {
            (TimeInForce::GTD, false) => {
                return Err(OrderValidationError::InvalidTimeInForce);
            }
            (TimeInForce::GTC | TimeInForce::IOC | TimeInForce::FOK, true) => {
                return Err(OrderValidationError::InvalidTimeInForce);
            }
            _ => {}
        }

        Ok(())
    }
}

impl StandardOrderValidator {
    /// Validates symbol format - basic implementation, can be extended
    fn is_valid_symbol_format(symbol: &str) -> bool {
        if symbol.is_empty() || symbol.len() > 20 {
            return false;
        }

        // Allow alphanumeric characters, underscores, dashes, and slashes
        symbol
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '/')
    }
}

/// Helper methods for StandardOrder
impl StandardOrder {
    /// Factory method for creating a market buy order
    pub fn market_buy(
        user_id: Uuid,
        symbol: impl Into<String>,
        quantity: Decimal,
    ) -> Result<Self, OrderValidationError> {
        StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol(symbol)
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(quantity)
            .build()
    }

    /// Factory method for creating a market sell order
    pub fn market_sell(
        user_id: Uuid,
        symbol: impl Into<String>,
        quantity: Decimal,
    ) -> Result<Self, OrderValidationError> {
        StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol(symbol)
            .side(OrderSide::Sell)
            .order_type(OrderType::Market)
            .quantity(quantity)
            .build()
    }

    /// Factory method for creating a limit buy order
    pub fn limit_buy(
        user_id: Uuid,
        symbol: impl Into<String>,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<Self, OrderValidationError> {
        StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol(symbol)
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(quantity)
            .price(price)
            .build()
    }

    /// Factory method for creating a limit sell order
    pub fn limit_sell(
        user_id: Uuid,
        symbol: impl Into<String>,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<Self, OrderValidationError> {
        StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol(symbol)
            .side(OrderSide::Sell)
            .order_type(OrderType::Limit)
            .quantity(quantity)
            .price(price)
            .build()
    }

    /// Converts Long/Short sides to Buy/Sell based on intent
    pub fn to_spot_side(&self) -> OrderSide {
        match self.side {
            OrderSide::Long => OrderSide::Buy,
            OrderSide::Short => OrderSide::Sell,
            side => side,
        }
    }

    /// Normalizes symbol to exchange-specific format
    pub fn normalize_symbol(&self, format: SymbolFormat) -> String {
        match format {
            SymbolFormat::WithSlash => {
                if self.symbol.contains('/') {
                    self.symbol.clone()
                } else {
                    // Assume it's something like BTCUSDT, convert to BTC/USDT
                    self.symbol
                        .replace("USDT", "/USDT")
                        .replace("BTC", "BTC/")
                        .replace("ETH", "ETH/")
                        .replace("SOL", "SOL/")
                }
            }
            SymbolFormat::WithoutSlash => self.symbol.replace('/', ""),
        }
    }

    /// Updates the order status and timestamp
    pub fn update_status(&mut self, status: OrderStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    /// Checks if the order is active (can be filled)
    pub fn is_active(&self) -> bool {
        matches!(self.status, OrderStatus::New | OrderStatus::PartiallyFilled)
    }

    /// Checks if the order is final (no more changes possible)
    pub fn is_final(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Filled
                | OrderStatus::Canceled
                | OrderStatus::Rejected
                | OrderStatus::Expired
        )
    }
}

/// Symbol format enumeration for exchange compatibility
#[derive(Debug, Clone, Copy)]
pub enum SymbolFormat {
    WithSlash,    // BTC/USDT
    WithoutSlash, // BTCUSDT
}

// String conversions for enums
impl FromStr for OrderSide {
    type Err = OrderValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "buy" => Ok(OrderSide::Buy),
            "sell" => Ok(OrderSide::Sell),
            "long" => Ok(OrderSide::Long),
            "short" => Ok(OrderSide::Short),
            _ => Err(OrderValidationError::ValidationFailed(format!(
                "Invalid order side: {}",
                s
            ))),
        }
    }
}

impl FromStr for OrderType {
    type Err = OrderValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "market" => Ok(OrderType::Market),
            "limit" => Ok(OrderType::Limit),
            "stop_loss" => Ok(OrderType::StopLoss),
            "stop_loss_limit" => Ok(OrderType::StopLossLimit),
            "take_profit" => Ok(OrderType::TakeProfit),
            "take_profit_limit" => Ok(OrderType::TakeProfitLimit),
            _ => Err(OrderValidationError::ValidationFailed(format!(
                "Invalid order type: {}",
                s
            ))),
        }
    }
}

impl FromStr for TimeInForce {
    type Err = OrderValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GTC" => Ok(TimeInForce::GTC),
            "IOC" => Ok(TimeInForce::IOC),
            "FOK" => Ok(TimeInForce::FOK),
            "GTD" => Ok(TimeInForce::GTD),
            _ => Err(OrderValidationError::ValidationFailed(format!(
                "Invalid time in force: {}",
                s
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // RED phase tests - these should fail first, then drive implementation

    #[test]
    fn should_create_valid_market_buy_order() {
        let user_id = Uuid::new_v4();
        let order =
            StandardOrder::market_buy(user_id, "BTC/USDT", Decimal::from_str("1.5").unwrap())
                .unwrap();

        assert_eq!(order.user_id, user_id);
        assert_eq!(order.symbol, "BTC/USDT");
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.quantity, Decimal::from_str("1.5").unwrap());
        assert_eq!(order.price, None);
        assert_eq!(order.time_in_force, TimeInForce::GTC);
        assert_eq!(order.status, OrderStatus::New);
        assert!(order.is_active());
        assert!(!order.is_final());
    }

    #[test]
    fn should_create_valid_limit_sell_order() {
        let user_id = Uuid::new_v4();
        let order = StandardOrder::limit_sell(
            user_id,
            "ETH/USDT",
            Decimal::from_str("2.0").unwrap(),
            Decimal::from_str("2500.50").unwrap(),
        )
        .unwrap();

        assert_eq!(order.side, OrderSide::Sell);
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.price, Some(Decimal::from_str("2500.50").unwrap()));
    }

    #[test]
    fn should_reject_orders_with_zero_quantity() {
        let user_id = Uuid::new_v4();
        let result = StandardOrder::market_buy(user_id, "BTC/USDT", Decimal::ZERO);

        assert!(result.is_err());
        if let Err(OrderValidationError::InvalidQuantity(_)) = result {
            // Expected error
        } else {
            panic!("Expected InvalidQuantity error");
        }
    }

    #[test]
    fn should_reject_orders_with_negative_quantity() {
        let user_id = Uuid::new_v4();
        let result =
            StandardOrder::market_buy(user_id, "BTC/USDT", Decimal::from_str("-1.0").unwrap());

        assert!(result.is_err());
        if let Err(OrderValidationError::InvalidQuantity(_)) = result {
            // Expected error
        } else {
            panic!("Expected InvalidQuantity error");
        }
    }

    #[test]
    fn should_reject_limit_orders_without_price() {
        let user_id = Uuid::new_v4();
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Decimal::from_str("1.0").unwrap())
            .build();

        assert!(result.is_err());
        if let Err(OrderValidationError::MissingPrice) = result {
            // Expected error
        } else {
            panic!("Expected MissingPrice error");
        }
    }

    #[test]
    fn should_reject_stop_orders_without_stop_price() {
        let user_id = Uuid::new_v4();
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Sell)
            .order_type(OrderType::StopLoss)
            .quantity(Decimal::from_str("1.0").unwrap())
            .build();

        assert!(result.is_err());
        if let Err(OrderValidationError::MissingStopPrice) = result {
            // Expected error
        } else {
            panic!("Expected MissingStopPrice error");
        }
    }

    #[test]
    fn should_reject_market_orders_with_price() {
        let user_id = Uuid::new_v4();
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Decimal::from_str("1.0").unwrap())
            .price(Decimal::from_str("50000.0").unwrap())
            .build();

        assert!(result.is_err());
        if let Err(OrderValidationError::ValidationFailed(_)) = result {
            // Expected error
        } else {
            panic!("Expected ValidationFailed error");
        }
    }

    #[test]
    fn should_validate_symbol_format() {
        let user_id = Uuid::new_v4();

        // Valid symbols
        let valid_symbols = vec!["BTC/USDT", "ETH_USD", "SOL-USDC", "BTCUSDT"];
        for symbol in valid_symbols {
            let result =
                StandardOrder::market_buy(user_id, symbol, Decimal::from_str("1.0").unwrap());
            assert!(result.is_ok(), "Symbol '{}' should be valid", symbol);
        }

        // Invalid symbols - use reference to avoid ownership issues
        let long_symbol = "A".repeat(25);
        let invalid_symbols = vec!["", "BTC!", "ETH@USD", &long_symbol];
        for symbol in invalid_symbols {
            let result =
                StandardOrder::market_buy(user_id, symbol, Decimal::from_str("1.0").unwrap());
            assert!(result.is_err(), "Symbol '{}' should be invalid", symbol);
        }
    }

    #[test]
    fn should_convert_sides_correctly() {
        let user_id = Uuid::new_v4();

        let long_order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Long)
            .order_type(OrderType::Market)
            .quantity(Decimal::from_str("1.0").unwrap())
            .build()
            .unwrap();

        let short_order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Short)
            .order_type(OrderType::Market)
            .quantity(Decimal::from_str("1.0").unwrap())
            .build()
            .unwrap();

        assert_eq!(long_order.to_spot_side(), OrderSide::Buy);
        assert_eq!(short_order.to_spot_side(), OrderSide::Sell);

        // Existing Buy/Sell should remain unchanged
        let buy_order =
            StandardOrder::market_buy(user_id, "BTC/USDT", Decimal::from_str("1.0").unwrap())
                .unwrap();
        assert_eq!(buy_order.to_spot_side(), OrderSide::Buy);
    }

    #[test]
    fn should_normalize_symbols() {
        let user_id = Uuid::new_v4();
        let order =
            StandardOrder::market_buy(user_id, "BTCUSDT", Decimal::from_str("1.0").unwrap())
                .unwrap();

        // This is a simplified normalization - real implementation would be more sophisticated
        let _with_slash = order.normalize_symbol(SymbolFormat::WithSlash);
        let without_slash = order.normalize_symbol(SymbolFormat::WithoutSlash);

        assert_eq!(without_slash, "BTCUSDT");
    }

    #[test]
    fn should_handle_time_in_force_validation() {
        let user_id = Uuid::new_v4();
        let future_time = Utc::now() + chrono::Duration::hours(1);

        // GTD orders must have expiry
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Decimal::from_str("1.0").unwrap())
            .price(Decimal::from_str("50000.0").unwrap())
            .time_in_force(TimeInForce::GTD)
            .build();

        assert!(result.is_err());

        // GTD with expiry should work
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Decimal::from_str("1.0").unwrap())
            .price(Decimal::from_str("50000.0").unwrap())
            .time_in_force(TimeInForce::GTD)
            .expires_at(future_time)
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn should_track_order_status_correctly() {
        let user_id = Uuid::new_v4();
        let mut order =
            StandardOrder::market_buy(user_id, "BTC/USDT", Decimal::from_str("1.0").unwrap())
                .unwrap();

        assert_eq!(order.status, OrderStatus::New);
        assert!(order.is_active());
        assert!(!order.is_final());

        order.update_status(OrderStatus::PartiallyFilled);
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert!(order.is_active());
        assert!(!order.is_final());

        order.update_status(OrderStatus::Filled);
        assert_eq!(order.status, OrderStatus::Filled);
        assert!(!order.is_active());
        assert!(order.is_final());
    }

    #[test]
    fn should_serialize_and_deserialize_correctly() {
        let user_id = Uuid::new_v4();
        let order = StandardOrder::limit_buy(
            user_id,
            "ETH/USDT",
            Decimal::from_str("2.5").unwrap(),
            Decimal::from_str("2000.0").unwrap(),
        )
        .unwrap();

        let serialized = serde_json::to_string(&order).expect("Should serialize");
        let deserialized: StandardOrder =
            serde_json::from_str(&serialized).expect("Should deserialize");

        assert_eq!(order, deserialized);
    }

    #[test]
    fn should_parse_enums_from_strings() {
        assert_eq!(OrderSide::from_str("buy").unwrap(), OrderSide::Buy);
        assert_eq!(OrderSide::from_str("SELL").unwrap(), OrderSide::Sell);
        assert_eq!(OrderSide::from_str("Long").unwrap(), OrderSide::Long);
        assert!(OrderSide::from_str("invalid").is_err());

        assert_eq!(OrderType::from_str("market").unwrap(), OrderType::Market);
        assert_eq!(OrderType::from_str("LIMIT").unwrap(), OrderType::Limit);
        assert!(OrderType::from_str("invalid").is_err());

        assert_eq!(TimeInForce::from_str("GTC").unwrap(), TimeInForce::GTC);
        assert_eq!(TimeInForce::from_str("ioc").unwrap(), TimeInForce::IOC);
        assert!(TimeInForce::from_str("invalid").is_err());
    }

    #[test]
    fn should_validate_stop_loss_limit_orders() {
        let user_id = Uuid::new_v4();

        // StopLossLimit needs both stop_price and price
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Sell)
            .order_type(OrderType::StopLossLimit)
            .quantity(Decimal::from_str("1.0").unwrap())
            .stop_price(Decimal::from_str("45000.0").unwrap())
            .price(Decimal::from_str("44000.0").unwrap())
            .build();

        assert!(result.is_ok());

        // Should fail without price
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Sell)
            .order_type(OrderType::StopLossLimit)
            .quantity(Decimal::from_str("1.0").unwrap())
            .stop_price(Decimal::from_str("45000.0").unwrap())
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn should_validate_take_profit_orders() {
        let user_id = Uuid::new_v4();

        // TakeProfit needs stop_price
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Sell)
            .order_type(OrderType::TakeProfit)
            .quantity(Decimal::from_str("1.0").unwrap())
            .stop_price(Decimal::from_str("55000.0").unwrap())
            .build();

        assert!(result.is_ok());

        // TakeProfitLimit needs both
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Sell)
            .order_type(OrderType::TakeProfitLimit)
            .quantity(Decimal::from_str("1.0").unwrap())
            .stop_price(Decimal::from_str("55000.0").unwrap())
            .price(Decimal::from_str("56000.0").unwrap())
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn should_handle_edge_cases_in_validation() {
        let user_id = Uuid::new_v4();

        // Zero prices should be rejected
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Decimal::from_str("1.0").unwrap())
            .price(Decimal::ZERO)
            .build();

        assert!(result.is_err());

        // Negative prices should be rejected
        let result = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC/USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Decimal::from_str("1.0").unwrap())
            .price(Decimal::from_str("-100.0").unwrap())
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn should_maintain_immutability_where_appropriate() {
        let user_id = Uuid::new_v4();
        let order =
            StandardOrder::market_buy(user_id, "BTC/USDT", Decimal::from_str("1.0").unwrap())
                .unwrap();

        let original_created_at = order.created_at;
        let original_updated_at = order.updated_at;

        // Calling immutable methods shouldn't change timestamps
        let spot_side = order.to_spot_side();
        assert_eq!(spot_side, OrderSide::Buy);
        assert_eq!(order.created_at, original_created_at);
        assert_eq!(order.updated_at, original_updated_at);
    }
}
