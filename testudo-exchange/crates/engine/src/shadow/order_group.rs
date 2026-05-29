//! Order Group Management
//!
//! Links entry orders with their associated stop-loss and take-profit orders.
//! Provides cascade cancellation and order group tracking.
//!
//! # From PRD (D.1, D.2, D.5)
//!
//! - Entry orders can have linked SL/TP orders
//! - When entry fills, SL/TP orders are auto-created
//! - When SL fills, TPs are cancelled (and vice versa)
//! - Cancelling entry cancels all linked orders

// @anchor exchange:engine:order_group
// @tags domain

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

/// Status of an order group
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderGroupStatus {
    /// Entry order is pending (not yet filled)
    Pending,
    /// Entry filled, SL/TP orders active
    Active,
    /// Position closed via SL
    StoppedOut,
    /// Position closed via TP (full or partial)
    TookProfit,
    /// All orders cancelled
    Cancelled,
    /// Position closed manually
    Closed,
    /// 019e: Placement timed out — actor lost track, awaiting reconciliation sweep.
    AwaitingReconciliation,
}

impl OrderGroupStatus {
    /// AUD-02: Check if this status is terminal (no further transitions expected).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OrderGroupStatus::StoppedOut
                | OrderGroupStatus::TookProfit
                | OrderGroupStatus::Cancelled
                | OrderGroupStatus::Closed
        )
    }
}

/// Configuration for break-even automation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakEvenConfig {
    /// Percentage profit at which to move SL to entry (e.g., 1.0 = 1%)
    pub trigger_percent: Decimal,
    /// Optional offset above entry price (e.g., 10 = SL at entry + 10)
    pub offset: Option<Decimal>,
    /// Whether break-even has been triggered
    pub triggered: bool,
}

/// A take-profit target for multi-target exits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeProfitTarget {
    /// Target price
    pub price: Decimal,
    /// Percentage of position to close at this level (e.g., 50 = 50%)
    pub percent_to_close: Decimal,
    /// Order ID once created
    pub order_id: Option<Uuid>,
    /// Whether this target has been filled
    pub filled: bool,
}

/// An order group linking entry with SL/TP orders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderGroup {
    pub id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub entry_order_id: Uuid,
    pub entry_price: Option<Decimal>,
    pub entry_quantity: Decimal,
    pub stop_loss_order_id: Option<Uuid>,
    pub stop_loss_price: Option<Decimal>,
    pub take_profit_order_ids: Vec<Uuid>,
    pub take_profit_targets: Vec<TakeProfitTarget>,
    pub status: OrderGroupStatus,
    pub break_even_config: Option<BreakEvenConfig>,
    /// EXT-21: Exchange order ID from live order placement (for cancellation).
    pub exchange_order_id: Option<String>,
    /// OCO: Exchange SL order ID for live cancellation when TP fills.
    pub exchange_sl_order_id: Option<String>,
    /// OCO: Exchange TP order ID for live cancellation when SL fills.
    pub exchange_tp_order_id: Option<String>,
    /// EXT-21: Exchange account ID used for this trade.
    pub exchange_account_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// AUD-02: Timestamp when group reached terminal state (for GC).
    #[serde(skip)]
    pub completed_at: Option<Instant>,

    /// CON-01a: Actual exchange name ("woo", "binance", "bybit", "hyperliquid").
    /// Set at trade placement or rehydration. Used in TradeClosed payload.
    pub exchange_name: Option<String>,

    /// Risk amount in quote currency (e.g. USDT) at time of entry.
    /// Used to compute R-multiple in journal: R = net_pnl / risk_amount.
    pub risk_amount: Option<Decimal>,

    /// RSK-02: Optional user-supplied setup tag captured at Alt+X entry time
    /// (e.g. "breakout", "mean reversion"). Flows into journal_trades on close.
    #[serde(default)]
    pub setup_tag: Option<String>,

    /// QNT-01a: Calibration snapshot captured at trade entry when the user
    /// has Dynamic Risk enabled. `None` for fixed-mode trades and for
    /// dynamic-mode trades without a `setup_tag`. Flows into
    /// `journal_trades.kelly_inputs` on close; `#[serde(default)]` keeps
    /// pre-spec serialized groups deserializable.
    #[serde(default)]
    pub kelly_inputs: Option<serde_json::Value>,
}

