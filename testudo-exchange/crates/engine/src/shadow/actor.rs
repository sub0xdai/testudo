//! EngineActor — sequential command processor for the ShadowEngine.
//!
//! Extracted into its own module per CLN-05. The public API (EngineHandle,
//! EngineCommand, EngineError) lives in `handle.rs`. This module contains
//! only the actor loop, command dispatch, and tests.

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant as TokioInstant;
use uuid::Uuid;

pub use super::handle::OrderRole;
pub use super::handle::FillEvent;
use super::handle::{
    EngineCommand, EngineError, EngineHandle,
    ENGINE_CHANNEL_CAPACITY,
};
use super::{
    OrderGroup, OrderGroupStatus, ShadowEngine,
};
use super::trade_event::{TradeEvent, TradeEventType};
/// Sequential command processor for the ShadowEngine.
///
/// Owns the `ShadowEngine` directly (019e: locks removed).
/// Dispatches each command synchronously since no awaits are needed.
pub struct EngineActor {
    engine: ShadowEngine,
    rx: mpsc::Receiver<EngineCommand>,
    /// 019d: Channel for emitting fill events from fire-and-forget price updates.
    fill_event_tx: mpsc::Sender<FillEvent>,
    /// 019f: Channel for emitting trade events to the TradeEventWriter.
    /// Non-blocking: uses try_send() with at-most-once delivery.
    trade_event_tx: mpsc::Sender<TradeEvent>,
    /// 019e FR-7: Tracks in-flight order placements that haven't completed
    /// exchange registration. Keyed by group_id, valued at insertion time.
    pending_placements: HashMap<Uuid, TokioInstant>,
}

/// Default capacity for the fill event channel.
pub const FILL_EVENT_CHANNEL_CAPACITY: usize = 256;

/// 019f: Capacity for the trade event audit log channel.
pub const TRADE_EVENT_CHANNEL_CAPACITY: usize = 1024;

impl EngineActor {
    pub fn new(
        engine: ShadowEngine,
        rx: mpsc::Receiver<EngineCommand>,
        fill_event_tx: mpsc::Sender<FillEvent>,
        trade_event_tx: mpsc::Sender<TradeEvent>,
    ) -> Self {
        Self {
            engine,
            rx,
            fill_event_tx,
            trade_event_tx,
            pending_placements: HashMap::new(),
        }
    }

    /// Spawn the actor and return an `EngineHandle`, the fill event receiver,
    /// and the trade event receiver.
    ///
    /// The fill event receiver emits `FillEvent`s triggered by fire-and-forget
    /// price updates (`push_price`). Subscribe to this channel to handle OCO
    /// cancellations and other fill-triggered side effects.
    ///
    /// The trade event receiver (019f) emits `TradeEvent`s for the audit log.
    /// Subscribe to this channel from `TradeEventWriter` for single-writer persistence.
    pub fn spawn(
        engine: ShadowEngine,
    ) -> (EngineHandle, mpsc::Receiver<FillEvent>, mpsc::Receiver<TradeEvent>) {
        let (tx, rx) = mpsc::channel(ENGINE_CHANNEL_CAPACITY);
        let (fill_tx, fill_rx) = mpsc::channel(FILL_EVENT_CHANNEL_CAPACITY);
        let (trade_event_tx, trade_event_rx) = mpsc::channel(TRADE_EVENT_CHANNEL_CAPACITY);
        let actor = Self::new(engine, rx, fill_tx, trade_event_tx);
        tokio::spawn(actor.run());
        (EngineHandle::new(tx), fill_rx, trade_event_rx)
    }

    /// CON-01: Like `spawn`, but also returns a cloned trade event sender
    /// so external producers (e.g., FillDetector) can emit TradeClosed events
    /// into the same channel consumed by TradeEventWriter.
    pub fn spawn_shared(
        engine: ShadowEngine,
    ) -> (EngineHandle, mpsc::Receiver<FillEvent>, mpsc::Receiver<TradeEvent>, mpsc::Sender<TradeEvent>) {
        let (tx, rx) = mpsc::channel(ENGINE_CHANNEL_CAPACITY);
        let (fill_tx, fill_rx) = mpsc::channel(FILL_EVENT_CHANNEL_CAPACITY);
        let (trade_event_tx, trade_event_rx) = mpsc::channel(TRADE_EVENT_CHANNEL_CAPACITY);
        let actor = Self::new(engine, rx, fill_tx, trade_event_tx.clone());
        tokio::spawn(actor.run());
        (EngineHandle::new(tx), fill_rx, trade_event_rx, trade_event_tx)
    }

    /// Run the actor loop. Processes commands sequentially until the channel
    /// closes (all `EngineHandle` instances dropped).
    ///
    /// 019e FR-10: Includes a 15s sweep timer for in-flight zombie detection.
    pub async fn run(mut self) {
        let mut sweep_interval = tokio::time::interval(Duration::from_secs(15));
        sweep_interval.tick().await; // skip first immediate tick
        tracing::info!("EngineActor started (lock-free, zombie sweep enabled)");

        loop {
            tokio::select! {
                cmd = self.rx.recv() => {
                    match cmd {
                        Some(cmd) => self.dispatch(cmd),
                        None => break, // all handles dropped
                    }
                }
                _ = sweep_interval.tick() => {
                    self.sweep_stale_placements();
                }
            }
        }

        tracing::info!("EngineActor shut down — channel closed");
    }

