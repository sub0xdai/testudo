//! Shadow Order Management
//!
//! Manages virtual orders for paper trading. Orders are checked against
//! live market data and filled when conditions are met.
//!
//! # Fill Logic (from PRD)
//!
//! - **Buy Limit**: Fills if `Low Price <= Limit Price`
//! - **Sell Limit**: Fills if `High Price >= Limit Price`
//! - **Buy Market**: Fills immediately at `Best Ask`
//! - **Sell Market**: Fills immediately at `Best Bid`
//! - **Stop Loss Buy**: Triggers when `High Price >= Stop Price`
//! - **Stop Loss Sell**: Triggers when `Low Price <= Stop Price`

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

use super::ShadowEngineError;

/// Order side - buy or sell
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShadowOrderSide {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowOrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShadowOrderStatus {
    Open,
    Filled,
    PartiallyFilled,
    Cancelled,
    Rejected,
}

/// A shadow (paper trading) order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowOrder {
    pub id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub side: ShadowOrderSide,
    pub order_type: ShadowOrderType,
    pub quantity: Decimal,
    pub filled_quantity: Decimal,
    pub price: Option<Decimal>,      // For limit orders
    pub stop_price: Option<Decimal>, // For stop/take-profit orders
    pub average_fill_price: Option<Decimal>,
    pub status: ShadowOrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub filled_at: Option<DateTime<Utc>>,

    // Optional linked orders for trade management
    pub stop_loss_price: Option<Decimal>,
    pub take_profit_price: Option<Decimal>,
    pub parent_order_id: Option<Uuid>,

    /// Risk validation flag - must be true for order to be accepted by Shadow Engine.
    /// Set to true by the Decision Loop after risk validation passes.
    /// Orders with risk_validated = false will be rejected by ShadowEngine::place_order().
    #[serde(default)]
    pub risk_validated: bool,

    /// Leverage multiplier for margin calculation (1 = no leverage).
    /// Margin required = notional / leverage. Per-pair limits enforced at exchange.
    #[serde(default = "default_leverage")]
    pub leverage: u8,

    /// AUD-02: Timestamp when order reached terminal state (for GC).
    #[serde(skip)]
    pub completed_at: Option<Instant>,
}

fn default_leverage() -> u8 {
    1
}

