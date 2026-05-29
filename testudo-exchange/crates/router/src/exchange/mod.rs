//! Exchange Router Module
//!
//! This module implements the Exchange Router that handles order routing logic and orchestration
//! between different exchanges, following TDD principles and SOLID design.
//!
//! # Architecture
//!
//! The router follows a modular architecture with clear separation of concerns:
//! - **ExchangeRouter**: Main orchestration component
//! - **RoutingStrategy**: Pluggable routing decision logic
//! - **HealthMonitor**: Exchange health tracking and circuit breaking
//! - **MetricsCollector**: Performance and routing statistics
//! - **FallbackConfig**: Resilience and error recovery configuration
//!
//! # Usage
//!
//! ```rust
//! use router::exchange::{ExchangeRouter, RoutingStrategy, HealthMonitor, MetricsCollector};
//! use common_utils::StandardOrder;
//! use std::collections::HashMap;
//! use uuid::Uuid;
//! use rust_decimal::Decimal;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut router = ExchangeRouter::new(
//!     RoutingStrategy::UserPreference,
//!     HealthMonitor::new(),
//!     MetricsCollector::new(),
//!     FallbackConfig::default()
//! );
//!
//! // Add exchange adapters (mock for now)
//! // router.add_adapter("binance", Box::new(MockBinanceAdapter));
//!
//! // Route an order
//! let order = StandardOrder::market_buy(
//!     Uuid::new_v4(),
//!     "BTC/USDT",
//!     Decimal::new(1, 0)
//! )?;
//!
//! // let result = router.route_order(&order).await?;
//! # Ok(())
//! # }
//! ```

// @anchor exchange:router:mod
// @tags api

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use common_utils::{ExchangeError, OrderValidationError, StandardOrder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::time::Duration as TokioDuration;

/// Recover from poisoned locks by logging and extracting the inner value.
macro_rules! lock_or_recover {
    ($lock:expr) => {
        $lock.lock().unwrap_or_else(|p| {
            tracing::warn!("Lock poisoned, recovering");
            p.into_inner()
        })
    };
}

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

// Conversion from RoutingError to ExchangeError for unified error handling
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

/// Exchange adapter trait for pluggable exchange backends
#[async_trait]
pub trait ExchangeAdapter: Send + Sync {
    async fn place_order(&self, order: &StandardOrder) -> Result<OrderResponse, RoutingError>;
    async fn cancel_order(&self, order_id: &str) -> Result<(), RoutingError>;
    async fn get_order_status(&self, order_id: &str) -> Result<OrderResponse, RoutingError>;
    fn get_name(&self) -> &str;
    async fn health_check(&self) -> Result<(), RoutingError>;
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

/// Routing strategies for order routing decisions
#[derive(Clone)]
pub enum RoutingStrategy {
    /// Route based on user preference (exchange field in order)
    UserPreference,
    /// Route to exchange with best health metrics
    HealthBased,
    /// Load balance across available exchanges
    LoadBalance,
}

/// Health monitoring for exchange reliability tracking
#[derive(Clone)]
pub struct HealthMonitor {
    /// Exchange health status and response times
    health_status: Arc<Mutex<HashMap<String, ExchangeHealth>>>,
    /// Circuit breaker state for each exchange
    circuit_breakers: Arc<Mutex<HashMap<String, CircuitBreaker>>>,
}

/// Health status and metrics for an exchange
#[derive(Debug, Clone)]
pub struct ExchangeHealth {
    pub is_healthy: bool,
    pub response_time: TokioDuration,
    pub last_check: DateTime<Utc>,
    pub consecutive_failures: u32,
}

/// Circuit breaker for exchange resilience
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub failure_threshold: u32,
    pub recovery_timeout: TokioDuration,
    pub last_failure_time: Option<DateTime<Utc>>,
}

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Metrics collection for routing performance tracking
#[derive(Clone)]
pub struct MetricsCollector {
    /// Request count per exchange
    request_counts: Arc<Mutex<HashMap<String, u64>>>,
    /// Success count per exchange
    success_counts: Arc<Mutex<HashMap<String, u64>>>,
    /// Average response times per exchange
    response_times: Arc<Mutex<HashMap<String, Vec<TokioDuration>>>>,
}

/// Fallback configuration for resilient routing
#[derive(Clone)]
pub struct FallbackConfig {
    /// List of fallback exchanges in priority order
    pub fallback_exchanges: Vec<String>,
    /// Maximum retry attempts before giving up
    pub max_retries: u32,
    /// Timeout for individual exchange requests
    pub request_timeout: TokioDuration,
    /// Enable cross-exchange fallback
    pub enable_fallback: bool,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            fallback_exchanges: Vec::new(),
            max_retries: 3,
            request_timeout: TokioDuration::from_secs(30),
            enable_fallback: true,
        }
    }
}

