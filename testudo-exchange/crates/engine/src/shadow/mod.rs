//! Shadow Engine Module
//!
//! Provides paper trading capabilities by simulating order execution
//! against live market data from Binance. This enables users to:
//!
//! - Practice trading with virtual funds
//! - Test strategies without risk
//! - Learn the platform before connecting real accounts
//!
//! # Architecture
//!
//! The Shadow Engine operates in parallel with the main exchange engine:
//! - **Balances**: Virtual account balances per user
//! - **Orders**: Limit orders that fill when price conditions are met
//! - **Positions**: Tracked positions with P&L calculated against mark price
//!
//! # Fill Logic (from PRD)
//!
//! - **Buy Limit**: Fills if `Low Price <= Limit Price`
//! - **Buy Market**: Fills immediately at `Best Ask`
//! - **Sell Limit**: Fills if `High Price >= Limit Price`
//! - **Sell Market**: Fills immediately at `Best Bid`
//!
//! # Example
//!
//! ```ignore
//! use shadow::{ShadowEngine, ShadowOrder};
//!
//! let mut engine = ShadowEngine::new();
//! let user_id = Uuid::new_v4();
//!
//! // Initialize user with demo balance
//! engine.init_user(user_id);
//!
//! // Place a shadow order
//! let order = ShadowOrder::limit_buy("BTC_USDT", dec!(50000), dec!(0.1));
//! engine.place_order(user_id, order);
//! ```

// @anchor exchange:engine:mod
// @tags domain

pub mod actor;
pub mod balances;
pub mod handle;
pub mod order_group;
pub mod orders;
pub mod positions;
pub mod trade_event;
pub mod transaction;

pub use actor::EngineActor;
pub use handle::{EngineCommand, EngineError, EngineHandle, FillEvent, OrderRole};
pub use balances::{ShadowBalance, ShadowBalanceManager};
pub use trade_event::{TradeEvent, TradeEventType};
pub use order_group::{
    BreakEvenConfig, OrderGroup, OrderGroupManager, OrderGroupStatus, TakeProfitTarget,
};
pub use orders::{
    ShadowOrder, ShadowOrderManager, ShadowOrderSide, ShadowOrderStatus, ShadowOrderType,
};
pub use positions::{ShadowPosition, ShadowPositionManager};
pub use transaction::{TransactionContext, TransactionError};

use rust_decimal::Decimal;
use uuid::Uuid;

/// An exchange order that needs cancelling due to OCO sibling fill.
#[derive(Debug, Clone)]
pub struct ExchangeCancel {
    pub user_id: Uuid,
    pub exchange_order_id: String,
    pub exchange_account_id: Option<Uuid>,
}

/// Result of a price update, containing filled orders and any exchange cancels needed.
#[derive(Debug)]
pub struct PriceUpdateResult {
    pub filled: Vec<ShadowOrder>,
    pub exchange_cancels: Vec<ExchangeCancel>,
}

/// Represents a fill operation computed during Phase 2 of Read-Compute-Write.
///
/// Contains all information needed to apply a fill atomically in Phase 3,
/// without requiring any reads from shared state.
struct FillOperation {
    /// The order to be filled (snapshot from read phase)
    order: ShadowOrder,
    /// The calculated fill price based on order type and market data
    fill_price: Decimal,
}

/// Default starting balance for demo accounts (in USDC)
pub const DEFAULT_DEMO_BALANCE_USDC: &str = "10000";

/// Default starting balance for demo accounts (in BTC)
pub const DEFAULT_DEMO_BALANCE_BTC: &str = "0";

/// Shadow Engine - Paper trading simulation engine
///
/// Coordinates balances, orders, and positions for virtual trading
/// against live market prices.
///
/// # Ownership (019e)
///
/// The EngineActor is the sole owner of ShadowEngine. All external access
/// goes through EngineHandle (actor message passing). No concurrent access
/// exists, so no locks are needed.
///
/// - `ShadowBalanceManager` uses DashMap internally for lock-free per-user access
/// - `ShadowOrderManager`, `ShadowPositionManager`, `OrderGroupManager` are owned directly
pub struct ShadowEngine {
    pub balances: ShadowBalanceManager,
    pub orders: ShadowOrderManager,
    pub positions: ShadowPositionManager,
    pub order_groups: OrderGroupManager,
}

impl Default for ShadowEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute the balance changes for a filled order (futures settlement).
///
/// Lock-free operation via DashMap (FR-2.3.1).
///
/// For entry orders: deducts reserved margin (notional / leverage).
/// For exit orders (SL/TP): settles margin + P&L back to available balance.
///
/// Standalone function to avoid borrow conflicts in process_price_update.
fn execute_fill(
    balances: &ShadowBalanceManager,
    order: &ShadowOrder,
    fill_price: Decimal,
    position_entry: Option<(Decimal, positions::PositionSide)>,
) -> Result<(), ShadowEngineError> {
    let quote = order.required_asset();
    let leverage = Decimal::from(order.leverage.max(1));

    if order.parent_order_id.is_none() {
        // Entry order: consume the reserved margin (matches required_amount())
        let margin = order.required_amount();
        balances.deduct_reserved(order.user_id, &quote, margin)?;
    } else if let Some((entry_price, position_side)) = position_entry {
        // Exit order (SL/TP): settle margin + P&L
        let margin = order.quantity * entry_price / leverage;
        let pnl = match position_side {
            positions::PositionSide::Long => order.quantity * (fill_price - entry_price),
            positions::PositionSide::Short => order.quantity * (entry_price - fill_price),
        };
        let settlement = margin + pnl;
        balances.add(order.user_id, &quote, settlement);
    }

    Ok(())
}

