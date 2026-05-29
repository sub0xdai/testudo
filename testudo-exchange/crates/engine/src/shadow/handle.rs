//! ShadowEngine public API — EngineCommand, EngineHandle, and supporting types.
//!
//! Extracted from actor.rs per CLN-05. This module defines the complete
//! command interface and async handle used by all external callers.
//! The actor loop and dispatch logic live in `actor.rs`.

// @anchor exchange:engine:handle
// @tags domain

use rust_decimal::Decimal;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::positions::PositionSide;
use super::{
    BreakEvenConfig, OrderGroup, OrderGroupStatus, PriceUpdateResult, ShadowBalance,
    ShadowOrder, TakeProfitTarget,
};
use super::positions::ShadowPosition;
/// Event emitted by the actor when a fire-and-forget price update triggers fills.
/// Subscribers (e.g., FillDetectorService) can react to fills and execute OCO
/// cancellations on the exchange.
#[derive(Debug)]
pub struct FillEvent {
    pub symbol: String,
    pub filled: Vec<ShadowOrder>,
    pub exchange_cancels: Vec<super::ExchangeCancel>,
}

// ---------------------------------------------------------------------------
// EngineError (FR-3)
// ---------------------------------------------------------------------------

/// Actor-level errors returned by `EngineHandle` methods.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The actor's command channel was closed (actor shut down).
    #[error("actor shutdown: command channel closed")]
    ActorShutdown,

    /// An engine operation failed.
    #[error("{0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// OrderRole
// ---------------------------------------------------------------------------

/// Identifies which exchange order ID field to set on an OrderGroup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderRole {
    Entry,
    StopLoss,
    TakeProfit,
}

// ---------------------------------------------------------------------------
// EngineCommand (FR-1)
// ---------------------------------------------------------------------------

/// Every ShadowEngine public method + manager methods accessed through locks
/// get a corresponding command variant. Each variant carries arguments and an
/// embedded `oneshot::Sender<T>` for the typed response.
pub enum EngineCommand {
    // --- User management ---
    UserExists {
        user_id: Uuid,
        reply: oneshot::Sender<bool>,
    },
    InitUser {
        user_id: Uuid,
        reply: oneshot::Sender<()>,
    },
    InitUserWithBalance {
        user_id: Uuid,
        usdt_balance: Decimal,
        reply: oneshot::Sender<()>,
    },
    ResetUser {
        user_id: Uuid,
        reply: oneshot::Sender<()>,
    },

    // --- Balances ---
    GetBalances {
        user_id: Uuid,
        reply: oneshot::Sender<Vec<ShadowBalance>>,
    },

    // --- Orders ---
    PlaceOrder {
        user_id: Uuid,
        order: ShadowOrder,
        reply: oneshot::Sender<Result<ShadowOrder, EngineError>>,
    },
    PlaceOrderNoGroup {
        user_id: Uuid,
        order: ShadowOrder,
        reply: oneshot::Sender<Result<ShadowOrder, EngineError>>,
    },
    CancelOrder {
        user_id: Uuid,
        order_id: Uuid,
        reply: oneshot::Sender<Result<ShadowOrder, EngineError>>,
    },
    CancelOrderNoCascade {
        user_id: Uuid,
        order_id: Uuid,
        reply: oneshot::Sender<Result<ShadowOrder, EngineError>>,
    },
    GetOrder {
        order_id: Uuid,
        reply: oneshot::Sender<Option<ShadowOrder>>,
    },
    GetOpenOrders {
        user_id: Uuid,
        reply: oneshot::Sender<Vec<ShadowOrder>>,
    },
    GetAllOrders {
        user_id: Uuid,
        reply: oneshot::Sender<Vec<ShadowOrder>>,
    },

    // --- Positions ---
    GetPositions {
        user_id: Uuid,
        reply: oneshot::Sender<Vec<ShadowPosition>>,
    },
    GetUnrealizedPnl {
        user_id: Uuid,
        reply: oneshot::Sender<Decimal>,
    },
    OpenPositionCount {
        user_id: Uuid,
        reply: oneshot::Sender<usize>,
    },
    GetEntryPrice {
        user_id: Uuid,
        symbol: String,
        reply: oneshot::Sender<Option<(Decimal, PositionSide)>>,
    },

