//! EXT-22: Fill Detection Service
//!
//! Subscribes to order update events from the CEX sidecar WebSocket and
//! implements OCO (one-cancels-other) logic: when an SL fills, cancel the TP
//! on the exchange, and vice versa. Uses the `groups_by_exchange_order` index
//! on `OrderGroupManager` to map exchange order IDs back to order groups.
//!
//! CEX-07: safe-cex handles bracket order sequencing natively, so deferred
//! SL/TP placement after entry fill is no longer needed here.
//!
//! CON-01: Journal writes are no longer fire-and-forget. Instead, TradeClosed
//! events are sent to the trade_event_tx channel for atomic co-write in
//! TradeEventWriter's flush transaction.

use crate::services::cex_client::OrderUpdateEvent;
use crate::services::exchange_api::{ExchangeApi, ExchangeApiError};
use engine::shadow::order_group::{OrderGroup, OrderGroupStatus};
use engine::{EngineHandle, FillEvent};
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing;
use uuid::Uuid;

/// Snapshot of an order group's OCO-relevant state, extracted while
/// holding the lock so the lock can be released before exchange calls.
struct FillAction {
    group_id: Uuid,
    user_id: Uuid,
    symbol: String,
    exchange_account_id: Option<Uuid>,
    exchange_sl_order_id: Option<String>,
    exchange_tp_order_id: Option<String>,
    event_timestamp: Option<i64>,
    kind: FillKind,
    /// CON-01: Full group snapshot for building TradeClosed event payload.
    group_snapshot: Option<OrderGroup>,
    /// CON-01: Close event side from exchange ("buy"/"sell") for deriving LONG/SHORT.
    close_event_side: Option<String>,
}

enum FillKind {
    StopLoss { filled_order_id: String },
    TakeProfit { filled_order_id: String },
    Entry,
    /// FIX-08: User closed the position directly on the exchange. Closing order matches
    /// neither our entry nor SL/TP IDs, but a tracked group exists for the symbol and
    /// the group has SL/TP IDs registered. Both siblings must be cancelled.
    ManualClose { filled_order_id: String },
}

/// Fill detection service that processes order update events and triggers
/// OCO cancellation on the live exchange.
///
/// 019d: Listens to both the WS order update channel (exchange fills) and
/// the actor fill event channel (shadow engine fills from price updates).
pub struct FillDetectorService {
    engine_handle: EngineHandle,
    exchange_api: Arc<dyn ExchangeApi>,
    event_tx: Option<tokio::sync::mpsc::Sender<crate::services::ManagementEvent>>,
}

impl FillDetectorService {
    pub fn new(
        engine_handle: EngineHandle,
        exchange_api: Arc<dyn ExchangeApi>,
    ) -> Self {
        Self {
            engine_handle,
            exchange_api,
            event_tx: None,
        }
    }

    pub fn with_event_sender(
        mut self,
        tx: tokio::sync::mpsc::Sender<crate::services::ManagementEvent>,
    ) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Run the fill detector loop. Listens to both channels:
    /// - `order_rx`: WS order update events from the exchange (via CEX sidecar)
    /// - `fill_rx`: Fill events from the shadow engine actor (fire-and-forget price updates)
    ///
    /// 017 FR-2: mpsc guarantees no silent event loss.
    pub async fn run(
        &self,
        mut order_rx: mpsc::Receiver<OrderUpdateEvent>,
        mut fill_rx: mpsc::Receiver<FillEvent>,
    ) {
        tracing::info!("FillDetectorService started (dual-channel)");

        loop {
            tokio::select! {
                event = order_rx.recv() => {
                    match event {
                        Some(e) => self.handle_order_update(e).await,
                        None => {
                            tracing::info!("FillDetector: order update channel closed");
                            break;
                        }
                    }
                }
                event = fill_rx.recv() => {
                    match event {
                        Some(e) => self.handle_fill_event(e).await,
                        None => {
                            tracing::info!("FillDetector: fill event channel closed");
                            break;
                        }
                    }
                }
            }
        }

        tracing::info!("FillDetector shutting down");
    }

