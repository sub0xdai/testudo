//! Exchange integration adapters

pub mod binance;
pub mod circuit_breaker;
pub mod failover;
pub mod mock;
pub mod rate_limiter;
pub mod websocket;

// Re-export shared types from the new crate
pub use testudo_types::*;

pub use binance::{BinanceAdapter, ExchangeConfig};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerState};
pub use failover::{FailoverManager, ExchangeFailoverConfig};
pub use mock::MockExchange;
pub use rate_limiter::ExchangeRateLimiter;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages multiple exchange adapters
pub struct ExchangeManager {
    /// Map of exchange names to their adapters
    adapters: Arc<RwLock<HashMap<String, Arc<dyn ExchangeAdapterTrait + Send + Sync>>>>,
    /// Failover manager for handling exchange outages
    failover_manager: Arc<RwLock<FailoverManager>>,
}

impl ExchangeManager {
    /// Create a new exchange manager
    pub fn new(failover_config: ExchangeFailoverConfig) -> Self {
        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            failover_manager: Arc::new(RwLock::new(FailoverManager::new(failover_config))),
        }
    }

    /// Add a new exchange adapter
    pub async fn add_adapter(&self, name: &str, adapter: Arc<dyn ExchangeAdapterTrait + Send + Sync>) {
        let mut adapters = self.adapters.write().await;
        adapters.insert(name.to_string(), adapter);
    }

    /// Get an adapter by name
    pub async fn get_adapter(&self, name: &str) -> Option<Arc<dyn ExchangeAdapterTrait + Send + Sync>> {
        let adapters = self.adapters.read().await;
        adapters.get(name).cloned()
    }

    /// Get the primary adapter based on failover status
    pub async fn get_primary_adapter(&self) -> Option<Arc<dyn ExchangeAdapterTrait + Send + Sync>> {
        let failover_manager = self.failover_manager.read().await;
        let primary_name = failover_manager.get_primary_exchange_name();
        self.get_adapter(&primary_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_exchange_manager() {
        let failover_config = ExchangeFailoverConfig {
            primary_exchange: "mock1".to_string(),
            backup_exchanges: vec!["mock2".to_string()],
            health_check_interval_secs: 60,
        };
        
        let manager = ExchangeManager::new(failover_config);
        
        let mock1 = Arc::new(MockExchange::with_name("mock1".to_string()));
        let mock2 = Arc::new(MockExchange::with_name("mock2".to_string()));
        
        manager.add_adapter("mock1", mock1.clone()).await;
        manager.add_adapter("mock2", mock2.clone()).await;
        
        // Get adapter by name
        let adapter = manager.get_adapter("mock1").await.unwrap();
        assert_eq!(adapter.exchange_name(), "mock1");
        
        // Get primary adapter
        let primary = manager.get_primary_adapter().await.unwrap();
        assert_eq!(primary.exchange_name(), "mock1");
    }
}
