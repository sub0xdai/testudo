//! Mock exchange implementation for testing
//!
//! Provides a controllable exchange adapter for unit and integration testing
//! of the OODA loop and risk management systems.

use testudo_types::{
    AccountBalance, ExchangeAdapterTrait, ExchangeError, MarketData, 
    OrderResult, OrderStatus, TradeOrder, OrderSide
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, Duration};
use tokio::sync::RwLock;
use rust_decimal_macros::dec;

/// Mock exchange state for testing
#[derive(Debug, Clone)]
pub struct MockExchangeState {
    /// Market data by symbol
    pub market_data: HashMap<String, MarketData>,
    /// Account balances by asset
    pub balances: HashMap<String, AccountBalance>,
    /// Orders placed (order_id -> OrderResult)
    pub orders: HashMap<String, OrderResult>,
    /// Whether the exchange is healthy
    pub is_healthy: bool,
    /// Counter for generating order IDs
    pub order_counter: u64,
    /// Simulated response delay for testing timeouts
    pub response_delay: Option<Duration>,
}

impl Default for MockExchangeState {
    fn default() -> Self {
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
        
        Self {
            market_data,
            balances,
            orders: HashMap::new(),
            is_healthy: true,
            order_counter: 1000,
            response_delay: None,
        }
    }
}

/// Mock exchange adapter for testing
pub struct MockExchange {
    state: Arc<RwLock<MockExchangeState>>,
    name: String,
}

impl MockExchange {
    /// Create a new mock exchange with default state
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MockExchangeState::default())),
            name: "MockExchange".to_string(),
        }
    }
    
    /// Create a mock exchange with custom name
    pub fn with_name(name: String) -> Self {
        Self {
            state: Arc::new(RwLock::new(MockExchangeState::default())),
            name,
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

    /// Set response delay for timeout simulation
    pub async fn set_response_delay(&self, delay: Duration) {
        let mut state = self.state.write().await;
        state.response_delay = Some(delay);
    }
    
    /// Clear response delay
    pub async fn clear_response_delay(&self) {
        let mut state = self.state.write().await;
        state.response_delay = None;
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
        
        // Simulate response delay if configured
        if let Some(delay) = state.response_delay {
            drop(state); // Release lock before sleeping
            tokio::time::sleep(delay).await;
            let state = self.state.read().await; // Re-acquire lock
            
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
        } else {
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
    }
    
    async fn place_order(&self, order: &TradeOrder) -> Result<OrderResult, ExchangeError> {
        let mut state = self.state.write().await;
        
        if !state.is_healthy {
            return Err(ExchangeError::ConnectionError {
                message: "Mock exchange is unhealthy".to_string(),
            });
        }
        
        // Extract asset from symbol (e.g., "BTC/USDT" -> "USDT" for buy, "BTC" for sell)
        let asset = if order.side == OrderSide::Buy {
            order.symbol.split('/').nth(1).unwrap_or("USDT")
        } else {
            order.symbol.split('/').nth(0).unwrap_or("BTC")
        };
        
        // Check balance
        let balance = state.balances.get(asset).ok_or(ExchangeError::InsufficientBalance)?;
        
        // Simple balance check (in real implementation would be more complex)
        let required = order.quantity * order.price.unwrap_or(dec!(50000.0));
        if balance.free < required && order.side == OrderSide::Buy {
            return Err(ExchangeError::InsufficientBalance);
        }
        
        // Generate order ID
        state.order_counter += 1;
        let order_id = format!("MOCK-{}", state.order_counter);
        
        // Create order result
        let result = OrderResult {
            order_id: order_id.clone(),
            client_order_id: order.client_order_id.clone(),
            symbol: order.symbol.clone(),
            status: OrderStatus::Filled,  // Mock always fills immediately
            executed_quantity: order.quantity,
            executed_price: order.price.unwrap_or_else(|| {
                state
                    .market_data
                    .get(&order.symbol)
                    .map(|d| d.last_price)
                    .unwrap_or(dec!(50000.0))
            }),
            commission: order.quantity * dec!(0.001),  // 0.1% commission
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

#[cfg(test)]
mod tests {
    use super::*;
    use testudo_types::{OrderSide, OrderType};
    
    #[tokio::test]
    async fn test_mock_exchange_market_data() {
        let exchange = MockExchange::new();
        
        // Test getting default market data
        let btc_data = exchange.get_market_data("BTC/USDT").await.unwrap();
        assert_eq!(btc_data.symbol, "BTC/USDT");
        assert_eq!(btc_data.last_price, dec!(50000.0));
        
        // Test unsupported symbol
        let result = exchange.get_market_data("XYZ/USDT").await;
        assert!(matches!(result, Err(ExchangeError::MarketDataUnavailable { .. })));
    }
    
    #[tokio::test]
    async fn test_mock_exchange_place_order() {
        let exchange = MockExchange::new();
        
        let order = TradeOrder {
            symbol: "BTC/USDT".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: dec!(0.01),
            price: Some(dec!(50000.0)),
            stop_price: None,
            client_order_id: "TEST-001".to_string(),
        };
        
        let result = exchange.place_order(&order).await.unwrap();
        assert_eq!(result.client_order_id, "TEST-001");
        assert_eq!(result.status, OrderStatus::Filled);
        assert_eq!(result.executed_quantity, dec!(0.01));
        
        // Verify order was recorded
        let placed_orders = exchange.get_placed_orders().await;
        assert_eq!(placed_orders.len(), 1);
        assert_eq!(placed_orders[0].client_order_id, "TEST-001");
    }
    
    #[tokio::test]
    async fn test_mock_exchange_health_check() {
        let exchange = MockExchange::new();
        
        // Default should be healthy
        assert!(exchange.health_check().await.unwrap());
        
        // Set unhealthy
        exchange.set_health(false).await;
        assert!(!exchange.health_check().await.unwrap());
        
        // Verify operations fail when unhealthy
        let result = exchange.get_market_data("BTC/USDT").await;
        assert!(matches!(result, Err(ExchangeError::ConnectionError { .. })));
    }
    
    #[tokio::test]
    async fn test_mock_exchange_balances() {
        let exchange = MockExchange::new();
        
        // Get default USDT balance
        let usdt_balance = exchange.get_balance("USDT").await.unwrap();
        assert_eq!(usdt_balance.asset, "USDT");
        assert_eq!(usdt_balance.free, dec!(10000.0));
        
        // Get all balances
        let all_balances = exchange.get_all_balances().await.unwrap();
        assert_eq!(all_balances.len(), 2);  // USDT and BTC by default
    }
    
    #[tokio::test]
    async fn test_mock_exchange_order_management() {
        let exchange = MockExchange::new();
        
        // Place an order
        let order = TradeOrder {
            symbol: "BTC/USDT".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: dec!(0.01),
            price: Some(dec!(50000.0)),
            stop_price: None,
            client_order_id: "TEST-002".to_string(),
        };
        
        let result = exchange.place_order(&order).await.unwrap();
        let order_id = result.order_id.clone();
        
        // Get order status
        let status = exchange.get_order_status(&order_id).await.unwrap();
        assert_eq!(status.status, OrderStatus::Filled);
        
        // Cancel order (even though it's filled, for testing)
        exchange.cancel_order(&order_id).await.unwrap();
        
        // Check status changed
        let status = exchange.get_order_status(&order_id).await.unwrap();
        assert_eq!(status.status, OrderStatus::Cancelled);
    }
}