impl ShadowOrder {
    /// Create a new shadow order
    ///
    /// Note: Orders are created with `risk_validated = false` by default.
    /// Use `mark_risk_validated()` after passing Decision Loop validation.
    pub fn new(
        user_id: Uuid,
        symbol: String,
        side: ShadowOrderSide,
        order_type: ShadowOrderType,
        quantity: Decimal,
        price: Option<Decimal>,
        stop_price: Option<Decimal>,
        parent_order_id: Option<Uuid>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            symbol,
            side,
            order_type,
            quantity,
            filled_quantity: dec!(0),
            price,
            stop_price,
            average_fill_price: None,
            status: ShadowOrderStatus::Open,
            created_at: now,
            updated_at: now,
            filled_at: None,
            stop_loss_price: None,
            take_profit_price: None,
            parent_order_id,
            risk_validated: false,
            leverage: 1,
            completed_at: None,
        }
    }

    /// Mark this order as risk-validated.
    ///
    /// Called by the Decision Loop after the order passes risk checks.
    /// Orders must have `risk_validated = true` to be accepted by Shadow Engine.
    pub fn mark_risk_validated(&mut self) {
        self.risk_validated = true;
        log::info!(
            "Order {} marked as risk validated (symbol={}, side={:?}, qty={})",
            self.id,
            self.symbol,
            self.side,
            self.quantity
        );
    }

    /// Check if this order has been risk-validated
    pub fn is_risk_validated(&self) -> bool {
        self.risk_validated
    }

    /// Create a market buy order
    pub fn market_buy(user_id: Uuid, symbol: &str, quantity: Decimal) -> Self {
        Self::new(
            user_id,
            symbol.to_string(),
            ShadowOrderSide::Buy,
            ShadowOrderType::Market,
            quantity,
            None,
            None,
            None,
        )
    }

    /// Create a market sell order
    pub fn market_sell(user_id: Uuid, symbol: &str, quantity: Decimal) -> Self {
        Self::new(
            user_id,
            symbol.to_string(),
            ShadowOrderSide::Sell,
            ShadowOrderType::Market,
            quantity,
            None,
            None,
            None,
        )
    }

    /// Create a limit buy order
    pub fn limit_buy(user_id: Uuid, symbol: &str, quantity: Decimal, price: Decimal) -> Self {
        Self::new(
            user_id,
            symbol.to_string(),
            ShadowOrderSide::Buy,
            ShadowOrderType::Limit,
            quantity,
            Some(price),
            None,
            None,
        )
    }

    /// Create a limit sell order
    pub fn limit_sell(user_id: Uuid, symbol: &str, quantity: Decimal, price: Decimal) -> Self {
        Self::new(
            user_id,
            symbol.to_string(),
            ShadowOrderSide::Sell,
            ShadowOrderType::Limit,
            quantity,
            Some(price),
            None,
            None,
        )
    }

    /// Get the asset required for this order (futures: always quote currency margin).
    /// Normalizes "USD" → "USDT" since perpetual futures margin is always USDT.
    pub fn required_asset(&self) -> String {
        let (_base, quote) = self.symbol_parts();
        // Perpetual futures: "USD" pairs settle in USDT
        if quote == "USD" {
            "USDT".to_string()
        } else {
            quote
        }
    }

    /// Get the margin required for this order (futures: notional / leverage)
    pub fn required_amount(&self) -> Decimal {
        let price = self.price.unwrap_or(dec!(0));
        let notional = self.quantity * price;
        notional / Decimal::from(self.leverage.max(1))
    }

    /// Parse symbol into base and quote assets
    pub fn symbol_parts(&self) -> (String, String) {
        let parts: Vec<&str> = self.symbol.split('_').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            // Fallback for symbols like "BTCUSDC"
            // Assume last 4 chars are quote for USDC/USDT
            if self.symbol.ends_with("USDC") {
                let base = &self.symbol[..self.symbol.len() - 4];
                (base.to_string(), "USDC".to_string())
            } else if self.symbol.ends_with("USDT") {
                let base = &self.symbol[..self.symbol.len() - 4];
                (base.to_string(), "USDT".to_string())
            } else {
                (self.symbol.clone(), "USDC".to_string())
            }
        }
    }

    /// Check if this order should fill at the given market prices.
    ///
    /// Uses current bid/ask for real-time fill checking (live price feed).
    /// - bid = best price someone will buy at (what you get when selling)
    /// - ask = best price someone will sell at (what you pay when buying)
    pub fn should_fill(&self, bid: Decimal, ask: Decimal, _high: Decimal, _low: Decimal) -> bool {
        match (&self.order_type, &self.side) {
            // Market orders fill immediately
            (ShadowOrderType::Market, _) => true,

            // Limit buy fills when ask drops to or below limit price
            (ShadowOrderType::Limit, ShadowOrderSide::Buy) => {
                self.price.map(|p| ask <= p).unwrap_or(false)
            }

            // Limit sell fills when bid rises to or above limit price
            (ShadowOrderType::Limit, ShadowOrderSide::Sell) => {
                self.price.map(|p| bid >= p).unwrap_or(false)
            }

            // Stop loss buy (short SL) triggers when ask rises to stop price
            (ShadowOrderType::StopLoss, ShadowOrderSide::Buy) => {
                self.stop_price.map(|p| ask >= p).unwrap_or(false)
            }

            // Stop loss sell (long SL) triggers when bid drops to stop price
            (ShadowOrderType::StopLoss, ShadowOrderSide::Sell) => {
                self.stop_price.map(|p| bid <= p).unwrap_or(false)
            }

            // Take profit buy (short TP) triggers when ask drops to target
            (ShadowOrderType::TakeProfit, ShadowOrderSide::Buy) => {
                self.stop_price.map(|p| ask <= p).unwrap_or(false)
            }

            // Take profit sell (long TP) triggers when bid rises to target
            (ShadowOrderType::TakeProfit, ShadowOrderSide::Sell) => {
                self.stop_price.map(|p| bid >= p).unwrap_or(false)
            }
        }
    }

    /// Fill this order at the given price
    pub fn fill(&mut self, fill_price: Decimal) {
        self.filled_quantity = self.quantity;
        self.average_fill_price = Some(fill_price);
        self.status = ShadowOrderStatus::Filled;
        self.filled_at = Some(Utc::now());
        self.updated_at = Utc::now();
        self.completed_at = Some(Instant::now());
    }

    /// Cancel this order
    pub fn cancel(&mut self) {
        self.status = ShadowOrderStatus::Cancelled;
        self.updated_at = Utc::now();
        self.completed_at = Some(Instant::now());
    }

    /// Check if order is still open
    pub fn is_open(&self) -> bool {
        matches!(
            self.status,
            ShadowOrderStatus::Open | ShadowOrderStatus::PartiallyFilled
        )
    }

    /// Set leverage for this order (affects margin calculation)
    pub fn with_leverage(mut self, leverage: u8) -> Self {
        self.leverage = leverage.max(1);
        self
    }

    /// Set stop loss price for this order
    pub fn with_stop_loss(mut self, price: Decimal) -> Self {
        self.stop_loss_price = Some(price);
        self
    }

    /// Set take profit price for this order
    pub fn with_take_profit(mut self, price: Decimal) -> Self {
        self.take_profit_price = Some(price);
        self
    }
}

