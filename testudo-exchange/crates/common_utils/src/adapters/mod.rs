/// Exchange Adapter Module
///
/// Provides exchange integration adapters for the Testudo platform.
/// Live trading is handled via the CCXT sidecar service (012-ccxt-multi-exchange).
/// This module retains: position sync, credential validation, market data,
/// execution types, and the Binance spot executor (used by position sync).
pub mod account_state;
pub mod binance_executor;
pub mod ccxt_auth;
pub mod ccxt_types;
pub mod credential_validator;
pub mod execution_types;
pub mod market_data;
pub mod position_sync;
pub mod position_types;

// Export account state adapter
pub use account_state::{
    AccountStateAdapter, AccountStateBuilder, AccountStateError, BalanceProvider,
};

// Export market data types
pub use market_data::{CachedData, ExchangeEndpoints, MarketCache, MarketDataLoader};

// Export credential validator types
pub use credential_validator::{
    CredentialValidationError, CredentialValidator, ValidatedPermissions,
};

// Export execution types
pub use execution_types::{
    symbol as execution_symbol, BinanceOrderResult, BinanceOrderStatus, ExecutionError,
    ExecutionMode, ExecutionOrderSide, ExecutionOrderType, ExecutionTimeInForce, ValidatedOrder,
};

// Export Binance executor (used by position sync)
pub use binance_executor::{BinanceExecutor, BINANCE_API_URL, BINANCE_TESTNET_URL};

// Export position sync types
pub use position_types::{
    BinanceBalance, BinancePosition, MatchedPosition, PositionDiff, PositionSide, QuantityMismatch,
    ReconcileAction, ShadowPositionInfo, SyncError, SyncResult,
};

// Export position syncer
pub use position_sync::PositionSyncer;