/// Create SL/TP orders for a filled entry order.
///
/// Standalone function to avoid borrow conflicts in process_price_update.
fn create_sl_tp_orders(
    entry_order: &ShadowOrder,
    group: &OrderGroup,
    _fill_price: Decimal,
) -> Vec<ShadowOrder> {
    let mut orders = Vec::new();
    let user_id = entry_order.user_id;
    let symbol = &entry_order.symbol;
    let quantity = entry_order.quantity;

    // Determine SL/TP sides based on entry side
    let (sl_side, tp_side) = match entry_order.side {
        ShadowOrderSide::Buy => (ShadowOrderSide::Sell, ShadowOrderSide::Sell),
        ShadowOrderSide::Sell => (ShadowOrderSide::Buy, ShadowOrderSide::Buy),
    };

    // Create stop loss order (inherits leverage from entry for settlement)
    if let Some(sl_price) = group.stop_loss_price {
        let mut sl_order = ShadowOrder::new(
            user_id,
            symbol.clone(),
            sl_side,
            ShadowOrderType::StopLoss,
            quantity,
            None,
            Some(sl_price),
            Some(entry_order.id),
        );
        sl_order.stop_loss_price = Some(sl_price);
        sl_order.leverage = entry_order.leverage;
        orders.push(sl_order);
    }

    // Create take profit order(s) (inherit leverage from entry for settlement)
    for target in &group.take_profit_targets {
        let tp_quantity = quantity * target.percent_to_close / Decimal::from(100);
        let mut tp_order = ShadowOrder::new(
            user_id,
            symbol.clone(),
            tp_side,
            ShadowOrderType::TakeProfit,
            tp_quantity,
            None,
            Some(target.price),
            Some(entry_order.id),
        );
        tp_order.take_profit_price = Some(target.price);
        tp_order.leverage = entry_order.leverage;
        orders.push(tp_order);
    }

    orders
}

impl ShadowEngine {
    /// Create a new Shadow Engine
    pub fn new() -> Self {
        Self {
            balances: ShadowBalanceManager::new(),
            orders: ShadowOrderManager::new(),
            positions: ShadowPositionManager::new(),
            order_groups: OrderGroupManager::new(),
        }
    }

    /// Initialize a user with default demo balances
    ///
    /// Lock-free operation via DashMap (FR-2.3.1)
    pub fn init_user(&self, user_id: Uuid) {
        self.balances.init_user_with_defaults(user_id);
    }

    /// Initialize a user with custom starting balance
    ///
    /// Lock-free operation via DashMap (FR-2.3.1)
    pub fn init_user_with_balance(&self, user_id: Uuid, usdt_balance: Decimal) {
        self.balances.set_balance(user_id, "USDT", usdt_balance);
    }

    /// Reset user's balances to default demo amounts
    /// Clears existing balances and gives a fresh start with 10,000 USDT
    ///
    /// Lock-free operation via DashMap (FR-2.3.1)
    pub fn reset_user(&self, user_id: Uuid) {
        self.balances.reset_user_to_defaults(user_id);
    }

    /// Check if a user has been initialized in the shadow engine
    ///
    /// Lock-free operation via DashMap (FR-2.3.1)
    pub fn user_exists(&self, user_id: Uuid) -> bool {
        self.balances.user_exists(user_id)
    }

    /// Get user's current balances
    ///
    /// Lock-free operation via DashMap (FR-2.3.1)
    pub fn get_balances(&self, user_id: Uuid) -> Vec<ShadowBalance> {
        self.balances.get_user_balances(user_id)
    }

    /// Get user's open orders
    pub fn get_open_orders(&self, user_id: Uuid) -> Vec<ShadowOrder> {
        self.orders.get_open_orders(user_id)
    }

    /// Get all symbols that currently have open orders (FR-2: Active Symbol Tracking)
    ///
    /// Used by PriceFeedService to determine which symbols need price polling.
    pub fn get_active_symbols(&self) -> Vec<String> {
        self.orders.get_active_symbols()
    }

    /// Get user's positions
    pub fn get_positions(&self, user_id: Uuid) -> Vec<ShadowPosition> {
        self.positions.get_positions(user_id)
    }

    /// Place a new shadow order
    ///
    /// Validates balance, reserves funds, and queues order for fill checking.
    /// If the order has stop_loss_price or take_profit_price set, creates an
    /// OrderGroup to track the linked orders.
    ///
    /// # FR-3 (003-risk-enforcement)
    ///
    /// Orders MUST be risk-validated before submission. The Decision Loop sets
    /// `order.risk_validated = true` after approving the order. Orders without
    /// this flag are rejected with `ShadowEngineError::RiskValidationRequired`.
    pub fn place_order(
        &mut self,
        user_id: Uuid,
        order: ShadowOrder,
    ) -> Result<ShadowOrder, ShadowEngineError> {
        // FR-3: Verify order has been risk-validated by Decision Loop
        if !order.is_risk_validated() {
            log::warn!(
                "Order {} rejected: risk validation required (symbol={}, user={}, side={:?}, qty={})",
                order.id,
                order.symbol,
                user_id,
                order.side,
                order.quantity
            );
            return Err(ShadowEngineError::RiskValidationRequired { order_id: order.id });
        }

        log::info!(
            "Order {} accepted: risk validation passed (symbol={}, user={}, side={:?}, qty={})",
            order.id,
            order.symbol,
            user_id,
            order.side,
            order.quantity
        );

        // Validate user has sufficient balance (lock-free via DashMap FR-2.3.1)
        let required_asset = order.required_asset();
        let required_amount = order.required_amount();

        let available = self.balances.get_available(user_id, &required_asset);
        if available < required_amount {
            return Err(ShadowEngineError::InsufficientBalance {
                required: required_amount,
                available,
                asset: required_asset,
            });
        }

        // Reserve the funds (lock-free via DashMap)
        self.balances
            .reserve(user_id, &required_asset, required_amount)?;

        // Add the order
        let placed_order = self.orders.add_order(user_id, order.clone());

        // Create order group if order has SL/TP attached
        if order.stop_loss_price.is_some() || order.take_profit_price.is_some() {
            let mut group = OrderGroup::new(
                user_id,
                placed_order.symbol.clone(),
                placed_order.id,
                placed_order.quantity,
            );

            if let Some(sl_price) = order.stop_loss_price {
                group = group.with_stop_loss(sl_price);
            }

            if let Some(tp_price) = order.take_profit_price {
                group = group.with_take_profit(tp_price, rust_decimal_macros::dec!(100));
            }

            self.order_groups.add_group(group);
        }

        Ok(placed_order)
    }