impl OrderGroup {
    /// Create a new order group
    pub fn new(
        user_id: Uuid,
        symbol: String,
        entry_order_id: Uuid,
        entry_quantity: Decimal,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            symbol,
            entry_order_id,
            entry_price: None,
            entry_quantity,
            stop_loss_order_id: None,
            stop_loss_price: None,
            take_profit_order_ids: Vec::new(),
            take_profit_targets: Vec::new(),
            status: OrderGroupStatus::Pending,
            break_even_config: None,
            exchange_order_id: None,
            exchange_sl_order_id: None,
            exchange_tp_order_id: None,
            exchange_account_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
            exchange_name: None,
            risk_amount: None,
            setup_tag: None,
            kelly_inputs: None,
        }
    }

    /// Set the stop loss price for this group
    pub fn with_stop_loss(mut self, price: Decimal) -> Self {
        self.stop_loss_price = Some(price);
        self
    }

    /// Add a take profit target
    pub fn with_take_profit(mut self, price: Decimal, percent: Decimal) -> Self {
        self.take_profit_targets.push(TakeProfitTarget {
            price,
            percent_to_close: percent,
            order_id: None,
            filled: false,
        });
        self
    }

    /// Enable break-even automation
    pub fn with_break_even(mut self, trigger_percent: Decimal, offset: Option<Decimal>) -> Self {
        self.break_even_config = Some(BreakEvenConfig {
            trigger_percent,
            offset,
            triggered: false,
        });
        self
    }

    /// Mark the entry as filled and update status to Active
    pub fn on_entry_filled(&mut self, fill_price: Decimal) {
        self.entry_price = Some(fill_price);
        self.status = OrderGroupStatus::Active;
        self.updated_at = Utc::now();
    }

    /// Set the stop loss order ID after it's created
    pub fn set_stop_loss_order(&mut self, order_id: Uuid) {
        self.stop_loss_order_id = Some(order_id);
        self.updated_at = Utc::now();
    }

    /// Add a take profit order ID after it's created
    pub fn add_take_profit_order(&mut self, order_id: Uuid, target_index: usize) {
        self.take_profit_order_ids.push(order_id);
        if let Some(target) = self.take_profit_targets.get_mut(target_index) {
            target.order_id = Some(order_id);
        }
        self.updated_at = Utc::now();
    }

    /// Check if break-even should trigger at the given price
    pub fn should_trigger_break_even(&self, current_price: Decimal) -> bool {
        if let (Some(config), Some(entry_price)) = (&self.break_even_config, self.entry_price) {
            if config.triggered {
                return false;
            }
            let profit_percent = ((current_price - entry_price) / entry_price) * Decimal::from(100);
            profit_percent >= config.trigger_percent
        } else {
            false
        }
    }

    /// Mark break-even as triggered
    pub fn mark_break_even_triggered(&mut self) {
        if let Some(config) = &mut self.break_even_config {
            config.triggered = true;
            self.updated_at = Utc::now();
        }
    }

    /// Get the break-even price (entry + offset)
    pub fn get_break_even_price(&self) -> Option<Decimal> {
        self.entry_price.map(|entry| {
            let offset = self
                .break_even_config
                .as_ref()
                .and_then(|c| c.offset)
                .unwrap_or(Decimal::ZERO);
            entry + offset
        })
    }

    /// Update status when stop loss fills
    pub fn on_stop_loss_filled(&mut self) {
        self.status = OrderGroupStatus::StoppedOut;
        self.updated_at = Utc::now();
        self.completed_at = Some(Instant::now());
    }

    /// Update status when take profit fills
    pub fn on_take_profit_filled(&mut self, order_id: Uuid) {
        // Mark the specific target as filled
        for target in &mut self.take_profit_targets {
            if target.order_id == Some(order_id) {
                target.filled = true;
            }
        }

        // Check if all targets are filled
        let all_filled = self.take_profit_targets.iter().all(|t| t.filled);
        if all_filled {
            self.status = OrderGroupStatus::TookProfit;
            self.completed_at = Some(Instant::now());
        }
        self.updated_at = Utc::now();
    }

    /// Cancel the order group
    pub fn cancel(&mut self) {
        self.status = OrderGroupStatus::Cancelled;
        self.updated_at = Utc::now();
        self.completed_at = Some(Instant::now());
    }

    /// Get all linked order IDs (SL + TPs)
    pub fn get_linked_order_ids(&self) -> Vec<Uuid> {
        let mut ids = Vec::new();
        if let Some(sl_id) = self.stop_loss_order_id {
            ids.push(sl_id);
        }
        ids.extend(&self.take_profit_order_ids);
        ids
    }

    /// Check if this group is still active (has open orders)
    pub fn is_active(&self) -> bool {
        matches!(self.status, OrderGroupStatus::Active)
    }
}

