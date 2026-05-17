//! Transaction Context for Atomic Cascade Operations
//!
//! Implements atomic transaction semantics for cascade operations (Entry + SL + TP).
//! When creating linked orders, all operations either succeed together or fail together.
//!
//! # Problem (from spec 005-atomic-cascades)
//!
//! When creating linked orders, `orders.add_order()` may succeed but
//! `order_groups.register_linked_order()` may fail, leaving orphan orders
//! without their protective stops.
//!
//! # Solution
//!
//! The TransactionContext accumulates pending changes without applying them.
//! On `commit()`, all changes are validated and applied atomically.
//! If any operation fails, no changes are applied (rollback).
//!
//! # Usage
//!
//! ```ignore
//! let mut tx = TransactionContext::new();
//! tx.add_order(entry);
//! tx.add_order(sl);
//! tx.add_order(tp);
//! tx.register_group(group);
//! tx.commit(&mut orders, &mut groups)?; // All or nothing
//! ```

use rust_decimal::Decimal;
use uuid::Uuid;

use super::order_group::{OrderGroup, OrderGroupManager};
use super::orders::{ShadowOrder, ShadowOrderManager};
use super::ShadowEngineError;

/// Pending order registration with its user
#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub user_id: Uuid,
    pub order: ShadowOrder,
}

/// Pending group registration
#[derive(Debug, Clone)]
pub struct PendingGroup {
    pub group: OrderGroup,
}

/// Pending linked order registration (order_id -> group_id)
#[derive(Debug, Clone)]
pub struct PendingLinkedOrder {
    pub order_id: Uuid,
    pub group_id: Uuid,
}

/// Transaction context for atomic cascade operations.
///
/// Accumulates pending changes and applies them atomically on commit.
/// If any validation fails, no changes are applied.
#[derive(Debug, Default)]
pub struct TransactionContext {
    /// Orders to be added
    pending_orders: Vec<PendingOrder>,
    /// Order groups to be registered
    pending_groups: Vec<PendingGroup>,
    /// Linked orders to be registered (for cascade lookup)
    pending_linked_orders: Vec<PendingLinkedOrder>,
}

/// Errors that can occur during transaction commit
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("Duplicate order ID: {0}")]
    DuplicateOrderId(Uuid),

    #[error("Duplicate group ID: {0}")]
    DuplicateGroupId(Uuid),

    #[error("Order group references non-existent entry order: {order_id}")]
    OrphanGroup { order_id: Uuid },

    #[error("Linked order references non-existent group: {group_id}")]
    OrphanLinkedOrder { group_id: Uuid },

    #[error("Linked order references non-existent order: {order_id}")]
    LinkedOrderNotFound { order_id: Uuid },

    #[error("Insufficient balance: need {required} {asset}, have {available}")]
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
        asset: String,
    },

    #[error("Engine error: {0}")]
    EngineError(#[from] ShadowEngineError),
}

impl TransactionContext {
    /// Create a new empty transaction context
    pub fn new() -> Self {
        Self {
            pending_orders: Vec::new(),
            pending_groups: Vec::new(),
            pending_linked_orders: Vec::new(),
        }
    }

    /// Add an order to the pending list
    ///
    /// The order will be added to the manager on commit.
    pub fn add_order(&mut self, user_id: Uuid, order: ShadowOrder) {
        self.pending_orders.push(PendingOrder { user_id, order });
    }

    /// Add an order group to the pending list
    ///
    /// The group will be registered on commit.
    pub fn add_group(&mut self, group: OrderGroup) {
        self.pending_groups.push(PendingGroup { group });
    }

    /// Register a linked order (SL or TP) for cascade lookup
    ///
    /// The registration will be applied on commit.
    pub fn register_linked_order(&mut self, order_id: Uuid, group_id: Uuid) {
        self.pending_linked_orders
            .push(PendingLinkedOrder { order_id, group_id });
    }

    /// Check if the transaction has any pending changes
    pub fn is_empty(&self) -> bool {
        self.pending_orders.is_empty()
            && self.pending_groups.is_empty()
            && self.pending_linked_orders.is_empty()
    }