    /// Place an order without creating an OrderGroup.
    ///
    /// Used by update operations that swap individual orders within an existing group.
    /// The caller is responsible for updating the group's order references.
    pub fn place_order_no_group(
        &mut self,
        user_id: Uuid,
        order: ShadowOrder,
    ) -> Result<ShadowOrder, ShadowEngineError> {
        if !order.is_risk_validated() {
            return Err(ShadowEngineError::RiskValidationRequired { order_id: order.id });
        }

        let required_asset = order.required_asset();
        let required_amount = order.required_amount();

        let available = self.balances.get_available(user_id, &required_asset);
        if available < required_amount {
            return Err(ShadowEngineError::InsufficientBalance {
                required: required_amount,
                available,
                asset: required_asset,
            });
        }

        self.balances
            .reserve(user_id, &required_asset, required_amount)?;

        let placed_order = self.orders.add_order(user_id, order);

        Ok(placed_order)
    }

    /// Cancel a single order without cascading to the OrderGroup.
    ///
    /// Used by update operations (e.g., update_stop_loss, update_entry_price)
    /// that need to swap an individual order without destroying the group.
    ///
    /// Balance operations are lock-free via DashMap (FR-2.3.1)
    pub fn cancel_order_no_cascade(
        &mut self,
        user_id: Uuid,
        order_id: Uuid,
    ) -> Result<ShadowOrder, ShadowEngineError> {
        let order = self.orders.cancel_order(user_id, order_id)?;

        // Release reserved funds (lock-free via DashMap)
        self.balances
            .release(user_id, &order.required_asset(), order.required_amount())?;

        Ok(order)
    }

    /// Cancel an open order
    ///
    /// If the cancelled order is an entry order with an OrderGroup,
    /// all linked SL/TP orders are also cancelled (cascade cancel - D.2).
    ///
    /// Balance operations are lock-free via DashMap (FR-2.3.1)
    pub fn cancel_order(
        &mut self,
        user_id: Uuid,
        order_id: Uuid,
    ) -> Result<ShadowOrder, ShadowEngineError> {
        let order = self.orders.cancel_order(user_id, order_id)?;

        // Release reserved funds (lock-free via DashMap)
        self.balances
            .release(user_id, &order.required_asset(), order.required_amount())?;

        // Check if this is an entry order with linked orders (cascade cancel)
        if let Some(group) = self.order_groups.get_by_entry_order_mut(order_id) {
            // Cancel all linked SL/TP orders
            let linked_ids = group.get_linked_order_ids();
            group.cancel();

            for linked_id in linked_ids {
                if let Ok(linked_order) = self.orders.cancel_order(user_id, linked_id) {
                    // Release reserved funds for linked orders (lock-free)
                    let _ = self.balances.release(
                        user_id,
                        &linked_order.required_asset(),
                        linked_order.required_amount(),
                    );
                }
            }
        }

        Ok(order)
    }

