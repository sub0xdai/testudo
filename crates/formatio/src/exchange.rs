//! Exchange adapter interface for OODA loop market data and trade execution
//!
//! This module provides the exchange integration layer for the OODA loop,
//! following Roman military principles of reliable intelligence and swift execution.

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

/// Unified exchange adapter trait for OODA loop integration
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

/// Mock exchange adapter for testing OODA loop integration
pub struct MockExchange {
    state: std::sync::Arc<tokio::sync::RwLock<MockExchangeState>>,
    name: String,
}

#[derive(Debug, Clone)]
struct MockExchangeState {
    /// Market data by symbol
    pub market_data: std::collections::HashMap<String, MarketData>,
    /// Account balances by asset
    pub balances: std::collections::HashMap<String, AccountBalance>,
    /// Orders placed (order_id -> OrderResult)
    pub orders: std::collections::HashMap<String, OrderResult>,
    /// Whether the exchange is healthy
    pub is_healthy: bool,
    /// Counter for generating order IDs
    pub order_counter: u64,
}

impl MockExchange {
    /// Create a new mock exchange with default test data
    pub fn new() -> Self {
        use rust_decimal_macros::dec;
        use std::collections::HashMap;
        
        let mut market_data = HashMap::new();
        let mut balances = HashMap::new();
        
        // Default BTC market data
        market_data.insert(
            "BTC/USDT".to_string(),
            MarketData {
                symbol: "BTC/USDT".to_string(),
                bid_price: dec!(49900.0),
                ask_price: dec!(50100.0),
                last_price: dec!(50000.0),
                volume_24h: dec!(1000.0),
                timestamp: SystemTime::now(),
            },
        );
        
        // Default ETH market data
        market_data.insert(
            "ETH/USDT".to_string(),
            MarketData {
                symbol: "ETH/USDT".to_string(),
                bid_price: dec!(2990.0),
                ask_price: dec!(3010.0),
                last_price: dec!(3000.0),
                volume_24h: dec!(5000.0),
                timestamp: SystemTime::now(),
            },
        );
        
        // Default USDT balance
        balances.insert(
            "USDT".to_string(),
            AccountBalance {
                asset: "USDT".to_string(),
                free: dec!(10000.0),
                locked: dec!(0.0),
                total: dec!(10000.0),
            },
        );
        
        // Default BTC balance
        balances.insert(
            "BTC".to_string(),
            AccountBalance {
                asset: "BTC".to_string(),
                free: dec!(0.5),
                locked: dec!(0.0),
                total: dec!(0.5),
            },
        );
        
        let state = MockExchangeState {
            market_data,
            balances,
            orders: HashMap::new(),
            is_healthy: true,
            order_counter: 1000,
        };
        
        Self {
            state: std::sync::Arc::new(tokio::sync::RwLock::new(state)),
            name: "MockExchange".to_string(),
        }
    }
    
    /// Set market data for a symbol
    pub async fn set_market_data(&self, symbol: String, data: MarketData) {
        let mut state = self.state.write().await;
        state.market_data.insert(symbol, data);
    }
    
    /// Set account balance for an asset
    pub async fn set_balance(&self, asset: String, balance: AccountBalance) {
        let mut state = self.state.write().await;
        state.balances.insert(asset, balance);
    }
    
    /// Set exchange health status
    pub async fn set_health(&self, is_healthy: bool) {
        let mut state = self.state.write().await;
        state.is_healthy = is_healthy;
    }
    
    /// Get all orders placed on this mock exchange
    pub async fn get_placed_orders(&self) -> Vec<OrderResult> {
        let state = self.state.read().await;
        state.orders.values().cloned().collect()
    }
    
    /// Clear all orders (useful for test cleanup)
    pub async fn clear_orders(&self) {
        let mut state = self.state.write().await;
        state.orders.clear();
    }
}

