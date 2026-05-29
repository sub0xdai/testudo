//! 020: Integration Test Infrastructure
//!
//! Provides shared test infrastructure for cross-service integration tests:
//! - `StatefulMockExchangeApi`: simulates exchange state (open orders, positions)
//! - Test helpers for actor setup and group creation
//!
//! All code in this module is `#[cfg(test)]` only — no production impact.

// @anchor exchange:router:integration_tests
// @tags api

use crate::services::exchange_api::{
    AmendRequest, ApiOrderType, ExchangeApi, ExchangeApiError, OrderSide, PlaceOrderRequest,
    PlaceOrderResult, PositionInfo,
};
use async_trait::async_trait;
use engine::shadow::actor::FillEvent;
use engine::shadow::order_group::{OrderGroup, OrderGroupStatus};
use engine::shadow::trade_event::TradeEvent;
use engine::{EngineActor, EngineHandle, ShadowEngine};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// PlacedOrder — internal record of an order placed via the mock
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PlacedOrder {
    id: String,
    symbol: String,
    side: OrderSide,
    order_type: ApiOrderType,
    quantity: Decimal,
    price: Option<Decimal>,
}

// ---------------------------------------------------------------------------
// StatefulMockExchangeApi (FR-1, FR-2, FR-3, FR-4)
// ---------------------------------------------------------------------------

/// A mock exchange that maintains realistic in-memory state.
///
/// Unlike `fill_detector::tests::MockExchangeApi` (which only counts cancels),
/// this mock tracks open orders, positions, and provides inspection methods
/// for assertions in integration tests.
pub(crate) struct StatefulMockExchangeApi {
    open_orders: tokio::sync::Mutex<HashMap<String, PlacedOrder>>,
    positions: tokio::sync::Mutex<HashMap<String, PositionInfo>>,
    cancel_log: tokio::sync::Mutex<Vec<String>>,
    cancel_all_log: tokio::sync::Mutex<Vec<String>>,
    place_log: tokio::sync::Mutex<Vec<PlaceOrderRequest>>,
    next_id: AtomicUsize,
}

impl StatefulMockExchangeApi {
    pub fn new() -> Self {
        Self {
            open_orders: tokio::sync::Mutex::new(HashMap::new()),
            positions: tokio::sync::Mutex::new(HashMap::new()),
            cancel_log: tokio::sync::Mutex::new(Vec::new()),
            cancel_all_log: tokio::sync::Mutex::new(Vec::new()),
            place_log: tokio::sync::Mutex::new(Vec::new()),
            next_id: AtomicUsize::new(1),
        }
    }

    // --- FR-3: Inspection methods ---

    pub async fn cancelled_ids(&self) -> Vec<String> {
        self.cancel_log.lock().await.clone()
    }

    pub async fn has_open_order(&self, id: &str) -> bool {
        self.open_orders.lock().await.contains_key(id)
    }

    pub async fn open_order_count(&self) -> usize {
        self.open_orders.lock().await.len()
    }

    pub async fn placed_ids(&self) -> Vec<String> {
        self.place_log
            .lock()
            .await
            .iter()
            .map(|_| {
                // Place log doesn't store returned IDs; use open_orders keys instead
                // This returns the symbols for place log inspection
                String::new()
            })
            .collect()
    }

    pub async fn placed_requests(&self) -> Vec<PlaceOrderRequest> {
        self.place_log.lock().await.clone()
    }

    pub async fn cancel_all_symbols(&self) -> Vec<String> {
        self.cancel_all_log.lock().await.clone()
    }

    // --- FR-4: State injection methods ---

    pub async fn inject_position(
        &self,
        symbol: &str,
        side: &str,
        quantity: Decimal,
        entry_price: Decimal,
    ) {
        self.positions.lock().await.insert(
            symbol.to_string(),
            PositionInfo {
                symbol: symbol.to_string(),
                side: side.to_string(),
                quantity,
                entry_price,
                unrealized_pnl: Decimal::ZERO,
            },
        );
    }

