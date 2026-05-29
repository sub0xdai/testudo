//! 018: Order Reconciliation Service
//!
//! Polls the exchange every 30 seconds and compares open orders/positions
//! against local OrderGroup state. Detects and cleans up orphaned orders
//! that the WebSocket fill chain missed (disconnect, fan-in abort, overflow).
//!
//! Defense-in-depth: WebSocket handles fills in <1s, reconciliation catches
//! anything that slips through within 30s.

// @anchor exchange:router:reconciliation
// @tags api

use crate::repositories::exchange_account::{ExchangeAccountRepository, RepoError};
use crate::services::cex_client::{CexClient, SidecarOpenOrderResponse};
use crate::services::exchange_api::{ExchangeApi, ExchangeApiError};
use crate::services::trade_manager::repository::PositionRepository;
use crate::services::ManagementEvent;
use engine::shadow::actor::OrderRole;
use engine::shadow::order_group::{OrderGroup, OrderGroupStatus};
use engine::EngineHandle;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Snapshot of a divergent group to reconcile.
///
/// 020: Made `pub(crate)` so integration tests can inspect reconciliation decisions.
#[derive(Debug)]
pub(crate) struct ReconcileAction {
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub exchange_account_id: Option<Uuid>,
    /// Sibling order IDs to cancel on the exchange.
    pub orders_to_cancel: Vec<String>,
    /// New status to assign.
    pub new_status: OrderGroupStatus,
    /// Label for tracing/events.
    pub event_type: &'static str,
    /// 019e: For zombie recovery — exchange order ID to register.
    pub recovered_exchange_id: Option<String>,
}

// ---------------------------------------------------------------------------
// 020 FR-5: Pure decision function extracted from reconcile_account()
// ---------------------------------------------------------------------------