    /// 019d: Handle fill events from the shadow engine actor (fire-and-forget price updates).
    /// Processes OCO exchange cancellations triggered by shadow engine matching.
    async fn handle_fill_event(&self, event: FillEvent) {
        if !event.filled.is_empty() {
            tracing::info!(
                "FillDetector: {} shadow fills for {}",
                event.filled.len(),
                event.symbol
            );
        }

        // Execute OCO cancels on the exchange
        for cancel in &event.exchange_cancels {
            match self
                .exchange_api
                .cancel_order(cancel.user_id, &cancel.exchange_order_id, &event.symbol, cancel.exchange_account_id)
                .await
            {
                Ok(()) => {
                    tracing::info!(
                        "FillDetector OCO cancel succeeded: order_id={} user_id={} symbol={}",
                        cancel.exchange_order_id,
                        cancel.user_id,
                        event.symbol
                    );
                }
                Err(ExchangeApiError::OrderNotFound(_)) => {
                    tracing::debug!(
                        "FillDetector OCO cancel no-op (OrderNotFound): order_id={} symbol={}",
                        cancel.exchange_order_id,
                        event.symbol
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "FillDetector OCO cancel failed: order_id={} symbol={} error={}",
                        cancel.exchange_order_id,
                        event.symbol,
                        e
                    );
                }
            }
        }
    }