    pub async fn remove_position(&self, symbol: &str) {
        self.positions.lock().await.remove(symbol);
    }

    /// Inject an open order directly (for test setup, bypassing place_order).
    pub async fn inject_open_order(&self, id: &str, symbol: &str) {
        self.open_orders.lock().await.insert(
            id.to_string(),
            PlacedOrder {
                id: id.to_string(),
                symbol: symbol.to_string(),
                side: OrderSide::Buy,
                order_type: ApiOrderType::Limit,
                quantity: Decimal::ONE,
                price: None,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// ExchangeApi trait implementation (FR-2)
// ---------------------------------------------------------------------------

#[async_trait]
impl ExchangeApi for StatefulMockExchangeApi {
    async fn get_balance(
        &self,
        _user_id: Uuid,
        _asset: &str,
        _exchange_account_id: Option<Uuid>,
    ) -> Result<Decimal, ExchangeApiError> {
        Ok(Decimal::new(10000, 0))
    }

    async fn place_order(&self, req: PlaceOrderRequest) -> Result<PlaceOrderResult, ExchangeApiError> {
        let id = format!("mock-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let order = PlacedOrder {
            id: id.clone(),
            symbol: req.symbol.clone(),
            side: req.side.clone(),
            order_type: req.order_type.clone(),
            quantity: req.quantity,
            price: req.price,
        };
        self.open_orders.lock().await.insert(id.clone(), order);
        self.place_log.lock().await.push(req);
        Ok(PlaceOrderResult {
            id,
            status: Some("open".to_string()),
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
        Ok(format!(
            "mock-amended-{}",
            self.next_id.fetch_add(1, Ordering::SeqCst)
        ))
    }

    async fn cancel_order(
        &self,
        _user_id: Uuid,
        order_id: &str,
        _symbol: &str,
        _exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        let removed = self.open_orders.lock().await.remove(order_id);
        self.cancel_log.lock().await.push(order_id.to_string());
        if removed.is_none() {
            return Err(ExchangeApiError::OrderNotFound(order_id.to_string()));
        }
        Ok(())
    }

    async fn cancel_all_orders(
        &self,
        _user_id: Uuid,
        symbol: &str,
        _exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        let mut orders = self.open_orders.lock().await;
        orders.retain(|_, o| o.symbol != symbol);
        self.cancel_all_log
            .lock()
            .await
            .push(symbol.to_string());
        Ok(())
    }

    async fn get_position(
        &self,
        _user_id: Uuid,
        symbol: &str,
        _exchange_account_id: Option<Uuid>,
    ) -> Result<Option<PositionInfo>, ExchangeApiError> {
        Ok(self.positions.lock().await.get(symbol).cloned())
    }
}

// ---------------------------------------------------------------------------
// Shared test helpers (FR-7)
// ---------------------------------------------------------------------------

/// Spawn a test actor and return the handle plus both receiver channels.
pub(crate) fn setup_test_actor() -> (
    EngineHandle,
    mpsc::Receiver<FillEvent>,
    mpsc::Receiver<TradeEvent>,
) {
    let engine = ShadowEngine::new();
    EngineActor::spawn(engine)
}

/// Create an Active order group pre-populated with exchange IDs.
///
/// Returns the group ID. The group is added directly to the engine before
/// spawning the actor, following the pattern from `fill_detector::tests::setup_with_group`.
pub(crate) fn setup_actor_with_active_group(
    entry_id: &str,
    sl_id: &str,
    tp_id: &str,
) -> (
    EngineHandle,
    mpsc::Receiver<FillEvent>,
    mpsc::Receiver<TradeEvent>,
    Uuid,
    Uuid,
) {
    let mut engine = ShadowEngine::new();
    let user_id = Uuid::new_v4();
    let entry_order = Uuid::new_v4();

    let mut group = OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_order, Decimal::new(1, 1));
    group.exchange_order_id = Some(entry_id.to_string());
    group.exchange_sl_order_id = Some(sl_id.to_string());
    group.exchange_tp_order_id = Some(tp_id.to_string());
    group.status = OrderGroupStatus::Active;
    group.entry_price = Some(Decimal::new(50000, 0));

    let added = engine.order_groups.add_group(group);
    let gid = added.id;
    engine
        .order_groups
        .register_exchange_order(entry_id.to_string(), gid);
    engine
        .order_groups
        .register_exchange_order(sl_id.to_string(), gid);
    engine
        .order_groups
        .register_exchange_order(tp_id.to_string(), gid);

    let (handle, fill_rx, trade_event_rx) = EngineActor::spawn(engine);
    (handle, fill_rx, trade_event_rx, gid, user_id)
}

/// Create a Pending order group pre-populated with exchange IDs.
pub(crate) fn setup_actor_with_pending_group(
    entry_id: &str,
    sl_id: &str,
    tp_id: &str,
) -> (
    EngineHandle,
    mpsc::Receiver<FillEvent>,
    mpsc::Receiver<TradeEvent>,
    Uuid,
    Uuid,
) {
    let mut engine = ShadowEngine::new();
    let user_id = Uuid::new_v4();
    let entry_order = Uuid::new_v4();

    let mut group = OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_order, Decimal::new(1, 1));
    group.exchange_order_id = Some(entry_id.to_string());
    group.exchange_sl_order_id = Some(sl_id.to_string());
    group.exchange_tp_order_id = Some(tp_id.to_string());
    group.status = OrderGroupStatus::Pending;

    let added = engine.order_groups.add_group(group);
    let gid = added.id;
    engine
        .order_groups
        .register_exchange_order(entry_id.to_string(), gid);
    engine
        .order_groups
        .register_exchange_order(sl_id.to_string(), gid);
    engine
        .order_groups
        .register_exchange_order(tp_id.to_string(), gid);

    let (handle, fill_rx, trade_event_rx) = EngineActor::spawn(engine);
    (handle, fill_rx, trade_event_rx, gid, user_id)
}

// ---------------------------------------------------------------------------
// Basic sanity tests for StatefulMockExchangeApi
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_stateful_mock_place_and_cancel() {
        let mock = StatefulMockExchangeApi::new();
        let user_id = Uuid::new_v4();

        // Place an order
        let result = mock
            .place_order(PlaceOrderRequest {
                user_id,
                symbol: "BTC_USDT".to_string(),
                side: OrderSide::Buy,
                order_type: ApiOrderType::Limit,
                quantity: dec!(0.1),
                price: Some(dec!(50000)),
                stop_price: None,
                leverage: 1,
                exchange_account_id: None,
                reduce_only: false,
                client_order_id: None,
                stop_loss_trigger: None,
                take_profit_trigger: None,
            })
            .await
            .unwrap();
        let id = result.id;

        assert!(mock.has_open_order(&id).await);
        assert_eq!(mock.open_order_count().await, 1);

        // Cancel it
        mock.cancel_order(user_id, &id, "BTC_USDT", None)
            .await
            .unwrap();

        assert!(!mock.has_open_order(&id).await);
        assert_eq!(mock.open_order_count().await, 0);
        assert_eq!(mock.cancelled_ids().await, vec![id]);
    }

    #[tokio::test]
    async fn test_stateful_mock_cancel_not_found() {
        let mock = StatefulMockExchangeApi::new();
        let user_id = Uuid::new_v4();

        let result = mock
            .cancel_order(user_id, "nonexistent", "BTC_USDT", None)
            .await;
        assert!(matches!(result, Err(ExchangeApiError::OrderNotFound(_))));
    }

    #[tokio::test]
    async fn test_stateful_mock_cancel_all() {
        let mock = StatefulMockExchangeApi::new();
        let user_id = Uuid::new_v4();

        // Place two BTC orders and one ETH order
        for symbol in ["BTC_USDT", "BTC_USDT", "ETH_USDT"] {
            mock.place_order(PlaceOrderRequest {
                user_id,
                symbol: symbol.to_string(),
                side: OrderSide::Buy,
                order_type: ApiOrderType::Limit,
                quantity: dec!(0.1),
                price: Some(dec!(50000)),
                stop_price: None,
                leverage: 1,
                exchange_account_id: None,
                reduce_only: false,
                client_order_id: None,
                stop_loss_trigger: None,
                take_profit_trigger: None,
            })
            .await
            .unwrap();
        }

        assert_eq!(mock.open_order_count().await, 3);

        // Cancel all BTC orders
        mock.cancel_all_orders(user_id, "BTC_USDT", None)
            .await
            .unwrap();

        assert_eq!(mock.open_order_count().await, 1);
        assert_eq!(mock.cancel_all_symbols().await, vec!["BTC_USDT"]);
    }

    #[tokio::test]
    async fn test_stateful_mock_position_inject_and_remove() {
        let mock = StatefulMockExchangeApi::new();
        let user_id = Uuid::new_v4();

        // No position initially
        let pos = mock
            .get_position(user_id, "BTC_USDT", None)
            .await
            .unwrap();
        assert!(pos.is_none());

        // Inject position
        mock.inject_position("BTC_USDT", "long", dec!(0.5), dec!(50000))
            .await;

        let pos = mock
            .get_position(user_id, "BTC_USDT", None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(pos.quantity, dec!(0.5));
        assert_eq!(pos.side, "long");

        // Remove position
        mock.remove_position("BTC_USDT").await;
        let pos = mock
            .get_position(user_id, "BTC_USDT", None)
            .await
            .unwrap();
        assert!(pos.is_none());
    }

    #[tokio::test]
    async fn test_stateful_mock_monotonic_ids() {
        let mock = StatefulMockExchangeApi::new();
        let user_id = Uuid::new_v4();

        let r1 = mock
            .place_order(PlaceOrderRequest {
                user_id,
                symbol: "BTC_USDT".to_string(),
                side: OrderSide::Buy,
                order_type: ApiOrderType::Limit,
                quantity: dec!(0.1),
                price: Some(dec!(50000)),
                stop_price: None,
                leverage: 1,
                exchange_account_id: None,
                reduce_only: false,
                client_order_id: None,
                stop_loss_trigger: None,
                take_profit_trigger: None,
            })
            .await
            .unwrap();

        let r2 = mock
            .place_order(PlaceOrderRequest {
                user_id,
                symbol: "ETH_USDT".to_string(),
                side: OrderSide::Sell,
                order_type: ApiOrderType::Market,
                quantity: dec!(1.0),
                price: None,
                stop_price: None,
                leverage: 1,
                exchange_account_id: None,
                reduce_only: false,
                client_order_id: None,
                stop_loss_trigger: None,
                take_profit_trigger: None,
            })
            .await
            .unwrap();

        assert_ne!(r1.id, r2.id);
        assert!(r1.id.starts_with("mock-"));
        assert!(r2.id.starts_with("mock-"));
    }

    #[tokio::test]
    async fn test_setup_test_actor() {
        let (handle, _fill_rx, _trade_rx) = setup_test_actor();
        // Actor should be alive and respond
        let exists = handle.user_exists(Uuid::new_v4()).await;
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_setup_actor_with_active_group() {
        let (handle, _fill_rx, _trade_rx, gid, _user_id) =
            setup_actor_with_active_group("entry-1", "sl-1", "tp-1");

        let group = handle.get_trade_group(gid).await.unwrap();
        assert_eq!(group.status, OrderGroupStatus::Active);
        assert_eq!(group.exchange_order_id.as_deref(), Some("entry-1"));
        assert_eq!(group.exchange_sl_order_id.as_deref(), Some("sl-1"));
        assert_eq!(group.exchange_tp_order_id.as_deref(), Some("tp-1"));
    }

    #[tokio::test]
    async fn test_setup_actor_with_pending_group() {
        let (handle, _fill_rx, _trade_rx, gid, _user_id) =
            setup_actor_with_pending_group("entry-2", "sl-2", "tp-2");

        let group = handle.get_trade_group(gid).await.unwrap();
        assert_eq!(group.status, OrderGroupStatus::Pending);
    }

    // =========================================================================
    // 021: Lifecycle Integration Tests (FR-1 through FR-7)
    // =========================================================================

    use crate::services::cex_client::{OrderUpdateEvent, SidecarOpenOrderResponse};
    use crate::services::fill_detector::FillDetectorService;
    use crate::services::reconciliation::determine_reconcile_actions;
    use engine::{OrderRole, ShadowOrder};
    use std::collections::HashSet;
    use std::sync::Arc;

    fn make_order_event(id: &str, status: &str) -> OrderUpdateEvent {
        OrderUpdateEvent {
            id: id.to_string(),
            symbol: "BTC/USDT:USDT".to_string(),
            status: status.to_string(),
            side: "buy".to_string(),
            average: Some(dec!(50000)),
            timestamp: Some(1709280000000),
            user_id: None,
        }
    }

    /// FR-1: SL fill via channel exercises the full run() loop with tokio::select!
    #[tokio::test]
    async fn test_fill_detector_sl_fill_via_channel() {
        let (handle, fill_rx, _trade_rx, gid, _user_id) =
            setup_actor_with_active_group("exch-entry-1", "exch-sl-1", "exch-tp-1");

        let mock = Arc::new(StatefulMockExchangeApi::new());
        mock.inject_open_order("exch-sl-1", "BTC_USDT").await;
        mock.inject_open_order("exch-tp-1", "BTC_USDT").await;

        let detector = FillDetectorService::new(handle.clone(), mock.clone());
        let (order_tx, order_rx) = mpsc::channel::<OrderUpdateEvent>(16);

        tokio::spawn(async move {
            detector.run(order_rx, fill_rx).await;
        });

        order_tx
            .send(make_order_event("exch-sl-1", "closed"))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let cancelled = mock.cancelled_ids().await;
        assert!(
            cancelled.contains(&"exch-tp-1".to_string()),
            "TP should be cancelled after SL fill via channel"
        );
        assert!(!mock.has_open_order("exch-tp-1").await);

        let group = handle.get_trade_group(gid).await.unwrap();
        assert_eq!(group.status, OrderGroupStatus::StoppedOut);
    }

    /// FR-2: Entry cancelled via channel triggers cleanup of SL and TP
    #[tokio::test]
    async fn test_fill_detector_entry_cancelled_via_channel() {
        let (handle, fill_rx, _trade_rx, gid, _user_id) =
            setup_actor_with_pending_group("exch-entry-2", "exch-sl-2", "exch-tp-2");

        let mock = Arc::new(StatefulMockExchangeApi::new());
        mock.inject_open_order("exch-entry-2", "BTC_USDT").await;
        mock.inject_open_order("exch-sl-2", "BTC_USDT").await;
        mock.inject_open_order("exch-tp-2", "BTC_USDT").await;

        let detector = FillDetectorService::new(handle.clone(), mock.clone());
        let (order_tx, order_rx) = mpsc::channel::<OrderUpdateEvent>(16);

        tokio::spawn(async move {
            detector.run(order_rx, fill_rx).await;
        });

        order_tx
            .send(make_order_event("exch-entry-2", "cancelled"))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let cancelled = mock.cancelled_ids().await;
        assert!(
            cancelled.contains(&"exch-sl-2".to_string()),
            "SL should be cancelled after entry cancellation"
        );
        assert!(
            cancelled.contains(&"exch-tp-2".to_string()),
            "TP should be cancelled after entry cancellation"
        );

        let group = handle.get_trade_group(gid).await.unwrap();
        assert_eq!(group.status, OrderGroupStatus::Cancelled);
    }

    /// FR-3: Shadow engine fill triggers exchange cancellation via fill_rx channel.
    /// Most architecturally significant test — exercises the dual-channel design
    /// where shadow engine fills trigger exchange cleanup via fire-and-forget events.
    #[tokio::test]
    async fn test_fill_detector_shadow_fill_cancels_exchange() {
        // 1. Create actor with fresh shadow engine
        let engine = ShadowEngine::new();
        let (handle, fill_rx, _trade_rx) = EngineActor::spawn(engine);

        // 2. Init user with default balance (10000 USDT)
        let user_id = Uuid::new_v4();
        handle.init_user(user_id).await.unwrap();

        // 3. Place entry order: limit buy BTC at 50000, SL=49000, TP=52000
        let mut order = ShadowOrder::limit_buy(user_id, "BTC_USDT", dec!(0.0001), dec!(50000))
            .with_stop_loss(dec!(49000))
            .with_take_profit(dec!(52000));
        order.mark_risk_validated();
        let placed = handle.place_order(user_id, order).await.unwrap();

        // 4. Fill entry via price update (ask=50000 triggers limit buy at 50000)
        //    Uses reply version — does NOT emit to fill_rx
        let _result = handle
            .process_price_update(
                "BTC_USDT".to_string(),
                dec!(50000),
                dec!(50000),
                dec!(50000),
                dec!(50000),
            )
            .await
            .unwrap();

        // 5. Get group and register exchange TP order ID
        let group = handle
            .get_group_by_entry_order(placed.id)
            .await
            .expect("group should exist after entry fill");
        let group_id = group.id;
        assert_eq!(group.status, OrderGroupStatus::Active);

        handle
            .register_exchange_order_id(
                group_id,
                OrderRole::TakeProfit,
                "exch-tp-3".to_string(),
            )
            .await
            .unwrap();

        // 6. Create mock with TP as open order
        let mock = Arc::new(StatefulMockExchangeApi::new());
        mock.inject_open_order("exch-tp-3", "BTC_USDT").await;

        // 7. Spawn FillDetector listening on both channels
        let detector = FillDetectorService::new(handle.clone(), mock.clone());
        let (_order_tx, order_rx) = mpsc::channel::<OrderUpdateEvent>(16);
        tokio::spawn(async move {
            detector.run(order_rx, fill_rx).await;
        });

        // 8. Push price that triggers SL (bid=48000 <= stop_price=49000)
        //    Uses fire-and-forget — emits FillEvent to fill_rx
        handle
            .push_price(
                "BTC_USDT".to_string(),
                dec!(48000),
                dec!(48000),
                dec!(48000),
                dec!(48000),
            )
            .await
            .unwrap();

        // 9. Wait for fill_rx delivery and FillDetector processing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // 10. Assert TP was cancelled on the exchange
        let cancelled = mock.cancelled_ids().await;
        assert!(
            cancelled.contains(&"exch-tp-3".to_string()),
            "TP exchange order should be cancelled after shadow SL fill"
        );
        assert!(!mock.has_open_order("exch-tp-3").await);
    }

    /// FR-4: Reconciliation detects orphaned TP after SL filled off-line
    #[tokio::test]
    async fn test_reconciliation_orphaned_tp_after_sl_fill() {
        let user_id = Uuid::new_v4();
        let entry_order = Uuid::new_v4();

        let mut group =
            OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_order, dec!(0.1));
        group.status = OrderGroupStatus::Active;
        group.entry_price = Some(dec!(50000));
        group.exchange_sl_order_id = Some("sl-1".to_string());
        group.exchange_tp_order_id = Some("tp-1".to_string());
        // Backdate past 60s grace period
        group.created_at = chrono::Utc::now() - chrono::Duration::seconds(120);

        // Exchange state: TP still open, SL gone, no position
        let open_order_ids: HashSet<String> = ["tp-1".to_string()].into_iter().collect();
        let symbols_with_position: HashSet<String> = HashSet::new();

        let actions =
            determine_reconcile_actions(&[group], &open_order_ids, &symbols_with_position, &[]);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].new_status, OrderGroupStatus::StoppedOut);
        assert_eq!(actions[0].orders_to_cancel, vec!["tp-1"]);
    }

    /// Grace period: Active groups younger than 60s are not reconcile-closed
    #[tokio::test]
    async fn test_reconciliation_grace_period_skips_young_active_groups() {
        let user_id = Uuid::new_v4();
        let entry_order = Uuid::new_v4();

        let mut group =
            OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_order, dec!(0.1));
        group.status = OrderGroupStatus::Active;
        group.exchange_sl_order_id = Some("sl-young".to_string());
        group.exchange_tp_order_id = Some("tp-young".to_string());
        // created_at defaults to now — within grace period

        // Exchange state: no orders, no position — would normally trigger reconciled_closed
        let open_order_ids: HashSet<String> = HashSet::new();
        let symbols_with_position: HashSet<String> = HashSet::new();

        let actions =
            determine_reconcile_actions(&[group], &open_order_ids, &symbols_with_position, &[]);

        assert_eq!(actions.len(), 0, "Young active group should be skipped by grace period");
    }

    /// Symbol format: has_position uses backend format (BTC_USDT), not CEX format
    #[tokio::test]
    async fn test_reconciliation_has_position_uses_backend_symbol_format() {
        let user_id = Uuid::new_v4();
        let entry_order = Uuid::new_v4();

        let mut group =
            OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_order, dec!(0.1));
        group.status = OrderGroupStatus::Active;
        group.exchange_sl_order_id = Some("sl-sym".to_string());
        group.exchange_tp_order_id = Some("tp-sym".to_string());
        group.created_at = chrono::Utc::now() - chrono::Duration::seconds(120);

        let open_order_ids: HashSet<String> = HashSet::new();
        // Position in backend format (as returned by sidecar)
        let symbols_with_position: HashSet<String> =
            ["BTC_USDT".to_string()].into_iter().collect();

        let actions =
            determine_reconcile_actions(&[group], &open_order_ids, &symbols_with_position, &[]);

        assert_eq!(actions.len(), 0, "Group with matching position should be skipped");
    }

