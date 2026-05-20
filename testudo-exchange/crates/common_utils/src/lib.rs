#![allow(clippy::type_complexity)]
#![allow(clippy::new_without_default)]

pub mod adapters;
pub mod agent;
pub mod journal;
pub mod auth;
pub mod columnar;
pub mod crypto;
pub mod errors;
pub mod models;
pub mod risk;
pub mod services;
pub mod types;

// Re-export commonly used types for convenience
pub use types::{
    ExchangeAdapter, ExchangeCredentials, OrderResponse, OrderSide, OrderStatus, OrderType,
    OrderValidationError, OrderValidator, RoutingError, StandardOrder, StandardOrderBuilder,
    StandardOrderValidator, SymbolFormat, TimeInForce,
};

pub use models::{
    ExchangeAccount, ExchangeAccountError, ExchangeAccountFactory, ExchangeValidator,
    StandardExchangeAccountFactory, StandardExchangeValidator, User, UserError,
};

pub use errors::ExchangeError;

// Re-export services
pub use services::{
    binance_data::{Candle, Market},
    pg_cache::{cache_keys, cache_ttl},
    BinanceDataService, CacheError, PgCacheService,
};

// Re-export exchange adapter types
pub use adapters::{BinanceExecutor, CredentialValidator};

// Re-export agent types (AGENT-02)
pub use agent::{AgentAlert, AlertSeverity, AlertType, ExecutionReport};

// Re-export risk management types
pub use risk::{
    PositionSizer, PgRiskConfigStorage, PgRiskStorageError, RiskConfig, RiskConfigError,
    RiskValidationResult, RiskValidator, RiskViolation,
};

// Re-export columnar data types
pub use columnar::{ColumnarOrderBook, DepthColumnStore};
