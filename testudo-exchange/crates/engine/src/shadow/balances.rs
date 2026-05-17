//! Shadow Balance Management
//!
//! Manages virtual balances for paper trading. Each user has:
//! - Available balance: Funds that can be used for new orders
//! - Reserved balance: Funds locked in open orders
//!
//! # Default Demo Balances
//!
//! New users receive:
//! - 10,000 USDC
//! - 0 BTC (must be earned through trading)
//!
//! # Concurrency (FR-2.3)
//!
//! Uses DashMap for lock-free concurrent access per user.
//! Two different users can modify their balances simultaneously without blocking.

use dashmap::DashMap;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::ShadowEngineError;

/// A user's balance for a single asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowBalance {
    pub asset: String,
    pub available: Decimal,
    pub reserved: Decimal,
}

impl ShadowBalance {
    pub fn new(asset: &str, available: Decimal) -> Self {
        Self {
            asset: asset.to_string(),
            available,
            reserved: dec!(0),
        }
    }

    /// Total balance (available + reserved)
    pub fn total(&self) -> Decimal {
        self.available + self.reserved
    }

    /// Reserve an amount (move from available to reserved)
    pub fn reserve(&mut self, amount: Decimal) -> Result<(), ShadowEngineError> {
        if self.available < amount {
            return Err(ShadowEngineError::BalanceError(format!(
                "Cannot reserve {} {}: only {} available",
                amount, self.asset, self.available
            )));
        }
        self.available -= amount;
        self.reserved += amount;
        // Financial invariant: balances must never go negative
        assert!(self.available >= Decimal::ZERO, "available balance negative after reserve");
        assert!(self.reserved >= Decimal::ZERO, "reserved balance negative after reserve");
        Ok(())
    }

    /// Release reserved amount back to available
    pub fn release(&mut self, amount: Decimal) -> Result<(), ShadowEngineError> {
        if self.reserved < amount {
            return Err(ShadowEngineError::BalanceError(format!(
                "Cannot release {} {}: only {} reserved",
                amount, self.asset, self.reserved
            )));
        }
        self.reserved -= amount;
        self.available += amount;
        assert!(self.available >= Decimal::ZERO, "available balance negative after release");
        assert!(self.reserved >= Decimal::ZERO, "reserved balance negative after release");
        Ok(())
    }

    /// Deduct from reserved (for filled orders)
    pub fn deduct_reserved(&mut self, amount: Decimal) -> Result<(), ShadowEngineError> {
        if self.reserved < amount {
            return Err(ShadowEngineError::BalanceError(format!(
                "Cannot deduct {} {}: only {} reserved",
                amount, self.asset, self.reserved
            )));
        }
        self.reserved -= amount;
        assert!(self.reserved >= Decimal::ZERO, "reserved balance negative after deduct");
        Ok(())
    }

    /// Add to available balance
    pub fn add(&mut self, amount: Decimal) {
        self.available += amount;
        assert!(self.available >= Decimal::ZERO, "available balance negative after add");
    }
}

/// Manages all user balances with lock-free per-user access (FR-2.3)
///
/// Uses DashMap to allow concurrent operations on different users.
/// Two users placing orders simultaneously will not block each other.
pub struct ShadowBalanceManager {
    /// Map of user_id -> asset -> balance
    /// DashMap provides fine-grained locking at the user level
    balances: DashMap<Uuid, HashMap<String, ShadowBalance>>,
}

impl Default for ShadowBalanceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ShadowBalanceManager {
    pub fn new() -> Self {
        Self {
            balances: DashMap::new(),
        }
    }

    /// Initialize a user with default demo balances
    /// Default: 10,000 USDT for perpetual futures trading
    pub fn init_user_with_defaults(&self, user_id: Uuid) {
        let mut user_balances = HashMap::new();

        // Default demo balance: 10,000 USDT (margin for perpetual futures)
        user_balances.insert("USDT".to_string(), ShadowBalance::new("USDT", dec!(10000)));

        self.balances.insert(user_id, user_balances);
    }