    /// FR-5: Reconciliation detects pending entry that vanished from exchange
    #[tokio::test]
    async fn test_reconciliation_pending_entry_gone() {
        let user_id = Uuid::new_v4();
        let entry_order = Uuid::new_v4();

        let mut group =
            OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_order, dec!(0.1));
        group.status = OrderGroupStatus::Pending;
        group.exchange_order_id = Some("entry-1".to_string());
        group.exchange_sl_order_id = Some("sl-1".to_string());
        group.exchange_tp_order_id = Some("tp-1".to_string());

        // Exchange state: entry NOT in open orders, SL and TP still open
        let open_order_ids: HashSet<String> =
            ["sl-1".to_string(), "tp-1".to_string()].into_iter().collect();
        let symbols_with_position: HashSet<String> = HashSet::new();

        // Backdate past the 60s grace period so reconciliation actually processes
        // the group (grace period prevents canceling newly-placed SL/TP orders).
        group.created_at = chrono::Utc::now() - chrono::Duration::seconds(120);

        let actions =
            determine_reconcile_actions(&[group], &open_order_ids, &symbols_with_position, &[]);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].new_status, OrderGroupStatus::Cancelled);
        assert!(actions[0].orders_to_cancel.contains(&"sl-1".to_string()));
        assert!(actions[0].orders_to_cancel.contains(&"tp-1".to_string()));
    }

    /// FR-6: Reconciliation recovers zombie group by matching clientOrderId
    #[tokio::test]
    async fn test_reconciliation_zombie_recovery() {
        let user_id = Uuid::new_v4();
        let entry_order = Uuid::new_v4();

        let mut group =
            OrderGroup::new(user_id, "BTC_USDT".to_string(), entry_order, dec!(0.1));
        group.status = OrderGroupStatus::AwaitingReconciliation;
        let group_id = group.id;

        // Exchange has the order with matching clientOrderId convention
        let open_orders = vec![SidecarOpenOrderResponse {
            id: "found-order-1".to_string(),
            client_order_id: Some(crate::services::numeric_client_order_id(group_id, 1)),
            symbol: Some("BTC/USDT:USDT".to_string()),
            status: Some("open".to_string()),
            side: Some("buy".to_string()),
            order_type: Some("limit".to_string()),
            price: Some("50000".to_string()),
            stop_price: None,
            amount: Some("0.1".to_string()),
            filled: Some("0".to_string()),
            remaining: Some("0.1".to_string()),
            timestamp: Some(1709280000000),
        }];

        let open_order_ids: HashSet<String> =
            ["found-order-1".to_string()].into_iter().collect();
        let symbols_with_position: HashSet<String> = HashSet::new();

        let actions = determine_reconcile_actions(
            &[group],
            &open_order_ids,
            &symbols_with_position,
            &open_orders,
        );

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].new_status, OrderGroupStatus::Pending);
        assert!(actions[0].orders_to_cancel.is_empty());
        assert_eq!(
            actions[0].recovered_exchange_id.as_deref(),
            Some("found-order-1")
        );
    }

    /// FR-7: Concurrent fill and cancel converge to terminal state without panics.
    /// Both FillDetector (via SL fill event) and a direct user cancel race to
    /// cancel the TP. Idempotent OrderNotFound from double-cancel is expected.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_concurrent_fill_and_cancel_converges() {
        let (handle, fill_rx, _trade_rx, gid, user_id) =
            setup_actor_with_active_group("exch-entry-7", "exch-sl-7", "exch-tp-7");

        let mock = Arc::new(StatefulMockExchangeApi::new());
        mock.inject_open_order("exch-sl-7", "BTC_USDT").await;
        mock.inject_open_order("exch-tp-7", "BTC_USDT").await;

        let detector = FillDetectorService::new(handle.clone(), mock.clone());
        let (order_tx, order_rx) = mpsc::channel::<OrderUpdateEvent>(16);

        tokio::spawn(async move {
            detector.run(order_rx, fill_rx).await;
        });

        // Concurrently: (a) send SL fill via channel, (b) cancel TP directly
        let tx = order_tx.clone();
        let m = mock.clone();

        tokio::join!(
            async move {
                tx.send(make_order_event("exch-sl-7", "closed"))
                    .await
                    .unwrap();
            },
            async move {
                let _ = m
                    .cancel_order(user_id, "exch-tp-7", "BTC_USDT", None)
                    .await;
            },
        );

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Group should be in terminal state (StoppedOut from SL fill)
        let group = handle.get_trade_group(gid).await.unwrap();
        assert!(
            group.status.is_terminal(),
            "Group should be in terminal state, got {:?}",
            group.status
        );
        assert_eq!(group.status, OrderGroupStatus::StoppedOut);

        // TP cancel attempted at most twice (once from FillDetector, once direct)
        let cancelled = mock.cancelled_ids().await;
        let tp_cancel_count = cancelled
            .iter()
            .filter(|id| *id == "exch-tp-7")
            .count();
        assert!(
            (1..=2).contains(&tp_cancel_count),
            "TP should be cancelled 1-2 times (got {})",
            tp_cancel_count
        );
    }
}