    /// Get the number of pending orders
    pub fn pending_order_count(&self) -> usize {
        self.pending_orders.len()
    }

    /// Get the number of pending groups
    pub fn pending_group_count(&self) -> usize {
        self.pending_groups.len()
    }

    /// Validate all pending operations can succeed
    ///
    /// Returns an error if any validation fails.
    fn validate(
        &self,
        orders: &ShadowOrderManager,
        groups: &OrderGroupManager,
    ) -> Result<(), TransactionError> {
        // Collect all order IDs (existing + pending)
        let mut order_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

        // Check for duplicate order IDs in pending
        for pending in &self.pending_orders {
            if order_ids.contains(&pending.order.id) {
                return Err(TransactionError::DuplicateOrderId(pending.order.id));
            }
            // Check if order already exists in manager
            if orders.get_order(pending.order.id).is_some() {
                return Err(TransactionError::DuplicateOrderId(pending.order.id));
            }
            order_ids.insert(pending.order.id);
        }

        // Collect all group IDs (existing + pending)
        let mut group_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

        // Check for duplicate group IDs in pending
        for pending in &self.pending_groups {
            if group_ids.contains(&pending.group.id) {
                return Err(TransactionError::DuplicateGroupId(pending.group.id));
            }
            // Check if group already exists in manager
            if groups.get_group(pending.group.id).is_some() {
                return Err(TransactionError::DuplicateGroupId(pending.group.id));
            }
            group_ids.insert(pending.group.id);

            // Check that the entry order exists (either in pending or existing)
            let entry_exists = order_ids.contains(&pending.group.entry_order_id)
                || orders.get_order(pending.group.entry_order_id).is_some();
            if !entry_exists {
                return Err(TransactionError::OrphanGroup {
                    order_id: pending.group.entry_order_id,
                });
            }
        }

        // Validate linked order registrations
        for pending in &self.pending_linked_orders {
            // Check that the order exists (either in pending or existing)
            let order_exists = order_ids.contains(&pending.order_id)
                || orders.get_order(pending.order_id).is_some();
            if !order_exists {
                return Err(TransactionError::LinkedOrderNotFound {
                    order_id: pending.order_id,
                });
            }

            // Check that the group exists (either in pending or existing)
            let group_exists = group_ids.contains(&pending.group_id)
                || groups.get_group(pending.group_id).is_some();
            if !group_exists {
                return Err(TransactionError::OrphanLinkedOrder {
                    group_id: pending.group_id,
                });
            }
        }

        Ok(())
    }

