//! Trade Manager Service
//!
//! Core service that monitors managed positions against price ticks
//! and executes management actions (break-even, trailing stop, partial TP).

use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

use super::evaluator;
use super::repository::PositionRepository;
use super::types::*;
use crate::services::exchange_api::{
    AmendRequest, ApiOrderType, ExchangeApi, ExchangeApiError, OrderSide, PlaceOrderRequest,
    PlaceOrderResult,
};
use crate::services::price_feed::PriceTick;
use engine::EngineHandle;

/// EXT-16 FR-4: Management event emitted when automated actions execute.
#[derive(Debug, Clone)]
pub struct ManagementEvent {
    pub user_id: Uuid,
    pub event_type: String,
    pub symbol: String,
    pub detail: String,
}

/// Trade manager service that monitors positions and executes management rules.
pub struct TradeManagerService {
    positions: RwLock<HashMap<Uuid, ManagedPosition>>,
    exchange_api: Arc<dyn ExchangeApi>,
    repository: Option<PositionRepository>,
    debounce_interval: Duration,
    last_amend: RwLock<HashMap<Uuid, Instant>>,
    /// EXT-16 FR-4 / AUD-03 FR-6: Bounded channel for publishing management events.
    event_tx: Option<mpsc::Sender<ManagementEvent>>,
    /// 019e: EngineHandle for reindexing SL order IDs after amend (replaces direct OrderGroupManager access).
    engine_handle: Option<EngineHandle>,
}

impl TradeManagerService {
    pub fn new(exchange_api: Arc<dyn ExchangeApi>, repository: Option<PositionRepository>) -> Self {
        Self {
            positions: RwLock::new(HashMap::new()),
            exchange_api,
            repository,
            debounce_interval: Duration::from_secs(5),
            last_amend: RwLock::new(HashMap::new()),
            event_tx: None,
            engine_handle: None,
        }
    }