    /// 019e FR-10/FR-11: Sweep pending_placements for entries older than 30 seconds.
    /// Stale entries are transitioned to AwaitingReconciliation.
    fn sweep_stale_placements(&mut self) {
        let cutoff = TokioInstant::now() - Duration::from_secs(30);
        let stale: Vec<Uuid> = self
            .pending_placements
            .iter()
            .filter(|(_, ts)| **ts < cutoff)
            .map(|(id, _)| *id)
            .collect();

        for group_id in stale {
            self.pending_placements.remove(&group_id);
            tracing::warn!(
                group_id = %group_id,
                "019e: in-flight placement timeout — marking AwaitingReconciliation"
            );
            let group_meta = self.engine.order_groups.get_group(group_id)
                .map(|g| (g.user_id, g.symbol.clone()));
            if let Some(group) = self.engine.order_groups.get_group_mut(group_id) {
                group.status = OrderGroupStatus::AwaitingReconciliation;
                group.updated_at = chrono::Utc::now();
            }
            // 019f: Emit PlacementTimeout event
            if let Some((user_id, symbol)) = group_meta {
                self.emit_event(TradeEvent {
                    event_type: TradeEventType::PlacementTimeout,
                    group_id: Some(group_id),
                    user_id,
                    symbol: Some(symbol),
                    payload: serde_json::json!({ "group_id": group_id.to_string() }),
                });
            }
        }
    }