    /// Reset a user's balances to default demo amounts
    /// Clears any existing balances and positions, giving a fresh start
    pub fn reset_user_to_defaults(&self, user_id: Uuid) {
        // Remove existing balances
        self.balances.remove(&user_id);
        // Re-initialize with defaults
        self.init_user_with_defaults(user_id);
    }

    /// Check if a user has been initialized
    pub fn user_exists(&self, user_id: Uuid) -> bool {
        self.balances.contains_key(&user_id)
    }

    /// Set a specific balance for a user
    pub fn set_balance(&self, user_id: Uuid, asset: &str, amount: Decimal) {
        self.balances
            .entry(user_id)
            .or_insert_with(HashMap::new)
            .insert(asset.to_string(), ShadowBalance::new(asset, amount));
    }

    /// Get all balances for a user
    pub fn get_user_balances(&self, user_id: Uuid) -> Vec<ShadowBalance> {
        self.balances
            .get(&user_id)
            .map(|balances| balances.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get available balance for a specific asset
    pub fn get_available(&self, user_id: Uuid, asset: &str) -> Decimal {
        self.balances
            .get(&user_id)
            .and_then(|balances| balances.get(asset).map(|b| b.available))
            .unwrap_or(dec!(0))
    }

    /// Get total balance for a specific asset
    pub fn get_total(&self, user_id: Uuid, asset: &str) -> Decimal {
        self.balances
            .get(&user_id)
            .and_then(|balances| balances.get(asset).map(|b| b.total()))
            .unwrap_or(dec!(0))
    }

    /// Reserve funds for an order
    pub fn reserve(
        &self,
        user_id: Uuid,
        asset: &str,
        amount: Decimal,
    ) -> Result<(), ShadowEngineError> {
        let mut user_balances = self.balances.entry(user_id).or_insert_with(HashMap::new);
        let balance = user_balances
            .entry(asset.to_string())
            .or_insert_with(|| ShadowBalance::new(asset, dec!(0)));
        balance.reserve(amount)
    }

    /// Release reserved funds (order cancelled)
    pub fn release(
        &self,
        user_id: Uuid,
        asset: &str,
        amount: Decimal,
    ) -> Result<(), ShadowEngineError> {
        let mut user_balances = self
            .balances
            .get_mut(&user_id)
            .ok_or_else(|| ShadowEngineError::BalanceError("User not found".to_string()))?;

        let balance = user_balances
            .get_mut(asset)
            .ok_or_else(|| ShadowEngineError::BalanceError("Asset not found".to_string()))?;

        balance.release(amount)
    }

    /// Add to available balance
    pub fn add(&self, user_id: Uuid, asset: &str, amount: Decimal) {
        let mut user_balances = self.balances.entry(user_id).or_insert_with(HashMap::new);
        let balance = user_balances
            .entry(asset.to_string())
            .or_insert_with(|| ShadowBalance::new(asset, dec!(0)));
        balance.add(amount);
    }

    /// Deduct from reserved (for fills)
    pub fn deduct_reserved(
        &self,
        user_id: Uuid,
        asset: &str,
        amount: Decimal,
    ) -> Result<(), ShadowEngineError> {
        let mut user_balances = self
            .balances
            .get_mut(&user_id)
            .ok_or_else(|| ShadowEngineError::BalanceError("User not found".to_string()))?;

        let balance = user_balances
            .get_mut(asset)
            .ok_or_else(|| ShadowEngineError::BalanceError("Asset not found".to_string()))?;

        balance.deduct_reserved(amount)
    }

    /// Get user's equity in a specific quote currency (e.g., USDC)
    ///
    /// This is the total value of all holdings converted to the quote currency.
    /// Requires current prices for conversion.
    pub fn calculate_equity(
        &self,
        user_id: Uuid,
        quote_currency: &str,
        prices: &HashMap<String, Decimal>,
    ) -> Decimal {
        let user_balances = match self.balances.get(&user_id) {
            Some(b) => b,
            None => return dec!(0),
        };

        let mut total = dec!(0);

        for (asset, balance) in user_balances.iter() {
            if asset == quote_currency {
                total += balance.total();
            } else {
                // Look up price for this asset in quote currency
                let price_key = format!("{}_{}", asset, quote_currency);
                if let Some(price) = prices.get(&price_key) {
                    total += balance.total() * price;
                }
            }
        }

        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_balance_creation() {
        let balance = ShadowBalance::new("USDC", dec!(1000));
        assert_eq!(balance.asset, "USDC");
        assert_eq!(balance.available, dec!(1000));
        assert_eq!(balance.reserved, dec!(0));
        assert_eq!(balance.total(), dec!(1000));
    }

    #[test]
    fn test_balance_reserve_and_release() {
        let mut balance = ShadowBalance::new("USDC", dec!(1000));

        // Reserve some
        balance.reserve(dec!(400)).unwrap();
        assert_eq!(balance.available, dec!(600));
        assert_eq!(balance.reserved, dec!(400));
        assert_eq!(balance.total(), dec!(1000));

        // Release some
        balance.release(dec!(100)).unwrap();
        assert_eq!(balance.available, dec!(700));
        assert_eq!(balance.reserved, dec!(300));

        // Try to reserve too much
        let result = balance.reserve(dec!(800));
        assert!(result.is_err());
    }

    #[test]
    fn test_balance_manager_init_user() {
        let manager = ShadowBalanceManager::new();
        let user_id = Uuid::new_v4();

        manager.init_user_with_defaults(user_id);

        // Default is 10,000 USDT for perpetual futures margin
        let usdt = manager.get_available(user_id, "USDT");
        assert_eq!(usdt, dec!(10000));

        // Non-existent assets return 0
        let btc = manager.get_available(user_id, "BTC");
        assert_eq!(btc, dec!(0));
    }

    #[test]
    fn test_balance_manager_reset_user() {
        let manager = ShadowBalanceManager::new();
        let user_id = Uuid::new_v4();

        manager.init_user_with_defaults(user_id);

        // Modify the balance
        manager.add(user_id, "USDT", dec!(5000));
        assert_eq!(manager.get_available(user_id, "USDT"), dec!(15000));

        // Reset should restore to default
        manager.reset_user_to_defaults(user_id);
        assert_eq!(manager.get_available(user_id, "USDT"), dec!(10000));
    }

    #[test]
    fn test_balance_manager_user_exists() {
        let manager = ShadowBalanceManager::new();
        let user_id = Uuid::new_v4();

        assert!(!manager.user_exists(user_id));
        manager.init_user_with_defaults(user_id);
        assert!(manager.user_exists(user_id));
    }

    #[test]
    fn test_balance_manager_reserve_release() {
        let manager = ShadowBalanceManager::new();
        let user_id = Uuid::new_v4();

        manager.init_user_with_defaults(user_id);

        // Reserve some USDT
        manager.reserve(user_id, "USDT", dec!(500)).unwrap();
        assert_eq!(manager.get_available(user_id, "USDT"), dec!(9500));
        assert_eq!(manager.get_total(user_id, "USDT"), dec!(10000));

        // Release it back
        manager.release(user_id, "USDT", dec!(500)).unwrap();
        assert_eq!(manager.get_available(user_id, "USDT"), dec!(10000));
    }

    #[test]
    fn test_balance_manager_add() {
        let manager = ShadowBalanceManager::new();
        let user_id = Uuid::new_v4();

        manager.init_user_with_defaults(user_id);

        // Add some BTC (simulating a fill)
        manager.add(user_id, "BTC", dec!(0.5));
        assert_eq!(manager.get_available(user_id, "BTC"), dec!(0.5));
    }

    #[test]
    fn test_calculate_equity() {
        let manager = ShadowBalanceManager::new();
        let user_id = Uuid::new_v4();

        manager.set_balance(user_id, "USDC", dec!(5000));
        manager.set_balance(user_id, "BTC", dec!(0.1));

        let mut prices = HashMap::new();
        prices.insert("BTC_USDC".to_string(), dec!(50000));

        let equity = manager.calculate_equity(user_id, "USDC", &prices);
        // 5000 USDC + 0.1 BTC * 50000 = 5000 + 5000 = 10000
        assert_eq!(equity, dec!(10000));
    }
}
