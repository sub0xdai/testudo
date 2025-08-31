//! Exchange integration adapters (legacy support)
//!
//! This module maintains the existing exchange integration functionality
//! while the crate transitions to focus on risk management.

pub mod adapters;
pub mod binance;
pub mod circuit_breaker;
pub mod failover;
pub mod mock;
pub mod rate_limiter;
pub mod websocket;

pub use adapters::{ExchangeAdapter, ExchangeAdapterTrait, MarketData, TradeOrder, OrderResult, OrderStatus, OrderSide, OrderType, AccountBalance, ExchangeError};
pub use binance::{BinanceAdapter, ExchangeConfig};  // Export ExchangeConfig
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerState};
pub use rate_limiter::ExchangeRateLimiter;
pub use failover::{FailoverManager, ExchangeFailoverConfig};
pub use mock::MockExchange;