    /// Process a single order update event.
    ///
    /// If the order status is "closed" (filled), look up the exchange order ID
    /// via EngineHandle, determine whether it's an SL or TP, cancel the
    /// sibling, and update the group status.
    pub async fn handle_order_update(&self, event: OrderUpdateEvent) {
        let started_at = Instant::now();
        match event.status.as_str() {
            "closed" => { /* fall through to existing logic */ }
            "canceled" | "cancelled" => {
                self.handle_cancelled_event(event, started_at).await;
                return;
            }
            _ => return,
        }

        let exchange_order_id = &event.id;

        // Phase 1: Look up group via EngineHandle, update status via handle commands.
        // CEX-08: If exchange order ID lookup fails (e.g. Bybit bracket orders where
        // SL/TP IDs aren't returned at placement), fall back to finding the active
        // group for this user+symbol. The close side tells us whether SL or TP filled.
        let group = match self.engine_handle.get_group_by_exchange_order(exchange_order_id.clone()).await {
            Some(g) => g,
            None => {
                // CEX-08: Symbol-based fallback for exchanges that don't return SL/TP order IDs
                if let Some(user_id) = event.user_id {
                    let active = self.engine_handle.get_active_groups(user_id).await;
                    let candidates: Vec<_> = active
                        .into_iter()
                        .filter(|g| g.symbol == event.symbol)
                        .collect();
                    if candidates.len() == 1 {
                        tracing::info!(
                            "FillDetector: CEX-08 symbol fallback matched group {} for exchange order {}",
                            candidates[0].id,
                            exchange_order_id
                        );
                        candidates.into_iter().next().unwrap()
                    } else {
                        tracing::debug!(
                            "FillDetector: unknown exchange order ID {}, {} symbol candidates, ignoring",
                            exchange_order_id,
                            candidates.len()
                        );
                        return;
                    }
                } else {
                    tracing::debug!(
                        "FillDetector: unknown exchange order ID {}, no user_id for fallback, ignoring",
                        exchange_order_id
                    );
                    return;
                }
            }
        };

        // Idempotency: skip if already in terminal state
        if matches!(
            group.status,
            OrderGroupStatus::StoppedOut
                | OrderGroupStatus::TookProfit
                | OrderGroupStatus::Cancelled
                | OrderGroupStatus::Closed
        ) {
            tracing::debug!(
                "FillDetector: group {} already in terminal state {:?}, skipping",
                group.id,
                group.status
            );
            return;
        }

        let group_id = group.id;
        let user_id = group.user_id;
        let symbol = group.symbol.clone();
        let exchange_account_id = group.exchange_account_id;
        let exchange_sl_order_id = group.exchange_sl_order_id.clone();
        let exchange_tp_order_id = group.exchange_tp_order_id.clone();

        let is_sl = exchange_sl_order_id.as_deref() == Some(exchange_order_id.as_str());
        let is_tp = exchange_tp_order_id.as_deref() == Some(exchange_order_id.as_str());
        let is_entry = group.exchange_order_id.as_deref() == Some(exchange_order_id.as_str());

        // CON-01: Capture close event side (used for OCO cancel logic).
        let close_event_side = event.side.clone();

        let action = if is_sl {
            if let Err(e) = self.engine_handle.on_stop_loss_filled(group_id).await {
                tracing::error!("FillDetector: failed to update SL fill for group {}: {}", group_id, e);
                return;
            }
            FillAction {
                group_id,
                user_id,
                symbol,
                exchange_account_id,
                exchange_sl_order_id,
                exchange_tp_order_id,
                event_timestamp: event.timestamp,
                kind: FillKind::StopLoss {
                    filled_order_id: exchange_order_id.to_string(),
                },
                group_snapshot: Some(group.clone()),
                close_event_side: Some(close_event_side.clone()),
            }
        } else if is_tp {
            if let Err(e) = self.engine_handle.update_group_status(group_id, OrderGroupStatus::TookProfit).await {
                tracing::error!("FillDetector: failed to update TP fill for group {}: {}", group_id, e);
                return;
            }
            FillAction {
                group_id,
                user_id,
                symbol,
                exchange_account_id,
                exchange_sl_order_id,
                exchange_tp_order_id,
                event_timestamp: event.timestamp,
                kind: FillKind::TakeProfit {
                    filled_order_id: exchange_order_id.to_string(),
                },
                group_snapshot: Some(group.clone()),
                close_event_side: Some(close_event_side),
            }
        } else if is_entry {
            // FIX-01: Already `Decimal` — no f64 conversion needed
            let fill_dec = event.average.unwrap_or(Decimal::ZERO);
            if let Err(e) = self.engine_handle.on_entry_filled(group_id, fill_dec).await {
                tracing::error!("FillDetector: failed to update entry fill for group {}: {}", group_id, e);
                return;
            }

            // CEX-07: safe-cex handles bracket order SL/TP natively.
            // No deferred placement needed here.

            FillAction {
                group_id,
                user_id,
                symbol,
                exchange_account_id,
                exchange_sl_order_id,
                exchange_tp_order_id,
                event_timestamp: event.timestamp,
                kind: FillKind::Entry,
                group_snapshot: None,
                close_event_side: None,
            }
        } else if event.user_id.is_some() {
            // FIX-09 FR-7: Classification (SL vs TP vs manual) is not tracked
            // at this layer. Both Bybit ID-less brackets and genuine manual
            // closes land here; the JournalSyncer derives close economics from
            // REST fill history.
            if let Err(e) = self.engine_handle.update_group_status(group_id, OrderGroupStatus::Closed).await {
                tracing::error!("FillDetector: failed to mark group {} as Closed: {}", group_id, e);
                return;
            }
            FillAction {
                group_id,
                user_id,
                symbol,
                exchange_account_id,
                exchange_sl_order_id,
                exchange_tp_order_id,
                event_timestamp: event.timestamp,
                kind: FillKind::ManualClose { filled_order_id: exchange_order_id.to_string() },
                group_snapshot: Some(group.clone()),
                close_event_side: Some(close_event_side),
            }
        } else {
            tracing::debug!(
                "FillDetector: exchange order {} belongs to group {} but doesn't match entry/SL/TP fields",
                exchange_order_id,
                group_id
            );
            return;
        };

        // Phase 2: Execute exchange operations without holding the lock
        match action.kind {
            FillKind::StopLoss { filled_order_id } => {
                tracing::info!(
                    kind = "stop_loss",
                    group_id = %action.group_id,
                    order_id = %filled_order_id,
                    symbol = %action.symbol,
                    event_ts = ?action.event_timestamp,
                    "FillDetector: matched fill"
                );
                self.cancel_all_related_orders(
                    action.user_id,
                    action.exchange_account_id,
                    &action.symbol,
                    action.group_id,
                    [action.exchange_sl_order_id, action.exchange_tp_order_id],
                    &filled_order_id,
                )
                .await;
                self.broadcast_fill_event(
                    action.user_id,
                    &action.symbol,
                    "stopped_out",
                    action.group_id,
                );
                tracing::info!(
                    group_id = %action.group_id,
                    latency_ms = started_at.elapsed().as_millis() as u64,
                    "FillDetector: SL filled, cancelled TP"
                );
            }
            FillKind::TakeProfit { filled_order_id } => {
                tracing::info!(
                    kind = "take_profit",
                    group_id = %action.group_id,
                    order_id = %filled_order_id,
                    symbol = %action.symbol,
                    event_ts = ?action.event_timestamp,
                    "FillDetector: matched fill"
                );
                self.cancel_all_related_orders(
                    action.user_id,
                    action.exchange_account_id,
                    &action.symbol,
                    action.group_id,
                    [action.exchange_sl_order_id, action.exchange_tp_order_id],
                    &filled_order_id,
                )
                .await;
                self.broadcast_fill_event(
                    action.user_id,
                    &action.symbol,
                    "took_profit",
                    action.group_id,
                );
                tracing::info!(
                    group_id = %action.group_id,
                    latency_ms = started_at.elapsed().as_millis() as u64,
                    "FillDetector: TP filled, cancelled SL"
                );
            }
            FillKind::Entry => {
                tracing::info!(
                    kind = "entry",
                    group_id = %action.group_id,
                    symbol = %action.symbol,
                    event_ts = ?action.event_timestamp,
                    "FillDetector: matched fill"
                );

                self.broadcast_fill_event(
                    action.user_id,
                    &action.symbol,
                    "entry_filled",
                    action.group_id,
                );
                tracing::info!(
                    group_id = %action.group_id,
                    latency_ms = started_at.elapsed().as_millis() as u64,
                    "FillDetector: entry filled"
                );
            }
            FillKind::ManualClose { filled_order_id } => {
                tracing::info!(
                    kind = "manual_close",
                    group_id = %action.group_id,
                    order_id = %filled_order_id,
                    symbol = %action.symbol,
                    event_ts = ?action.event_timestamp,
                    "FillDetector: matched fill"
                );
                self.cancel_all_related_orders(
                    action.user_id,
                    action.exchange_account_id,
                    &action.symbol,
                    action.group_id,
                    [action.exchange_sl_order_id, action.exchange_tp_order_id],
                    &filled_order_id,
                )
                .await;
                self.broadcast_fill_event(
                    action.user_id,
                    &action.symbol,
                    "manual_closed",
                    action.group_id,
                );
                tracing::info!(
                    group_id = %action.group_id,
                    latency_ms = started_at.elapsed().as_millis() as u64,
                    "FillDetector: manual close detected, cancelled SL+TP siblings"
                );
            }
        }
    }

