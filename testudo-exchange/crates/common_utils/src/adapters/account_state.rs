//! Account State Adapter
//!
//! Provides account state for risk validation, supporting both shadow (paper trading)
//! and live (real trading) modes. Returns balance, position count, and daily P&L
//! information required by the risk service.
//!
//! Supports injected `BalanceProvider` implementations for real data fetching.
//! Without a provider, falls back to default paper trading values.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use uuid::Uuid;

use super::execution_types::ExecutionMode;
use crate::risk::AccountState;

/// Error types for account state operations
#[derive(Debug, thiserror::Error)]
pub enum AccountStateError {
    #[error("Failed to fetch balance from exchange: {0}")]
    ExchangeFetchError(String),

    #[error("User not found: {0}")]
    UserNotFound(Uuid),

    #[error("No credentials configured for user")]
    NoCredentials,
}

/// Trait for providing real account balance data.
/// Implement this for shadow engine access or live exchange API calls.
#[async_trait::async_trait]
pub trait BalanceProvider: Send + Sync {
    async fn get_account_state(&self, user_id: Uuid) -> Result<AccountState, AccountStateError>;
}

/// Account state adapter for fetching user account information
#[derive(Clone)]
pub struct AccountStateAdapter {
    /// Quote currency for balance calculation (e.g., "USDT")
    quote_currency: String,
    /// Optional shadow balance provider (for real shadow engine data)
    shadow_provider: Option<Arc<dyn BalanceProvider>>,
    /// Optional live balance provider (for real exchange data)
    live_provider: Option<Arc<dyn BalanceProvider>>,
}

impl AccountStateAdapter {
    /// Create a new account state adapter
    pub fn new(quote_currency: impl Into<String>) -> Self {
        Self {
            quote_currency: quote_currency.into(),
            shadow_provider: None,
            live_provider: None,
        }
    }

    /// Create adapter with USDT as default quote currency
    pub fn usdt() -> Self {
        Self::new("USDT")
    }

    /// Inject a shadow balance provider for real shadow engine data.
    pub fn with_shadow_provider(mut self, provider: Arc<dyn BalanceProvider>) -> Self {
        self.shadow_provider = Some(provider);
        self
    }

    /// Inject a live balance provider for real exchange data.
    pub fn with_live_provider(mut self, provider: Arc<dyn BalanceProvider>) -> Self {
        self.live_provider = Some(provider);
        self
    }

    /// Get account state based on execution mode
    pub async fn get_account_state(
        &self,
        user_id: Uuid,
        mode: ExecutionMode,
    ) -> Result<AccountState, AccountStateError> {
        match mode {
            ExecutionMode::Shadow => self.get_shadow_account_state(user_id).await,
            ExecutionMode::Live => self.get_live_account_state(user_id).await,
        }
    }

    /// Get shadow (paper trading) account state.
    /// Uses injected provider if available, otherwise falls back to defaults.
    async fn get_shadow_account_state(
        &self,
        user_id: Uuid,
    ) -> Result<AccountState, AccountStateError> {
        if let Some(ref provider) = self.shadow_provider {
            return provider.get_account_state(user_id).await;
        }

        // Fallback: default paper trading balance
        Ok(AccountState {
            balance: dec!(10000),
            open_position_count: 0,
            daily_pnl: dec!(0),
            starting_balance: dec!(10000),
        })
    }

    /// Get live (real trading) account state from exchange.
    /// Uses injected provider if available, otherwise falls back to defaults.
    async fn get_live_account_state(
        &self,
        user_id: Uuid,
    ) -> Result<AccountState, AccountStateError> {
        if let Some(ref provider) = self.live_provider {
            return provider.get_account_state(user_id).await;
        }

        // Fallback: same as shadow defaults
        Ok(AccountState {
            balance: dec!(10000),
            open_position_count: 0,
            daily_pnl: dec!(0),
            starting_balance: dec!(10000),
        })
    }