    /// Process market data update - check for order fills
    ///
    /// Called when new price data arrives. Checks all open orders
    /// against current prices and fills those that meet conditions.
    ///
    /// When an entry order fills and has an associated OrderGroup,
    /// automatically creates SL/TP orders.
    ///
    /// When a SL/TP order fills, cancels sibling orders.
    pub fn process_price_update(
        &mut self,
        symbol: &str,
        bid: Decimal,
        ask: Decimal,
        high: Decimal,
        low: Decimal,
    ) -> PriceUpdateResult {
        // Identify triggered orders
        let triggered_orders = self.orders.get_triggerable_orders(symbol, bid, ask, high, low);

        if triggered_orders.is_empty() {
            return PriceUpdateResult {
                filled: Vec::new(),
                exchange_cancels: Vec::new(),
            };
        }

        // Calculate fills
        let fill_operations: Vec<FillOperation> = triggered_orders
            .into_iter()
            .map(|order| {
                let fill_price = match (&order.order_type, &order.side) {
                    (ShadowOrderType::Market, ShadowOrderSide::Buy) => ask,
                    (ShadowOrderType::Market, ShadowOrderSide::Sell) => bid,
                    (ShadowOrderType::Limit, _) => order.price.unwrap_or(ask),
                    (ShadowOrderType::StopLoss, _) => order.stop_price.unwrap_or(bid),
                    (ShadowOrderType::TakeProfit, _) => order.stop_price.unwrap_or(ask),
                };
                FillOperation { order, fill_price }
            })
            .collect();

        // Apply all changes
        let mut filled_orders = Vec::new();
        let mut sl_tp_orders_to_create: Vec<ShadowOrder> = Vec::new();
        let mut orders_to_cancel: Vec<Uuid> = Vec::new();
        let mut exchange_cancels: Vec<ExchangeCancel> = Vec::new();

        // Mark all triggered orders as filled
        let order_ids: Vec<Uuid> = fill_operations.iter().map(|op| op.order.id).collect();
        self.orders.apply_fills(&order_ids);

        // Process each fill operation
        for FillOperation {
            mut order,
            fill_price,
        } in fill_operations
        {
            // Execute the fill on our copy
            order.fill(fill_price);

            // Look up position entry price for exit order settlement
            let position_entry = if order.parent_order_id.is_some() {
                self.positions.get_entry_price(order.user_id, &order.symbol)
            } else {
                None
            };

            // Update balances (lock-free via DashMap)
            let _ = execute_fill(&self.balances, &order, fill_price, position_entry);

            // Update positions
            self.positions.update_from_fill(&order, fill_price);

            // Check if this is an entry order with an order group
            if let Some(group) = self.order_groups.get_by_entry_order_mut(order.id) {
                // Entry order filled - create SL/TP orders
                group.on_entry_filled(fill_price);

                let sl_tp = create_sl_tp_orders(&order, group, fill_price);
                sl_tp_orders_to_create.extend(sl_tp);
            } else if let Some(group) = self.order_groups.get_by_linked_order_mut(order.id) {
                // SL or TP order filled - handle sibling cancellation
                if group.stop_loss_order_id == Some(order.id) {
                    // SL filled - cancel all TPs (shadow)
                    group.on_stop_loss_filled();
                    orders_to_cancel.extend(group.take_profit_order_ids.clone());
                    // OCO: cancel TP on exchange
                    if let Some(ref tp_exch_id) = group.exchange_tp_order_id {
                        exchange_cancels.push(ExchangeCancel {
                            user_id: group.user_id,
                            exchange_order_id: tp_exch_id.clone(),
                            exchange_account_id: group.exchange_account_id,
                        });
                    }
                } else if group.take_profit_order_ids.contains(&order.id) {
                    // TP filled - cancel SL (and other TPs for full exit)
                    group.on_take_profit_filled(order.id);
                    if let Some(sl_id) = group.stop_loss_order_id {
                        orders_to_cancel.push(sl_id);
                    }
                    // OCO: cancel SL on exchange
                    if let Some(ref sl_exch_id) = group.exchange_sl_order_id {
                        exchange_cancels.push(ExchangeCancel {
                            user_id: group.user_id,
                            exchange_order_id: sl_exch_id.clone(),
                            exchange_account_id: group.exchange_account_id,
                        });
                    }
                    // Cancel other unfilled TPs
                    for tp_id in &group.take_profit_order_ids {
                        if *tp_id != order.id {
                            orders_to_cancel.push(*tp_id);
                        }
                    }
                }
            }

            filled_orders.push(order);
        }

        // Add the SL/TP orders that were created and register them
        for sl_tp_order in sl_tp_orders_to_create {
            let user_id = sl_tp_order.user_id;
            let parent_id = sl_tp_order.parent_order_id;
            let order_type = sl_tp_order.order_type;
            let added = self.orders.add_order(user_id, sl_tp_order);

            // Register in order group for cascade lookup
            if let Some(parent_id) = parent_id {
                if let Some(group) = self.order_groups.get_by_entry_order_mut(parent_id) {
                    let group_id = group.id;
                    if order_type == ShadowOrderType::StopLoss {
                        group.set_stop_loss_order(added.id);
                    } else if order_type == ShadowOrderType::TakeProfit {
                        group.add_take_profit_order(added.id, 0);
                    }
                    self.order_groups.register_linked_order(added.id, group_id);
                }
            }
        }

        // Cancel sibling orders
        for order_id in orders_to_cancel {
            if let Some(order) = self.orders.get_order(order_id) {
                if order.is_open() {
                    let user_id = order.user_id;
                    let _ = self.orders.cancel_order(user_id, order_id);
                }
            }
        }

        PriceUpdateResult {
            filled: filled_orders,
            exchange_cancels,
        }
    }

    /// Update mark prices for P&L calculation
    pub fn update_mark_price(&mut self, symbol: &str, mark_price: Decimal) {
        self.positions.update_mark_price(symbol, mark_price);
    }

    /// Check and trigger break-even for active order groups
    ///
    /// Called with current market price. If position profit >= trigger_percent,
    /// moves the SL to entry price (+ offset).
    pub fn check_break_even(&mut self, symbol: &str, current_price: Decimal) {
        // Find all active groups for this symbol that have break-even enabled
        let groups_to_check = self.order_groups.get_break_even_candidates(symbol);

        for group_id in groups_to_check {
            if let Some(group) = self.order_groups.get_group_mut(group_id) {
                if group.should_trigger_break_even(current_price) {
                    // Get the new SL price (entry + offset)
                    if let Some(new_sl_price) = group.get_break_even_price() {
                        // Update the SL order price
                        if let Some(sl_order_id) = group.stop_loss_order_id {
                            self.orders.update_stop_price(sl_order_id, new_sl_price);
                        }
                        // Mark break-even as triggered
                        group.mark_break_even_triggered();
                    }
                }
            }
        }
    }

    /// Enable break-even for an existing order group
    pub fn enable_break_even(
        &mut self,
        group_id: Uuid,
        trigger_percent: Decimal,
        offset: Option<Decimal>,
    ) -> Result<(), ShadowEngineError> {
        if let Some(group) = self.order_groups.get_group_mut(group_id) {
            group.break_even_config = Some(BreakEvenConfig {
                trigger_percent,
                offset,
                triggered: false,
            });
            Ok(())
        } else {
            Err(ShadowEngineError::OrderNotFound(group_id))
        }
    }

    /// Get total unrealized P&L for a user
    pub fn get_unrealized_pnl(&self, user_id: Uuid) -> Decimal {
        self.positions.get_total_unrealized_pnl(user_id)
    }
}