/// Manages order groups
pub struct OrderGroupManager {
    /// All order groups by ID
    groups: HashMap<Uuid, OrderGroup>,
    /// Index by user
    groups_by_user: HashMap<Uuid, Vec<Uuid>>,
    /// Index by entry order ID (for quick lookup on fill)
    groups_by_entry_order: HashMap<Uuid, Uuid>,
    /// Index by SL/TP order ID (for cascade cancellation)
    groups_by_linked_order: HashMap<Uuid, Uuid>,
    /// EXT-22: Index by exchange order ID (entry, SL, TP) for fill detection
    groups_by_exchange_order: HashMap<String, Uuid>,
}

impl Default for OrderGroupManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderGroupManager {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
            groups_by_user: HashMap::new(),
            groups_by_entry_order: HashMap::new(),
            groups_by_linked_order: HashMap::new(),
            groups_by_exchange_order: HashMap::new(),
        }
    }

    /// Add a new order group
    pub fn add_group(&mut self, group: OrderGroup) -> OrderGroup {
        let group_id = group.id;
        let user_id = group.user_id;
        let entry_order_id = group.entry_order_id;

        // Store the group
        self.groups.insert(group_id, group.clone());

        // Index by user
        self.groups_by_user
            .entry(user_id)
            .or_default()
            .push(group_id);

        // Index by entry order
        self.groups_by_entry_order.insert(entry_order_id, group_id);

        group
    }

    /// Get an order group by ID
    pub fn get_group(&self, group_id: Uuid) -> Option<&OrderGroup> {
        self.groups.get(&group_id)
    }

    /// Get a mutable order group by ID
    pub fn get_group_mut(&mut self, group_id: Uuid) -> Option<&mut OrderGroup> {
        self.groups.get_mut(&group_id)
    }

    /// Get order group by entry order ID
    pub fn get_by_entry_order(&self, entry_order_id: Uuid) -> Option<&OrderGroup> {
        self.groups_by_entry_order
            .get(&entry_order_id)
            .and_then(|group_id| self.groups.get(group_id))
    }

    /// Get mutable order group by entry order ID
    pub fn get_by_entry_order_mut(&mut self, entry_order_id: Uuid) -> Option<&mut OrderGroup> {
        self.groups_by_entry_order
            .get(&entry_order_id)
            .copied()
            .and_then(move |group_id| self.groups.get_mut(&group_id))
    }

    /// Get order group by a linked order ID (SL or TP)
    pub fn get_by_linked_order(&self, order_id: Uuid) -> Option<&OrderGroup> {
        self.groups_by_linked_order
            .get(&order_id)
            .and_then(|group_id| self.groups.get(group_id))
    }

    /// Get mutable order group by a linked order ID
    pub fn get_by_linked_order_mut(&mut self, order_id: Uuid) -> Option<&mut OrderGroup> {
        self.groups_by_linked_order
            .get(&order_id)
            .copied()
            .and_then(move |group_id| self.groups.get_mut(&group_id))
    }

    /// Register a linked order (SL or TP) for cascade lookup
    pub fn register_linked_order(&mut self, order_id: Uuid, group_id: Uuid) {
        self.groups_by_linked_order.insert(order_id, group_id);
    }

    /// EXT-22: Register an exchange order ID for fill detection lookup
    pub fn register_exchange_order(&mut self, exchange_order_id: String, group_id: Uuid) {
        self.groups_by_exchange_order
            .insert(exchange_order_id, group_id);
    }

    /// EXT-22: Get order group by exchange order ID (entry, SL, or TP)
    pub fn get_by_exchange_order(&self, exchange_order_id: &str) -> Option<&OrderGroup> {
        self.groups_by_exchange_order
            .get(exchange_order_id)
            .and_then(|group_id| self.groups.get(group_id))
    }

    /// EXT-22: Get mutable order group by exchange order ID
    pub fn get_by_exchange_order_mut(
        &mut self,
        exchange_order_id: &str,
    ) -> Option<&mut OrderGroup> {
        self.groups_by_exchange_order
            .get(exchange_order_id)
            .copied()
            .and_then(move |group_id| self.groups.get_mut(&group_id))
    }

    /// Update the entry order index when entry price is modified
    ///
    /// # FR-5.4 (007-editable-position-levels)
    ///
    /// When a pending entry order's price is updated, the old order is cancelled
    /// and a new one created. This method updates the index to point to the new order.
    pub fn update_entry_order(&mut self, group_id: Uuid, old_entry_id: Uuid, new_entry_id: Uuid) {
        self.groups_by_entry_order.remove(&old_entry_id);
        self.groups_by_entry_order.insert(new_entry_id, group_id);
    }

    /// Get all order groups for a user
    pub fn get_user_groups(&self, user_id: Uuid) -> Vec<OrderGroup> {
        self.groups_by_user
            .get(&user_id)
            .map(|group_ids| {
                group_ids
                    .iter()
                    .filter_map(|id| self.groups.get(id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get active order groups for a user
    pub fn get_active_groups(&self, user_id: Uuid) -> Vec<OrderGroup> {
        self.get_user_groups(user_id)
            .into_iter()
            .filter(|g| g.is_active())
            .collect()
    }

    /// AUD-02 FR-3: Remove terminal groups older than cutoff from all 5 maps.
    pub fn prune_terminal(&mut self, cutoff: Instant) -> usize {
        let to_remove: Vec<Uuid> = self
            .groups
            .iter()
            .filter(|(_, g)| {
                g.status.is_terminal() && g.completed_at.is_some_and(|t| t < cutoff)
            })
            .map(|(id, _)| *id)
            .collect();

        for id in &to_remove {
            if let Some(group) = self.groups.remove(id) {
                // Clean groups_by_entry_order
                self.groups_by_entry_order.remove(&group.entry_order_id);

                // Clean groups_by_linked_order
                if let Some(sl_id) = group.stop_loss_order_id {
                    self.groups_by_linked_order.remove(&sl_id);
                }
                for tp_id in &group.take_profit_order_ids {
                    self.groups_by_linked_order.remove(tp_id);
                }

                // Clean groups_by_exchange_order
                if let Some(ref eo_id) = group.exchange_order_id {
                    self.groups_by_exchange_order.remove(eo_id);
                }
                if let Some(ref sl_eo_id) = group.exchange_sl_order_id {
                    self.groups_by_exchange_order.remove(sl_eo_id);
                }
                if let Some(ref tp_eo_id) = group.exchange_tp_order_id {
                    self.groups_by_exchange_order.remove(tp_eo_id);
                }
            }
        }

        // Clean groups_by_user
        for ids in self.groups_by_user.values_mut() {
            ids.retain(|id| !to_remove.contains(id));
        }

        to_remove.len()
    }

    /// Reindex the exchange SL order ID after an amend (break-even, trailing stop).
    ///
    /// Atomically removes the old ID from `groups_by_exchange_order`, inserts the
    /// new ID, and updates `group.exchange_sl_order_id`. Returns `true` if the
    /// group was found and reindexed, `false` if `old_id` was not in the index.
    pub fn reindex_exchange_sl_order(&mut self, old_id: &str, new_id: String) -> bool {
        let group_id = match self.groups_by_exchange_order.remove(old_id) {
            Some(gid) => gid,
            None => return false,
        };
        self.groups_by_exchange_order
            .insert(new_id.clone(), group_id);
        if let Some(group) = self.groups.get_mut(&group_id) {
            group.exchange_sl_order_id = Some(new_id);
            group.updated_at = chrono::Utc::now();
        }
        true
    }

    /// Get all groups for a symbol with break-even enabled but not triggered
    /// 017 FR-3: Count non-terminal order groups for reconciliation logging.
    pub fn active_count(&self) -> usize {
        self.groups.values().filter(|g| !g.status.is_terminal()).count()
    }

    /// 018: Get all non-terminal groups with exchange_account_id (live trades only).
    pub fn get_live_groups(&self) -> Vec<OrderGroup> {
        self.groups
            .values()
            .filter(|g| !g.status.is_terminal() && g.exchange_account_id.is_some())
            .cloned()
            .collect()
    }

    pub fn get_break_even_candidates(&self, symbol: &str) -> Vec<Uuid> {
        self.groups
            .values()
            .filter(|g| {
                g.symbol == symbol
                    && g.is_active()
                    && g.break_even_config.as_ref()
                        .is_some_and(|c| !c.triggered)
            })
            .map(|g| g.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_order_group_creation() {
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1))
            .with_stop_loss(dec!(49000))
            .with_take_profit(dec!(52000), dec!(100));

        assert_eq!(group.user_id, user_id);
        assert_eq!(group.entry_order_id, entry_id);
        assert_eq!(group.stop_loss_price, Some(dec!(49000)));
        assert_eq!(group.take_profit_targets.len(), 1);
        assert_eq!(group.take_profit_targets[0].price, dec!(52000));
        assert_eq!(group.status, OrderGroupStatus::Pending);
    }

    #[test]
    fn test_order_group_on_entry_filled() {
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let mut group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));
        group.on_entry_filled(dec!(50000));

        assert_eq!(group.entry_price, Some(dec!(50000)));
        assert_eq!(group.status, OrderGroupStatus::Active);
    }

    #[test]
    fn test_break_even_trigger() {
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let mut group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1))
            .with_break_even(dec!(1), None); // Trigger at 1% profit

        group.on_entry_filled(dec!(50000));

        // 0.5% profit - should not trigger
        assert!(!group.should_trigger_break_even(dec!(50250)));

        // 1% profit - should trigger
        assert!(group.should_trigger_break_even(dec!(50500)));

        // 2% profit - should trigger
        assert!(group.should_trigger_break_even(dec!(51000)));
    }

    #[test]
    fn test_break_even_with_offset() {
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let mut group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1))
            .with_break_even(dec!(1), Some(dec!(10))); // Trigger at 1%, offset +10

        group.on_entry_filled(dec!(50000));

        let be_price = group.get_break_even_price();
        assert_eq!(be_price, Some(dec!(50010))); // entry + offset
    }

    #[test]
    fn test_multi_target_exits() {
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(1.0))
            .with_take_profit(dec!(52000), dec!(50)) // 50% at T1
            .with_take_profit(dec!(55000), dec!(25)); // 25% at T2

        assert_eq!(group.take_profit_targets.len(), 2);
        assert_eq!(group.take_profit_targets[0].percent_to_close, dec!(50));
        assert_eq!(group.take_profit_targets[1].percent_to_close, dec!(25));
    }

    #[test]
    fn test_order_group_manager() {
        let mut manager = OrderGroupManager::new();
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));
        let added = manager.add_group(group);

        // Should be findable by group ID
        assert!(manager.get_group(added.id).is_some());

        // Should be findable by entry order ID
        assert!(manager.get_by_entry_order(entry_id).is_some());

        // Should be in user's groups
        let user_groups = manager.get_user_groups(user_id);
        assert_eq!(user_groups.len(), 1);
    }

    #[test]
    fn test_linked_order_lookup() {
        let mut manager = OrderGroupManager::new();
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();
        let sl_id = Uuid::new_v4();

        let group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));
        let added = manager.add_group(group);

        // Register the SL order
        manager.register_linked_order(sl_id, added.id);

        // Should be findable by SL order ID
        let found = manager.get_by_linked_order(sl_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, added.id);
    }

    #[test]
    fn test_exchange_order_id_index_register_and_lookup() {
        let mut manager = OrderGroupManager::new();
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let mut group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));
        group.exchange_order_id = Some("entry-001".to_string());
        group.exchange_sl_order_id = Some("sl-002".to_string());
        group.exchange_tp_order_id = Some("tp-003".to_string());
        let added = manager.add_group(group);

        // Register exchange order IDs
        manager.register_exchange_order("entry-001".to_string(), added.id);
        manager.register_exchange_order("sl-002".to_string(), added.id);
        manager.register_exchange_order("tp-003".to_string(), added.id);

        // Immutable lookup
        assert!(manager.get_by_exchange_order("entry-001").is_some());
        assert_eq!(
            manager.get_by_exchange_order("entry-001").unwrap().id,
            added.id
        );
        assert!(manager.get_by_exchange_order("sl-002").is_some());
        assert!(manager.get_by_exchange_order("tp-003").is_some());

        // Unknown exchange ID returns None
        assert!(manager.get_by_exchange_order("unknown-999").is_none());
    }

    #[test]
    fn test_exchange_order_id_index_mutable_lookup() {
        let mut manager = OrderGroupManager::new();
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));
        let added = manager.add_group(group);
        manager.register_exchange_order("sl-100".to_string(), added.id);

        // Mutable lookup — update status
        let group_mut = manager.get_by_exchange_order_mut("sl-100").unwrap();
        group_mut.on_stop_loss_filled();

        // Verify mutation persisted
        let group_ref = manager.get_by_exchange_order("sl-100").unwrap();
        assert_eq!(group_ref.status, OrderGroupStatus::StoppedOut);
    }

    #[test]
    fn test_order_group_exchange_sl_tp_ids() {
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let mut group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));

        // Initially None
        assert!(group.exchange_sl_order_id.is_none());
        assert!(group.exchange_tp_order_id.is_none());

        // Set and verify round-trip
        group.exchange_sl_order_id = Some("sl-123:BTC/USDT:USDT".to_string());
        group.exchange_tp_order_id = Some("tp-456:BTC/USDT:USDT".to_string());

        assert_eq!(
            group.exchange_sl_order_id.as_deref(),
            Some("sl-123:BTC/USDT:USDT")
        );
        assert_eq!(
            group.exchange_tp_order_id.as_deref(),
            Some("tp-456:BTC/USDT:USDT")
        );
    }

    // AUD-02: GC tests

    #[test]
    fn test_prune_terminal_removes_stopped_out_groups_from_all_maps() {
        let mut manager = OrderGroupManager::new();
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();
        let sl_id = Uuid::new_v4();
        let tp_id = Uuid::new_v4();

        let mut group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));
        group.stop_loss_order_id = Some(sl_id);
        group.take_profit_order_ids = vec![tp_id];
        group.exchange_order_id = Some("exch-entry".to_string());
        group.exchange_sl_order_id = Some("exch-sl".to_string());
        group.exchange_tp_order_id = Some("exch-tp".to_string());
        let added = manager.add_group(group);
        let group_id = added.id;

        // Register all indexes
        manager.register_linked_order(sl_id, group_id);
        manager.register_linked_order(tp_id, group_id);
        manager.register_exchange_order("exch-entry".to_string(), group_id);
        manager.register_exchange_order("exch-sl".to_string(), group_id);
        manager.register_exchange_order("exch-tp".to_string(), group_id);

        // Transition to terminal
        manager.get_group_mut(group_id).unwrap().on_stop_loss_filled();

        // Prune with future cutoff
        let cutoff = Instant::now() + std::time::Duration::from_secs(1);
        let pruned = manager.prune_terminal(cutoff);
        assert_eq!(pruned, 1);

        // All 5 maps should be clean
        assert!(manager.get_group(group_id).is_none());
        assert!(manager.get_user_groups(user_id).is_empty());
        assert!(manager.get_by_entry_order(entry_id).is_none());
        assert!(manager.get_by_linked_order(sl_id).is_none());
        assert!(manager.get_by_linked_order(tp_id).is_none());
        assert!(manager.get_by_exchange_order("exch-entry").is_none());
        assert!(manager.get_by_exchange_order("exch-sl").is_none());
        assert!(manager.get_by_exchange_order("exch-tp").is_none());
    }

    #[test]
    fn test_prune_terminal_keeps_active_groups() {
        let mut manager = OrderGroupManager::new();
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let mut group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));
        group.on_entry_filled(dec!(50000)); // Active, not terminal
        manager.add_group(group);

        // Even with future cutoff, active groups stay
        let cutoff = Instant::now() + std::time::Duration::from_secs(1);
        let pruned = manager.prune_terminal(cutoff);
        assert_eq!(pruned, 0);
        assert_eq!(manager.get_user_groups(user_id).len(), 1);
    }

    #[test]
    fn test_reindex_exchange_sl_order_swaps_index_and_updates_group() {
        let mut manager = OrderGroupManager::new();
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let mut group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));
        group.exchange_sl_order_id = Some("old-sl-id".to_string());
        let added = manager.add_group(group);
        let group_id = added.id;

        manager.register_exchange_order("old-sl-id".to_string(), group_id);

        // Reindex
        assert!(manager.reindex_exchange_sl_order("old-sl-id", "new-sl-id".to_string()));

        // Old ID gone, new ID points to same group
        assert!(manager.get_by_exchange_order("old-sl-id").is_none());
        let found = manager.get_by_exchange_order("new-sl-id").unwrap();
        assert_eq!(found.id, group_id);
        assert_eq!(found.exchange_sl_order_id.as_deref(), Some("new-sl-id"));
    }

    #[test]
    fn test_reindex_exchange_sl_order_returns_false_for_unknown_id() {
        let mut manager = OrderGroupManager::new();
        assert!(!manager.reindex_exchange_sl_order("nonexistent", "new-id".to_string()));
    }

    #[test]
    fn test_prune_terminal_keeps_recent_terminal_groups() {
        let mut manager = OrderGroupManager::new();
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_id, dec!(0.1));
        let added = manager.add_group(group);
        manager.get_group_mut(added.id).unwrap().cancel();

        // Cutoff in the past — group is too recent to prune
        let cutoff = Instant::now() - std::time::Duration::from_secs(3600);
        let pruned = manager.prune_terminal(cutoff);
        assert_eq!(pruned, 0);
        assert!(manager.get_group(added.id).is_some());
    }
}