/// Manages all shadow orders
pub struct ShadowOrderManager {
    /// All orders by ID
    orders: HashMap<Uuid, ShadowOrder>,
    /// Index of orders by user
    orders_by_user: HashMap<Uuid, Vec<Uuid>>,
    /// Index of open orders by symbol
    open_orders_by_symbol: HashMap<String, Vec<Uuid>>,
}

impl Default for ShadowOrderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ShadowOrderManager {
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            orders_by_user: HashMap::new(),
            open_orders_by_symbol: HashMap::new(),
        }
    }

    /// Add a new order
    pub fn add_order(&mut self, user_id: Uuid, mut order: ShadowOrder) -> ShadowOrder {
        order.user_id = user_id;
        let order_id = order.id;
        let symbol = order.symbol.clone();

        // Store the order
        self.orders.insert(order_id, order.clone());

        // Index by user
        self.orders_by_user
            .entry(user_id)
            .or_default()
            .push(order_id);

        // Index open orders by symbol
        if order.is_open() {
            self.open_orders_by_symbol
                .entry(symbol)
                .or_default()
                .push(order_id);
        }

        order
    }

    /// Get all open orders for a user
    pub fn get_open_orders(&self, user_id: Uuid) -> Vec<ShadowOrder> {
        self.orders_by_user
            .get(&user_id)
            .map(|order_ids| {
                order_ids
                    .iter()
                    .filter_map(|id| self.orders.get(id))
                    .filter(|o| o.is_open())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all orders for a user (including filled/cancelled)
    pub fn get_all_orders(&self, user_id: Uuid) -> Vec<ShadowOrder> {
        self.orders_by_user
            .get(&user_id)
            .map(|order_ids| {
                order_ids
                    .iter()
                    .filter_map(|id| self.orders.get(id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get a specific order
    pub fn get_order(&self, order_id: Uuid) -> Option<&ShadowOrder> {
        self.orders.get(&order_id)
    }

    /// Cancel an order
    pub fn cancel_order(
        &mut self,
        user_id: Uuid,
        order_id: Uuid,
    ) -> Result<ShadowOrder, ShadowEngineError> {
        let order = self
            .orders
            .get_mut(&order_id)
            .ok_or(ShadowEngineError::OrderNotFound(order_id))?;

        if order.user_id != user_id {
            return Err(ShadowEngineError::OrderNotFound(order_id));
        }

        if !order.is_open() {
            return Err(ShadowEngineError::InvalidOrder(
                "Order is not open".to_string(),
            ));
        }

        order.cancel();

        // Remove from open orders index
        if let Some(ids) = self.open_orders_by_symbol.get_mut(&order.symbol) {
            ids.retain(|id| *id != order_id);
        }

        Ok(order.clone())
    }

    /// Check for orders that should fill at current prices (READ-ONLY)
    ///
    /// Returns copies of orders that would trigger at the given prices.
    /// Does NOT modify any state - use `apply_fills` to actually fill orders.
    ///
    /// Part of the Read-Compute-Write pattern (004-read-compute-write):
    /// - READ: This method - identify triggerable orders without locking
    /// - COMPUTE: Calculate fills in memory
    /// - WRITE: `apply_fills` - update state atomically
    pub fn get_triggerable_orders(
        &self,
        symbol: &str,
        bid: Decimal,
        ask: Decimal,
        high: Decimal,
        low: Decimal,
    ) -> Vec<ShadowOrder> {
        let order_ids: Vec<Uuid> = self
            .open_orders_by_symbol
            .get(symbol)
            .cloned()
            .unwrap_or_default();

        order_ids
            .iter()
            .filter_map(|order_id| self.orders.get(order_id))
            .filter(|order| order.should_fill(bid, ask, high, low))
            .cloned()
            .collect()
    }

    /// Apply fills to orders (WRITE operation)
    ///
    /// Marks orders as filled and removes them from the open orders index.
    /// Part of the Read-Compute-Write pattern - call after computing fills.
    pub fn apply_fills(&mut self, order_ids: &[Uuid]) {
        for order_id in order_ids {
            if let Some(order) = self.orders.get(order_id) {
                let symbol = order.symbol.clone();

                // Remove from open orders index
                if let Some(ids) = self.open_orders_by_symbol.get_mut(&symbol) {
                    ids.retain(|id| id != order_id);
                }
            }

            // Update the order status
            if let Some(stored_order) = self.orders.get_mut(order_id) {
                stored_order.status = ShadowOrderStatus::Filled;
                stored_order.completed_at = Some(Instant::now());
            }
        }
    }

    /// Check for orders that should fill at current prices
    ///
    /// Returns orders that have been filled (removed from open orders)
    ///
    /// DEPRECATED: Use `get_triggerable_orders` + `apply_fills` for the
    /// Read-Compute-Write pattern which minimizes lock contention.
    pub fn check_fills(
        &mut self,
        symbol: &str,
        bid: Decimal,
        ask: Decimal,
        high: Decimal,
        low: Decimal,
    ) -> Vec<ShadowOrder> {
        let mut filled = Vec::new();

        let order_ids: Vec<Uuid> = self
            .open_orders_by_symbol
            .get(symbol)
            .cloned()
            .unwrap_or_default();

        for order_id in order_ids {
            if let Some(order) = self.orders.get(&order_id) {
                if order.should_fill(bid, ask, high, low) {
                    filled.push(order.clone());
                }
            }
        }

        // Remove filled orders from the open index
        for order in &filled {
            if let Some(ids) = self.open_orders_by_symbol.get_mut(&order.symbol) {
                ids.retain(|id| *id != order.id);
            }
            // Update the order status in our storage
            if let Some(stored_order) = self.orders.get_mut(&order.id) {
                stored_order.status = ShadowOrderStatus::Filled;
                stored_order.completed_at = Some(Instant::now());
            }
        }

        filled
    }

    /// Get all symbols that currently have open orders
    ///
    /// Used by PriceFeedService to know which symbols need price polling.
    /// Returns a deduplicated list of symbols with at least one open order.
    pub fn get_active_symbols(&self) -> Vec<String> {
        self.open_orders_by_symbol
            .iter()
            .filter(|(_, ids)| !ids.is_empty())
            .map(|(symbol, _)| symbol.clone())
            .collect()
    }

    /// Get count of open orders
    pub fn open_order_count(&self) -> usize {
        self.orders.values().filter(|o| o.is_open()).count()
    }

    /// AUD-02 FR-1: Remove terminal orders older than cutoff.
    pub fn prune_terminal(&mut self, cutoff: Instant) -> usize {
        let to_remove: Vec<Uuid> = self
            .orders
            .iter()
            .filter(|(_, o)| {
                !o.is_open() && o.completed_at.is_some_and(|t| t < cutoff)
            })
            .map(|(id, _)| *id)
            .collect();

        for id in &to_remove {
            self.orders.remove(id);
        }

        // Clean orders_by_user
        for ids in self.orders_by_user.values_mut() {
            ids.retain(|id| !to_remove.contains(id));
        }

        to_remove.len()
    }

    /// Update the stop price of an existing order (for break-even)
    pub fn update_stop_price(&mut self, order_id: Uuid, new_stop_price: Decimal) -> bool {
        if let Some(order) = self.orders.get_mut(&order_id) {
            order.stop_price = Some(new_stop_price);
            order.updated_at = chrono::Utc::now();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_creation() {
        let user_id = Uuid::new_v4();
        let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));

        assert_eq!(order.symbol, "BTC_USDC");
        assert_eq!(order.side, ShadowOrderSide::Buy);
        assert_eq!(order.order_type, ShadowOrderType::Limit);
        assert_eq!(order.quantity, dec!(0.1));
        assert_eq!(order.price, Some(dec!(50000)));
        assert_eq!(order.status, ShadowOrderStatus::Open);
    }

    #[test]
    fn test_symbol_parts() {
        let user_id = Uuid::new_v4();
        let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));
        let (base, quote) = order.symbol_parts();
        assert_eq!(base, "BTC");
        assert_eq!(quote, "USDC");
    }

    #[test]
    fn test_required_asset_and_amount() {
        let user_id = Uuid::new_v4();

        // Buy order at 1x leverage: full notional as margin
        let buy_order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));
        assert_eq!(buy_order.required_asset(), "USDC");
        assert_eq!(buy_order.required_amount(), dec!(5000)); // 0.1 * 50000 / 1

        // Sell order at 1x leverage: same
        let sell_order = ShadowOrder::limit_sell(user_id, "BTC_USDC", dec!(0.1), dec!(50000));
        assert_eq!(sell_order.required_asset(), "USDC");
        assert_eq!(sell_order.required_amount(), dec!(5000)); // 0.1 * 50000 / 1
    }

    #[test]
    fn test_required_amount_with_leverage() {
        let user_id = Uuid::new_v4();

        // 0.3 BTC @ 67746 with 27x leverage
        // Notional = 0.3 * 67746 = 20323.8
        // Margin = 20323.8 / 27 = 752.73333...
        let order =
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.3), dec!(67746)).with_leverage(27);
        assert_eq!(order.leverage, 27);

        let margin = order.required_amount();
        // 20323.8 / 27 = 752.7333...
        assert!(
            margin > dec!(752) && margin < dec!(753),
            "margin was {}",
            margin
        );
    }

    #[test]
    fn test_required_asset_normalizes_usd_to_usdt() {
        let user_id = Uuid::new_v4();

        // "BTC_USD" (from TradingView) should resolve to USDT margin
        let order = ShadowOrder::limit_buy(user_id, "BTC_USD", dec!(0.1), dec!(50000));
        assert_eq!(order.required_asset(), "USDT");

        // "BTC_USDT" should stay USDT
        let order = ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000));
        assert_eq!(order.required_asset(), "USDT");

        // "BTC_USDC" should stay USDC (different stablecoin)
        let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));
        assert_eq!(order.required_asset(), "USDC");
    }

    #[test]
    fn test_limit_buy_should_fill() {
        let user_id = Uuid::new_v4();
        let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));

        // Ask at 49999 should fill (ask dropped below limit)
        assert!(order.should_fill(dec!(49900), dec!(49999), dec!(50200), dec!(49800)));

        // Ask at 50000 should fill (ask equal to limit)
        assert!(order.should_fill(dec!(49900), dec!(50000), dec!(50200), dec!(49800)));

        // Ask at 50001 should NOT fill (ask still above limit)
        assert!(!order.should_fill(dec!(49900), dec!(50001), dec!(50200), dec!(49800)));
    }

    #[test]
    fn test_limit_sell_should_fill() {
        let user_id = Uuid::new_v4();
        let order = ShadowOrder::limit_sell(user_id, "BTC_USDC", dec!(0.1), dec!(50000));

        // Bid at 50001 should fill (bid rose above limit)
        assert!(order.should_fill(dec!(50001), dec!(50100), dec!(50200), dec!(49800)));

        // Bid at 50000 should fill (bid equal to limit)
        assert!(order.should_fill(dec!(50000), dec!(50100), dec!(50200), dec!(49800)));

        // Bid at 49999 should NOT fill (bid still below limit)
        assert!(!order.should_fill(dec!(49999), dec!(50100), dec!(50200), dec!(49800)));
    }

    #[test]
    fn test_market_order_should_always_fill() {
        let user_id = Uuid::new_v4();
        let buy = ShadowOrder::market_buy(user_id, "BTC_USDC", dec!(0.1));
        let sell = ShadowOrder::market_sell(user_id, "BTC_USDC", dec!(0.1));

        assert!(buy.should_fill(dec!(50000), dec!(50100), dec!(50200), dec!(49900)));
        assert!(sell.should_fill(dec!(50000), dec!(50100), dec!(50200), dec!(49900)));
    }

    #[test]
    fn test_order_manager_add_and_get() {
        let mut manager = ShadowOrderManager::new();
        let user_id = Uuid::new_v4();

        let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));
        let added = manager.add_order(user_id, order);

        let open_orders = manager.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 1);
        assert_eq!(open_orders[0].id, added.id);
    }

    #[test]
    fn test_order_manager_cancel() {
        let mut manager = ShadowOrderManager::new();
        let user_id = Uuid::new_v4();

        let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));
        let added = manager.add_order(user_id, order);

        let cancelled = manager.cancel_order(user_id, added.id).unwrap();
        assert_eq!(cancelled.status, ShadowOrderStatus::Cancelled);

        let open_orders = manager.get_open_orders(user_id);
        assert!(open_orders.is_empty());
    }

    #[test]
    fn test_order_manager_check_fills() {
        let mut manager = ShadowOrderManager::new();
        let user_id = Uuid::new_v4();

        // Add a limit buy at 50000
        let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));
        manager.add_order(user_id, order);

        // Price hasn't reached limit yet
        let filled = manager.check_fills(
            "BTC_USDC",
            dec!(50000),
            dec!(50100),
            dec!(50200),
            dec!(50001),
        );
        assert!(filled.is_empty());

        // Price reaches limit
        let filled = manager.check_fills(
            "BTC_USDC",
            dec!(49900),
            dec!(50000),
            dec!(50100),
            dec!(49999),
        );
        assert_eq!(filled.len(), 1);

        // No more open orders
        assert_eq!(manager.open_order_count(), 0);
    }

    // FR-1: Tests for risk_validated field (003-risk-enforcement)

    #[test]
    fn test_order_created_without_risk_validation() {
        let user_id = Uuid::new_v4();
        let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));

        // New orders should NOT be risk validated by default
        assert!(!order.risk_validated);
        assert!(!order.is_risk_validated());
    }

    #[test]
    fn test_order_mark_risk_validated() {
        let user_id = Uuid::new_v4();
        let mut order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000));

        // Initially not validated
        assert!(!order.is_risk_validated());

        // Mark as validated
        order.mark_risk_validated();

        // Now should be validated
        assert!(order.is_risk_validated());
        assert!(order.risk_validated);
    }

    #[test]
    fn test_market_order_without_risk_validation() {
        let user_id = Uuid::new_v4();
        let order = ShadowOrder::market_buy(user_id, "BTC_USDC", dec!(0.1));

        // Market orders should also start without risk validation
        assert!(!order.risk_validated);
    }

    #[test]
    fn test_get_active_symbols_empty() {
        let manager = ShadowOrderManager::new();
        assert!(manager.get_active_symbols().is_empty());
    }

    #[test]
    fn test_get_active_symbols_with_orders() {
        let mut manager = ShadowOrderManager::new();
        let user_id = Uuid::new_v4();

        // Add orders for two different symbols
        manager.add_order(
            user_id,
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000)),
        );
        manager.add_order(
            user_id,
            ShadowOrder::limit_buy(user_id, "ETH_USDT", dec!(1.0), dec!(3000)),
        );

        let symbols = manager.get_active_symbols();
        assert_eq!(symbols.len(), 2);
        assert!(symbols.contains(&"BTC_USDT".to_string()));
        assert!(symbols.contains(&"ETH_USDT".to_string()));
    }

    #[test]
    fn test_get_active_symbols_excludes_cancelled() {
        let mut manager = ShadowOrderManager::new();
        let user_id = Uuid::new_v4();

        let order = manager.add_order(
            user_id,
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000)),
        );
        assert_eq!(manager.get_active_symbols().len(), 1);

        // Cancel it
        manager.cancel_order(user_id, order.id).unwrap();
        assert!(manager.get_active_symbols().is_empty());
    }

    #[test]
    fn test_order_with_sl_tp_without_risk_validation() {
        let user_id = Uuid::new_v4();
        let order = ShadowOrder::limit_buy(user_id, "BTC_USDC", dec!(0.1), dec!(50000))
            .with_stop_loss(dec!(49000))
            .with_take_profit(dec!(52000));

        // Orders with SL/TP should also start without risk validation
        assert!(!order.risk_validated);
    }

    // AUD-02: GC tests

    #[test]
    fn test_prune_terminal_removes_old_cancelled_orders() {
        let mut manager = ShadowOrderManager::new();
        let user_id = Uuid::new_v4();

        let order = manager.add_order(
            user_id,
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000)),
        );
        manager.cancel_order(user_id, order.id).unwrap();

        // Cutoff in the future — should prune
        let cutoff = Instant::now() + std::time::Duration::from_secs(1);
        let pruned = manager.prune_terminal(cutoff);
        assert_eq!(pruned, 1);
        assert!(manager.get_order(order.id).is_none());
        assert!(manager.get_all_orders(user_id).is_empty());
    }

    #[test]
    fn test_prune_terminal_keeps_recent_terminal_orders() {
        let mut manager = ShadowOrderManager::new();
        let user_id = Uuid::new_v4();

        let order = manager.add_order(
            user_id,
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000)),
        );
        manager.cancel_order(user_id, order.id).unwrap();

        // Cutoff in the past — should NOT prune (order is too recent)
        let cutoff = Instant::now() - std::time::Duration::from_secs(3600);
        let pruned = manager.prune_terminal(cutoff);
        assert_eq!(pruned, 0);
        assert!(manager.get_order(order.id).is_some());
    }

    #[test]
    fn test_prune_terminal_keeps_open_orders() {
        let mut manager = ShadowOrderManager::new();
        let user_id = Uuid::new_v4();

        manager.add_order(
            user_id,
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000)),
        );

        // Even with future cutoff, open orders stay
        let cutoff = Instant::now() + std::time::Duration::from_secs(1);
        let pruned = manager.prune_terminal(cutoff);
        assert_eq!(pruned, 0);
        assert_eq!(manager.get_open_orders(user_id).len(), 1);
    }
}