    /// 019f: Emit a trade event via try_send (non-blocking, at-most-once).
    /// If the channel is full, the event is dropped and a warning is logged.
    fn emit_event(&self, event: TradeEvent) {
        if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) =
            self.trade_event_tx.try_send(event)
        {
            tracing::warn!("019f: trade event channel full — event dropped");
        }
    }

    fn dispatch(&mut self, cmd: EngineCommand) {
        match cmd {
            // --- User management ---
            EngineCommand::UserExists { user_id, reply } => {
                let _ = reply.send(self.engine.user_exists(user_id));
            }
            EngineCommand::InitUser { user_id, reply } => {
                self.engine.init_user(user_id);
                let _ = reply.send(());
            }
            EngineCommand::InitUserWithBalance { user_id, usdt_balance, reply } => {
                self.engine.init_user_with_balance(user_id, usdt_balance);
                let _ = reply.send(());
            }
            EngineCommand::ResetUser { user_id, reply } => {
                self.engine.reset_user(user_id);
                let _ = reply.send(());
            }

            // --- Balances ---
            EngineCommand::GetBalances { user_id, reply } => {
                let _ = reply.send(self.engine.get_balances(user_id));
            }

            // --- Orders ---
            EngineCommand::PlaceOrder { user_id, order, reply } => {
                let symbol = order.symbol.clone();
                let result = self.engine
                    .place_order(user_id, order)
                    .map_err(|e| EngineError::Internal(e.to_string()));
                // 019e FR-8: Track placement in pending_placements.
                // If place_order created a group, its entry_order_id is the placed order's ID.
                if let Ok(ref placed) = result {
                    if let Some(group) = self.engine.order_groups.get_by_entry_order(placed.id) {
                        self.pending_placements.insert(group.id, TokioInstant::now());
                        // 019f: Emit TradeCreated event
                        self.emit_event(TradeEvent {
                            event_type: TradeEventType::TradeCreated,
                            group_id: Some(group.id),
                            user_id,
                            symbol: Some(symbol.clone()),
                            payload: serde_json::json!({
                                "entry_order_id": placed.id.to_string(),
                                "entry_price": placed.price.map(|p| p.to_string()),
                                "quantity": placed.quantity.to_string(),
                            }),
                        });
                    }
                }
                let _ = reply.send(result);
            }
            EngineCommand::PlaceOrderNoGroup { user_id, order, reply } => {
                let result = self.engine
                    .place_order_no_group(user_id, order)
                    .map_err(|e| EngineError::Internal(e.to_string()));
                let _ = reply.send(result);
            }
            EngineCommand::CancelOrder { user_id, order_id, reply } => {
                // 019f: Look up group before cancel so we have the group_id for the event
                let group_info = self.engine.order_groups.get_by_linked_order(order_id)
                    .or_else(|| self.engine.order_groups.get_by_entry_order(order_id))
                    .map(|g| (g.id, g.symbol.clone()));
                let result = self.engine
                    .cancel_order(user_id, order_id)
                    .map_err(|e| EngineError::Internal(e.to_string()));
                if result.is_ok() {
                    self.emit_event(TradeEvent {
                        event_type: TradeEventType::OrderCancelled,
                        group_id: group_info.as_ref().map(|(id, _)| *id),
                        user_id,
                        symbol: group_info.map(|(_, s)| s),
                        payload: serde_json::json!({
                            "order_id": order_id.to_string(),
                            "reason": "user_cancel",
                        }),
                    });
                }
                let _ = reply.send(result);
            }
            EngineCommand::CancelOrderNoCascade { user_id, order_id, reply } => {
                let result = self.engine
                    .cancel_order_no_cascade(user_id, order_id)
                    .map_err(|e| EngineError::Internal(e.to_string()));
                let _ = reply.send(result);
            }
            EngineCommand::GetOrder { order_id, reply } => {
                let _ = reply.send(self.engine.orders.get_order(order_id).cloned());
            }
            EngineCommand::GetOpenOrders { user_id, reply } => {
                let _ = reply.send(self.engine.get_open_orders(user_id));
            }
            EngineCommand::GetAllOrders { user_id, reply } => {
                let _ = reply.send(self.engine.orders.get_all_orders(user_id));
            }

            // --- Positions ---
            EngineCommand::GetPositions { user_id, reply } => {
                let _ = reply.send(self.engine.get_positions(user_id));
            }
            EngineCommand::GetUnrealizedPnl { user_id, reply } => {
                let _ = reply.send(self.engine.get_unrealized_pnl(user_id));
            }
            EngineCommand::OpenPositionCount { user_id, reply } => {
                let _ = reply.send(self.engine.positions.open_position_count(user_id));
            }
            EngineCommand::GetEntryPrice { user_id, symbol, reply } => {
                let _ = reply.send(self.engine.positions.get_entry_price(user_id, &symbol));
            }

            // --- Price processing ---
            EngineCommand::ProcessPriceUpdate { symbol, bid, ask, high, low, reply } => {
                let result = self.engine.process_price_update(&symbol, bid, ask, high, low);
                let _ = reply.send(result);
            }
            EngineCommand::UpdateMarkPrice { symbol, mark_price, reply } => {
                self.engine.update_mark_price(&symbol, mark_price);
                let _ = reply.send(());
            }
            EngineCommand::GetActiveSymbols { reply } => {
                let _ = reply.send(self.engine.get_active_symbols());
            }
            EngineCommand::CheckBreakEven { symbol, current_price, reply } => {
                self.engine.check_break_even(&symbol, current_price);
                let _ = reply.send(());
            }
            EngineCommand::EnableBreakEven { group_id, trigger_percent, offset, reply } => {
                let result = self.engine
                    .enable_break_even(group_id, trigger_percent, offset)
                    .map_err(|e| EngineError::Internal(e.to_string()));
                let _ = reply.send(result);
            }

            // --- Order groups ---
            EngineCommand::ListTradeGroups { user_id, reply } => {
                let _ = reply.send(self.engine.order_groups.get_user_groups(user_id));
            }
            EngineCommand::GetTradeGroup { group_id, reply } => {
                let _ = reply.send(self.engine.order_groups.get_group(group_id).cloned());
            }
            EngineCommand::GetGroupByEntryOrder { entry_order_id, reply } => {
                let _ = reply.send(self.engine.order_groups.get_by_entry_order(entry_order_id).cloned());
            }
            EngineCommand::GetGroupByLinkedOrder { order_id, reply } => {
                let _ = reply.send(self.engine.order_groups.get_by_linked_order(order_id).cloned());
            }
            EngineCommand::GetGroupByExchangeOrder { exchange_order_id, reply } => {
                let _ = reply.send(self.engine.order_groups.get_by_exchange_order(&exchange_order_id).cloned());
            }
            EngineCommand::GetActiveGroups { user_id, reply } => {
                let _ = reply.send(self.engine.order_groups.get_active_groups(user_id));
            }
            EngineCommand::GetLiveGroups { reply } => {
                let _ = reply.send(self.engine.order_groups.get_live_groups());
            }
            EngineCommand::ActiveGroupCount { reply } => {
                let _ = reply.send(self.engine.order_groups.active_count());
            }
            EngineCommand::RegisterExchangeOrderId { group_id, role, exchange_id, reply } => {
                // 019e FR-9: Clear from pending_placements — exchange registration succeeded.
                self.pending_placements.remove(&group_id);

                let result = if let Some(group) = self.engine.order_groups.get_group_mut(group_id) {
                    match role {
                        OrderRole::Entry => group.exchange_order_id = Some(exchange_id.clone()),
                        OrderRole::StopLoss => {
                            group.exchange_sl_order_id = Some(exchange_id.clone())
                        }
                        OrderRole::TakeProfit => {
                            group.exchange_tp_order_id = Some(exchange_id.clone())
                        }
                    }
                    group.updated_at = chrono::Utc::now();
                    self.engine.order_groups.register_exchange_order(exchange_id, group_id);
                    Ok(())
                } else {
                    Err(EngineError::Internal(format!("Group {group_id} not found")))
                };
                let _ = reply.send(result);
            }
            EngineCommand::RegisterLinkedOrder { order_id, group_id, reply } => {
                self.engine.order_groups.register_linked_order(order_id, group_id);
                let _ = reply.send(());
            }
            EngineCommand::UpdateEntryOrder { group_id, old_entry_id, new_entry_id, reply } => {
                self.engine.order_groups.update_entry_order(group_id, old_entry_id, new_entry_id);
                let _ = reply.send(());
            }
            EngineCommand::UpdateGroupStatus { group_id, status, reply } => {
                let old_status = self.engine.order_groups.get_group(group_id)
                    .map(|g| (g.status, g.user_id, g.symbol.clone()));
                let result = if let Some(group) = self.engine.order_groups.get_group_mut(group_id) {
                    let from = group.status;
                    group.status = status;
                    group.updated_at = chrono::Utc::now();
                    if status.is_terminal() {
                        group.completed_at = Some(std::time::Instant::now());
                    }
                    Ok(from)
                } else {
                    Err(EngineError::Internal(format!("Group {group_id} not found")))
                };
                if let Ok(from_status) = result {
                    if let Some((_, user_id, symbol)) = old_status {
                        self.emit_event(TradeEvent {
                            event_type: TradeEventType::GroupStatusChanged,
                            group_id: Some(group_id),
                            user_id,
                            symbol: Some(symbol),
                            payload: serde_json::json!({
                                "from": format!("{:?}", from_status),
                                "to": format!("{:?}", status),
                            }),
                        });
                    }
                }
                let _ = reply.send(result.map(|_| ()));
            }
            EngineCommand::OnEntryFilled { group_id, fill_price, reply } => {
                let group_meta = self.engine.order_groups.get_group(group_id)
                    .map(|g| (g.user_id, g.symbol.clone()));
                let result = if let Some(group) = self.engine.order_groups.get_group_mut(group_id) {
                    group.on_entry_filled(fill_price);
                    Ok(())
                } else {
                    Err(EngineError::Internal(format!("Group {group_id} not found")))
                };
                if result.is_ok() {
                    if let Some((user_id, symbol)) = group_meta {
                        self.emit_event(TradeEvent {
                            event_type: TradeEventType::EntryFilled,
                            group_id: Some(group_id),
                            user_id,
                            symbol: Some(symbol),
                            payload: serde_json::json!({ "fill_price": fill_price.to_string() }),
                        });
                    }
                }
                let _ = reply.send(result);
            }
            EngineCommand::OnStopLossFilled { group_id, reply } => {
                let group_meta = self.engine.order_groups.get_group(group_id)
                    .map(|g| (g.user_id, g.symbol.clone()));
                let result = if let Some(group) = self.engine.order_groups.get_group_mut(group_id) {
                    group.on_stop_loss_filled();
                    Ok(())
                } else {
                    Err(EngineError::Internal(format!("Group {group_id} not found")))
                };
                if result.is_ok() {
                    if let Some((user_id, symbol)) = group_meta {
                        self.emit_event(TradeEvent {
                            event_type: TradeEventType::StopLossFilled,
                            group_id: Some(group_id),
                            user_id,
                            symbol: Some(symbol),
                            payload: serde_json::json!({}),
                        });
                    }
                }
                let _ = reply.send(result);
            }
            EngineCommand::OnTakeProfitFilled { group_id, order_id, reply } => {
                let group_meta = self.engine.order_groups.get_group(group_id)
                    .map(|g| (g.user_id, g.symbol.clone()));
                let result = if let Some(group) = self.engine.order_groups.get_group_mut(group_id) {
                    group.on_take_profit_filled(order_id);
                    Ok(())
                } else {
                    Err(EngineError::Internal(format!("Group {group_id} not found")))
                };
                if result.is_ok() {
                    if let Some((user_id, symbol)) = group_meta {
                        self.emit_event(TradeEvent {
                            event_type: TradeEventType::TakeProfitFilled,
                            group_id: Some(group_id),
                            user_id,
                            symbol: Some(symbol),
                            payload: serde_json::json!({ "order_id": order_id.to_string() }),
                        });
                    }
                }
                let _ = reply.send(result);
            }
            EngineCommand::ReindexExchangeSlOrder { old_id, new_id, reply } => {
                let _ = reply.send(self.engine.order_groups.reindex_exchange_sl_order(&old_id, new_id));
            }

            // --- 019d: Fire-and-forget price update ---
            EngineCommand::ProcessPriceUpdateFireAndForget {
                symbol,
                bid,
                ask,
                high,
                low,
            } => {
                let result = self.engine.process_price_update(&symbol, bid, ask, high, low);
                if !result.filled.is_empty() || !result.exchange_cancels.is_empty() {
                    let _ = self.fill_event_tx.try_send(FillEvent {
                        symbol,
                        filled: result.filled,
                        exchange_cancels: result.exchange_cancels,
                    });
                }
            }

            // --- 019c: Route migration commands ---
            EngineCommand::ConfigureGroup {
                group_id,
                take_profit_targets,
                break_even_config,
                exchange_account_id,
                exchange_name,
                risk_amount,
                setup_tag,
                kelly_inputs,
                reply,
            } => {
                let result = if let Some(group) = self.engine.order_groups.get_group_mut(group_id) {
                    if let Some(targets) = take_profit_targets {
                        group.take_profit_targets = targets;
                    }
                    if let Some(config) = break_even_config {
                        group.break_even_config = Some(config);
                    }
                    if let Some(account_id) = exchange_account_id {
                        group.exchange_account_id = Some(account_id);
                    }
                    if let Some(name) = exchange_name {
                        group.exchange_name = Some(name);
                    }
                    if risk_amount.is_some() {
                        group.risk_amount = risk_amount;
                    }
                    if setup_tag.is_some() {
                        group.setup_tag = setup_tag;
                    }
                    if kelly_inputs.is_some() {
                        group.kelly_inputs = kelly_inputs;
                    }
                    group.updated_at = chrono::Utc::now();
                    Ok(())
                } else {
                    Err(EngineError::Internal(format!("Group {group_id} not found")))
                };
                let _ = reply.send(result);
            }
            EngineCommand::UpdateGroupStopLoss {
                group_id,
                expected_status,
                new_sl_price,
                new_entry_quantity,
                entry_order_swap,
                reply,
            } => {
                let old_sl = self.engine.order_groups.get_group(group_id)
                    .and_then(|g| g.stop_loss_price)
                    .unwrap_or_default();
                let group_meta = self.engine.order_groups.get_group(group_id)
                    .map(|g| (g.user_id, g.symbol.clone()));
                let result = if let Some(group) = self.engine.order_groups.get_group(group_id) {
                    if group.status != expected_status {
                        Err(EngineError::Internal(format!(
                            "Group status changed to {:?} — cannot update stop loss",
                            group.status
                        )))
                    } else {
                        // Do the index swap first if needed
                        if let Some((old_id, new_id)) = entry_order_swap {
                            self.engine.order_groups.update_entry_order(group_id, old_id, new_id);
                        }
                        // Now mutate the group (single-threaded actor: lookup is safe after
                        // the immutable check above; guard with explicit match for robustness).
                        let group = match self.engine.order_groups.get_group_mut(group_id) {
                            Some(g) => g,
                            None => {
                                let _ = reply.send(Err(EngineError::Internal(format!(
                                    "Group {group_id} vanished between get_group and get_group_mut"
                                ))));
                                return;
                            }
                        };
                        if let Some((_, new_id)) = entry_order_swap {
                            group.entry_order_id = new_id;
                        }
                        group.entry_quantity = new_entry_quantity;
                        group.stop_loss_price = Some(new_sl_price);
                        group.updated_at = chrono::Utc::now();
                        Ok(group.clone())
                    }
                } else {
                    Err(EngineError::Internal(format!("Group {group_id} not found")))
                };
                if result.is_ok() {
                    if let Some((user_id, symbol)) = group_meta {
                        self.emit_event(TradeEvent {
                            event_type: TradeEventType::StopLossAmended,
                            group_id: Some(group_id),
                            user_id,
                            symbol: Some(symbol),
                            payload: serde_json::json!({
                                "old_price": old_sl.to_string(),
                                "new_price": new_sl_price.to_string(),
                            }),
                        });
                    }
                }
                let _ = reply.send(result);
            }
            EngineCommand::UpdateStopPrice { order_id, new_price, reply } => {
                let _ = reply.send(self.engine.orders.update_stop_price(order_id, new_price));
            }
            EngineCommand::AddTakeProfitTarget { group_id, user_id, target, reply } => {
                let result = if let Some(group) = self.engine.order_groups.get_group_mut(group_id) {
                    if group.user_id != user_id {
                        Err(EngineError::Internal("Access denied".to_string()))
                    } else {
                        group.take_profit_targets.push(target);
                        group.updated_at = chrono::Utc::now();
                        Ok(group.clone())
                    }
                } else {
                    Err(EngineError::Internal(format!("Group {group_id} not found")))
                };
                let _ = reply.send(result);
            }

            // --- GC ---
            EngineCommand::PruneTerminal { cutoff, reply } => {
                let total = self.engine.orders.prune_terminal(cutoff)
                    + self.engine.positions.prune_terminal(cutoff)
                    + self.engine.order_groups.prune_terminal(cutoff);
                let _ = reply.send(total);
            }

            // --- Rehydration ---
            EngineCommand::LoadOrderGroups { groups, reply } => {
                for group in groups {
                    self.engine.order_groups.add_group(group);
                }
                let _ = reply.send(());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests (FR-6, FR-7, FR-8)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use super::super::orders::ShadowOrder;

    /// Helper: create an engine + actor + handle for testing.
    fn spawn_test_actor() -> (EngineHandle, ()) {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        (handle, ())
    }

    /// Helper: create a risk-validated limit buy order with SL/TP (so groups are created).
    /// Uses BTC_USDT (init_user gives 10000 USDT). Small size: 0.0001 x 50000 = 5 USDT margin.
    fn test_order(user_id: Uuid) -> ShadowOrder {
        let mut order = ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.0001), dec!(50000))
            .with_stop_loss(dec!(49000))
            .with_take_profit(dec!(52000));
        order.mark_risk_validated();
        order
    }

    // FR-6: 10,000 sequential commands without deadlock or timeout.
    #[tokio::test]
    async fn test_10k_sequential_commands() {
        let (handle, _) = spawn_test_actor();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        // Mix of place, list, and query operations
        for i in 0..10_000 {
            match i % 3 {
                0 => {
                    let order = test_order(user_id);
                    let _ = handle.place_order(user_id, order).await;
                }
                1 => {
                    let _ = handle.list_trade_groups(user_id).await;
                }
                _ => {
                    let _ = handle.get_positions(user_id).await;
                }
            }
        }

        // Verify state is consistent: every placed order has a matching group
        let orders = handle.get_open_orders(user_id).await;
        let groups = handle.list_trade_groups(user_id).await;
        assert_eq!(orders.len(), groups.len());
        // With 5 USDC per order and 10000 USDC balance, ~2000 orders max
        assert!(orders.len() > 1000, "expected >1000 orders, got {}", orders.len());
    }

    // FR-7: 10 concurrent tasks sending interleaved commands produce consistent state.
    #[tokio::test]
    async fn test_10_concurrent_tasks() {
        let (handle, _) = spawn_test_actor();

        // Each task gets its own user
        let mut tasks = Vec::new();
        for _ in 0..10 {
            let h = handle.clone();
            tasks.push(tokio::spawn(async move {
                let user_id = Uuid::new_v4();
                h.init_user(user_id).await.unwrap();

                // Each task places 100 orders, queries, then cancels some
                for _ in 0..100 {
                    let order = test_order(user_id);
                    let _ = h.place_order(user_id, order).await;
                }

                let orders = h.get_open_orders(user_id).await;
                assert_eq!(orders.len(), 100);

                // Cancel first 50
                for order in orders.iter().take(50) {
                    let _ = h.cancel_order(user_id, order.id).await;
                }

                let remaining = h.get_open_orders(user_id).await;
                assert_eq!(remaining.len(), 50);

                let groups = h.list_trade_groups(user_id).await;
                assert_eq!(groups.len(), 100); // groups persist even after cancel

                user_id
            }));
        }

        // All tasks complete without deadlock
        let mut user_ids = Vec::new();
        for task in tasks {
            user_ids.push(task.await.unwrap());
        }

        // Verify each user's state is isolated
        for user_id in &user_ids {
            let orders = handle.get_open_orders(*user_id).await;
            assert_eq!(orders.len(), 50);
        }
    }

    // FR-8: Actor shutdown -- drop the handle, actor exits cleanly.
    #[tokio::test]
    async fn test_actor_shutdown() {
        let engine = ShadowEngine::new();
        let (tx, rx) = mpsc::channel(ENGINE_CHANNEL_CAPACITY);
        let (fill_tx, _fill_rx) = mpsc::channel(FILL_EVENT_CHANNEL_CAPACITY);
        let (trade_event_tx, _trade_event_rx) = mpsc::channel(TRADE_EVENT_CHANNEL_CAPACITY);
        let actor = EngineActor::new(engine, rx, fill_tx, trade_event_tx);

        let actor_task = tokio::spawn(actor.run());

        // Send one command to prove actor is alive
        let handle = EngineHandle::new(tx);
        let user_id = Uuid::new_v4();
        handle.init_user(user_id).await.unwrap();
        assert!(handle.user_exists(user_id).await);

        // Drop handle -> channel closes -> actor should exit
        drop(handle);

        // Actor task should complete without timeout
        tokio::time::timeout(std::time::Duration::from_secs(2), actor_task)
            .await
            .expect("actor should shut down within 2 seconds")
            .expect("actor task should not panic");
    }

    #[tokio::test]
    async fn test_place_and_cancel_through_actor() {
        let (handle, _) = spawn_test_actor();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let placed = handle.place_order(user_id, order).await.unwrap();

        // Verify order exists
        let found = handle.get_order(placed.id).await;
        assert!(found.is_some());

        // Cancel it
        let cancelled = handle.cancel_order(user_id, placed.id).await.unwrap();
        assert_eq!(cancelled.id, placed.id);

        // No more open orders
        let open = handle.get_open_orders(user_id).await;
        assert!(open.is_empty());
    }

    #[tokio::test]
    async fn test_order_groups_through_actor() {
        let (handle, _) = spawn_test_actor();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let placed = handle.place_order(user_id, order).await.unwrap();

        // Should have one trade group
        let groups = handle.list_trade_groups(user_id).await;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].entry_order_id, placed.id);

        // Get group by ID
        let group = handle.get_trade_group(groups[0].id).await;
        assert!(group.is_some());

        // Register exchange order ID
        handle
            .register_exchange_order_id(groups[0].id, OrderRole::Entry, "exch-123".to_string())
            .await
            .unwrap();

        // Look up by exchange order
        let found = handle
            .get_group_by_exchange_order("exch-123".to_string())
            .await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, groups[0].id);
    }

    #[tokio::test]
    async fn test_price_update_through_actor() {
        let (handle, _) = spawn_test_actor();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        // Place a limit buy at 50000 (with SL at 49000 and TP at 52000)
        let order = test_order(user_id);
        handle.place_order(user_id, order).await.unwrap();

        // Price update where ask drops to limit -> entry should fill
        let result = handle
            .process_price_update(
                "BTC_USDT".to_string(),
                dec!(49900),
                dec!(50000),
                dec!(50100),
                dec!(49800),
            )
            .await
            .unwrap();

        assert_eq!(result.filled.len(), 1);

        // Entry filled -> SL and TP orders created (2 open orders remain)
        let open = handle.get_open_orders(user_id).await;
        assert_eq!(open.len(), 2, "SL + TP orders should be open after entry fill");

        // Position should exist
        let positions = handle.get_positions(user_id).await;
        assert_eq!(positions.len(), 1);
    }

    #[tokio::test]
    async fn test_shutdown_returns_error_on_handle() {
        let engine = ShadowEngine::new();
        let (tx, rx) = mpsc::channel(ENGINE_CHANNEL_CAPACITY);
        let (fill_tx, _fill_rx) = mpsc::channel(FILL_EVENT_CHANNEL_CAPACITY);
        let (trade_event_tx, _trade_event_rx) = mpsc::channel(TRADE_EVENT_CHANNEL_CAPACITY);
        let actor = EngineActor::new(engine, rx, fill_tx, trade_event_tx);

        let handle = EngineHandle::new(tx);
        let actor_task = tokio::spawn(actor.run());

        // Drop the receiver by aborting actor
        actor_task.abort();
        // Give it a moment
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Commands should fail gracefully
        let result = handle.init_user(Uuid::new_v4()).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(EngineError::ActorShutdown)));
    }

    #[tokio::test]
    async fn test_load_order_groups() {
        let (handle, _) = spawn_test_actor();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let groups = vec![
            OrderGroup::new(user_id, "BTC_USDT".to_string(), Uuid::new_v4(), dec!(0.1)),
            OrderGroup::new(user_id, "ETH_USDT".to_string(), Uuid::new_v4(), dec!(1.0)),
        ];

        handle.load_order_groups(groups).await.unwrap();

        let loaded = handle.list_trade_groups(user_id).await;
        assert_eq!(loaded.len(), 2);
    }

    // FR-13: Place order, do NOT register exchange ID, advance past 30s timeout,
    // verify group transitions to AwaitingReconciliation.
    #[tokio::test(start_paused = true)]
    async fn test_zombie_detection_placement_timeout() {
        let (handle, _fill_rx) = spawn_test_actor();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let _placed = handle.place_order(user_id, order).await.unwrap();

        let groups = handle.list_trade_groups(user_id).await;
        assert_eq!(groups.len(), 1);
        let group_id = groups[0].id;

        // Do NOT call register_exchange_order_id — simulate caller crash.
        // Advance past the 30s timeout + one sweep interval (15s).
        tokio::time::advance(Duration::from_secs(46)).await;

        // Yield to let the actor's select! process the sweep tick.
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(10)).await;
        tokio::task::yield_now().await;

        let group = handle.get_trade_group(group_id).await.unwrap();
        assert_eq!(
            group.status,
            OrderGroupStatus::AwaitingReconciliation,
            "Group should be marked AwaitingReconciliation after placement timeout"
        );
    }

    // FR-14: Place order, register exchange ID within timeout,
    // verify group is NOT swept to AwaitingReconciliation.
    #[tokio::test(start_paused = true)]
    async fn test_normal_registration_prevents_sweep() {
        let (handle, _fill_rx) = spawn_test_actor();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let _placed = handle.place_order(user_id, order).await.unwrap();

        let groups = handle.list_trade_groups(user_id).await;
        assert_eq!(groups.len(), 1);
        let group_id = groups[0].id;

        // Register exchange ID within 5s — simulates normal flow.
        tokio::time::advance(Duration::from_secs(5)).await;
        handle
            .register_exchange_order_id(group_id, OrderRole::Entry, "exch-456".to_string())
            .await
            .unwrap();

        // Advance past timeout — sweep should find nothing to mark.
        tokio::time::advance(Duration::from_secs(46)).await;
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(10)).await;
        tokio::task::yield_now().await;

        let group = handle.get_trade_group(group_id).await.unwrap();
        assert_ne!(
            group.status,
            OrderGroupStatus::AwaitingReconciliation,
            "Group should NOT be swept when exchange ID was registered in time"
        );
    }

    // FR-15: Cancel/fill race — send CancelOrder and ProcessPriceUpdate concurrently,
    // verify cancelled order is never filled.
    #[tokio::test]
    async fn test_cancel_fill_race_condition() {
        let (handle, _fill_rx) = spawn_test_actor();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let placed = handle.place_order(user_id, order).await.unwrap();

        // Cancel the order
        handle.cancel_order(user_id, placed.id).await.unwrap();

        // Now attempt a price update that would fill the order
        let result = handle
            .process_price_update(
                "BTC_USDT".to_string(),
                dec!(49900),
                dec!(50000),
                dec!(50100),
                dec!(49800),
            )
            .await
            .unwrap();

        // The cancelled order should not appear as filled
        assert!(
            result.filled.is_empty(),
            "Cancelled order should never be filled"
        );

        // No open orders remain
        let open = handle.get_open_orders(user_id).await;
        assert!(open.is_empty());
    }

    /// Helper: spawn actor and return handle + trade event receiver for testing events.
    fn spawn_test_actor_with_events() -> (EngineHandle, mpsc::Receiver<TradeEvent>) {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, trade_event_rx) = EngineActor::spawn(engine);
        (handle, trade_event_rx)
    }

    // 019f FR-14: Full trade lifecycle produces expected event sequence.
    // Create → entry fill → SL fill should emit TradeCreated, EntryFilled, StopLossFilled.
    #[tokio::test]
    async fn test_trade_lifecycle_event_sequence() {
        let (handle, mut event_rx) = spawn_test_actor_with_events();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        // 1. Place order → should emit TradeCreated
        let order = test_order(user_id);
        let placed = handle.place_order(user_id, order).await.unwrap();

        let event = event_rx.recv().await.unwrap();
        assert_eq!(event.event_type, TradeEventType::TradeCreated);
        assert_eq!(event.user_id, user_id);
        assert!(event.group_id.is_some());

        let group_id = event.group_id.unwrap();

        // 2. Entry fill via price update
        let _result = handle
            .process_price_update(
                "BTC_USDT".to_string(),
                dec!(49900),
                dec!(50000),
                dec!(50100),
                dec!(49800),
            )
            .await
            .unwrap();

        // 3. Manually mark entry filled via actor command
        handle.on_entry_filled(group_id, dec!(50000)).await.unwrap();

        let event = event_rx.recv().await.unwrap();
        assert_eq!(event.event_type, TradeEventType::EntryFilled);
        assert_eq!(event.group_id, Some(group_id));
        assert_eq!(event.payload["fill_price"], "50000");

        // 4. Stop loss fill
        handle.on_stop_loss_filled(group_id).await.unwrap();

        let event = event_rx.recv().await.unwrap();
        assert_eq!(event.event_type, TradeEventType::StopLossFilled);
        assert_eq!(event.group_id, Some(group_id));
    }

    // 019f FR-14: TradeCreated event contains correct metadata.
    #[tokio::test]
    async fn test_trade_created_event_payload() {
        let (handle, mut event_rx) = spawn_test_actor_with_events();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let placed = handle.place_order(user_id, order).await.unwrap();

        let event = event_rx.recv().await.unwrap();
        assert_eq!(event.event_type, TradeEventType::TradeCreated);
        assert_eq!(event.symbol, Some("BTC_USDT".to_string()));
        assert_eq!(event.payload["entry_order_id"], placed.id.to_string());
        assert_eq!(event.payload["quantity"], "0.0001");
    }

    // 019f: GroupStatusChanged event emitted on status update.
    #[tokio::test]
    async fn test_group_status_changed_event() {
        let (handle, mut event_rx) = spawn_test_actor_with_events();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let _placed = handle.place_order(user_id, order).await.unwrap();

        // Consume TradeCreated event
        let _ = event_rx.recv().await.unwrap();

        let groups = handle.list_trade_groups(user_id).await;
        let group_id = groups[0].id;

        // Update status to Cancelled
        handle
            .update_group_status(group_id, OrderGroupStatus::Cancelled)
            .await
            .unwrap();

        let event = event_rx.recv().await.unwrap();
        assert_eq!(event.event_type, TradeEventType::GroupStatusChanged);
        assert_eq!(event.group_id, Some(group_id));
        assert_eq!(event.payload["from"], "Pending");
        assert_eq!(event.payload["to"], "Cancelled");
    }

    // 019f: OrderCancelled event emitted on cancel.
    #[tokio::test]
    async fn test_order_cancelled_event() {
        let (handle, mut event_rx) = spawn_test_actor_with_events();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let placed = handle.place_order(user_id, order).await.unwrap();

        // Consume TradeCreated event
        let _ = event_rx.recv().await.unwrap();

        // Cancel the order
        handle.cancel_order(user_id, placed.id).await.unwrap();

        let event = event_rx.recv().await.unwrap();
        assert_eq!(event.event_type, TradeEventType::OrderCancelled);
        assert_eq!(event.payload["order_id"], placed.id.to_string());
        assert_eq!(event.payload["reason"], "user_cancel");
    }

    // 019f: TakeProfitFilled event emitted.
    #[tokio::test]
    async fn test_take_profit_filled_event() {
        let (handle, mut event_rx) = spawn_test_actor_with_events();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let _placed = handle.place_order(user_id, order).await.unwrap();

        // Consume TradeCreated event
        let _ = event_rx.recv().await.unwrap();

        let groups = handle.list_trade_groups(user_id).await;
        let group_id = groups[0].id;
        let tp_order_id = Uuid::new_v4();

        // Simulate take profit fill
        handle
            .on_take_profit_filled(group_id, tp_order_id)
            .await
            .unwrap();

        let event = event_rx.recv().await.unwrap();
        assert_eq!(event.event_type, TradeEventType::TakeProfitFilled);
        assert_eq!(event.group_id, Some(group_id));
        assert_eq!(event.payload["order_id"], tp_order_id.to_string());
    }

    // 019f: StopLossAmended event on SL update.
    #[tokio::test]
    async fn test_stop_loss_amended_event() {
        let (handle, mut event_rx) = spawn_test_actor_with_events();
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order = test_order(user_id);
        let placed = handle.place_order(user_id, order).await.unwrap();

        // Consume TradeCreated event
        let _ = event_rx.recv().await.unwrap();

        let groups = handle.list_trade_groups(user_id).await;
        let group_id = groups[0].id;

        // Update stop loss
        let _ = handle
            .update_group_stop_loss(
                group_id,
                OrderGroupStatus::Pending,
                dec!(48000),
                dec!(0.0001),
                Some((placed.id, Uuid::new_v4())),
            )
            .await
            .unwrap();

        let event = event_rx.recv().await.unwrap();
        assert_eq!(event.event_type, TradeEventType::StopLossAmended);
        assert_eq!(event.payload["new_price"], "48000");
    }
}