    /// Cancel all related pending orders on trade close.
    /// Idempotent — OrderNotFound is a no-op.
    async fn cancel_all_related_orders(
        &self,
        user_id: Uuid,
        exchange_account_id: Option<Uuid>,
        symbol: &str,
        group_id: Uuid,
        order_ids: [Option<String>; 2],
        filled_order_id: &str,
    ) {
        let mut seen = HashSet::new();
        for order_id in order_ids.into_iter().flatten() {
            if order_id == filled_order_id || !seen.insert(order_id.clone()) {
                continue;
            }

            match self
                .exchange_api
                .cancel_order(user_id, &order_id, symbol, exchange_account_id)
                .await
            {
                Ok(()) => {
                    tracing::info!(
                        "FillDetector: cancelled related order {} for {} (group {})",
                        order_id,
                        symbol,
                        group_id
                    );
                }
                Err(ExchangeApiError::OrderNotFound(_)) => {
                    tracing::debug!(
                        "FillDetector: related order {} already gone (OrderNotFound) for {} (group {})",
                        order_id,
                        symbol,
                        group_id
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "FillDetector: failed to cancel related order {} for {} (group {}): {}",
                        order_id,
                        symbol,
                        group_id,
                        e
                    );
                }
            }
        }
    }

    /// Handle a cancelled order event. If the cancelled order is the entry,
    /// mark the group as Cancelled and persist to DB.
    async fn handle_cancelled_event(&self, event: OrderUpdateEvent, started_at: Instant) {
        let exchange_order_id = &event.id;

        let group = match self.engine_handle.get_group_by_exchange_order(exchange_order_id.clone()).await {
            Some(g) => g,
            None => return,
        };

        if group.status.is_terminal() {
            return;
        }

        // Only entry cancellation kills the group
        let is_entry = group.exchange_order_id.as_deref() == Some(exchange_order_id.as_str());
        if !is_entry {
            return;
        }

        let group_id = group.id;
        let user_id = group.user_id;
        let symbol = group.symbol.clone();
        let exchange_account_id = group.exchange_account_id;
        let exchange_sl_order_id = group.exchange_sl_order_id.clone();
        let exchange_tp_order_id = group.exchange_tp_order_id.clone();

        if let Err(e) = self.engine_handle.update_group_status(group_id, OrderGroupStatus::Cancelled).await {
            tracing::error!("FillDetector: failed to cancel group {}: {}", group_id, e);
            return;
        }

        // Cancel sibling orders on exchange
        self.cancel_all_related_orders(
            user_id,
            exchange_account_id,
            &symbol,
            group_id,
            [exchange_sl_order_id, exchange_tp_order_id],
            "", // no filled order to skip
        )
        .await;

        self.broadcast_fill_event(
            user_id,
            &symbol,
            "cancelled",
            group_id,
        );
        tracing::info!(
            group_id = %group_id,
            latency_ms = started_at.elapsed().as_millis() as u64,
            "FillDetector: entry cancelled, cleaned up group"
        );
    }