    // --- Price processing ---
    ProcessPriceUpdate {
        symbol: String,
        bid: Decimal,
        ask: Decimal,
        high: Decimal,
        low: Decimal,
        reply: oneshot::Sender<PriceUpdateResult>,
    },
    UpdateMarkPrice {
        symbol: String,
        mark_price: Decimal,
        reply: oneshot::Sender<()>,
    },
    GetActiveSymbols {
        reply: oneshot::Sender<Vec<String>>,
    },
    CheckBreakEven {
        symbol: String,
        current_price: Decimal,
        reply: oneshot::Sender<()>,
    },
    EnableBreakEven {
        group_id: Uuid,
        trigger_percent: Decimal,
        offset: Option<Decimal>,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },

    // --- Order groups ---
    ListTradeGroups {
        user_id: Uuid,
        reply: oneshot::Sender<Vec<OrderGroup>>,
    },
    GetTradeGroup {
        group_id: Uuid,
        reply: oneshot::Sender<Option<OrderGroup>>,
    },
    GetGroupByEntryOrder {
        entry_order_id: Uuid,
        reply: oneshot::Sender<Option<OrderGroup>>,
    },
    GetGroupByLinkedOrder {
        order_id: Uuid,
        reply: oneshot::Sender<Option<OrderGroup>>,
    },
    GetGroupByExchangeOrder {
        exchange_order_id: String,
        reply: oneshot::Sender<Option<OrderGroup>>,
    },
    GetActiveGroups {
        user_id: Uuid,
        reply: oneshot::Sender<Vec<OrderGroup>>,
    },
    GetLiveGroups {
        reply: oneshot::Sender<Vec<OrderGroup>>,
    },
    ActiveGroupCount {
        reply: oneshot::Sender<usize>,
    },
    RegisterExchangeOrderId {
        group_id: Uuid,
        role: OrderRole,
        exchange_id: String,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    RegisterLinkedOrder {
        order_id: Uuid,
        group_id: Uuid,
        reply: oneshot::Sender<()>,
    },
    UpdateEntryOrder {
        group_id: Uuid,
        old_entry_id: Uuid,
        new_entry_id: Uuid,
        reply: oneshot::Sender<()>,
    },
    UpdateGroupStatus {
        group_id: Uuid,
        status: OrderGroupStatus,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    OnEntryFilled {
        group_id: Uuid,
        fill_price: Decimal,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    OnStopLossFilled {
        group_id: Uuid,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    OnTakeProfitFilled {
        group_id: Uuid,
        order_id: Uuid,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    ReindexExchangeSlOrder {
        old_id: String,
        new_id: String,
        reply: oneshot::Sender<bool>,
    },

    // --- 019c: Route migration commands ---

    /// Configure a group's TP targets, break-even, and exchange account.
    /// Used by create_trade after placing the order but before exchange I/O.
    ConfigureGroup {
        group_id: Uuid,
        take_profit_targets: Option<Vec<TakeProfitTarget>>,
        break_even_config: Option<BreakEvenConfig>,
        exchange_account_id: Option<Uuid>,
        exchange_name: Option<String>,
        risk_amount: Option<Decimal>,
        setup_tag: Option<String>,
        /// QNT-01a: Calibrated Kelly snapshot captured at entry. `None`
        /// when the trade is sized via the fixed-fractional path.
        kelly_inputs: Option<serde_json::Value>,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },

    /// Atomically validate group status and update SL price + entry quantity.
    /// For Pending groups, also swaps the entry order ID.
    /// Returns the updated group on success for response construction.
    UpdateGroupStopLoss {
        group_id: Uuid,
        expected_status: OrderGroupStatus,
        new_sl_price: Decimal,
        new_entry_quantity: Decimal,
        /// For Pending groups: swap entry order. None for Active groups.
        entry_order_swap: Option<(Uuid, Uuid)>, // (old_id, new_id)
        reply: oneshot::Sender<Result<OrderGroup, EngineError>>,
    },

    /// Update the stop price on a specific order.
    UpdateStopPrice {
        order_id: Uuid,
        new_price: Decimal,
        reply: oneshot::Sender<bool>,
    },

    /// Add a take-profit target to a group.
    AddTakeProfitTarget {
        group_id: Uuid,
        user_id: Uuid,
        target: TakeProfitTarget,
        reply: oneshot::Sender<Result<OrderGroup, EngineError>>,
    },

    // --- 019d: Fire-and-forget price update ---
    /// Price update with no reply channel. Fills are emitted to the actor's
    /// `fill_event_tx` channel instead of being returned to the caller.
    ProcessPriceUpdateFireAndForget {
        symbol: String,
        bid: Decimal,
        ask: Decimal,
        high: Decimal,
        low: Decimal,
    },

    // --- GC ---
    PruneTerminal {
        cutoff: Instant,
        reply: oneshot::Sender<usize>,
    },

    // --- Rehydration ---
    LoadOrderGroups {
        groups: Vec<OrderGroup>,
        reply: oneshot::Sender<()>,
    },
}

// ---------------------------------------------------------------------------
// EngineHandle (FR-2)
// ---------------------------------------------------------------------------

/// Async handle to the `EngineActor`. Wraps `mpsc::Sender<EngineCommand>`.
/// Methods mirror the ShadowEngine public API.
///
/// - Mutation methods return `Result<T, EngineError>`.
/// - Query methods return the value directly (graceful degradation on shutdown).
#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::Sender<EngineCommand>,
}

/// Channel capacity (FR-5): callers block on backpressure when saturated.
pub const ENGINE_CHANNEL_CAPACITY: usize = 256;

impl EngineHandle {
    pub fn new(tx: mpsc::Sender<EngineCommand>) -> Self {
        Self { tx }
    }

    // --- User management ---

    pub async fn user_exists(&self, user_id: Uuid) -> bool {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::UserExists { user_id, reply: tx }).await;
        rx.await.unwrap_or(false)
    }

    pub async fn init_user(&self, user_id: Uuid) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(EngineCommand::InitUser { user_id, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)
    }

    pub async fn init_user_with_balance(
        &self,
        user_id: Uuid,
        usdt_balance: Decimal,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::InitUserWithBalance { user_id, usdt_balance, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)
    }

    pub async fn reset_user(&self, user_id: Uuid) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(EngineCommand::ResetUser { user_id, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)
    }

    // --- Balances ---

    pub async fn get_balances(&self, user_id: Uuid) -> Vec<ShadowBalance> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::GetBalances { user_id, reply: tx }).await;
        rx.await.unwrap_or_default()
    }

    // --- Orders ---

    pub async fn place_order(
        &self,
        user_id: Uuid,
        order: ShadowOrder,
    ) -> Result<ShadowOrder, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::PlaceOrder { user_id, order, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn place_order_no_group(
        &self,
        user_id: Uuid,
        order: ShadowOrder,
    ) -> Result<ShadowOrder, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::PlaceOrderNoGroup { user_id, order, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn cancel_order(
        &self,
        user_id: Uuid,
        order_id: Uuid,
    ) -> Result<ShadowOrder, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::CancelOrder { user_id, order_id, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn cancel_order_no_cascade(
        &self,
        user_id: Uuid,
        order_id: Uuid,
    ) -> Result<ShadowOrder, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::CancelOrderNoCascade { user_id, order_id, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn get_order(&self, order_id: Uuid) -> Option<ShadowOrder> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::GetOrder { order_id, reply: tx }).await;
        rx.await.unwrap_or(None)
    }

    pub async fn get_open_orders(&self, user_id: Uuid) -> Vec<ShadowOrder> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::GetOpenOrders { user_id, reply: tx }).await;
        rx.await.unwrap_or_default()
    }

    pub async fn get_all_orders(&self, user_id: Uuid) -> Vec<ShadowOrder> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::GetAllOrders { user_id, reply: tx }).await;
        rx.await.unwrap_or_default()
    }

    // --- Positions ---

    pub async fn get_positions(&self, user_id: Uuid) -> Vec<ShadowPosition> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::GetPositions { user_id, reply: tx }).await;
        rx.await.unwrap_or_default()
    }

    pub async fn get_unrealized_pnl(&self, user_id: Uuid) -> Decimal {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::GetUnrealizedPnl { user_id, reply: tx })
            .await;
        rx.await.unwrap_or(Decimal::ZERO)
    }

    pub async fn open_position_count(&self, user_id: Uuid) -> usize {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::OpenPositionCount { user_id, reply: tx })
            .await;
        rx.await.unwrap_or(0)
    }

    pub async fn get_entry_price(
        &self,
        user_id: Uuid,
        symbol: String,
    ) -> Option<(Decimal, PositionSide)> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::GetEntryPrice { user_id, symbol, reply: tx })
            .await;
        rx.await.unwrap_or(None)
    }

    // --- Price processing ---

    pub async fn process_price_update(
        &self,
        symbol: String,
        bid: Decimal,
        ask: Decimal,
        high: Decimal,
        low: Decimal,
    ) -> Result<PriceUpdateResult, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::ProcessPriceUpdate { symbol, bid, ask, high, low, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)
    }

    pub async fn update_mark_price(
        &self,
        symbol: String,
        mark_price: Decimal,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::UpdateMarkPrice { symbol, mark_price, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)
    }

    pub async fn get_active_symbols(&self) -> Vec<String> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::GetActiveSymbols { reply: tx }).await;
        rx.await.unwrap_or_default()
    }

    pub async fn check_break_even(
        &self,
        symbol: String,
        current_price: Decimal,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::CheckBreakEven { symbol, current_price, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)
    }

    pub async fn enable_break_even(
        &self,
        group_id: Uuid,
        trigger_percent: Decimal,
        offset: Option<Decimal>,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::EnableBreakEven {
                group_id,
                trigger_percent,
                offset,
                reply: tx,
            })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    // --- Order groups ---

    pub async fn list_trade_groups(&self, user_id: Uuid) -> Vec<OrderGroup> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::ListTradeGroups { user_id, reply: tx })
            .await;
        rx.await.unwrap_or_default()
    }

    pub async fn get_trade_group(&self, group_id: Uuid) -> Option<OrderGroup> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::GetTradeGroup { group_id, reply: tx })
            .await;
        rx.await.unwrap_or(None)
    }

    pub async fn get_group_by_entry_order(&self, entry_order_id: Uuid) -> Option<OrderGroup> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::GetGroupByEntryOrder { entry_order_id, reply: tx })
            .await;
        rx.await.unwrap_or(None)
    }

    pub async fn get_group_by_linked_order(&self, order_id: Uuid) -> Option<OrderGroup> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::GetGroupByLinkedOrder { order_id, reply: tx })
            .await;
        rx.await.unwrap_or(None)
    }

    pub async fn get_group_by_exchange_order(
        &self,
        exchange_order_id: String,
    ) -> Option<OrderGroup> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::GetGroupByExchangeOrder { exchange_order_id, reply: tx })
            .await;
        rx.await.unwrap_or(None)
    }

    pub async fn get_active_groups(&self, user_id: Uuid) -> Vec<OrderGroup> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::GetActiveGroups { user_id, reply: tx })
            .await;
        rx.await.unwrap_or_default()
    }

    pub async fn get_live_groups(&self) -> Vec<OrderGroup> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::GetLiveGroups { reply: tx }).await;
        rx.await.unwrap_or_default()
    }

    pub async fn active_group_count(&self) -> usize {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::ActiveGroupCount { reply: tx }).await;
        rx.await.unwrap_or(0)
    }

    pub async fn register_exchange_order_id(
        &self,
        group_id: Uuid,
        role: OrderRole,
        exchange_id: String,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::RegisterExchangeOrderId {
                group_id,
                role,
                exchange_id,
                reply: tx,
            })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn register_linked_order(
        &self,
        order_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::RegisterLinkedOrder { order_id, group_id, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)
    }

    pub async fn update_entry_order(
        &self,
        group_id: Uuid,
        old_entry_id: Uuid,
        new_entry_id: Uuid,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::UpdateEntryOrder {
                group_id,
                old_entry_id,
                new_entry_id,
                reply: tx,
            })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)
    }

    pub async fn update_group_status(
        &self,
        group_id: Uuid,
        status: OrderGroupStatus,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::UpdateGroupStatus { group_id, status, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn on_entry_filled(
        &self,
        group_id: Uuid,
        fill_price: Decimal,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::OnEntryFilled { group_id, fill_price, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn on_stop_loss_filled(&self, group_id: Uuid) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::OnStopLossFilled { group_id, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn on_take_profit_filled(
        &self,
        group_id: Uuid,
        order_id: Uuid,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::OnTakeProfitFilled { group_id, order_id, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn reindex_exchange_sl_order(
        &self,
        old_id: String,
        new_id: String,
    ) -> bool {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::ReindexExchangeSlOrder { old_id, new_id, reply: tx })
            .await;
        rx.await.unwrap_or(false)
    }

    // --- 019d: Fire-and-forget price update ---

    /// Send a price update without waiting for the result. Awaits only for
    /// channel backpressure, not for processing. Fills are emitted to the
    /// actor's internal fill_event channel.
    pub async fn push_price(
        &self,
        symbol: String,
        bid: Decimal,
        ask: Decimal,
        high: Decimal,
        low: Decimal,
    ) -> Result<(), EngineError> {
        self.tx
            .send(EngineCommand::ProcessPriceUpdateFireAndForget {
                symbol,
                bid,
                ask,
                high,
                low,
            })
            .await
            .map_err(|_| EngineError::ActorShutdown)
    }

    // --- 019c: Route migration methods ---

    /// Configure a group's TP targets, break-even config, and exchange account.
    pub async fn configure_group(
        &self,
        group_id: Uuid,
        take_profit_targets: Option<Vec<TakeProfitTarget>>,
        break_even_config: Option<BreakEvenConfig>,
        exchange_account_id: Option<Uuid>,
        exchange_name: Option<String>,
        risk_amount: Option<Decimal>,
        setup_tag: Option<String>,
        kelly_inputs: Option<serde_json::Value>,
    ) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::ConfigureGroup {
                group_id,
                take_profit_targets,
                break_even_config,
                exchange_account_id,
                exchange_name,
                risk_amount,
                setup_tag,
                kelly_inputs,
                reply: tx,
            })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    /// Atomically validate status and update SL price + entry quantity.
    /// For Pending groups, pass `entry_order_swap = Some((old_id, new_id))`.
    pub async fn update_group_stop_loss(
        &self,
        group_id: Uuid,
        expected_status: OrderGroupStatus,
        new_sl_price: Decimal,
        new_entry_quantity: Decimal,
        entry_order_swap: Option<(Uuid, Uuid)>,
    ) -> Result<OrderGroup, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::UpdateGroupStopLoss {
                group_id,
                expected_status,
                new_sl_price,
                new_entry_quantity,
                entry_order_swap,
                reply: tx,
            })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    /// Update the stop price on a specific order.
    pub async fn update_stop_price(&self, order_id: Uuid, new_price: Decimal) -> bool {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .tx
            .send(EngineCommand::UpdateStopPrice { order_id, new_price, reply: tx })
            .await;
        rx.await.unwrap_or(false)
    }

    /// Add a take-profit target to a group. Returns updated group.
    pub async fn add_take_profit_target(
        &self,
        group_id: Uuid,
        user_id: Uuid,
        target: TakeProfitTarget,
    ) -> Result<OrderGroup, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::AddTakeProfitTarget { group_id, user_id, target, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    // --- GC ---

    pub async fn prune_terminal(&self, cutoff: Instant) -> usize {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::PruneTerminal { cutoff, reply: tx }).await;
        rx.await.unwrap_or(0)
    }

    // --- Rehydration ---

    pub async fn load_order_groups(&self, groups: Vec<OrderGroup>) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(EngineCommand::LoadOrderGroups { groups, reply: tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        rx.await.map_err(|_| EngineError::ActorShutdown)
    }
}

// ---------------------------------------------------------------------------
// EngineActor (FR-4)
// ---------------------------------------------------------------------------