    /// Commit all pending changes atomically
    ///
    /// If any operation fails validation, no changes are applied.
    /// On success, all orders, groups, and linked order registrations are applied.
    ///
    /// # Arguments
    ///
    /// * `orders` - The order manager to add orders to
    /// * `groups` - The group manager to add groups and registrations to
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<ShadowOrder>)` - The orders that were added (for caller to track)
    /// * `Err(TransactionError)` - If validation or commit fails
    pub fn commit(
        self,
        orders: &mut ShadowOrderManager,
        groups: &mut OrderGroupManager,
    ) -> Result<Vec<ShadowOrder>, TransactionError> {
        // Phase 1: Validate all operations
        self.validate(orders, groups)?;

        // Phase 2: Apply all changes (no failures possible after validation)
        let mut added_orders = Vec::with_capacity(self.pending_orders.len());

        // Add all orders
        for pending in self.pending_orders {
            let added = orders.add_order(pending.user_id, pending.order);
            added_orders.push(added);
        }

        // Add all groups
        for pending in self.pending_groups {
            groups.add_group(pending.group);
        }

        // Register all linked orders
        for pending in self.pending_linked_orders {
            groups.register_linked_order(pending.order_id, pending.group_id);
        }

        Ok(added_orders)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shadow::orders::{ShadowOrderSide, ShadowOrderType};
    use rust_decimal_macros::dec;

    fn create_test_order(user_id: Uuid) -> ShadowOrder {
        let mut order = ShadowOrder::new(
            user_id,
            "BTC_USDT".to_string(),
            ShadowOrderSide::Buy,
            ShadowOrderType::Limit,
            dec!(0.1),
            Some(dec!(50000)),
            None,
            None,
        );
        order.mark_risk_validated();
        order
    }

    #[test]
    fn test_transaction_context_creation() {
        let tx = TransactionContext::new();
        assert!(tx.is_empty());
        assert_eq!(tx.pending_order_count(), 0);
        assert_eq!(tx.pending_group_count(), 0);
    }

    #[test]
    fn test_add_order_to_transaction() {
        let mut tx = TransactionContext::new();
        let user_id = Uuid::new_v4();
        let order = create_test_order(user_id);

        tx.add_order(user_id, order);

        assert!(!tx.is_empty());
        assert_eq!(tx.pending_order_count(), 1);
    }

    #[test]
    fn test_add_group_to_transaction() {
        let mut tx = TransactionContext::new();
        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        let group = OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_id, dec!(0.1));
        tx.add_group(group);

        assert!(!tx.is_empty());
        assert_eq!(tx.pending_group_count(), 1);
    }

    #[test]
    fn test_commit_single_order_success() {
        let mut orders = ShadowOrderManager::new();
        let mut groups = OrderGroupManager::new();

        let mut tx = TransactionContext::new();
        let user_id = Uuid::new_v4();
        let order = create_test_order(user_id);
        let order_id = order.id;

        tx.add_order(user_id, order);

        let result = tx.commit(&mut orders, &mut groups);
        assert!(result.is_ok());

        // Order should exist in manager
        assert!(orders.get_order(order_id).is_some());
    }

    #[test]
    fn test_commit_cascade_success() {
        let mut orders = ShadowOrderManager::new();
        let mut groups = OrderGroupManager::new();

        let mut tx = TransactionContext::new();
        let user_id = Uuid::new_v4();

        // Create entry order
        let entry = create_test_order(user_id);
        let entry_id = entry.id;

        // Create SL order
        let mut sl = ShadowOrder::new(
            user_id,
            "BTC_USDT".to_string(),
            ShadowOrderSide::Sell,
            ShadowOrderType::StopLoss,
            dec!(0.1),
            None,
            Some(dec!(49000)),
            Some(entry_id),
        );
        sl.mark_risk_validated();
        let sl_id = sl.id;

        // Create TP order
        let mut tp = ShadowOrder::new(
            user_id,
            "BTC_USDT".to_string(),
            ShadowOrderSide::Sell,
            ShadowOrderType::TakeProfit,
            dec!(0.1),
            None,
            Some(dec!(52000)),
            Some(entry_id),
        );
        tp.mark_risk_validated();
        let tp_id = tp.id;

        // Create order group
        let mut group = OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_id, dec!(0.1));
        group = group.with_stop_loss(dec!(49000));
        group = group.with_take_profit(dec!(52000), dec!(100));
        let group_id = group.id;

        // Add all to transaction
        tx.add_order(user_id, entry);
        tx.add_order(user_id, sl);
        tx.add_order(user_id, tp);
        tx.add_group(group);
        tx.register_linked_order(sl_id, group_id);
        tx.register_linked_order(tp_id, group_id);

        // Commit
        let result = tx.commit(&mut orders, &mut groups);
        assert!(result.is_ok());

        // All orders should exist
        assert!(orders.get_order(entry_id).is_some());
        assert!(orders.get_order(sl_id).is_some());
        assert!(orders.get_order(tp_id).is_some());

        // Group should exist and be findable
        assert!(groups.get_group(group_id).is_some());
        assert!(groups.get_by_entry_order(entry_id).is_some());

        // Linked orders should be registered
        assert!(groups.get_by_linked_order(sl_id).is_some());
        assert!(groups.get_by_linked_order(tp_id).is_some());
    }

    #[test]
    fn test_commit_fails_duplicate_order_id() {
        let mut orders = ShadowOrderManager::new();
        let mut groups = OrderGroupManager::new();

        let user_id = Uuid::new_v4();
        let order = create_test_order(user_id);
        let order_id = order.id;

        // Add order directly to manager
        orders.add_order(user_id, order.clone());

        // Try to add same order via transaction
        let mut tx = TransactionContext::new();
        tx.add_order(user_id, order);

        let result = tx.commit(&mut orders, &mut groups);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TransactionError::DuplicateOrderId(id) if id == order_id
        ));
    }

    #[test]
    fn test_commit_fails_orphan_group() {
        let mut orders = ShadowOrderManager::new();
        let mut groups = OrderGroupManager::new();

        let user_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4(); // This order doesn't exist!

        // Create group referencing non-existent entry
        let group = OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_id, dec!(0.1));

        let mut tx = TransactionContext::new();
        tx.add_group(group);

        let result = tx.commit(&mut orders, &mut groups);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TransactionError::OrphanGroup { order_id } if order_id == entry_id
        ));
    }

    #[test]
    fn test_commit_fails_orphan_linked_order() {
        let mut orders = ShadowOrderManager::new();
        let mut groups = OrderGroupManager::new();

        let user_id = Uuid::new_v4();
        let order = create_test_order(user_id);
        let order_id = order.id;

        let group_id = Uuid::new_v4(); // This group doesn't exist!

        let mut tx = TransactionContext::new();
        tx.add_order(user_id, order);
        tx.register_linked_order(order_id, group_id);

        let result = tx.commit(&mut orders, &mut groups);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TransactionError::OrphanLinkedOrder { group_id: gid } if gid == group_id
        ));
    }

    #[test]
    fn test_rollback_on_failure_no_partial_state() {
        let mut orders = ShadowOrderManager::new();
        let mut groups = OrderGroupManager::new();

        let user_id = Uuid::new_v4();

        // Create a valid entry order
        let entry = create_test_order(user_id);
        let entry_id = entry.id;

        // Create a valid SL order
        let mut sl = ShadowOrder::new(
            user_id,
            "BTC_USDT".to_string(),
            ShadowOrderSide::Sell,
            ShadowOrderType::StopLoss,
            dec!(0.1),
            None,
            Some(dec!(49000)),
            Some(entry_id),
        );
        sl.mark_risk_validated();
        let sl_id = sl.id;

        // Create group
        let group = OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_id, dec!(0.1));
        let group_id = group.id;

        // Create transaction with a failing linked order registration
        let mut tx = TransactionContext::new();
        tx.add_order(user_id, entry);
        tx.add_order(user_id, sl);
        tx.add_group(group);
        // This will fail because it references a non-existent group
        let bad_group_id = Uuid::new_v4();
        tx.register_linked_order(sl_id, bad_group_id);

        // Commit should fail
        let result = tx.commit(&mut orders, &mut groups);
        assert!(result.is_err());

        // NO orders should exist (rollback)
        assert!(orders.get_order(entry_id).is_none());
        assert!(orders.get_order(sl_id).is_none());

        // NO groups should exist (rollback)
        assert!(groups.get_group(group_id).is_none());
    }

    #[test]
    fn test_empty_transaction_commits_successfully() {
        let mut orders = ShadowOrderManager::new();
        let mut groups = OrderGroupManager::new();

        let tx = TransactionContext::new();
        let result = tx.commit(&mut orders, &mut groups);

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_multiple_orders_same_transaction() {
        let mut orders = ShadowOrderManager::new();
        let mut groups = OrderGroupManager::new();

        let mut tx = TransactionContext::new();
        let user_id = Uuid::new_v4();

        // Add multiple orders
        for _ in 0..5 {
            let order = create_test_order(user_id);
            tx.add_order(user_id, order);
        }

        assert_eq!(tx.pending_order_count(), 5);

        let result = tx.commit(&mut orders, &mut groups);
        assert!(result.is_ok());

        let added = result.unwrap();
        assert_eq!(added.len(), 5);

        // All should be in manager
        for order in added {
            assert!(orders.get_order(order.id).is_some());
        }
    }
}