/// Determine reconciliation actions for a set of order groups.
///
/// This is a pure function with no I/O — all exchange state is passed in.
/// The caller is responsible for:
/// 1. Re-querying groups via EngineHandle for freshness
/// 2. Executing the returned actions (cancel orders, update status, etc.)
///
/// For `AwaitingReconciliation` groups, `recovered_exchange_id` is set when
/// the order was found on the exchange via `clientOrderId` matching.
pub(crate) fn determine_reconcile_actions(
    groups: &[OrderGroup],
    open_order_ids: &HashSet<String>,
    symbols_with_position: &HashSet<String>,
    open_orders: &[SidecarOpenOrderResponse],
) -> Vec<ReconcileAction> {
    let mut actions = Vec::new();

    for current in groups {
        if current.status.is_terminal() {
            continue;
        }

        let has_position = symbols_with_position.contains(&current.symbol);

        let sl_alive = current
            .exchange_sl_order_id
            .as_ref()
            .map_or(false, |id| open_order_ids.contains(id));
        let tp_alive = current
            .exchange_tp_order_id
            .as_ref()
            .map_or(false, |id| open_order_ids.contains(id));
        let entry_alive = current
            .exchange_order_id
            .as_ref()
            .map_or(false, |id| open_order_ids.contains(id));

        let action = match current.status {
            OrderGroupStatus::Active => {
                // Grace period: don't reconcile-close groups younger than 60s.
                // Protects against race conditions where SL/TP IDs haven't been
                // registered yet or the position hasn't settled on the exchange.
                let age = chrono::Utc::now() - current.created_at;
                if age < chrono::Duration::seconds(60) {
                    continue;
                }

                if has_position {
                    continue;
                }

                if !sl_alive && tp_alive {
                    let tp_id = current.exchange_tp_order_id.clone().unwrap();
                    Some(ReconcileAction {
                        group_id: current.id,
                        user_id: current.user_id,
                        symbol: current.symbol.clone(),
                        exchange_account_id: current.exchange_account_id,
                        orders_to_cancel: vec![tp_id],
                        new_status: OrderGroupStatus::StoppedOut,
                        event_type: "reconciled_stopped_out",
                        recovered_exchange_id: None,
                    })
                } else if !tp_alive && sl_alive {
                    let sl_id = current.exchange_sl_order_id.clone().unwrap();
                    Some(ReconcileAction {
                        group_id: current.id,
                        user_id: current.user_id,
                        symbol: current.symbol.clone(),
                        exchange_account_id: current.exchange_account_id,
                        orders_to_cancel: vec![sl_id],
                        new_status: OrderGroupStatus::TookProfit,
                        event_type: "reconciled_took_profit",
                        recovered_exchange_id: None,
                    })
                } else if !sl_alive && !tp_alive {
                    Some(ReconcileAction {
                        group_id: current.id,
                        user_id: current.user_id,
                        symbol: current.symbol.clone(),
                        exchange_account_id: current.exchange_account_id,
                        orders_to_cancel: vec![],
                        new_status: OrderGroupStatus::Closed,
                        event_type: "reconciled_closed",
                        recovered_exchange_id: None,
                    })
                } else {
                    continue;
                }
            }
            OrderGroupStatus::Pending => {
                // Grace period: don't reconcile-cancel groups younger than 60s.
                // Pending → Active transition requires the entry-fill WS event to
                // propagate through FillDetector → on_entry_filled. Between the
                // entry filling on the exchange (entry no longer in open_orders)
                // and the engine updating group status, there is a window where
                // entry_alive=false and status=Pending — which previously triggered
                // a destructive cancel of just-placed SL+TP. Mirrors Active grace.
                let age = chrono::Utc::now() - current.created_at;
                if age < chrono::Duration::seconds(60) {
                    continue;
                }

                // If a position now exists for this symbol, the entry filled;
                // wait for the WS path to promote the group to Active rather than
                // cancel it here.
                if has_position {
                    continue;
                }

                if !entry_alive {
                    let mut to_cancel = Vec::new();
                    if let Some(ref sl_id) = current.exchange_sl_order_id {
                        if open_order_ids.contains(sl_id) {
                            to_cancel.push(sl_id.clone());
                        }
                    }
                    if let Some(ref tp_id) = current.exchange_tp_order_id {
                        if open_order_ids.contains(tp_id) {
                            to_cancel.push(tp_id.clone());
                        }
                    }
                    Some(ReconcileAction {
                        group_id: current.id,
                        user_id: current.user_id,
                        symbol: current.symbol.clone(),
                        exchange_account_id: current.exchange_account_id,
                        orders_to_cancel: to_cancel,
                        new_status: OrderGroupStatus::Cancelled,
                        event_type: "reconciled_cancelled",
                        recovered_exchange_id: None,
                    })
                } else {
                    continue;
                }
            }
            // 019e FR-12: Zombie group — placement timed out before exchange
            // registration. Check if the order actually made it to the exchange
            // by matching numeric clientOrderId (WOO X requires numeric IDs).
            OrderGroupStatus::AwaitingReconciliation => {
                let expected_client_id = crate::services::numeric_client_order_id(current.id, 1);
                let found_order = open_orders.iter().find(|o| {
                    o.client_order_id.as_deref() == Some(expected_client_id.as_str())
                });

                if let Some(order) = found_order {
                    // Order exists on exchange — signal recovery.
                    Some(ReconcileAction {
                        group_id: current.id,
                        user_id: current.user_id,
                        symbol: current.symbol.clone(),
                        exchange_account_id: current.exchange_account_id,
                        orders_to_cancel: vec![],
                        new_status: OrderGroupStatus::Pending,
                        event_type: "reconciled_zombie_recovered",
                        recovered_exchange_id: Some(order.id.clone()),
                    })
                } else {
                    // Order never reached exchange — cancel the group.
                    Some(ReconcileAction {
                        group_id: current.id,
                        user_id: current.user_id,
                        symbol: current.symbol.clone(),
                        exchange_account_id: current.exchange_account_id,
                        orders_to_cancel: vec![],
                        new_status: OrderGroupStatus::Cancelled,
                        event_type: "reconciled_zombie_cancelled",
                        recovered_exchange_id: None,
                    })
                }
            }
            _ => continue,
        };

        if let Some(action) = action {
            actions.push(action);
        }
    }

    actions
}