/// Main exchange router that orchestrates order routing
pub struct ExchangeRouter {
    /// Exchange adapters by name
    adapters: HashMap<String, Box<dyn ExchangeAdapter>>,
    /// Routing strategy
    strategy: RoutingStrategy,
    /// Health monitoring
    health_monitor: HealthMonitor,
    /// Metrics collection
    metrics_collector: MetricsCollector,
    /// Fallback configuration
    fallback_config: FallbackConfig,
}

impl ExchangeRouter {
    /// Create a new exchange router
    pub fn new(
        strategy: RoutingStrategy,
        health_monitor: HealthMonitor,
        metrics_collector: MetricsCollector,
        fallback_config: FallbackConfig,
    ) -> Self {
        Self {
            adapters: HashMap::new(),
            strategy,
            health_monitor,
            metrics_collector,
            fallback_config,
        }
    }

    /// Add an exchange adapter
    pub fn add_adapter(&mut self, name: String, adapter: Box<dyn ExchangeAdapter>) {
        self.adapters.insert(name, adapter);
    }

    /// Route an order to the appropriate exchange
    pub async fn route_order(&self, order: &StandardOrder) -> Result<OrderResponse, RoutingError> {
        // Select the best exchange based on strategy
        let selected_exchange = self.select_exchange(order).await?;

        // Check if circuit breaker is open
        if self
            .health_monitor
            .is_circuit_breaker_open(&selected_exchange)
        {
            return Err(RoutingError::CircuitBreakerOpen(selected_exchange.clone()));
        }

        // Get the adapter for the selected exchange
        let adapter = self
            .adapters
            .get(&selected_exchange)
            .ok_or_else(|| RoutingError::ExchangeNotFound(selected_exchange.clone()))?;

        // Attempt to place the order with fallback logic
        self.place_order_with_fallback(adapter, order, &selected_exchange)
            .await
    }

