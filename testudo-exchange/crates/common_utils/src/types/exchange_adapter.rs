//! Exchange Adapter Types and Traits
//!
//! This module defines the core traits and types for exchange adapter implementations.
//! These are placed in common_utils to avoid circular dependencies and provide a
//! shared interface for all exchange integrations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ExchangeError, OrderValidationError, StandardOrder};

/// Exchange routing errors with detailed context
#[derive(Debug, Error)]
pub enum RoutingError {
    #[error("No available exchanges for routing")]
    NoAvailableExchanges,
    #[error("Exchange not found: {0}")]
    ExchangeNotFound(String),
    #[error("All exchanges are unhealthy")]
    AllExchangesUnhealthy,
    #[error("Routing strategy failed: {0}")]
    RoutingStrategyFailed(String),
    #[error("Order validation failed: {0}")]
    OrderValidationFailed(#[from] OrderValidationError),
    #[error("Exchange error: {0}")]
    ExchangeError(Box<dyn std::error::Error + Send + Sync>),
    #[error("Circuit breaker is open for exchange: {0}")]
    CircuitBreakerOpen(String),
    #[error("Timeout waiting for exchange response")]
    Timeout,
}

/// Conversion from RoutingError to ExchangeError for unified error handling
impl From<RoutingError> for ExchangeError {
    fn from(error: RoutingError) -> Self {
        match error {
            RoutingError::NoAvailableExchanges => {
                ExchangeError::ExchangeUnavailable("No exchanges available".to_string())
            }
            RoutingError::ExchangeNotFound(name) => {
                ExchangeError::ExchangeUnavailable(format!("Exchange {} not found", name))
            }
            RoutingError::AllExchangesUnhealthy => {
                ExchangeError::ExchangeUnavailable("All exchanges unhealthy".to_string())
            }
            RoutingError::RoutingStrategyFailed(reason) => {
                ExchangeError::ExchangeUnavailable(format!("Routing failed: {}", reason))
            }
            RoutingError::OrderValidationFailed(validation_error) => {
                ExchangeError::OrderRejected(format!("Validation failed: {}", validation_error))
            }
            RoutingError::ExchangeError(_) => {
                ExchangeError::ExchangeUnavailable("Exchange error".to_string())
            }
            RoutingError::CircuitBreakerOpen(exchange) => {
                ExchangeError::ExchangeUnavailable(format!("Circuit breaker open for {}", exchange))
            }
            RoutingError::Timeout => ExchangeError::ConnectionError("Exchange timeout".to_string()),
        }
    }
}

/// Response from exchange adapter operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub exchange_order_id: String,
    pub status: String,
    pub filled_quantity: rust_decimal::Decimal,
    pub remaining_quantity: rust_decimal::Decimal,
    pub average_price: Option<rust_decimal::Decimal>,
}

/// Exchange adapter trait for unified exchange interaction
#[async_trait]
pub trait ExchangeAdapter: Send + Sync {
    /// Place an order on the exchange
    async fn place_order(&self, order: &StandardOrder) -> Result<OrderResponse, RoutingError>;

    /// Cancel an existing order
    async fn cancel_order(&self, order_id: &str) -> Result<(), RoutingError>;

    /// Get the current status of an order
    async fn get_order_status(&self, order_id: &str) -> Result<OrderResponse, RoutingError>;

    /// Get the name/identifier of this exchange adapter
    fn get_name(&self) -> &str;

    /// Perform a health check on the exchange connection
    async fn health_check(&self) -> Result<(), RoutingError>;
}