/// Errors that can occur in the Shadow Engine
#[derive(Debug, thiserror::Error)]
pub enum ShadowEngineError {
    #[error("Insufficient balance: need {required} {asset}, have {available}")]
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
        asset: String,
    },

    #[error("Order not found: {0}")]
    OrderNotFound(Uuid),

    #[error("Invalid order: {0}")]
    InvalidOrder(String),

    #[error("Balance error: {0}")]
    BalanceError(String),

    /// FR-3 (003-risk-enforcement): Order must be risk-validated before submission
    #[error("Risk validation required: order {order_id} was not validated by Decision Loop")]
    RiskValidationRequired { order_id: Uuid },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    /// Helper: Create a risk-validated order for testing.
    /// In production, the Decision Loop calls mark_risk_validated().
    fn validated_order(mut order: ShadowOrder) -> ShadowOrder {
        order.mark_risk_validated();
        order
    }

    #[test]
    fn test_shadow_engine_creation() {
        let engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        let balances = engine.get_balances(user_id);
        assert!(!balances.is_empty());
    }

    // FR-3: Test that order without risk_validated is rejected
    #[test]
    fn test_place_order_rejected_without_risk_validation() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        // Create order WITHOUT calling mark_risk_validated()
        let order = ShadowOrder::new(
            user_id,
            "BTC_USDT".to_string(),
            ShadowOrderSide::Buy,
            ShadowOrderType::Limit,
            dec!(0.01),
            Some(dec!(50000)),
            None,
            None,
        );

        // Should be rejected because risk_validated = false
        let result = engine.place_order(user_id, order);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ShadowEngineError::RiskValidationRequired { .. }
        ));
    }

    // FR-3: Test that order with risk_validated is accepted
    #[test]
    fn test_place_order_accepted_with_risk_validation() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        // Create order WITH risk validation
        let order = validated_order(ShadowOrder::new(
            user_id,
            "BTC_USDT".to_string(),
            ShadowOrderSide::Buy,
            ShadowOrderType::Limit,
            dec!(0.01),
            Some(dec!(50000)),
            None,
            None,
        ));

        // Should be accepted
        let result = engine.place_order(user_id, order);
        assert!(result.is_ok());
    }

    #[test]
    fn test_place_order_insufficient_balance() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        // Initialize with small balance
        engine.init_user_with_balance(user_id, dec!(100));

        // Try to place risk-validated order for more than we have
        let order = validated_order(ShadowOrder::new(
            user_id,
            "BTC_USDT".to_string(),
            ShadowOrderSide::Buy,
            ShadowOrderType::Limit,
            dec!(0.01),
            Some(dec!(50000)), // 0.01 BTC @ 50000 = 500 USDT needed
            None,
            None,
        ));

        let result = engine.place_order(user_id, order);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ShadowEngineError::InsufficientBalance { .. }
        ));
    }

    #[test]
    fn test_place_and_cancel_order() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        let order = validated_order(ShadowOrder::new(
            user_id,
            "BTC_USDT".to_string(),
            ShadowOrderSide::Buy,
            ShadowOrderType::Limit,
            dec!(0.01),
            Some(dec!(50000)),
            None,
            None,
        ));

        let placed = engine.place_order(user_id, order).unwrap();
        assert_eq!(placed.status, ShadowOrderStatus::Open);

        let cancelled = engine.cancel_order(user_id, placed.id).unwrap();
        assert_eq!(cancelled.status, ShadowOrderStatus::Cancelled);
    }

    // FR-2: Active symbol tracking for price feed
    #[test]
    fn test_get_active_symbols() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        // No orders = no active symbols
        assert!(engine.get_active_symbols().is_empty());

        // Place order for BTC_USDT
        let order = validated_order(ShadowOrder::limit_buy(
            user_id,
            "BTC_USDT",
            dec!(0.01),
            dec!(50000),
        ));
        let placed = engine.place_order(user_id, order).unwrap();

        let symbols = engine.get_active_symbols();
        assert_eq!(symbols.len(), 1);
        assert!(symbols.contains(&"BTC_USDT".to_string()));

        // Cancel it
        engine.cancel_order(user_id, placed.id).unwrap();
        assert!(engine.get_active_symbols().is_empty());
    }

    // D.1: SL/TP auto-creation on entry fill
    #[test]
    fn test_sl_tp_created_on_entry_fill() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        // Place risk-validated entry order with SL/TP
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );

        let placed = engine.place_order(user_id, order).unwrap();
        assert_eq!(placed.status, ShadowOrderStatus::Open);

        // Verify order group was created
        let group = engine.order_groups.get_by_entry_order(placed.id);
        assert!(group.is_some());
        let group = group.unwrap();
        assert_eq!(group.stop_loss_price, Some(dec!(49000)));
        assert_eq!(group.status, OrderGroupStatus::Pending);

        // At this point, NO SL/TP orders should exist yet
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 1); // Only entry order

        // Simulate price hitting entry (ask <= limit price)
        let _result = engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Now SL and TP orders should exist
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 2); // SL + TP

        // Verify SL order
        let sl_order = open_orders
            .iter()
            .find(|o| o.order_type == ShadowOrderType::StopLoss);
        assert!(sl_order.is_some());
        let sl_order = sl_order.unwrap();
        assert_eq!(sl_order.stop_price, Some(dec!(49000)));
        assert_eq!(sl_order.side, ShadowOrderSide::Sell); // Exit side opposite of entry

        // Verify TP order
        let tp_order = open_orders
            .iter()
            .find(|o| o.order_type == ShadowOrderType::TakeProfit);
        assert!(tp_order.is_some());
        let tp_order = tp_order.unwrap();
        assert_eq!(tp_order.stop_price, Some(dec!(52000)));

        // Verify order group status is now Active
        let group = engine.order_groups.get_by_entry_order(placed.id).unwrap();
        assert_eq!(group.status, OrderGroupStatus::Active);
    }

    // D.5: Sibling cancellation when SL fills
    #[test]
    fn test_sl_fill_cancels_tp() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        // Place risk-validated entry order with SL/TP
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );

        let placed = engine.place_order(user_id, order).unwrap();

        // Fill the entry (ask <= limit price of 50000)
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Verify SL and TP exist
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 2);

        // Now trigger the SL (bid <= stop price of 49000)
        engine.process_price_update(
            "BTC_USDT",
            dec!(48900),
            dec!(49100),
            dec!(49200),
            dec!(48900),
        );

        // TP should be cancelled, no open orders left
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 0);

        // Verify order group status
        let group = engine.order_groups.get_by_entry_order(placed.id).unwrap();
        assert_eq!(group.status, OrderGroupStatus::StoppedOut);
    }

    // D.2: Cascade cancel when entry cancelled (before fill)
    #[test]
    fn test_cancel_entry_before_fill() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        // Place risk-validated entry order with SL/TP
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );

        let placed = engine.place_order(user_id, order).unwrap();

        // Entry is open, no SL/TP yet (not filled)
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 1);

        // Cancel entry
        let cancelled = engine.cancel_order(user_id, placed.id).unwrap();
        assert_eq!(cancelled.status, ShadowOrderStatus::Cancelled);

        // Order group should be cancelled
        let group = engine.order_groups.get_by_entry_order(placed.id).unwrap();
        assert_eq!(group.status, OrderGroupStatus::Cancelled);
    }

    // D.2: Manual cancel of SL/TP doesn't cascade (only fill cascades)
    // This is intentional - user may want to cancel SL but keep TP running
    #[test]
    fn test_manual_cancel_sl_does_not_cascade() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        // Place risk-validated entry order with SL/TP
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );

        engine.place_order(user_id, order).unwrap();

        // Fill the entry (ask <= limit price of 50000)
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // SL and TP should exist
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 2);

        // Get SL order ID
        let sl_order = open_orders
            .iter()
            .find(|o| o.order_type == ShadowOrderType::StopLoss)
            .unwrap();
        let sl_id = sl_order.id;

        // Cancel just the SL order manually
        // Note: SL/TP orders are exit orders - they don't reserve funds
        // because the position already holds the asset from the entry fill
        let _ = engine.orders.cancel_order(user_id, sl_id);

        // TP should still be open (manual cancel doesn't cascade)
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 1);
        assert_eq!(open_orders[0].order_type, ShadowOrderType::TakeProfit);
    }

    // D.3: Break-even automation - move SL to entry at 1% profit
    #[test]
    fn test_break_even_moves_sl_to_entry() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        // Place risk-validated entry order with SL/TP
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );

        let placed = engine.place_order(user_id, order).unwrap();

        // Enable break-even at 1% profit
        let group = engine.order_groups.get_by_entry_order(placed.id).unwrap();
        let group_id = group.id;

        engine
            .enable_break_even(group_id, dec!(1), None)
            .unwrap();

        // Fill the entry at 50000 (ask <= limit price)
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Verify SL is at original 49000
        let open_orders = engine.get_open_orders(user_id);
        let sl_order = open_orders
            .iter()
            .find(|o| o.order_type == ShadowOrderType::StopLoss)
            .unwrap();
        assert_eq!(sl_order.stop_price, Some(dec!(49000)));

        // Price moves up to 0.5% profit (50250) - break-even should NOT trigger
        engine.check_break_even("BTC_USDT", dec!(50250));

        let open_orders = engine.get_open_orders(user_id);
        let sl_order = open_orders
            .iter()
            .find(|o| o.order_type == ShadowOrderType::StopLoss)
            .unwrap();
        assert_eq!(sl_order.stop_price, Some(dec!(49000))); // Still at original

        // Price moves up to 1% profit (50500) - break-even should trigger
        engine.check_break_even("BTC_USDT", dec!(50500));

        let open_orders = engine.get_open_orders(user_id);
        let sl_order = open_orders
            .iter()
            .find(|o| o.order_type == ShadowOrderType::StopLoss)
            .unwrap();
        assert_eq!(sl_order.stop_price, Some(dec!(50000))); // Moved to entry!

        // Verify break-even is marked as triggered (won't trigger again)
        let group = engine.order_groups.get_group(group_id).unwrap();
        assert!(group.break_even_config.as_ref().unwrap().triggered);
    }

    // D.3: Break-even with offset
    #[test]
    fn test_break_even_with_offset() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );

        let placed = engine.place_order(user_id, order).unwrap();

        let group_id = engine.order_groups.get_by_entry_order(placed.id).unwrap().id;

        // Enable break-even with +50 offset
        engine
            .enable_break_even(group_id, dec!(1), Some(dec!(50)))
            .unwrap();

        // Fill entry (ask <= limit price)
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Trigger break-even at 1% profit
        engine.check_break_even("BTC_USDT", dec!(50500));

        // SL should be at entry + offset = 50050
        let open_orders = engine.get_open_orders(user_id);
        let sl_order = open_orders
            .iter()
            .find(|o| o.order_type == ShadowOrderType::StopLoss)
            .unwrap();
        assert_eq!(sl_order.stop_price, Some(dec!(50050)));
    }

    // D.5: Sibling cancellation when single TP fills (100% exit)
    #[test]
    fn test_tp_fill_cancels_sl() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        // Place risk-validated entry order with SL/TP
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );

        let placed = engine.place_order(user_id, order).unwrap();

        // Fill the entry (ask <= limit price of 50000)
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Now trigger the TP (bid >= take profit price of 52000)
        engine.process_price_update(
            "BTC_USDT",
            dec!(52100),
            dec!(52200),
            dec!(52200),
            dec!(51800),
        );

        // SL should be cancelled, no open orders left
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 0);

        // Verify order group status
        let group = engine.order_groups.get_by_entry_order(placed.id).unwrap();
        assert_eq!(group.status, OrderGroupStatus::TookProfit);
    }

    // Futures: SHORT orders should work with USDT margin
    #[test]
    fn test_place_short_order() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        // Place a short (sell) order - should work in futures with USDT margin
        let order = validated_order(ShadowOrder::new(
            user_id,
            "BTC_USDT".to_string(),
            ShadowOrderSide::Sell,
            ShadowOrderType::Limit,
            dec!(0.01),
            Some(dec!(50000)), // 0.01 * 50000 = 500 USDT margin
            None,
            None,
        ));

        // This should succeed - shorts in futures only need USDT margin
        let result = engine.place_order(user_id, order);
        assert!(
            result.is_ok(),
            "Short order should succeed with USDT margin: {:?}",
            result.err()
        );
    }

    // Futures: SHORT lifecycle with correct P&L settlement
    #[test]
    fn test_short_lifecycle_correct_pnl() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        // Check starting balance
        let start_usdt = engine.balances.get_available(user_id, "USDT");
        assert_eq!(start_usdt, dec!(10000));

        // Place SHORT entry with SL/TP
        let order = validated_order(
            ShadowOrder::limit_sell(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(51000)) // SL above entry (loss for short)
                .with_take_profit(dec!(48000)), // TP below entry (profit for short)
        );
        engine.place_order(user_id, order).unwrap();

        // Margin reserved: 0.1 * 50000 = 5000 USDT
        let available = engine.balances.get_available(user_id, "USDT");
        assert_eq!(available, dec!(5000)); // 10000 - 5000 reserved

        // Fill the entry (bid >= limit price for sell)
        engine.process_price_update(
            "BTC_USDT",
            dec!(50000),
            dec!(50100),
            dec!(50200),
            dec!(49900),
        );

        // SL and TP should exist
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 2); // SL + TP

        // Verify SL is a BUY (exit for short)
        let sl_order = open_orders
            .iter()
            .find(|o| o.order_type == ShadowOrderType::StopLoss)
            .unwrap();
        assert_eq!(sl_order.side, ShadowOrderSide::Buy);
        assert_eq!(sl_order.stop_price, Some(dec!(51000)));

        // Trigger TP (ask drops to TP price of 48000 for short profit)
        engine.process_price_update(
            "BTC_USDT",
            dec!(47900),
            dec!(48000),
            dec!(48100),
            dec!(47800),
        );

        // All orders should be closed
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 0);

        // P&L check: short from 50000, exit at 48000
        // Profit = 0.1 * (50000 - 48000) = 200 USDT
        // Final balance should be 10000 + 200 = 10200
        let final_usdt = engine.balances.get_available(user_id, "USDT");
        assert_eq!(final_usdt, dec!(10200));
    }

    // Futures: LONG lifecycle with correct P&L settlement (verify no regression)
    #[test]
    fn test_long_lifecycle_correct_pnl() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        let start_usdt = engine.balances.get_available(user_id, "USDT");
        assert_eq!(start_usdt, dec!(10000));

        // Place LONG entry with SL/TP
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );
        engine.place_order(user_id, order).unwrap();

        // Margin reserved: 0.1 * 50000 = 5000 USDT
        let available = engine.balances.get_available(user_id, "USDT");
        assert_eq!(available, dec!(5000));

        // Fill entry (ask <= limit price)
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Trigger TP (bid >= 52000)
        engine.process_price_update(
            "BTC_USDT",
            dec!(52100),
            dec!(52200),
            dec!(52200),
            dec!(51800),
        );

        // P&L: long from 50000, exit at 52000
        // Profit = 0.1 * (52000 - 50000) = 200 USDT
        // Final balance should be 10000 + 200 = 10200
        let final_usdt = engine.balances.get_available(user_id, "USDT");
        assert_eq!(final_usdt, dec!(10200));
    }

    // Leveraged trade: margin = notional / leverage, P&L unaffected by leverage
    #[test]
    fn test_leveraged_long_lifecycle() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        let start = engine.balances.get_available(user_id, "USDT");
        assert_eq!(start, dec!(10000));

        // 0.3 BTC @ 50000 with 10x leverage
        // Notional = 15000, Margin = 15000 / 10 = 1500
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.3), dec!(50000))
                .with_leverage(10)
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );
        engine.place_order(user_id, order).unwrap();

        let available = engine.balances.get_available(user_id, "USDT");
        assert_eq!(available, dec!(8500)); // 10000 - 1500 margin

        // Fill entry
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49800),
        );

        // Trigger TP at 52000
        engine.process_price_update(
            "BTC_USDT",
            dec!(52100),
            dec!(52200),
            dec!(52200),
            dec!(51800),
        );

        // P&L: 0.3 * (52000 - 50000) = 600 USDT
        // Settlement: margin (1500) + pnl (600) = 2100 returned
        // Final: 8500 + 2100 = 10600
        let final_usdt = engine.balances.get_available(user_id, "USDT");
        assert_eq!(final_usdt, dec!(10600));
    }

    // BTC_USD symbol (from TradingView) should resolve to USDT balance
    #[test]
    fn test_usd_symbol_uses_usdt_balance() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        // "BTC_USD" -- the extension normalizes BTCUSD from TradingView to BTC_USD
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USD", dec!(0.01), dec!(67000)).with_leverage(27),
        );

        // Should succeed: margin = 0.01 * 67000 / 27 ~= 24.81 USDT
        let result = engine.place_order(user_id, order);
        assert!(
            result.is_ok(),
            "BTC_USD order should use USDT balance: {:?}",
            result.err()
        );

        // Verify USDT balance was reduced
        let available = engine.balances.get_available(user_id, "USDT");
        assert!(
            available < dec!(10000),
            "USDT should have been reserved, got {}",
            available
        );
    }

    // D.4: Multi-target exits - scale out at multiple levels
    #[test]
    fn test_multi_target_exits() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();

        engine.init_user(user_id);

        // Place risk-validated entry with SL and multiple TPs via order group
        // Use 0.1 BTC @ 50000 = 5000 USDC (within 10000 balance)
        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000)),
        );

        let placed = engine.place_order(user_id, order).unwrap();

        // Add multiple TP targets to the order group
        {
            let group = engine.order_groups.get_by_entry_order_mut(placed.id).unwrap();
            // Clear any default TP and add multi-targets
            group.take_profit_targets.clear();
            group.take_profit_targets.push(TakeProfitTarget {
                price: dec!(52000),
                percent_to_close: dec!(50), // 50% at T1 (0.05 BTC)
                order_id: None,
                filled: false,
            });
            group.take_profit_targets.push(TakeProfitTarget {
                price: dec!(55000),
                percent_to_close: dec!(50), // 50% at T2 (0.05 BTC)
                order_id: None,
                filled: false,
            });
        }

        // Fill the entry (ask <= limit price of 50000)
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Should have SL + 2 TPs
        let open_orders = engine.get_open_orders(user_id);
        assert_eq!(open_orders.len(), 3); // 1 SL + 2 TPs

        // Verify TP quantities
        let tp_orders: Vec<_> = open_orders
            .iter()
            .filter(|o| o.order_type == ShadowOrderType::TakeProfit)
            .collect();
        assert_eq!(tp_orders.len(), 2);
        assert!(tp_orders.iter().all(|o| o.quantity == dec!(0.05))); // 50% of 0.1 = 0.05 each

        // Verify SL covers full position
        let sl_order = open_orders
            .iter()
            .find(|o| o.order_type == ShadowOrderType::StopLoss)
            .unwrap();
        assert_eq!(sl_order.quantity, dec!(0.1));
    }

    // OCO: SL fill returns exchange cancel for TP
    #[test]
    fn test_price_update_returns_exchange_cancel_on_sl_fill() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );
        let placed = engine.place_order(user_id, order).unwrap();

        // Fill entry
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Set exchange TP ID on the group (simulates create_trade storing it)
        let group_id = {
            let group = engine.order_groups.get_by_entry_order_mut(placed.id).unwrap();
            group.exchange_tp_order_id = Some("tp-123:BTC/USDT:USDT".to_string());
            group.exchange_account_id = Some(Uuid::new_v4());
            group.id
        };

        // Trigger SL fill
        let result = engine.process_price_update(
            "BTC_USDT",
            dec!(48900),
            dec!(49100),
            dec!(49200),
            dec!(48900),
        );

        // Should have exchange cancel for the TP
        assert_eq!(result.exchange_cancels.len(), 1);
        assert_eq!(
            result.exchange_cancels[0].exchange_order_id,
            "tp-123:BTC/USDT:USDT"
        );
        assert_eq!(result.exchange_cancels[0].user_id, user_id);

        // Verify group is stopped out
        let group = engine.order_groups.get_group(group_id).unwrap();
        assert_eq!(group.status, OrderGroupStatus::StoppedOut);
    }

    // OCO: TP fill returns exchange cancel for SL
    #[test]
    fn test_price_update_returns_exchange_cancel_on_tp_fill() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );
        let placed = engine.place_order(user_id, order).unwrap();

        // Fill entry
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Set exchange SL ID on the group
        {
            let group = engine.order_groups.get_by_entry_order_mut(placed.id).unwrap();
            group.exchange_sl_order_id = Some("sl-456:BTC/USDT:USDT".to_string());
            group.exchange_account_id = Some(Uuid::new_v4());
        }

        // Trigger TP fill
        let result = engine.process_price_update(
            "BTC_USDT",
            dec!(52100),
            dec!(52200),
            dec!(52200),
            dec!(51800),
        );

        // Should have exchange cancel for the SL
        assert_eq!(result.exchange_cancels.len(), 1);
        assert_eq!(
            result.exchange_cancels[0].exchange_order_id,
            "sl-456:BTC/USDT:USDT"
        );
    }

    // OCO: Paper trades (no exchange IDs) produce no exchange cancels
    #[test]
    fn test_price_update_no_exchange_cancel_for_paper_trades() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        let order = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.1), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );
        engine.place_order(user_id, order).unwrap();

        // Fill entry
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Trigger SL (no exchange IDs set -- paper trade)
        let result = engine.process_price_update(
            "BTC_USDT",
            dec!(48900),
            dec!(49100),
            dec!(49200),
            dec!(48900),
        );

        assert!(result.exchange_cancels.is_empty());
        assert!(!result.filled.is_empty()); // SL did fill in shadow
    }

    // OCO: Exchange cancels are isolated to the affected group
    #[test]
    fn test_price_update_exchange_cancel_isolated_to_group() {
        let mut engine = ShadowEngine::new();
        let user_id = Uuid::new_v4();
        engine.init_user(user_id);

        // Group A: BTC trade with exchange IDs
        let order_a = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.05), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(52000)),
        );
        let placed_a = engine.place_order(user_id, order_a).unwrap();

        // Group B: another BTC trade with exchange IDs
        let order_b = validated_order(
            ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.05), dec!(50000))
                .with_stop_loss(dec!(49000))
                .with_take_profit(dec!(53000)),
        );
        let placed_b = engine.place_order(user_id, order_b).unwrap();

        // Fill both entries
        engine.process_price_update(
            "BTC_USDT",
            dec!(49900),
            dec!(50000),
            dec!(50200),
            dec!(49900),
        );

        // Set exchange IDs on both groups
        {
            let group_a = engine.order_groups.get_by_entry_order_mut(placed_a.id).unwrap();
            group_a.exchange_tp_order_id = Some("tp-A:BTC/USDT:USDT".to_string());

            let group_b = engine.order_groups.get_by_entry_order_mut(placed_b.id).unwrap();
            group_b.exchange_tp_order_id = Some("tp-B:BTC/USDT:USDT".to_string());
        }

        // Trigger SL for both groups (same symbol, same price)
        let result = engine.process_price_update(
            "BTC_USDT",
            dec!(48900),
            dec!(49100),
            dec!(49200),
            dec!(48900),
        );

        // Both groups' TPs should be in exchange_cancels
        assert_eq!(result.exchange_cancels.len(), 2);
        let cancel_ids: Vec<&str> = result
            .exchange_cancels
            .iter()
            .map(|c| c.exchange_order_id.as_str())
            .collect();
        assert!(cancel_ids.contains(&"tp-A:BTC/USDT:USDT"));
        assert!(cancel_ids.contains(&"tp-B:BTC/USDT:USDT"));
    }
}