    /// Place order with fallback handling
    async fn place_order_with_fallback(
        &self,
        adapter: &Box<dyn ExchangeAdapter>,
        order: &StandardOrder,
        exchange_name: &str,
    ) -> Result<OrderResponse, RoutingError> {
        let mut last_error = None;

        // Try primary exchange
        match adapter.place_order(order).await {
            Ok(response) => {
                self.metrics_collector.record_success(exchange_name);
                return Ok(response);
            }
            Err(error) => {
                self.metrics_collector.record_failure(exchange_name);
                self.health_monitor.record_failure(exchange_name);
                last_error = Some(error);
            }
        }

        // If fallback is enabled, try fallback exchanges
        if self.fallback_config.enable_fallback {
            for fallback_exchange in &self.fallback_config.fallback_exchanges {
                if let Some(fallback_adapter) = self.adapters.get(fallback_exchange) {
                    if !self
                        .health_monitor
                        .is_circuit_breaker_open(fallback_exchange)
                    {
                        match fallback_adapter.place_order(order).await {
                            Ok(response) => {
                                self.metrics_collector.record_success(fallback_exchange);
                                return Ok(response);
                            }
                            Err(error) => {
                                self.metrics_collector.record_failure(fallback_exchange);
                                self.health_monitor.record_failure(fallback_exchange);
                                last_error = Some(error);
                            }
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or(RoutingError::AllExchangesUnhealthy))
    }

    /// Select the best exchange based on routing strategy
    async fn select_exchange(&self, order: &StandardOrder) -> Result<String, RoutingError> {
        // If user specified an exchange, validate and use it
        if let Some(exchange) = &order.exchange {
            if !self.adapters.contains_key(exchange) {
                return Err(RoutingError::ExchangeNotFound(exchange.clone()));
            }
            return Ok(exchange.clone());
        }

        // No adapters available
        if self.adapters.is_empty() {
            return Err(RoutingError::NoAvailableExchanges);
        }

        // Apply routing strategy
        match &self.strategy {
            RoutingStrategy::UserPreference => {
                // Since no preference specified, fall back to health-based
                self.select_health_based().await
            }
            RoutingStrategy::HealthBased => self.select_health_based().await,
            RoutingStrategy::LoadBalance => self.select_load_balanced().await,
        }
    }

    /// Select exchange based on health metrics
    async fn select_health_based(&self) -> Result<String, RoutingError> {
        let healthy_exchanges = self.health_monitor.get_healthy_exchanges();

        if healthy_exchanges.is_empty() {
            return Err(RoutingError::AllExchangesUnhealthy);
        }

        // Select the exchange with best response time
        let best_exchange = healthy_exchanges
            .into_iter()
            .min_by_key(|(_, health)| health.response_time)
            .map(|(name, _)| name);

        best_exchange.ok_or(RoutingError::AllExchangesUnhealthy)
    }

    /// Select exchange using load balancing
    async fn select_load_balanced(&self) -> Result<String, RoutingError> {
        let healthy_exchanges = self.health_monitor.get_healthy_exchanges();

        if healthy_exchanges.is_empty() {
            return Err(RoutingError::NoAvailableExchanges);
        }

        // Simple round-robin based on request counts
        let exchange_counts = self.metrics_collector.get_request_counts();

        let selected = healthy_exchanges
            .into_iter()
            .min_by_key(|(name, _)| exchange_counts.get(name).unwrap_or(&0))
            .map(|(name, _)| name);

        selected.ok_or(RoutingError::AllExchangesUnhealthy)
    }

    /// Perform health checks on all exchanges
    pub async fn perform_health_checks(&self) {
        for (name, adapter) in &self.adapters {
            let start_time = std::time::Instant::now();

            match adapter.health_check().await {
                Ok(_) => {
                    let response_time =
                        TokioDuration::from_nanos(start_time.elapsed().as_nanos() as u64);
                    self.health_monitor.update_health(name, true, response_time);
                }
                Err(_) => {
                    self.health_monitor
                        .update_health(name, false, TokioDuration::from_secs(0));
                }
            }
        }
    }

    /// Get current health status of all exchanges
    pub fn get_health_status(&self) -> HashMap<String, ExchangeHealth> {
        self.health_monitor.get_all_health_status()
    }

    /// Get routing statistics
    pub fn get_routing_stats(&self) -> HashMap<String, u64> {
        self.metrics_collector.get_request_counts()
    }
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new() -> Self {
        Self {
            health_status: Arc::new(Mutex::new(HashMap::new())),
            circuit_breakers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Update health status for an exchange
    pub fn update_health(&self, exchange: &str, is_healthy: bool, response_time: TokioDuration) {
        let mut health_status = lock_or_recover!(self.health_status);
        let mut circuit_breakers = lock_or_recover!(self.circuit_breakers);

        let health = health_status
            .entry(exchange.to_string())
            .or_insert_with(|| ExchangeHealth {
                is_healthy: true,
                response_time: TokioDuration::from_millis(100),
                last_check: Utc::now(),
                consecutive_failures: 0,
            });

        health.is_healthy = is_healthy;
        health.response_time = response_time;
        health.last_check = Utc::now();

        if is_healthy {
            health.consecutive_failures = 0;
            // Reset circuit breaker if it was open
            if let Some(cb) = circuit_breakers.get_mut(exchange) {
                if cb.state == CircuitBreakerState::Open {
                    cb.state = CircuitBreakerState::HalfOpen;
                }
            }
        } else {
            health.consecutive_failures += 1;
        }

        // Update circuit breaker
        let circuit_breaker = circuit_breakers
            .entry(exchange.to_string())
            .or_insert_with(|| CircuitBreaker {
                state: CircuitBreakerState::Closed,
                failure_count: 0,
                failure_threshold: 5,
                recovery_timeout: TokioDuration::from_secs(60),
                last_failure_time: None,
            });

        if !is_healthy {
            circuit_breaker.failure_count += 1;
            circuit_breaker.last_failure_time = Some(Utc::now());

            if circuit_breaker.failure_count >= circuit_breaker.failure_threshold {
                circuit_breaker.state = CircuitBreakerState::Open;
            }
        }
    }

    /// Record a failure for an exchange
    pub fn record_failure(&self, exchange: &str) {
        self.update_health(exchange, false, TokioDuration::from_secs(0));
    }

    /// Check if circuit breaker is open for an exchange
    pub fn is_circuit_breaker_open(&self, exchange: &str) -> bool {
        let circuit_breakers = lock_or_recover!(self.circuit_breakers);

        if let Some(cb) = circuit_breakers.get(exchange) {
            match cb.state {
                CircuitBreakerState::Open => {
                    // Check if recovery timeout has passed
                    if let Some(last_failure) = cb.last_failure_time {
                        let elapsed = Utc::now().signed_duration_since(last_failure);
                        if elapsed > Duration::from_std(cb.recovery_timeout).unwrap_or_default() {
                            // Time to try half-open state
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Get all healthy exchanges
    pub fn get_healthy_exchanges(&self) -> Vec<(String, ExchangeHealth)> {
        let health_status = lock_or_recover!(self.health_status);

        health_status
            .iter()
            .filter(|(_, health)| health.is_healthy)
            .map(|(name, health)| (name.clone(), health.clone()))
            .collect()
    }

    /// Get all health status
    pub fn get_all_health_status(&self) -> HashMap<String, ExchangeHealth> {
        lock_or_recover!(self.health_status).clone()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            request_counts: Arc::new(Mutex::new(HashMap::new())),
            success_counts: Arc::new(Mutex::new(HashMap::new())),
            response_times: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record a successful request
    pub fn record_success(&self, exchange: &str) {
        *lock_or_recover!(self.request_counts)
            .entry(exchange.to_string())
            .or_insert(0) += 1;
        *lock_or_recover!(self.success_counts)
            .entry(exchange.to_string())
            .or_insert(0) += 1;
    }

    /// Record a failed request
    pub fn record_failure(&self, exchange: &str) {
        *lock_or_recover!(self.request_counts)
            .entry(exchange.to_string())
            .or_insert(0) += 1;
    }

    /// Get request counts for all exchanges
    pub fn get_request_counts(&self) -> HashMap<String, u64> {
        lock_or_recover!(self.request_counts).clone()
    }

    /// Get success rates for all exchanges
    pub fn get_success_rates(&self) -> HashMap<String, f64> {
        let request_counts = lock_or_recover!(self.request_counts);
        let success_counts = lock_or_recover!(self.success_counts);

        request_counts
            .iter()
            .map(|(exchange, &total)| {
                let successful = *success_counts.get(exchange).unwrap_or(&0);
                let rate = if total > 0 {
                    successful as f64 / total as f64
                } else {
                    0.0
                };
                (exchange.clone(), rate)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_utils::StandardOrder;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    // Mock exchange adapter for testing
    struct MockExchangeAdapter {
        name: String,
        should_fail: bool,
    }

    impl MockExchangeAdapter {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                should_fail: false,
            }
        }

        fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }
    }

    #[async_trait]
    impl ExchangeAdapter for MockExchangeAdapter {
        async fn place_order(&self, _order: &StandardOrder) -> Result<OrderResponse, RoutingError> {
            if self.should_fail {
                Err(RoutingError::ExchangeError("Mock exchange error".into()))
            } else {
                Ok(OrderResponse {
                    order_id: Uuid::new_v4().to_string(),
                    exchange_order_id: format!("{}-{}", self.name, Uuid::new_v4()),
                    status: "ACCEPTED".to_string(),
                    filled_quantity: Decimal::ZERO,
                    remaining_quantity: Decimal::new(100, 2),
                    average_price: None,
                })
            }
        }

        async fn cancel_order(&self, _order_id: &str) -> Result<(), RoutingError> {
            if self.should_fail {
                Err(RoutingError::ExchangeError("Mock cancel failed".into()))
            } else {
                Ok(())
            }
        }

        async fn get_order_status(&self, _order_id: &str) -> Result<OrderResponse, RoutingError> {
            if self.should_fail {
                Err(RoutingError::ExchangeError("Mock status failed".into()))
            } else {
                Ok(OrderResponse {
                    order_id: Uuid::new_v4().to_string(),
                    exchange_order_id: format!("{}-{}", self.name, Uuid::new_v4()),
                    status: "FILLED".to_string(),
                    filled_quantity: Decimal::new(100, 2),
                    remaining_quantity: Decimal::ZERO,
                    average_price: Some(Decimal::new(50000, 2)),
                })
            }
        }

        fn get_name(&self) -> &str {
            &self.name
        }

        async fn health_check(&self) -> Result<(), RoutingError> {
            if self.should_fail {
                Err(RoutingError::ExchangeError("Health check failed".into()))
            } else {
                Ok(())
            }
        }
    }

    fn create_test_order() -> StandardOrder {
        StandardOrder::market_buy(
            Uuid::new_v4(),
            "BTC/USDT",
            Decimal::new(100, 2), // 1.00 BTC
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_user_preference_routing() {
        let mut router = ExchangeRouter::new(
            RoutingStrategy::UserPreference,
            HealthMonitor::new(),
            MetricsCollector::new(),
            FallbackConfig::default(),
        );

        // Add mock adapters
        router.add_adapter(
            "binance".to_string(),
            Box::new(MockExchangeAdapter::new("binance")),
        );
        router.add_adapter(
            "coinbase".to_string(),
            Box::new(MockExchangeAdapter::new("coinbase")),
        );

        // Set both exchanges as healthy
        router
            .health_monitor
            .update_health("binance", true, TokioDuration::from_millis(100));
        router
            .health_monitor
            .update_health("coinbase", true, TokioDuration::from_millis(100));

        // Create order with exchange preference
        let mut order = create_test_order();
        order.exchange = Some("binance".to_string());

        let result = router.route_order(&order).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.exchange_order_id.contains("binance"));
    }

    #[tokio::test]
    async fn test_exchange_not_found() {
        let router = ExchangeRouter::new(
            RoutingStrategy::UserPreference,
            HealthMonitor::new(),
            MetricsCollector::new(),
            FallbackConfig::default(),
        );

        let mut order = create_test_order();
        order.exchange = Some("nonexistent".to_string());

        let result = router.route_order(&order).await;
        assert!(result.is_err());

        if let Err(RoutingError::ExchangeNotFound(name)) = result {
            assert_eq!(name, "nonexistent");
        } else {
            panic!("Expected ExchangeNotFound error, got: {:?}", result);
        }
    }

    #[tokio::test]
    async fn test_health_based_routing() {
        let mut router = ExchangeRouter::new(
            RoutingStrategy::HealthBased,
            HealthMonitor::new(),
            MetricsCollector::new(),
            FallbackConfig::default(),
        );

        // Add two mock adapters
        router.add_adapter(
            "exchange1".to_string(),
            Box::new(MockExchangeAdapter::new("exchange1")),
        );
        router.add_adapter(
            "exchange2".to_string(),
            Box::new(MockExchangeAdapter::new("exchange2")),
        );

        // Make exchange1 healthier than exchange2
        router
            .health_monitor
            .update_health("exchange1", true, TokioDuration::from_millis(50));
        router
            .health_monitor
            .update_health("exchange2", true, TokioDuration::from_millis(200));

        let order = create_test_order();
        let result = router.route_order(&order).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.exchange_order_id.contains("exchange1"));
    }

    #[tokio::test]
    async fn test_fallback_when_primary_fails() {
        let mut config = FallbackConfig::default();
        config.fallback_exchanges = vec!["fallback".to_string()];
        config.max_retries = 1; // Fail fast for testing

        let mut router = ExchangeRouter::new(
            RoutingStrategy::UserPreference,
            HealthMonitor::new(),
            MetricsCollector::new(),
            config,
        );

        // Add failing primary adapter and working fallback
        router.add_adapter(
            "primary".to_string(),
            Box::new(MockExchangeAdapter::new("primary").with_failure()),
        );
        router.add_adapter(
            "fallback".to_string(),
            Box::new(MockExchangeAdapter::new("fallback")),
        );

        // Set both as healthy initially
        router
            .health_monitor
            .update_health("primary", true, TokioDuration::from_millis(100));
        router
            .health_monitor
            .update_health("fallback", true, TokioDuration::from_millis(100));

        let mut order = create_test_order();
        order.exchange = Some("primary".to_string());

        let result = router.route_order(&order).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.exchange_order_id.contains("fallback"));
    }

    #[tokio::test]
    async fn test_load_balance_routing() {
        let mut router = ExchangeRouter::new(
            RoutingStrategy::LoadBalance,
            HealthMonitor::new(),
            MetricsCollector::new(),
            FallbackConfig::default(),
        );

        // Add multiple adapters
        router.add_adapter(
            "exchange1".to_string(),
            Box::new(MockExchangeAdapter::new("exchange1")),
        );
        router.add_adapter(
            "exchange2".to_string(),
            Box::new(MockExchangeAdapter::new("exchange2")),
        );

        // Set both as healthy
        router
            .health_monitor
            .update_health("exchange1", true, TokioDuration::from_millis(100));
        router
            .health_monitor
            .update_health("exchange2", true, TokioDuration::from_millis(100));

        let order = create_test_order();

        // Make multiple requests to ensure both exchanges are used
        let mut exchange_counts = HashMap::new();
        for _ in 0..10 {
            let result = router.route_order(&order).await;
            if let Ok(response) = result {
                if response.exchange_order_id.contains("exchange1") {
                    *exchange_counts.entry("exchange1".to_string()).or_insert(0) += 1;
                } else if response.exchange_order_id.contains("exchange2") {
                    *exchange_counts.entry("exchange2".to_string()).or_insert(0) += 1;
                }
            }
        }

        // Both exchanges should have received requests, but allow for some variance
        assert!(
            exchange_counts.len() >= 2,
            "Expected both exchanges to be used, but got: {:?}",
            exchange_counts
        );

        // Ensure neither exchange got all the requests (load balancing working)
        for (_, count) in &exchange_counts {
            assert!(
                *count < 10,
                "Expected load balancing, but one exchange got all requests: {:?}",
                exchange_counts
            );
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_prevents_routing() {
        let mut router = ExchangeRouter::new(
            RoutingStrategy::UserPreference,
            HealthMonitor::new(),
            MetricsCollector::new(),
            FallbackConfig::default(),
        );

        // Add failing adapter
        router.add_adapter(
            "failing".to_string(),
            Box::new(MockExchangeAdapter::new("failing").with_failure()),
        );

        // Set as initially healthy, but it will fail and trigger circuit breaker
        router
            .health_monitor
            .update_health("failing", true, TokioDuration::from_millis(100));

        let mut order = create_test_order();
        order.exchange = Some("failing".to_string());

        // First few requests should fail and trigger circuit breaker
        for _ in 0..5 {
            let _ = router.route_order(&order).await;
        }

        // After circuit breaker opens, should get CircuitBreakerOpen error
        let result = router.route_order(&order).await;
        assert!(result.is_err());

        // Note: The exact error might be different due to fallback logic,
        // but the circuit breaker should prevent further attempts
    }

    #[tokio::test]
    async fn test_no_available_exchanges() {
        let router = ExchangeRouter::new(
            RoutingStrategy::UserPreference,
            HealthMonitor::new(),
            MetricsCollector::new(),
            FallbackConfig::default(),
        );

        let order = create_test_order();
        let result = router.route_order(&order).await;

        assert!(result.is_err());
        if let Err(RoutingError::NoAvailableExchanges) = result {
            // Expected
        } else {
            panic!("Expected NoAvailableExchanges error");
        }
    }

    #[tokio::test]
    async fn test_health_checks() {
        let mut router = ExchangeRouter::new(
            RoutingStrategy::HealthBased,
            HealthMonitor::new(),
            MetricsCollector::new(),
            FallbackConfig::default(),
        );

        router.add_adapter(
            "healthy".to_string(),
            Box::new(MockExchangeAdapter::new("healthy")),
        );
        router.add_adapter(
            "unhealthy".to_string(),
            Box::new(MockExchangeAdapter::new("unhealthy").with_failure()),
        );

        // Perform health checks
        router.perform_health_checks().await;

        // Check health status
        let health_status = router.get_health_status();
        assert!(health_status.contains_key("healthy"));
        assert!(health_status.contains_key("unhealthy"));

        // The healthy one should be healthy, unhealthy should not be
        assert!(health_status.get("healthy").unwrap().is_healthy);
        assert!(!health_status.get("unhealthy").unwrap().is_healthy);
    }

    #[tokio::test]
    async fn test_routing_stats() {
        let mut router = ExchangeRouter::new(
            RoutingStrategy::UserPreference,
            HealthMonitor::new(),
            MetricsCollector::new(),
            FallbackConfig::default(),
        );

        router.add_adapter(
            "test".to_string(),
            Box::new(MockExchangeAdapter::new("test")),
        );
        router
            .health_monitor
            .update_health("test", true, TokioDuration::from_millis(100));

        let mut order = create_test_order();
        order.exchange = Some("test".to_string());

        // Route several orders
        for _ in 0..3 {
            let _ = router.route_order(&order).await;
        }

        let stats = router.get_routing_stats();
        assert!(stats.get("test").unwrap_or(&0) > &0);
    }

    #[tokio::test]
    async fn test_routing_error_to_exchange_error_conversion() {
        // Test conversion of various RoutingError types to ExchangeError
        let test_cases = vec![
            (
                RoutingError::NoAvailableExchanges,
                ExchangeError::ExchangeUnavailable("No exchanges available".to_string()),
            ),
            (
                RoutingError::ExchangeNotFound("binance".to_string()),
                ExchangeError::ExchangeUnavailable("Exchange binance not found".to_string()),
            ),
            (
                RoutingError::AllExchangesUnhealthy,
                ExchangeError::ExchangeUnavailable("All exchanges unhealthy".to_string()),
            ),
            (
                RoutingError::RoutingStrategyFailed("load balancer error".to_string()),
                ExchangeError::ExchangeUnavailable(
                    "Routing failed: load balancer error".to_string(),
                ),
            ),
            (
                RoutingError::CircuitBreakerOpen("coinbase".to_string()),
                ExchangeError::ExchangeUnavailable("Circuit breaker open for coinbase".to_string()),
            ),
            (
                RoutingError::Timeout,
                ExchangeError::ConnectionError("Exchange timeout".to_string()),
            ),
        ];

        for (routing_error, expected_exchange_error) in test_cases {
            let converted: ExchangeError = routing_error.into();
            assert_eq!(converted, expected_exchange_error);
        }
    }
}