// ---------------------------------------------------------------------------
// ReconciliationService
// ---------------------------------------------------------------------------

/// Polls the exchange and reconciles local OrderGroup state with exchange reality.
///
/// 019d: Uses EngineHandle instead of direct Arc<RwLock<OrderGroupManager>> access.
pub struct ReconciliationService {
    engine_handle: EngineHandle,
    cex_client: Arc<CexClient>,
    exchange_account_repo: ExchangeAccountRepository,
    exchange_api: Arc<dyn ExchangeApi>,
    position_repo: Option<PositionRepository>,
    event_tx: Option<mpsc::Sender<ManagementEvent>>,
    sandbox: bool,
}

impl ReconciliationService {
    pub fn new(
        engine_handle: EngineHandle,
        cex_client: Arc<CexClient>,
        exchange_account_repo: ExchangeAccountRepository,
        exchange_api: Arc<dyn ExchangeApi>,
        sandbox: bool,
    ) -> Self {
        Self {
            engine_handle,
            cex_client,
            exchange_account_repo,
            exchange_api,
            position_repo: None,
            event_tx: None,
            sandbox,
        }
    }

    pub fn with_position_repo(mut self, repo: PositionRepository) -> Self {
        self.position_repo = Some(repo);
        self
    }

    pub fn with_event_sender(mut self, tx: mpsc::Sender<ManagementEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Run one reconciliation sweep.
    pub async fn sweep(&self) {
        let started_at = Instant::now();

        // Phase 1: Query live groups via EngineHandle (no lock held).
        let live_groups = self.engine_handle.get_live_groups().await;

        if live_groups.is_empty() {
            return;
        }

        // Group by (user_id, exchange_account_id) to batch API calls.
        let mut by_account: HashMap<(Uuid, Uuid), Vec<OrderGroup>> = HashMap::new();
        for group in live_groups {
            if let Some(account_id) = group.exchange_account_id {
                by_account
                    .entry((group.user_id, account_id))
                    .or_default()
                    .push(group);
            }
        }

        let mut total_reconciled = 0usize;

        for ((user_id, account_id), groups) in &by_account {
            match self
                .reconcile_account(*user_id, *account_id, groups)
                .await
            {
                Ok(count) => total_reconciled += count,
                Err(e) => {
                    tracing::warn!(
                        user_id = %user_id,
                        account_id = %account_id,
                        "Reconciliation sweep failed for account: {}",
                        e
                    );
                }
            }
        }

        if total_reconciled > 0 {
            tracing::info!(
                reconciled = total_reconciled,
                latency_ms = started_at.elapsed().as_millis() as u64,
                "Reconciliation sweep completed"
            );
        }
    }

    /// Force-cancel all non-terminal groups for an orphaned/deactivated account.
    /// These groups can never reconcile because credentials are gone.
    async fn force_cancel_orphaned_groups(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        groups: &[OrderGroup],
    ) -> usize {
        let mut cancelled = 0usize;
        for group in groups {
            if group.status.is_terminal() {
                continue;
            }
            if let Err(e) = self
                .engine_handle
                .update_group_status(group.id, OrderGroupStatus::Cancelled)
                .await
            {
                tracing::error!(group_id = %group.id, "Failed to force-cancel orphaned group: {}", e);
                continue;
            }
            if let Some(ref repo) = self.position_repo {
                if let Err(e) = repo.mark_closed(group.id).await {
                    tracing::error!(group_id = %group.id, "Failed to persist orphan cancellation: {}", e);
                }
            }
            tracing::warn!(
                group_id = %group.id,
                user_id = %user_id,
                account_id = %account_id,
                symbol = %group.symbol,
                "Reconciliation: force-cancelled orphaned group (account deactivated)"
            );
            cancelled += 1;
        }
        cancelled
    }

    /// Minimal reconciliation for Hyperliquid groups.
    /// Without sidecar, we can only cancel groups that clearly failed:
    /// - AwaitingReconciliation with no exchange_order_id (placement never confirmed)
    /// - Pending with no exchange_order_id (entry never reached exchange)
    async fn reconcile_hyperliquid_minimal(
        &self,
        user_id: Uuid,
        groups: &[OrderGroup],
    ) -> usize {
        let mut cancelled = 0usize;
        for group in groups {
            let should_cancel = match group.status {
                OrderGroupStatus::AwaitingReconciliation => group.exchange_order_id.is_none(),
                OrderGroupStatus::Pending => group.exchange_order_id.is_none(),
                _ => false,
            };
            if !should_cancel {
                continue;
            }
            if let Err(e) = self
                .engine_handle
                .update_group_status(group.id, OrderGroupStatus::Cancelled)
                .await
            {
                tracing::error!(group_id = %group.id, "Failed to cancel HL orphaned group: {}", e);
                continue;
            }
            if let Some(ref repo) = self.position_repo {
                if let Err(e) = repo.mark_closed(group.id).await {
                    tracing::error!(group_id = %group.id, "Failed to persist HL orphan cancellation: {}", e);
                }
            }
            if let Some(ref tx) = self.event_tx {
                let _ = tx.try_send(ManagementEvent {
                    user_id,
                    event_type: "reconciled_hl_orphan_cancelled".to_string(),
                    symbol: group.symbol.clone(),
                    detail: format!("group_id={}", group.id),
                });
            }
            tracing::warn!(
                group_id = %group.id,
                symbol = %group.symbol,
                status = ?group.status,
                "Reconciliation: cancelled Hyperliquid orphaned group (no exchange order ID)"
            );
            cancelled += 1;
        }
        cancelled
    }

    /// Reconcile all groups for a single exchange account.
    ///
    /// 020 FR-6: Delegates decision logic to `determine_reconcile_actions()`.
    async fn reconcile_account(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        groups: &[OrderGroup],
    ) -> Result<usize, String> {
        // Load credentials — handle deactivated accounts
        let creds = match self
            .exchange_account_repo
            .load_credentials(account_id, user_id)
            .await
        {
            Ok(c) => c,
            Err(RepoError::NotFound) => {
                // Account deactivated — force-cancel all orphaned groups
                let count = self
                    .force_cancel_orphaned_groups(user_id, account_id, groups)
                    .await;
                return Ok(count);
            }
            Err(e) => return Err(format!("credential load: {}", e)),
        };

        // Hyperliquid: minimal reconciliation (no CEX sidecar)
        if creds.exchange_name.eq_ignore_ascii_case(crate::types::exchange_names::exchanges::HYPERLIQUID) {
            let count = self.reconcile_hyperliquid_minimal(user_id, groups).await;
            return Ok(count);
        }

        let sidecar_creds = crate::services::SidecarCredentials {
            api_key: creds.api_key,
            secret: creds.api_secret,
            password: creds.passphrase,
        };

        // Fetch all open orders for this account (empty symbol = all)
        let open_orders = self
            .cex_client
            .fetch_open_orders(&creds.exchange_name, &sidecar_creds, self.sandbox, "")
            .await
            .map_err(|e| format!("fetch_open_orders: {}", e))?;

        let open_order_ids: HashSet<String> = open_orders.iter().map(|o| o.id.clone()).collect();

        // Fetch all positions for this account
        let positions = self
            .cex_client
            .fetch_positions(&creds.exchange_name, &sidecar_creds, self.sandbox, None)
            .await
            .map_err(|e| format!("fetch_positions: {}", e))?;

        // Build set of symbols with open positions (non-zero contracts)
        let symbols_with_position: HashSet<String> = positions
            .iter()
            .filter(|p| {
                p.contracts
                    .parse::<f64>()
                    .map(|c| c.abs() > 0.0)
                    .unwrap_or(false)
            })
            .map(|p| p.symbol.clone())
            .collect();

        // Phase 2: Re-query each group via handle for freshness, then determine actions.
        let mut fresh_groups = Vec::new();
        for group in groups {
            if let Some(current) = self.engine_handle.get_trade_group(group.id).await {
                fresh_groups.push(current);
            }
        }

        let actions = determine_reconcile_actions(
            &fresh_groups,
            &open_order_ids,
            &symbols_with_position,
            &open_orders,
        );

        // Phase 3: Execute actions — update status, cancel orders, persist, broadcast.
        let count = actions.len();
        for action in actions {
            // 019e: Handle zombie recovery — register exchange order ID
            if let Some(ref exchange_id) = action.recovered_exchange_id {
                if let Err(e) = self
                    .engine_handle
                    .register_exchange_order_id(
                        action.group_id,
                        OrderRole::Entry,
                        exchange_id.clone(),
                    )
                    .await
                {
                    tracing::error!(
                        group_id = %action.group_id,
                        "019e: failed to register recovered exchange ID: {}",
                        e
                    );
                } else {
                    tracing::info!(
                        group_id = %action.group_id,
                        exchange_id = %exchange_id,
                        "019e: recovered zombie group — exchange order found, re-registered"
                    );
                }
            }

            // Update group status via EngineHandle
            if let Err(e) = self
                .engine_handle
                .update_group_status(action.group_id, action.new_status)
                .await
            {
                tracing::error!(
                    "Reconciliation: failed to update group {} status: {}",
                    action.group_id,
                    e
                );
                continue;
            }

            tracing::warn!(
                group_id = %action.group_id,
                symbol = %action.symbol,
                event = action.event_type,
                orders_to_cancel = ?action.orders_to_cancel,
                "Reconciliation: detected divergence"
            );

            // Cancel orphaned orders
            for order_id in &action.orders_to_cancel {
                match self
                    .exchange_api
                    .cancel_order(action.user_id, order_id, &action.symbol, action.exchange_account_id)
                    .await
                {
                    Ok(()) => {
                        tracing::info!(
                            group_id = %action.group_id,
                            order_id = %order_id,
                            "Reconciliation: cancelled orphaned order"
                        );
                    }
                    Err(ExchangeApiError::OrderNotFound(_)) => {
                        tracing::debug!(
                            group_id = %action.group_id,
                            order_id = %order_id,
                            "Reconciliation: order already gone (OrderNotFound)"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            group_id = %action.group_id,
                            order_id = %order_id,
                            "Reconciliation: failed to cancel order: {}",
                            e
                        );
                    }
                }
            }

            // Persist to DB
            if let Some(ref repo) = self.position_repo {
                if let Err(e) = repo.mark_closed(action.group_id).await {
                    tracing::error!(
                        group_id = %action.group_id,
                        "Reconciliation: failed to persist closed state: {}",
                        e
                    );
                }
            }

            // Broadcast management event (WebSocket push to extension)
            if let Some(ref tx) = self.event_tx {
                if let Err(e) = tx.try_send(ManagementEvent {
                    user_id: action.user_id,
                    event_type: action.event_type.to_string(),
                    symbol: action.symbol.clone(),
                    detail: format!("group_id={}", action.group_id),
                }) {
                    tracing::warn!(
                        "Reconciliation: management event channel full: {}",
                        e
                    );
                }
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::shadow::order_group::{OrderGroup, TakeProfitTarget};
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn make_active_long_group() -> OrderGroup {
        let mut g = OrderGroup::new(
            Uuid::new_v4(),
            "BTC_USDT".to_string(),
            Uuid::new_v4(),
            dec!(0.1),
        );
        g.status = OrderGroupStatus::Active;
        g.entry_price = Some(dec!(50000));
        g.stop_loss_price = Some(dec!(49000)); // LONG: SL < entry
        g.take_profit_targets = vec![TakeProfitTarget {
            price: dec!(52000),
            percent_to_close: dec!(100),
            order_id: None,
            filled: false,
        }];
        // Force age > 60s so grace period doesn't apply.
        g.created_at = chrono::Utc::now() - chrono::Duration::seconds(120);
        g
    }

    fn make_active_short_group() -> OrderGroup {
        let mut g = OrderGroup::new(
            Uuid::new_v4(),
            "ETH_USDT".to_string(),
            Uuid::new_v4(),
            dec!(1.0),
        );
        g.status = OrderGroupStatus::Active;
        g.entry_price = Some(dec!(3000));
        g.stop_loss_price = Some(dec!(3200)); // SHORT: SL > entry
        g.take_profit_targets = vec![TakeProfitTarget {
            price: dec!(2800),
            percent_to_close: dec!(100),
            order_id: None,
            filled: false,
        }];
        g.created_at = chrono::Utc::now() - chrono::Duration::seconds(120);
        g
    }

    #[test]
    fn test_stopped_out_long_seeded_price_and_side() {
        let mut group = make_active_long_group();
        group.exchange_tp_order_id = Some("tp-1".to_string());
        group.exchange_sl_order_id = Some("sl-1".to_string());

        // TP alive, SL gone → StoppedOut
        let open_ids: HashSet<String> = ["tp-1".to_string()].into_iter().collect();
        let actions = determine_reconcile_actions(&[group.clone()], &open_ids, &HashSet::new(), &[]);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].new_status, OrderGroupStatus::StoppedOut);
    }

    #[test]
    fn test_took_profit_long_seeded_price_and_side() {
        let mut group = make_active_long_group();
        group.exchange_tp_order_id = Some("tp-2".to_string());
        group.exchange_sl_order_id = Some("sl-2".to_string());

        // SL alive, TP gone → TookProfit
        let open_ids: HashSet<String> = ["sl-2".to_string()].into_iter().collect();
        let actions = determine_reconcile_actions(&[group.clone()], &open_ids, &HashSet::new(), &[]);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].new_status, OrderGroupStatus::TookProfit);
    }