    /// Broadcast a fill event to the extension via the management event channel (AUD-03 FR-7).
    fn broadcast_fill_event(&self, user_id: Uuid, symbol: &str, event_type: &str, group_id: Uuid) {
        if let Some(ref tx) = self.event_tx {
            if let Err(e) = tx.try_send(crate::services::ManagementEvent {
                user_id,
                event_type: event_type.to_string(),
                symbol: symbol.to_string(),
                detail: format!("group_id={}", group_id),
            }) {
                tracing::warn!("Management event channel full, dropping fill event: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::exchange_api::{AmendRequest, ExchangeApiError, PlaceOrderRequest, PlaceOrderResult};
    use async_trait::async_trait;
    use engine::shadow::order_group::{OrderGroup, OrderGroupManager};
    use engine::{EngineActor, ShadowEngine};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::RwLock;

    /// Mock exchange API that tracks cancel_order calls.
    struct MockExchangeApi {
        cancel_count: AtomicUsize,
        cancelled_ids: tokio::sync::Mutex<Vec<String>>,
    }

    impl MockExchangeApi {
        fn new() -> Self {
            Self {
                cancel_count: AtomicUsize::new(0),
                cancelled_ids: tokio::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl ExchangeApi for MockExchangeApi {
        async fn get_balance(
            &self,
            _user_id: Uuid,
            _asset: &str,
            _exchange_account_id: Option<Uuid>,
        ) -> Result<Decimal, ExchangeApiError> {
            Ok(dec!(10000))
        }

        async fn place_order(&self, _req: PlaceOrderRequest) -> Result<PlaceOrderResult, ExchangeApiError> {
            Ok(PlaceOrderResult {
                id: "mock-order-id".to_string(),
                status: None,
                average: None,
                stop_loss_order_id: None,
                take_profit_order_id: None,
            })
        }

        async fn amend_order(
            &self,
            _user_id: Uuid,
            _order_id: &str,
            _symbol: &str,
            _amend: AmendRequest,
            _exchange_account_id: Option<Uuid>,
        ) -> Result<String, ExchangeApiError> {
            Ok("mock-amended-id".to_string())
        }

        async fn cancel_order(
            &self,
            _user_id: Uuid,
            order_id: &str,
            _symbol: &str,
            _exchange_account_id: Option<Uuid>,
        ) -> Result<(), ExchangeApiError> {
            self.cancel_count.fetch_add(1, Ordering::SeqCst);
            self.cancelled_ids.lock().await.push(order_id.to_string());
            Ok(())
        }

        async fn get_position(
            &self,
            _user_id: Uuid,
            _symbol: &str,
            _exchange_account_id: Option<Uuid>,
        ) -> Result<Option<crate::services::exchange_api::PositionInfo>, ExchangeApiError> {
            Ok(None)
        }
    }

    fn make_event(id: &str, status: &str) -> OrderUpdateEvent {
        OrderUpdateEvent {
            id: id.to_string(),
            symbol: "BTC/USDT:USDT".to_string(),
            status: status.to_string(),
            side: "buy".to_string(),
            average: Some(dec!(49998.5)),
            timestamp: Some(1709280000000),
            user_id: None,
        }
    }

    /// Helper: set up an EngineHandle with a pre-populated order group.
    async fn setup_with_group(
        entry_id: &str,
        sl_id: &str,
        tp_id: &str,
        status: OrderGroupStatus,
    ) -> (EngineHandle, Arc<MockExchangeApi>, Uuid) {
        let mut engine = ShadowEngine::new();

        // Pre-populate group via direct engine access before spawning actor
        let user_id = Uuid::new_v4();
        let entry_order = Uuid::new_v4();
        let mut group = OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_order, dec!(0.1));
        group.exchange_order_id = Some(entry_id.to_string());
        group.exchange_sl_order_id = Some(sl_id.to_string());
        group.exchange_tp_order_id = Some(tp_id.to_string());
        group.status = status;
        if status == OrderGroupStatus::Active {
            group.entry_price = Some(dec!(50000));
        }

        let added = engine.order_groups.add_group(group);
        let gid = added.id;
        engine.order_groups.register_exchange_order(entry_id.to_string(), gid);
        engine.order_groups.register_exchange_order(sl_id.to_string(), gid);
        engine.order_groups.register_exchange_order(tp_id.to_string(), gid);

        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        let mock_api = Arc::new(MockExchangeApi::new());

        // Look up the group_id via handle
        let group = handle.get_group_by_exchange_order(entry_id.to_string()).await.unwrap();
        (handle, mock_api, group.id)
    }

    #[tokio::test]
    async fn test_sl_fill_cancels_tp() {
        let (handle, mock_api, group_id) =
            setup_with_group("entry-1", "sl-1", "tp-1", OrderGroupStatus::Active).await;

        let detector = FillDetectorService::new(handle.clone(), mock_api.clone());
        detector
            .handle_order_update(make_event("sl-1", "closed"))
            .await;

        // TP should be cancelled
        assert_eq!(mock_api.cancel_count.load(Ordering::SeqCst), 1);
        let cancelled = mock_api.cancelled_ids.lock().await;
        assert_eq!(cancelled[0], "tp-1");

        // Group should be StoppedOut
        let group = handle.get_trade_group(group_id).await.unwrap();
        assert_eq!(group.status, OrderGroupStatus::StoppedOut);
    }

    #[tokio::test]
    async fn test_tp_fill_cancels_sl() {
        let (handle, mock_api, group_id) =
            setup_with_group("entry-2", "sl-2", "tp-2", OrderGroupStatus::Active).await;

        let detector = FillDetectorService::new(handle.clone(), mock_api.clone());
        detector
            .handle_order_update(make_event("tp-2", "closed"))
            .await;

        // SL should be cancelled
        assert_eq!(mock_api.cancel_count.load(Ordering::SeqCst), 1);
        let cancelled = mock_api.cancelled_ids.lock().await;
        assert_eq!(cancelled[0], "sl-2");

        // Group should be TookProfit
        let group = handle.get_trade_group(group_id).await.unwrap();
        assert_eq!(group.status, OrderGroupStatus::TookProfit);
    }

    #[tokio::test]
    async fn test_ignores_unknown_exchange_order_ids() {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        let mock_api = Arc::new(MockExchangeApi::new());
        let detector = FillDetectorService::new(handle, mock_api.clone());

        detector
            .handle_order_update(make_event("unknown-999", "closed"))
            .await;

        assert_eq!(mock_api.cancel_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_idempotent_second_fill_is_noop() {
        let (handle, mock_api, _group_id) =
            setup_with_group("entry-3", "sl-3", "tp-3", OrderGroupStatus::Active).await;

        let detector = FillDetectorService::new(handle, mock_api.clone());

        // First SL fill
        detector
            .handle_order_update(make_event("sl-3", "closed"))
            .await;
        assert_eq!(mock_api.cancel_count.load(Ordering::SeqCst), 1);

        // Second SL fill — group already StoppedOut, should be no-op
        detector
            .handle_order_update(make_event("sl-3", "closed"))
            .await;
        assert_eq!(mock_api.cancel_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_ignores_non_closed_status() {
        let (handle, mock_api, _) =
            setup_with_group("entry-4", "sl-4", "tp-4", OrderGroupStatus::Active).await;

        let detector = FillDetectorService::new(handle, mock_api.clone());

        detector
            .handle_order_update(make_event("sl-4", "open"))
            .await;
        assert_eq!(mock_api.cancel_count.load(Ordering::SeqCst), 0);

        detector
            .handle_order_update(make_event("sl-4", "partially_filled"))
            .await;
        assert_eq!(mock_api.cancel_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_entry_fill_marks_active() {
        let (handle, mock_api, group_id) =
            setup_with_group("entry-5", "sl-5", "tp-5", OrderGroupStatus::Pending).await;

        let detector = FillDetectorService::new(handle.clone(), mock_api.clone());
        detector
            .handle_order_update(make_event("entry-5", "closed"))
            .await;

        let group = handle.get_trade_group(group_id).await.unwrap();
        assert_eq!(group.status, OrderGroupStatus::Active);
        assert!(group.entry_price.is_some());

        // No cancellations for entry fill
        assert_eq!(mock_api.cancel_count.load(Ordering::SeqCst), 0);
    }

    /// E2E test: After SL order is amended (reindexed), fill on the new ID
    /// should be recognized by FillDetector and trigger OCO cancellation of TP.
    #[tokio::test]
    async fn test_amended_sl_fill_triggers_oco_cancellation() {
        let mut engine = ShadowEngine::new();

        let user_id = Uuid::new_v4();
        let entry_order = Uuid::new_v4();
        let old_sl_id = "old-sl-100";
        let new_sl_id = "new-sl-200";
        let tp_id = "tp-100";

        let mut group =
            OrderGroup::new(user_id, "BTC_USDC".to_string(), entry_order, dec!(0.1));
        group.exchange_order_id = Some("entry-100".to_string());
        group.exchange_sl_order_id = Some(old_sl_id.to_string());
        group.exchange_tp_order_id = Some(tp_id.to_string());
        group.status = OrderGroupStatus::Active;
        group.entry_price = Some(dec!(50000));

        let added = engine.order_groups.add_group(group);
        let gid = added.id;
        engine.order_groups.register_exchange_order("entry-100".to_string(), gid);
        engine.order_groups.register_exchange_order(old_sl_id.to_string(), gid);
        engine.order_groups.register_exchange_order(tp_id.to_string(), gid);

        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        let mock_api = Arc::new(MockExchangeApi::new());

        // Simulate amend: reindex from old SL ID to new SL ID
        assert!(handle.reindex_exchange_sl_order(old_sl_id.to_string(), new_sl_id.to_string()).await);

        // Now FillDetector receives a fill on the NEW SL ID
        let detector = FillDetectorService::new(handle.clone(), mock_api.clone());
        detector
            .handle_order_update(make_event(new_sl_id, "closed"))
            .await;

        // TP should be cancelled (OCO)
        assert_eq!(mock_api.cancel_count.load(Ordering::SeqCst), 1);
        let cancelled = mock_api.cancelled_ids.lock().await;
        assert_eq!(cancelled[0], tp_id);

        // Group should be StoppedOut
        let group = handle.get_trade_group(gid).await.unwrap();
        assert_eq!(group.status, OrderGroupStatus::StoppedOut);
    }

    /// Verify that a fill on the OLD SL ID (pre-amend) is ignored after reindex.
    #[tokio::test]
    async fn test_old_sl_id_ignored_after_reindex() {
        let (handle, mock_api, _gid) =
            setup_with_group("entry-6", "old-sl-6", "tp-6", OrderGroupStatus::Active).await;

        // Reindex SL
        handle.reindex_exchange_sl_order("old-sl-6".to_string(), "new-sl-6".to_string()).await;

        let detector = FillDetectorService::new(handle, mock_api.clone());

        // Fill on OLD SL ID — should be unknown now
        detector
            .handle_order_update(make_event("old-sl-6", "closed"))
            .await;

        assert_eq!(
            mock_api.cancel_count.load(Ordering::SeqCst),
            0,
            "Old SL ID should not trigger OCO after reindex"
        );
    }

    /// 017 FR-7: Verify that 1024+ events sent through the mpsc fill channel
    /// are all delivered without loss.
    #[tokio::test]
    async fn test_mpsc_fill_channel_delivers_all_events() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<OrderUpdateEvent>(1024);
        let event_count: usize = 2048;

        let producer = tokio::spawn(async move {
            for i in 0..event_count {
                let event = OrderUpdateEvent {
                    id: format!("order-{}", i),
                    symbol: "BTC/USDT:USDT".to_string(),
                    status: "closed".to_string(),
                    side: "buy".to_string(),
                    average: Some(dec!(50000)),
                    timestamp: Some(1709280000000 + i as i64),
                    user_id: None,
                };
                tx.send(event).await.expect("mpsc send should not fail");
            }
        });

        let consumer = tokio::spawn(async move {
            let mut received = 0usize;
            while let Some(_event) = rx.recv().await {
                received += 1;
                if received == event_count {
                    break;
                }
            }
            received
        });

        producer.await.expect("producer task should complete");
        let received = consumer.await.expect("consumer task should complete");
        assert_eq!(
            received, event_count,
            "All {} events must be delivered, got {}",
            event_count, received
        );
    }

}