    /// Get quote currency used for balance
    pub fn quote_currency(&self) -> &str {
        &self.quote_currency
    }
}

impl std::fmt::Debug for AccountStateAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountStateAdapter")
            .field("quote_currency", &self.quote_currency)
            .field("has_shadow_provider", &self.shadow_provider.is_some())
            .field("has_live_provider", &self.live_provider.is_some())
            .finish()
    }
}

/// Builder for AccountState with custom values
pub struct AccountStateBuilder {
    balance: Decimal,
    open_position_count: u32,
    daily_pnl: Decimal,
    starting_balance: Decimal,
}

impl AccountStateBuilder {
    pub fn new() -> Self {
        Self {
            balance: dec!(10000),
            open_position_count: 0,
            daily_pnl: dec!(0),
            starting_balance: dec!(10000),
        }
    }

    pub fn balance(mut self, balance: Decimal) -> Self {
        self.balance = balance;
        self
    }

    pub fn open_positions(mut self, count: u32) -> Self {
        self.open_position_count = count;
        self
    }

    pub fn daily_pnl(mut self, pnl: Decimal) -> Self {
        self.daily_pnl = pnl;
        self
    }

    pub fn starting_balance(mut self, balance: Decimal) -> Self {
        self.starting_balance = balance;
        self
    }

    pub fn build(self) -> AccountState {
        AccountState {
            balance: self.balance,
            open_position_count: self.open_position_count,
            daily_pnl: self.daily_pnl,
            starting_balance: self.starting_balance,
        }
    }
}

impl Default for AccountStateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shadow_account_state_default() {
        let adapter = AccountStateAdapter::usdt();
        let user_id = Uuid::new_v4();

        let state = adapter
            .get_account_state(user_id, ExecutionMode::Shadow)
            .await
            .unwrap();

        assert_eq!(state.balance, dec!(10000));
        assert_eq!(state.open_position_count, 0);
        assert_eq!(state.daily_pnl, dec!(0));
    }

    #[tokio::test]
    async fn test_shadow_account_state_with_provider() {
        struct TestProvider;

        #[async_trait::async_trait]
        impl BalanceProvider for TestProvider {
            async fn get_account_state(
                &self,
                _user_id: Uuid,
            ) -> Result<AccountState, AccountStateError> {
                Ok(AccountState {
                    balance: dec!(5000),
                    open_position_count: 2,
                    daily_pnl: dec!(150),
                    starting_balance: dec!(4850),
                })
            }
        }

        let adapter = AccountStateAdapter::usdt().with_shadow_provider(Arc::new(TestProvider));
        let user_id = Uuid::new_v4();

        let state = adapter
            .get_account_state(user_id, ExecutionMode::Shadow)
            .await
            .unwrap();

        assert_eq!(state.balance, dec!(5000));
        assert_eq!(state.open_position_count, 2);
        assert_eq!(state.daily_pnl, dec!(150));
    }

    #[tokio::test]
    async fn test_live_account_state_fallback() {
        let adapter = AccountStateAdapter::usdt();
        let user_id = Uuid::new_v4();

        let state = adapter
            .get_account_state(user_id, ExecutionMode::Live)
            .await
            .unwrap();

        assert!(state.balance > dec!(0));
    }

    #[test]
    fn test_account_state_builder() {
        let state = AccountStateBuilder::new()
            .balance(dec!(25000))
            .open_positions(3)
            .daily_pnl(dec!(500))
            .starting_balance(dec!(24500))
            .build();

        assert_eq!(state.balance, dec!(25000));
        assert_eq!(state.open_position_count, 3);
        assert_eq!(state.daily_pnl, dec!(500));
        assert_eq!(state.starting_balance, dec!(24500));
    }

    #[test]
    fn test_quote_currency() {
        let adapter = AccountStateAdapter::new("USDC");
        assert_eq!(adapter.quote_currency(), "USDC");

        let adapter = AccountStateAdapter::usdt();
        assert_eq!(adapter.quote_currency(), "USDT");
    }
}