impl Default for MockExchange {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExchangeAdapterTrait for MockExchange {
    async fn get_market_data(&self, symbol: &str) -> Result<MarketData, ExchangeError> {
        let state = self.state.read().await;
        
        if !state.is_healthy {
            return Err(ExchangeError::ConnectionError {
                message: "Mock exchange is unhealthy".to_string(),
            });
        }
        
        state
            .market_data
            .get(symbol)
            .cloned()
            .ok_or_else(|| ExchangeError::MarketDataUnavailable {
                symbol: symbol.to_string(),
            })
    }
    
    async fn place_order(&self, order: &TradeOrder) -> Result<OrderResult, ExchangeError> {
        use rust_decimal_macros::dec;
        
        let mut state = self.state.write().await;
        
        if !state.is_healthy {
            return Err(ExchangeError::ConnectionError {
                message: "Mock exchange is unhealthy".to_string(),
            });
        }
        
        // Generate order ID
        state.order_counter += 1;
        let order_id = format!("MOCK-{}", state.order_counter);
        
        // Create order result (simplified - always fills immediately)
        let result = OrderResult {
            order_id: order_id.clone(),
            client_order_id: order.client_order_id.clone(),
            symbol: order.symbol.clone(),
            status: OrderStatus::Filled,
            executed_quantity: order.quantity,
            executed_price: order.price.unwrap_or_else(|| {
                state
                    .market_data
                    .get(&order.symbol)
                    .map(|d| d.last_price)
                    .unwrap_or(dec!(50000.0))
            }),
            commission: order.quantity * dec!(0.001), // 0.1% commission
            timestamp: SystemTime::now(),
        };
        
        // Store order
        state.orders.insert(order_id.clone(), result.clone());
        
        Ok(result)
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<(), ExchangeError> {
        let mut state = self.state.write().await;
        
        if !state.is_healthy {
            return Err(ExchangeError::ConnectionError {
                message: "Mock exchange is unhealthy".to_string(),
            });
        }
        
        match state.orders.get_mut(order_id) {
            Some(order) => {
                order.status = OrderStatus::Cancelled;
                Ok(())
            }
            None => Err(ExchangeError::OrderNotFound {
                order_id: order_id.to_string(),
            }),
        }
    }
    
    async fn get_order_status(&self, order_id: &str) -> Result<OrderResult, ExchangeError> {
        let state = self.state.read().await;
        
        if !state.is_healthy {
            return Err(ExchangeError::ConnectionError {
                message: "Mock exchange is unhealthy".to_string(),
            });
        }
        
        state
            .orders
            .get(order_id)
            .cloned()
            .ok_or_else(|| ExchangeError::OrderNotFound {
                order_id: order_id.to_string(),
            })
    }
    
    async fn get_balance(&self, asset: &str) -> Result<AccountBalance, ExchangeError> {
        let state = self.state.read().await;
        
        if !state.is_healthy {
            return Err(ExchangeError::ConnectionError {
                message: "Mock exchange is unhealthy".to_string(),
            });
        }
        
        state
            .balances
            .get(asset)
            .cloned()
            .ok_or(ExchangeError::InsufficientBalance)
    }
    
    async fn get_all_balances(&self) -> Result<Vec<AccountBalance>, ExchangeError> {
        let state = self.state.read().await;
        
        if !state.is_healthy {
            return Err(ExchangeError::ConnectionError {
                message: "Mock exchange is unhealthy".to_string(),
            });
        }
        
        Ok(state.balances.values().cloned().collect())
    }
    
    async fn health_check(&self) -> Result<bool, ExchangeError> {
        let state = self.state.read().await;
        Ok(state.is_healthy)
    }
    
    fn exchange_name(&self) -> &str {
        &self.name
    }
    
    async fn is_symbol_supported(&self, symbol: &str) -> Result<bool, ExchangeError> {
        let state = self.state.read().await;
        
        if !state.is_healthy {
            return Err(ExchangeError::ConnectionError {
                message: "Mock exchange is unhealthy".to_string(),
            });
        }
        
        Ok(state.market_data.contains_key(symbol))
    }
}