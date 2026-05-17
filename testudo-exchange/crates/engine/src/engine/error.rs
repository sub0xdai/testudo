use rust_decimal::Decimal;

/// Typed error for the classic matching engine.
///
/// Replaces all `Result<_, &str>` and `Result<_, ()>` return types
/// with explicit, pattern-matchable variants.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CoreEngineError {
    /// No orderbook registered for the given market ticker.
    #[error("no orderbook found for market: {market}")]
    OrderbookNotFound { market: String },

    /// User does not exist in the engine's balance registry.
    #[error("user not found: {user_id}")]
    UserNotFound { user_id: String },

    /// User exists but has no balance entry for the requested asset.
    #[error("no balance for asset {asset} in user {user_id}")]
    BalanceNotFound { user_id: String, asset: String },

    /// Mutex protecting user balances is poisoned.
    #[error("balance mutex lock failed")]
    MutexLockFailed,

    /// Insufficient quote asset (USDC/USDT) to cover order cost.
    #[error("insufficient funds for user {user_id}: required {required}, available {available}")]
    InsufficientFunds {
        user_id: String,
        required: Decimal,
        available: Decimal,
    },

    /// Insufficient base asset (SOL/BTC/ETH) to cover sell quantity.
    #[error("insufficient quantity for user {user_id}: required {required}, available {available}")]
    InsufficientQuantity {
        user_id: String,
        required: Decimal,
        available: Decimal,
    },

    /// Order cancellation failed (order not found or already closed).
    #[error("failed to cancel order")]
    CancelOrderFailed,

    /// Asset string could not be parsed.
    #[error("unsupported asset: {asset}")]
    AssetParseError { asset: String },

    /// An internal error occurred (DB failure, invariant violation, etc.).
    #[error("internal error: {detail}")]
    Internal { detail: String },
}