    /// Set event sender for management event publishing (EXT-16 FR-4).
    pub fn with_event_sender(mut self, tx: mpsc::Sender<ManagementEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// 019e: Set EngineHandle for SL order ID reindexing after amend.
    pub fn with_engine_handle(mut self, handle: EngineHandle) -> Self {
        self.engine_handle = Some(handle);
        self
    }

    /// Reindex the exchange SL order ID via EngineHandle after an amend.
    /// Ensures FillDetector can find the group when the amended SL fills.
    async fn sync_sl_order_id(&self, old_id: &str, new_id: &str) {
        if let Some(ref handle) = self.engine_handle {
            if handle.reindex_exchange_sl_order(old_id.to_string(), new_id.to_string()).await {
                tracing::info!(
                    old_id = %old_id,
                    new_id = %new_id,
                    "TradeManager: reindexed SL order ID via EngineHandle"
                );
            } else {
                tracing::warn!(
                    old_id = %old_id,
                    "TradeManager: old SL order ID not found in OrderGroupManager index"
                );
            }
        }
    }

    /// Publish a management event (non-blocking, drops on full channel — AUD-03 FR-7).
    fn emit_event(&self, user_id: Uuid, event_type: &str, symbol: &str, detail: &str) {
        if let Some(ref tx) = self.event_tx {
            if let Err(e) = tx.try_send(ManagementEvent {
                user_id,
                event_type: event_type.to_string(),
                symbol: symbol.to_string(),
                detail: detail.to_string(),
            }) {
                tracing::warn!("Management event channel full, dropping event: {}", e);
            }
        }
    }

    /// Load active positions from the database on startup (FR-9).
    pub async fn load_from_db(&self) -> Result<usize, String> {
        if let Some(ref repo) = self.repository {
            match repo.load_active().await {
                Ok(positions) => {
                    let count = positions.len();
                    let mut map = self.positions.write().await;
                    for pos in positions {
                        map.insert(pos.id, pos);
                    }
                    tracing::info!("TradeManager loaded {} active positions from DB", count);
                    Ok(count)
                }
                Err(e) => {
                    tracing::error!("TradeManager failed to load from DB: {}", e);
                    Err(e.to_string())
                }
            }
        } else {
            Ok(0)
        }
    }

    /// EXT-21: Place an order on the exchange via the underlying ExchangeApi.
    /// Used by create_trade to send the initial limit order to the live exchange.
    pub async fn place_order(&self, req: PlaceOrderRequest) -> Result<PlaceOrderResult, ExchangeApiError> {
        self.exchange_api.place_order(req).await
    }

    /// EXT-21: Get balance from the live exchange for position sizing.
    pub async fn get_balance(
        &self,
        user_id: Uuid,
        asset: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Decimal, ExchangeApiError> {
        self.exchange_api
            .get_balance(user_id, asset, exchange_account_id)
            .await
    }

    /// EXT-21: Cancel an order on the exchange.
    pub async fn cancel_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        self.exchange_api
            .cancel_order(user_id, order_id, symbol, exchange_account_id)
            .await
    }

    /// Get the exchange position for a symbol.
    pub async fn get_exchange_position(
        &self,
        user_id: Uuid,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Option<crate::services::exchange_api::PositionInfo>, ExchangeApiError> {
        self.exchange_api
            .get_position(user_id, symbol, exchange_account_id)
            .await
    }

    /// Cancel ALL open orders for a symbol. Defense-in-depth fallback.
    pub async fn cancel_all_orders(
        &self,
        user_id: Uuid,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        self.exchange_api
            .cancel_all_orders(user_id, symbol, exchange_account_id)
            .await
    }

    /// Register a new managed position.
    pub async fn register(&self, position: ManagedPosition) -> Result<(), String> {
        let id = position.id;

        // Persist to DB first
        if let Some(ref repo) = self.repository {
            repo.insert(&position)
                .await
                .map_err(|e| format!("Failed to persist position: {}", e))?;
        }

        let mut map = self.positions.write().await;
        map.insert(id, position);
        tracing::info!("TradeManager registered position {}", id);
        Ok(())
    }

    /// Process a price tick: evaluate rules for matching positions and execute actions.
    pub async fn process_tick(&self, tick: &PriceTick) {
        // Use mid price for evaluation
        let current_price = (tick.bid + tick.ask) / Decimal::from(2);

        // Read positions matching this symbol
        let matching: Vec<ManagedPosition> = {
            let positions = self.positions.read().await;
            positions
                .values()
                .filter(|p| p.symbol == tick.symbol)
                .cloned()
                .collect()
        };

        for position in matching {
            if position.state == PositionState::Pending {
                self.promote_pending_if_filled(&position).await;
            }

            let position = if let Some(updated) = self.get_position(position.id).await {
                updated
            } else {
                continue;
            };

            let actions = evaluator::evaluate(&position, current_price);
            if actions.is_empty() {
                continue;
            }

            // Check debounce
            if self.is_debounced(position.id).await {
                continue;
            }

            let mut any_executed = false;
            for action in actions {
                match self.execute_action(&position, &action).await {
                    Ok(()) => {
                        self.apply_action(position.id, &action).await;
                        // EXT-16 FR-4: Emit management event
                        match &action {
                            ManagementAction::MoveStopToEntry => {
                                self.emit_event(
                                    position.user_id,
                                    "break_even",
                                    &position.symbol,
                                    &format!("Break-even triggered on {}", position.symbol),
                                );
                            }
                            ManagementAction::AdjustTrailingStop { new_price } => {
                                self.emit_event(
                                    position.user_id,
                                    "trailing_moved",
                                    &position.symbol,
                                    &format!(
                                        "Trailing stop moved to {} on {}",
                                        new_price, position.symbol
                                    ),
                                );
                            }
                            ManagementAction::PartialClose { quantity } => {
                                self.emit_event(
                                    position.user_id,
                                    "partial_tp",
                                    &position.symbol,
                                    &format!(
                                        "Partial TP: closed {} on {}",
                                        quantity, position.symbol
                                    ),
                                );
                            }
                        }
                        any_executed = true;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "TradeManager: failed to execute {:?} for position {}: {}",
                            action,
                            position.id,
                            e
                        );
                    }
                }
            }
            if any_executed {
                self.record_amend(position.id).await;
            }
        }
    }

    async fn promote_pending_if_filled(&self, position: &ManagedPosition) {
        if position.state != PositionState::Pending {
            return;
        }

        let has_position = match self
            .exchange_api
            .get_position(
                position.user_id,
                &position.symbol,
                position.exchange_account_id,
            )
            .await
        {
            Ok(Some(info)) => info.quantity > Decimal::ZERO,
            Ok(None) => false,
            Err(e) => {
                tracing::debug!(
                    "TradeManager: pending fill check failed for {}: {}",
                    position.id,
                    e
                );
                false
            }
        };

        if !has_position {
            return;
        }

        let mut positions = self.positions.write().await;
        if let Some(pos) = positions.get_mut(&position.id) {
            if pos.state == PositionState::Pending {
                pos.state = PositionState::Filled;
                if let Some(ref repo) = self.repository {
                    let _ = repo
                        .update_state(
                            pos.id,
                            &pos.state,
                            pos.be_triggered,
                            pos.partial_tp_fired,
                            pos.current_stop,
                            pos.remaining_qty,
                        )
                        .await;
                }
                tracing::info!(
                    "TradeManager: promoted pending position {} to Filled (symbol={})",
                    pos.id,
                    pos.symbol
                );
            }
        }
    }

    /// Execute a management action via the exchange API.
    async fn execute_action(
        &self,
        position: &ManagedPosition,
        action: &ManagementAction,
    ) -> Result<(), ExchangeApiError> {
        match action {
            ManagementAction::MoveStopToEntry => {
                if let Some(ref sl_id) = position.exchange_order_ids.stop_loss_order_id {
                    let close_side = match position.side {
                        PositionSide::Long => OrderSide::Sell,
                        PositionSide::Short => OrderSide::Buy,
                    };
                    let new_id = self
                        .exchange_api
                        .amend_order(
                            position.user_id,
                            sl_id,
                            &position.symbol,
                            AmendRequest {
                                new_price: None,
                                new_stop_price: Some(position.entry_price),
                                new_quantity: None,
                                order_type: Some(ApiOrderType::StopLoss),
                                side: Some(close_side),
                                quantity: Some(position.remaining_qty),
                                reduce_only: true,
                            },
                            position.exchange_account_id,
                        )
                        .await?;
                    // Sync OrderGroupManager index before updating ManagedPosition
                    self.sync_sl_order_id(sl_id, &new_id).await;
                    let mut positions = self.positions.write().await;
                    if let Some(pos) = positions.get_mut(&position.id) {
                        pos.exchange_order_ids.stop_loss_order_id = Some(new_id);
                    }
                }
                Ok(())
            }
            ManagementAction::AdjustTrailingStop { new_price } => {
                if let Some(ref sl_id) = position.exchange_order_ids.stop_loss_order_id {
                    let close_side = match position.side {
                        PositionSide::Long => OrderSide::Sell,
                        PositionSide::Short => OrderSide::Buy,
                    };
                    let new_id = self
                        .exchange_api
                        .amend_order(
                            position.user_id,
                            sl_id,
                            &position.symbol,
                            AmendRequest {
                                new_price: None,
                                new_stop_price: Some(*new_price),
                                new_quantity: None,
                                order_type: Some(ApiOrderType::StopLoss),
                                side: Some(close_side),
                                quantity: Some(position.remaining_qty),
                                reduce_only: true,
                            },
                            position.exchange_account_id,
                        )
                        .await?;
                    // Sync OrderGroupManager index before updating ManagedPosition
                    self.sync_sl_order_id(sl_id, &new_id).await;
                    let mut positions = self.positions.write().await;
                    if let Some(pos) = positions.get_mut(&position.id) {
                        pos.exchange_order_ids.stop_loss_order_id = Some(new_id);
                    }
                }
                Ok(())
            }
            ManagementAction::PartialClose { quantity } => {
                let side = match position.side {
                    PositionSide::Long => OrderSide::Sell,
                    PositionSide::Short => OrderSide::Buy,
                };
                self.exchange_api
                    .place_order(PlaceOrderRequest {
                        user_id: position.user_id,
                        symbol: position.symbol.clone(),
                        side,
                        order_type: ApiOrderType::Market,
                        quantity: *quantity,
                        price: None,
                        stop_price: None,
                        leverage: position.rules.leverage,
                        exchange_account_id: position.exchange_account_id,
                        reduce_only: true,
                        client_order_id: None,
                        stop_loss_trigger: None,
                        take_profit_trigger: None,
                    })
                    .await?;
                Ok(())
            }
        }
    }

    /// Apply action to in-memory state and persist.
    async fn apply_action(&self, position_id: Uuid, action: &ManagementAction) {
        let mut positions = self.positions.write().await;
        if let Some(pos) = positions.get_mut(&position_id) {
            match action {
                ManagementAction::MoveStopToEntry => {
                    pos.be_triggered = true;
                    pos.current_stop = pos.entry_price;
                    pos.state = PositionState::Managing;
                }
                ManagementAction::AdjustTrailingStop { new_price } => {
                    pos.current_stop = *new_price;
                }
                ManagementAction::PartialClose { quantity } => {
                    pos.partial_tp_fired = true;
                    pos.remaining_qty -= quantity;
                }
            }

            // Persist state
            if let Some(ref repo) = self.repository {
                let _ = repo
                    .update_state(
                        pos.id,
                        &pos.state,
                        pos.be_triggered,
                        pos.partial_tp_fired,
                        pos.current_stop,
                        pos.remaining_qty,
                    )
                    .await;
            }
        }
    }

    async fn is_debounced(&self, position_id: Uuid) -> bool {
        let last = self.last_amend.read().await;
        if let Some(t) = last.get(&position_id) {
            t.elapsed() < self.debounce_interval
        } else {
            false
        }
    }

    async fn record_amend(&self, position_id: Uuid) {
        let mut last = self.last_amend.write().await;
        last.insert(position_id, Instant::now());
    }

    /// Run the trade manager event loop, consuming price ticks from the broadcast channel.
    pub async fn run(&self, mut price_rx: broadcast::Receiver<PriceTick>) {
        tracing::info!("TradeManagerService started");

        loop {
            match price_rx.recv().await {
                Ok(tick) => {
                    self.process_tick(&tick).await;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("TradeManager lagged {} ticks", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::info!("TradeManager: price feed channel closed, shutting down");
                    break;
                }
            }
        }
    }

    /// Get the count of actively managed positions.
    pub async fn position_count(&self) -> usize {
        self.positions.read().await.len()
    }

    /// FR-2 (016): Get unique symbols for all non-closed positions.
    /// Used by PriceFeedService to poll prices for live-only trades.
    pub async fn get_active_symbols(&self) -> Vec<String> {
        let positions = self.positions.read().await;
        positions
            .values()
            .filter(|p| {
                matches!(
                    p.state,
                    PositionState::Pending | PositionState::Filled | PositionState::Managing
                )
            })
            .map(|p| p.symbol.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Get a snapshot of a managed position by ID.
    pub async fn get_position(&self, id: Uuid) -> Option<ManagedPosition> {
        self.positions.read().await.get(&id).cloned()
    }

    /// Mark a position as closed in both in-memory map and DB.
    /// Used by cancel_trade and cleanup_stale_trades to persist cancellations.
    pub async fn mark_position_closed(&self, id: Uuid) -> Result<(), String> {
        self.positions.write().await.remove(&id);
        if let Some(ref repo) = self.repository {
            repo.mark_closed(id)
                .await
                .map_err(|e| format!("DB error: {}", e))?;
        }
        Ok(())
    }

    /// AUD-02 FR-6: Remove closed positions and stale debounce timestamps.
    pub async fn prune_closed(&self) -> usize {
        let mut positions = self.positions.write().await;
        let before = positions.len();
        let closed_ids: Vec<Uuid> = positions
            .iter()
            .filter(|(_, p)| p.state == PositionState::Closed)
            .map(|(id, _)| *id)
            .collect();

        for id in &closed_ids {
            positions.remove(id);
        }
        drop(positions);

        // Clean stale debounce entries for removed positions
        let mut last = self.last_amend.write().await;
        for id in &closed_ids {
            last.remove(id);
        }

        before - (before - closed_ids.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rust_decimal_macros::dec;
    use std::sync::Mutex;

    /// Mock ExchangeApi that records calls.
    struct MockExchangeApi {
        amend_calls: Mutex<Vec<(Uuid, String, AmendRequest)>>,
        place_calls: Mutex<Vec<PlaceOrderRequest>>,
        cancel_calls: Mutex<Vec<(Uuid, String)>>,
        open_position: Option<crate::services::exchange_api::PositionInfo>,
        /// Predictable amend return ID (if set, returned instead of random UUID).
        amend_return_id: Option<String>,
    }

    impl MockExchangeApi {
        fn new() -> Self {
            Self {
                amend_calls: Mutex::new(Vec::new()),
                place_calls: Mutex::new(Vec::new()),
                cancel_calls: Mutex::new(Vec::new()),
                open_position: None,
                amend_return_id: None,
            }
        }

        fn with_open_position() -> Self {
            Self {
                amend_calls: Mutex::new(Vec::new()),
                place_calls: Mutex::new(Vec::new()),
                cancel_calls: Mutex::new(Vec::new()),
                open_position: Some(crate::services::exchange_api::PositionInfo {
                    symbol: "BTC_USDT".to_string(),
                    side: "long".to_string(),
                    quantity: dec!(0.2),
                    entry_price: dec!(50000),
                    unrealized_pnl: dec!(0),
                }),
                amend_return_id: None,
            }
        }

        fn with_amend_return_id(mut self, id: &str) -> Self {
            self.amend_return_id = Some(id.to_string());
            self
        }

        fn amend_count(&self) -> usize {
            self.amend_calls.lock().unwrap().len()
        }

        fn place_count(&self) -> usize {
            self.place_calls.lock().unwrap().len()
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

        async fn place_order(&self, req: PlaceOrderRequest) -> Result<PlaceOrderResult, ExchangeApiError> {
            self.place_calls.lock().unwrap().push(req);
            Ok(PlaceOrderResult {
                id: Uuid::new_v4().to_string(),
                status: None,
                average: None,
                stop_loss_order_id: None,
                take_profit_order_id: None,
            })
        }

        async fn amend_order(
            &self,
            user_id: Uuid,
            order_id: &str,
            _symbol: &str,
            amend: AmendRequest,
            _exchange_account_id: Option<Uuid>,
        ) -> Result<String, ExchangeApiError> {
            self.amend_calls
                .lock()
                .unwrap()
                .push((user_id, order_id.to_string(), amend));
            Ok(self
                .amend_return_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()))
        }

        async fn cancel_order(
            &self,
            user_id: Uuid,
            order_id: &str,
            _symbol: &str,
            _exchange_account_id: Option<Uuid>,
        ) -> Result<(), ExchangeApiError> {
            self.cancel_calls
                .lock()
                .unwrap()
                .push((user_id, order_id.to_string()));
            Ok(())
        }

        async fn get_position(
            &self,
            _user_id: Uuid,
            _symbol: &str,
            _exchange_account_id: Option<Uuid>,
        ) -> Result<Option<crate::services::exchange_api::PositionInfo>, ExchangeApiError> {
            Ok(self.open_position.clone())
        }
    }

    fn test_position() -> ManagedPosition {
        let mut pos = ManagedPosition::new(
            Uuid::new_v4(),
            "BTC_USDT".to_string(),
            PositionSide::Long,
            dec!(50000),
            dec!(49000),
            dec!(52000),
            dec!(0.2),
            ManagementRules {
                risk_percent: dec!(2),
                break_even_at: 50,
                leverage: 1,
                trailing_stop: Some(TrailingStopRule {
                    enabled: true,
                    distance_percent: 20,
                }),
                partial_tp: Some(PartialTpRule {
                    enabled: true,
                    close_percent: 50,
                }),
            },
        );
        pos.state = PositionState::Filled;
        pos.exchange_order_ids.stop_loss_order_id = Some(Uuid::new_v4().to_string());
        pos
    }

    #[tokio::test]
    async fn test_register_and_count() {
        let api = Arc::new(MockExchangeApi::new());
        let service = TradeManagerService::new(api, None);

        let pos = test_position();
        service.register(pos).await.unwrap();

        assert_eq!(service.position_count().await, 1);
    }

    #[tokio::test]
    async fn test_process_tick_triggers_be() {
        let api = Arc::new(MockExchangeApi::new());
        let service = TradeManagerService::new(api.clone(), None);

        let pos = test_position();
        let pos_id = pos.id;
        service.register(pos).await.unwrap();

        // Price at 51000 = 50% progress, should trigger BE
        let tick = PriceTick {
            symbol: "BTC_USDT".to_string(),
            bid: dec!(51000),
            ask: dec!(51000),
            high: dec!(51000),
            low: dec!(51000),
        };

        service.process_tick(&tick).await;

        // Should have amended the SL order
        assert_eq!(api.amend_count(), 1);

        // Position should be updated
        let updated = service.get_position(pos_id).await.unwrap();
        assert!(updated.be_triggered);
        assert_eq!(updated.current_stop, dec!(50000)); // moved to entry
    }

    #[tokio::test]
    async fn test_pending_position_promotes_and_triggers_be() {
        let api = Arc::new(MockExchangeApi::with_open_position());
        let service = TradeManagerService::new(api.clone(), None);

        let mut pos = test_position();
        pos.state = PositionState::Pending;
        let pos_id = pos.id;
        service.register(pos).await.unwrap();

        let tick = PriceTick {
            symbol: "BTC_USDT".to_string(),
            bid: dec!(51000),
            ask: dec!(51000),
            high: dec!(51000),
            low: dec!(51000),
        };

        service.process_tick(&tick).await;

        assert_eq!(api.amend_count(), 1);
        let updated = service.get_position(pos_id).await.unwrap();
        assert!(updated.be_triggered);
        assert_eq!(updated.current_stop, dec!(50000));
        assert_eq!(updated.state, PositionState::Managing);
    }

    #[tokio::test]
    async fn test_process_tick_trailing_after_be() {
        let api = Arc::new(MockExchangeApi::new());
        let service = TradeManagerService::new(api.clone(), None);

        let mut pos = test_position();
        pos.be_triggered = true;
        pos.current_stop = dec!(50000);
        pos.state = PositionState::Managing;
        let pos_id = pos.id;
        service.register(pos).await.unwrap();

        // Price at 51500, trailing dist = 2000*20% = 400
        // New stop = 51500 - 400 = 51100 (> current 50000)
        let tick = PriceTick {
            symbol: "BTC_USDT".to_string(),
            bid: dec!(51500),
            ask: dec!(51500),
            high: dec!(51500),
            low: dec!(51500),
        };

        service.process_tick(&tick).await;

        assert_eq!(api.amend_count(), 1);

        let updated = service.get_position(pos_id).await.unwrap();
        assert_eq!(updated.current_stop, dec!(51100));
    }

    #[tokio::test]
    async fn test_process_tick_partial_tp() {
        let api = Arc::new(MockExchangeApi::new());
        let service = TradeManagerService::new(api.clone(), None);

        let mut pos = test_position();
        pos.be_triggered = true;
        pos.current_stop = dec!(51500);
        pos.state = PositionState::Managing;
        let pos_id = pos.id;
        service.register(pos).await.unwrap();

        // Price at target 52000, trailing dist=400, new_stop=51600 > 51500
        let tick = PriceTick {
            symbol: "BTC_USDT".to_string(),
            bid: dec!(52000),
            ask: dec!(52000),
            high: dec!(52000),
            low: dec!(52000),
        };

        service.process_tick(&tick).await;

        // Should have trailing stop amend + partial close
        assert_eq!(api.amend_count(), 1); // trailing
        assert_eq!(api.place_count(), 1); // partial close

        let updated = service.get_position(pos_id).await.unwrap();
        assert!(updated.partial_tp_fired);
        assert_eq!(updated.remaining_qty, dec!(0.1)); // 50% of 0.2
    }

    #[tokio::test]
    async fn test_debounce_prevents_rapid_amends() {
        let api = Arc::new(MockExchangeApi::new());
        let service = TradeManagerService::new(api.clone(), None);

        let pos = test_position();
        service.register(pos).await.unwrap();

        let tick = PriceTick {
            symbol: "BTC_USDT".to_string(),
            bid: dec!(51000),
            ask: dec!(51000),
            high: dec!(51000),
            low: dec!(51000),
        };

        // First tick - triggers
        service.process_tick(&tick).await;
        assert_eq!(api.amend_count(), 1);

        // Second tick immediately - should be debounced
        service.process_tick(&tick).await;
        assert_eq!(api.amend_count(), 1); // still 1
    }

    #[tokio::test]
    async fn test_get_active_symbols_returns_non_closed() {
        let api = Arc::new(MockExchangeApi::new());
        let service = TradeManagerService::new(api, None);

        // Register positions in different states
        let mut pending = test_position();
        pending.state = PositionState::Pending;
        pending.symbol = "SOL_USDT".to_string();
        service.register(pending).await.unwrap();

        let mut filled = test_position();
        filled.state = PositionState::Filled;
        filled.symbol = "BTC_USDT".to_string();
        service.register(filled).await.unwrap();

        let mut managing = test_position();
        managing.state = PositionState::Managing;
        managing.symbol = "ETH_USDT".to_string();
        service.register(managing).await.unwrap();

        let mut closed = test_position();
        closed.state = PositionState::Closed;
        closed.symbol = "DOGE_USDT".to_string();
        service.register(closed).await.unwrap();

        let symbols = service.get_active_symbols().await;
        assert_eq!(symbols.len(), 3);
        assert!(symbols.contains(&"SOL_USDT".to_string()));
        assert!(symbols.contains(&"BTC_USDT".to_string()));
        assert!(symbols.contains(&"ETH_USDT".to_string()));
        assert!(!symbols.contains(&"DOGE_USDT".to_string()));
    }

    #[tokio::test]
    async fn test_get_active_symbols_deduplicates() {
        let api = Arc::new(MockExchangeApi::new());
        let service = TradeManagerService::new(api, None);

        // Two positions with same symbol
        let mut a = test_position();
        a.state = PositionState::Filled;
        a.symbol = "BTC_USDT".to_string();
        service.register(a).await.unwrap();

        let mut b = test_position();
        b.state = PositionState::Managing;
        b.symbol = "BTC_USDT".to_string();
        service.register(b).await.unwrap();

        let symbols = service.get_active_symbols().await;
        assert_eq!(symbols.len(), 1);
        assert!(symbols.contains(&"BTC_USDT".to_string()));
    }

    #[tokio::test]
    async fn test_no_actions_for_non_matching_symbol() {
        let api = Arc::new(MockExchangeApi::new());
        let service = TradeManagerService::new(api.clone(), None);

        let pos = test_position();
        service.register(pos).await.unwrap();

        let tick = PriceTick {
            symbol: "ETH_USDT".to_string(),
            bid: dec!(51000),
            ask: dec!(51000),
            high: dec!(51000),
            low: dec!(51000),
        };

        service.process_tick(&tick).await;
        assert_eq!(api.amend_count(), 0);
    }

    #[tokio::test]
    async fn test_be_trigger_syncs_order_group_manager_index() {
        use engine::shadow::order_group::OrderGroup;
        use engine::{EngineActor, ShadowEngine};

        let old_sl_id = "old-sl-exchange-id";
        let new_sl_id = "new-sl-exchange-id";
        let api = Arc::new(MockExchangeApi::new().with_amend_return_id(new_sl_id));

        // Set up engine with pre-populated OrderGroup
        let mut engine = ShadowEngine::new();
        let mut pos = test_position();
        pos.exchange_order_ids.stop_loss_order_id = Some(old_sl_id.to_string());

        let mut group = OrderGroup::new(
            pos.user_id,
            pos.symbol.clone(),
            Uuid::new_v4(),
            pos.rules.risk_percent,
        );
        group.exchange_sl_order_id = Some(old_sl_id.to_string());
        let added = engine.order_groups.add_group(group);
        engine.order_groups.register_exchange_order(old_sl_id.to_string(), added.id);

        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);

        let service = TradeManagerService::new(api.clone(), None)
            .with_engine_handle(handle.clone());
        service.register(pos).await.unwrap();

        // Price triggers BE (50% progress toward TP)
        let tick = PriceTick {
            symbol: "BTC_USDT".to_string(),
            bid: dec!(51000),
            ask: dec!(51000),
            high: dec!(51000),
            low: dec!(51000),
        };
        service.process_tick(&tick).await;

        assert_eq!(api.amend_count(), 1);

        // Verify OrderGroupManager index was reindexed via EngineHandle
        assert!(
            handle.get_group_by_exchange_order(old_sl_id.to_string()).await.is_none(),
            "old SL ID should be removed from index"
        );
        assert!(
            handle.get_group_by_exchange_order(new_sl_id.to_string()).await.is_some(),
            "new SL ID should be in index"
        );
        let group = handle.get_group_by_exchange_order(new_sl_id.to_string()).await.unwrap();
        assert_eq!(
            group.exchange_sl_order_id.as_deref(),
            Some(new_sl_id),
            "group field should be updated"
        );
    }
}