    #[test]
    fn test_closed_both_brackets_gone_seeded_zero() {
        let mut group = make_active_long_group();
        group.exchange_tp_order_id = Some("tp-3".to_string());
        group.exchange_sl_order_id = Some("sl-3".to_string());

        // Neither SL nor TP alive → Closed
        let actions =
            determine_reconcile_actions(&[group.clone()], &HashSet::new(), &HashSet::new(), &[]);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].new_status, OrderGroupStatus::Closed);
    }

    #[test]
    fn test_short_position_close_side_is_buy() {
        let mut group = make_active_short_group();
        group.exchange_tp_order_id = Some("tp-4".to_string());
        group.exchange_sl_order_id = Some("sl-4".to_string());

        // TP alive, SL gone → StoppedOut (short stopped = bought back at SL)
        let open_ids: HashSet<String> = ["tp-4".to_string()].into_iter().collect();
        let actions = determine_reconcile_actions(&[group.clone()], &open_ids, &HashSet::new(), &[]);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].new_status, OrderGroupStatus::StoppedOut);
    }

    #[test]
    fn test_pending_cancelled_no_trade_closed_emit() {
        let mut group = make_active_long_group();
        group.status = OrderGroupStatus::Pending;
        group.exchange_order_id = Some("entry-5".to_string());

        // Entry not alive → Cancelled
        let actions =
            determine_reconcile_actions(&[group], &HashSet::new(), &HashSet::new(), &[]);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].new_status, OrderGroupStatus::Cancelled);
    }

    #[test]
    fn test_grace_period_skips_young_active_group() {
        let mut group = make_active_long_group();
        // Override created_at to within the 60s grace period
        group.created_at = chrono::Utc::now() - chrono::Duration::seconds(30);
        group.exchange_sl_order_id = Some("sl-6".to_string());
        group.exchange_tp_order_id = Some("tp-6".to_string());

        // Neither order alive — would be Closed if not for grace period
        let actions =
            determine_reconcile_actions(&[group], &HashSet::new(), &HashSet::new(), &[]);

        assert!(actions.is_empty(), "Grace period must suppress young groups");
    }
}
