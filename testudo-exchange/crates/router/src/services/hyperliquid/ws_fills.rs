//! HL-04: Native WebSocket Fill Subscription
//! FIX-02: Fill Price Reconciliation via REST
//!
//! Connects directly to Hyperliquid's WebSocket API via tokio-tungstenite,
//! subscribes to order update events, translates them into `OrderUpdateEvent`,
//! and forwards them into the existing `FillDetectorService` pipeline.
//!
//! Bypasses the SDK's `RawWsProvider` which has a fatal design flaw:
//! `start_reading()` consumes `self.ws`, making `ping()` impossible afterward.
//! This implementation owns the read/write split and manages its own keepalive.
//!
//! FIX-02 additions:
//! - REST fill price enrichment (avg_px from `user_fills_by_time`)
//! - Watermark tracking for reconnect gap detection
//! - Reconnect reconciliation via REST fill query
//! - OID-based deduplication

// @anchor exchange:router:ws_fills
// @tags api

use alloy::primitives::Address;
use futures_util::{SinkExt, StreamExt};
use hyperliquid_sdk_rs::{
    types::info_types::UserFillByTime,
    InfoProvider, Network,
};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite;
use uuid::Uuid;

use super::AssetUniverse;
use crate::services::exchange_api::{ExchangeApi, ExchangeApiError};
use crate::services::OrderUpdateEvent;
use engine::shadow::order_group::OrderGroupStatus;
use engine::EngineHandle;

/// Maximum number of OIDs tracked for deduplication before clearing.
const MAX_SEEN_OIDS: usize = 1000;

/// Maximum number of TIDs tracked for journal deduplication before clearing.
const MAX_SEEN_TIDS: usize = 1000;

/// Time window (ms) to search for fills before the event timestamp.
const FILL_LOOKBACK_MS: u64 = 60_000;

/// Startup reconciliation lookback (24 hours in ms).
const STARTUP_LOOKBACK_MS: u64 = 24 * 60 * 60 * 1000;

/// Periodic REST poll interval (30 seconds).
const REST_POLL_INTERVAL_SECS: u64 = 30;

/// REST poll lookback window (5 minutes in ms).
const REST_POLL_LOOKBACK_MS: u64 = 5 * 60 * 1000;

/// Application-level ping interval (seconds).
/// Hyperliquid expects `{"method":"ping"}` to keep the subscription alive.
const PING_INTERVAL_SECS: u64 = 30;

/// Read timeout: if no frame (data, pong, or anything) arrives within this
/// duration, consider the connection dead and force reconnect.
const READ_TIMEOUT_SECS: u64 = 60;

/// Hyperliquid WS endpoints.
fn ws_url(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "wss://api.hyperliquid.xyz/ws",
        Network::Testnet => "wss://api.hyperliquid-testnet.xyz/ws",
    }
}

// ── HL WS JSON types (minimal, avoid SDK's Message type) ──────────────

/// Incoming order update from Hyperliquid WS.
#[derive(Debug, serde::Deserialize)]
struct HlWsMessage {
    channel: Option<String>,
    data: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HlOrderUpdate {
    order: HlBasicOrder,
    status: String,
    status_timestamp: u64,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HlBasicOrder {
    coin: String,
    side: String,
    limit_px: String,
    sz: String,
    oid: u64,
    orig_sz: String,
    #[allow(dead_code)]
    cloid: Option<String>,
}

// ── Subscriber ────────────────────────────────────────────────────────

/// Native Hyperliquid WebSocket fill subscriber.
///
/// Connects directly to HL's WebSocket via tokio-tungstenite with:
/// - Application-level ping every 30s (`{"method":"ping"}`)
/// - 60s read timeout (catches silent death / half-open TCP)
/// - Startup reconciliation (last 24h via REST)
/// - Periodic REST poll every 30s (reliable fallback)
/// - OID-based deduplication across WS + REST
pub struct HyperliquidFillSubscriber {
    network: Network,
    user_address: Address,
    order_update_sender: mpsc::Sender<OrderUpdateEvent>,
    info: InfoProvider,
    /// FIX-02: Watermark — timestamp of last processed event for reconnect reconciliation.
    last_event_timestamp: Option<u64>,
    /// FIX-02: Bounded set of recently-seen OIDs for deduplication.
    seen_oids: HashSet<u64>,
    /// JNL-SYNC-01: user_id for pg_notify and group reconciliation on closing fills.
    user_id: Option<Uuid>,
    /// JNL-SYNC-01: PgPool for pg_notify fallback when no engine handle.
    notify_pool: Option<PgPool>,
    /// REL-02: Bounded set of recently-seen TIDs for dedup on reconnect.
    seen_tids: HashSet<i64>,
    /// REL-03: Optional engine handle for group status transitions on close detection.
    engine_handle: Option<EngineHandle>,
    /// REL-03: Optional exchange API for cancelling sibling orders on close detection.
    exchange_api: Option<Arc<dyn ExchangeApi>>,
}

impl HyperliquidFillSubscriber {
    pub fn new(
        network: Network,
        user_address: Address,
        order_update_sender: mpsc::Sender<OrderUpdateEvent>,
    ) -> Self {
        let info = InfoProvider::new(network);
        Self {
            network,
            user_address,
            order_update_sender,
            info,
            last_event_timestamp: None,
            seen_oids: HashSet::new(),
            user_id: None,
            notify_pool: None,
            seen_tids: HashSet::new(),
            engine_handle: None,
            exchange_api: None,
        }
    }

    /// JNL-SYNC-01: Set user_id and notify_pool for live-state updates on closing fills.
    pub fn with_user(mut self, user_id: Uuid, pool: PgPool) -> Self {
        self.user_id = Some(user_id);
        self.notify_pool = Some(pool);
        self
    }

    /// REL-03: Enable group reconciliation on close detection.
    pub fn with_engine(
        mut self,
        engine_handle: EngineHandle,
        exchange_api: Arc<dyn ExchangeApi>,
    ) -> Self {
        self.engine_handle = Some(engine_handle);
        self.exchange_api = Some(exchange_api);
        self
    }

    /// Run the fill subscription loop with auto-reconnect, keepalive pings,
    /// read timeout, and periodic REST polling.
    pub async fn run(&mut self, mut stop_rx: watch::Receiver<bool>) {
        let mut attempt: u32 = 0;
        let mut startup_reconciled = false;

        loop {
            if *stop_rx.borrow() {
                tracing::info!("HL fill subscriber: stop signal received, exiting");
                return;
            }

            // Startup reconciliation: query REST for fills from last 24h.
            if !startup_reconciled {
                let since = now_ms().saturating_sub(STARTUP_LOOKBACK_MS);
                tracing::info!(
                    "HL fill subscriber: startup reconciliation (last 24h from {})",
                    since
                );
                self.reconcile_since(since).await;
                startup_reconciled = true;
            }

            match self.connect_subscribe_and_run(&mut stop_rx).await {
                Ok(()) => {
                    // Clean shutdown via stop signal
                    return;
                }
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        error = %e,
                        "HL fill subscriber: connection cycle ended"
                    );
                }
            }

            // Exponential backoff before reconnecting
            let delay = reconnect_delay(attempt);
            tracing::info!("HL fill subscriber: reconnecting in {delay:?}");
            if wait_or_cancel(delay, &mut stop_rx).await {
                tracing::info!("HL fill subscriber: stop signal during backoff, exiting");
                return;
            }
            attempt += 1;
        }
    }

    /// Single connection lifecycle: connect → subscribe → read/write loop.
    /// Returns Ok(()) only on graceful shutdown. Returns Err on any failure.
    async fn connect_subscribe_and_run(
        &mut self,
        stop_rx: &mut watch::Receiver<bool>,
    ) -> Result<(), String> {
        let url = ws_url(self.network);

        // 1. Connect via tokio-tungstenite (TLS handled by native-tls feature)
        let (ws_stream, _response) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| format!("WS connect to {url}: {e}"))?;

        let (mut write, mut read) = ws_stream.split();

        // 2. Send subscription request
        let sub_payload = serde_json::json!({
            "method": "subscribe",
            "subscription": {
                "type": "orderUpdates",
                "user": format!("{:#x}", self.user_address),
            }
        });
        write
            .send(tungstenite::Message::Text(sub_payload.to_string()))
            .await
            .map_err(|e| format!("WS subscribe send: {e}"))?;

        tracing::info!(
            user = %format!("{:#x}", self.user_address),
            "HL fill subscriber: connected and subscribed (direct tokio-tungstenite)"
        );

        // 3. Reconcile fills missed during disconnect gap
        if let Some(last_ts) = self.last_event_timestamp {
            self.reconcile_since(last_ts).await;
        }

        // 4. Control loop: read frames, send pings, poll REST, check watchdog
        let mut ping_interval = tokio::time::interval(Duration::from_secs(PING_INTERVAL_SECS));
        ping_interval.tick().await; // skip first immediate tick

        let mut poll_interval = tokio::time::interval(Duration::from_secs(REST_POLL_INTERVAL_SECS));
        poll_interval.tick().await;

        let read_timeout = Duration::from_secs(READ_TIMEOUT_SECS);

        loop {
            tokio::select! {
                // Stop signal
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        tracing::info!("HL fill subscriber: stop signal, shutting down");
                        let _ = write.send(tungstenite::Message::Close(None)).await;
                        return Ok(());
                    }
                }

                // Read incoming frame (with timeout)
                result = tokio::time::timeout(read_timeout, read.next()) => {
                    match result {
                        Ok(Some(Ok(frame))) => {
                            self.handle_frame(frame).await;
                        }
                        Ok(Some(Err(e))) => {
                            return Err(format!("WS read error: {e}"));
                        }
                        Ok(None) => {
                            return Err("WS stream ended (server closed)".to_string());
                        }
                        Err(_) => {
                            return Err(format!(
                                "WS read timeout: no frame received in {}s",
                                READ_TIMEOUT_SECS
                            ));
                        }
                    }
                }

                // Send application-level ping every 30s
                _ = ping_interval.tick() => {
                    let ping_msg = r#"{"method":"ping"}"#;
                    if let Err(e) = write.send(tungstenite::Message::Text(ping_msg.to_string())).await {
                        return Err(format!("WS ping send failed: {e}"));
                    }
                    tracing::trace!("HL fill subscriber: sent keepalive ping");
                }

                // Periodic REST poll every 30s
                _ = poll_interval.tick() => {
                    let since = now_ms().saturating_sub(REST_POLL_LOOKBACK_MS);
                    self.reconcile_since(since).await;
                }
            }
        }
    }

    /// Process a single WebSocket frame.
    async fn handle_frame(&mut self, frame: tungstenite::Message) {
        match frame {
            tungstenite::Message::Text(text) => {
                self.handle_text_message(&text).await;
            }
            tungstenite::Message::Ping(data) => {
                // Protocol-level ping — pong is handled automatically by tungstenite
                tracing::trace!(len = data.len(), "HL fill subscriber: received WS ping");
            }
            tungstenite::Message::Close(_) => {
                tracing::info!("HL fill subscriber: received close frame");
            }
            _ => {
                // Binary, Pong, Frame — ignore
            }
        }
    }

    /// Parse and route a text message from the WS.
    async fn handle_text_message(&mut self, text: &str) {
        // HL sends `{"channel":"pong"}` in response to our ping
        if text.contains("\"pong\"") {
            tracing::trace!("HL fill subscriber: received pong");
            return;
        }

        // HL sends subscription confirmation: `{"channel":"subscriptionResponse",...}`
        if text.contains("\"subscriptionResponse\"") {
            tracing::debug!("HL fill subscriber: subscription confirmed");
            return;
        }

        // Parse order updates: `{"channel":"orderUpdates","data":[...]}`
        let msg: HlWsMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => {
                tracing::trace!(
                    text_len = text.len(),
                    "HL fill subscriber: ignoring unparseable message"
                );
                return;
            }
        };

        if msg.channel.as_deref() != Some("orderUpdates") {
            return;
        }

        let Some(data) = msg.data else { return };

        // data is an array of OrderUpdate objects
        let updates: Vec<HlOrderUpdate> = match serde_json::from_value(data) {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("HL fill subscriber: failed to parse orderUpdates data: {e}");
                return;
            }
        };

        for update in &updates {
            self.handle_order_update(update).await;
        }
    }

    /// Handle a single order update: translate, enrich fill price, dedup, send.
    async fn handle_order_update(&mut self, update: &HlOrderUpdate) {
        // FIX-02: Update watermark
        self.last_event_timestamp = Some(update.status_timestamp);

        // FIX-02: Dedup by OID
        if !self.record_oid(update.order.oid) {
            tracing::debug!(
                oid = %update.order.oid,
                "HL fill subscriber: duplicate OID, skipping"
            );
            return;
        }

        match Self::translate(update) {
            Some(mut event) => {
                // FIX-02: Enrich fill price for closed orders via REST
                if event.status == "closed" {
                    enrich_fill_price(
                        &self.info,
                        self.user_address,
                        &mut event,
                        update.status_timestamp,
                    )
                    .await;
                }

                if let Err(e) = self.order_update_sender.send(event).await {
                    tracing::error!("HL fill subscriber: channel send failed: {e}");
                }
            }
            None => {
                // FIX-01: Parse failure logged in translate(), skip this update
            }
        }
    }

    /// FIX-02: Reconcile fills that may have been missed during WS disconnect.
    /// REL-02: Also writes closing fills directly to journal via JournalService.
    async fn reconcile_since(&mut self, since_ts: u64) {
        tracing::info!(
            since_ts = %since_ts,
            "FIX-02: reconciling fills since watermark"
        );

        let fills = match self.info.user_fills_by_time(
            self.user_address,
            since_ts,
            None,
            None,
        ).await {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("FIX-02: fill reconciliation query failed: {e}");
                return;
            }
        };

        // Group fills by OID for FillDetector path (entry fills)
        let mut oid_fills: HashMap<u64, Vec<&UserFillByTime>> = HashMap::new();
        for fill in &fills {
            oid_fills.entry(fill.oid).or_default().push(fill);
        }

        let mut reconciled = 0u32;
        for (oid, fills) in &oid_fills {
            if !self.record_oid(*oid) {
                continue; // Already seen
            }

            if let Some(event) = build_event_from_fills(*oid, fills) {
                if let Err(e) = self.order_update_sender.send(event).await {
                    tracing::error!("FIX-02: reconciliation channel send failed: {e}");
                    return;
                }
                reconciled += 1;
            }
        }

        if reconciled > 0 {
            tracing::info!(
                reconciled = %reconciled,
                "FIX-02: reconciled missed fills"
            );
        }

        // JNL-SYNC-01 CP-6: Journal writes removed — JournalSyncer is now the sole
        // journal authority. Retain only live-state updates: group reconciliation + pg_notify.
        let user_id = self.user_id;
        let notify_pool = self.notify_pool.clone();
        let engine_handle = self.engine_handle.clone();
        let exchange_api = self.exchange_api.clone();
        if let Some(user_id) = user_id {
            for fill in &fills {
                if fill.closed_pnl == "0" || fill.closed_pnl == "0.0" {
                    continue;
                }
                // Dedup by TID (prevents re-processing same fill on WS reconnect).
                if !self.record_tid(fill.tid as i64) {
                    continue;
                }
                if let (Some(ref eh), Some(ref ea)) = (&engine_handle, &exchange_api) {
                    if let Ok(exit_price) = Decimal::from_str(&fill.px) {
                        Self::reconcile_group(
                            eh,
                            ea,
                            user_id,
                            &fill.coin,
                            exit_price,
                            &fill.dir,
                            notify_pool.as_ref(),
                        )
                        .await;
                    }
                } else if let Some(pool) = notify_pool.as_ref() {
                    let channel = format!("order.{}", user_id);
                    let payload = serde_json::json!({
                        "stream": channel,
                        "data": {
                            "e": "trade_closed",
                            "s": format!("{}_USDT", fill.coin),
                            "status": "closed",
                        }
                    });
                    if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
                        .bind(&channel)
                        .bind(payload.to_string())
                        .execute(pool)
                        .await
                    {
                        tracing::warn!("pg_notify failed: {e}");
                    }
                }
            }
        }
    }

    /// REL-03: After a journal write, find the matching OrderGroup and transition
    /// to terminal state. Best-effort cancel sibling orders and emit pg_notify.
    async fn reconcile_group(
        engine_handle: &EngineHandle,
        exchange_api: &Arc<dyn ExchangeApi>,
        user_id: Uuid,
        symbol: &str,
        exit_price: Decimal,
        fill_side: &str,
        notify_pool: Option<&PgPool>,
    ) {
        let active_groups = engine_handle.get_active_groups(user_id).await;
        // HL uses coin name (e.g. "BTC"), groups use "BTC_USDT" format
        let symbol_usdt = format!("{}_USDT", symbol);
        let matching: Vec<_> = active_groups
            .iter()
            .filter(|g| g.symbol == symbol_usdt || g.symbol == symbol)
            .collect();

        match matching.len() {
            1 => {
                let group = &matching[0];

                // Skip if already terminal (race with FillDetector)
                if group.status.is_terminal() {
                    tracing::debug!(
                        group_id = %group.id,
                        status = ?group.status,
                        "REL-03: group already terminal, skipping"
                    );
                    return;
                }

                // Determine terminal status based on P&L direction
                let terminal_status = if fill_side.starts_with("Close Long") {
                    // Closing a long: exit < entry = stopped out
                    if exit_price < group.entry_price.unwrap_or(exit_price) {
                        OrderGroupStatus::StoppedOut
                    } else {
                        OrderGroupStatus::TookProfit
                    }
                } else {
                    // Closing a short: exit > entry = stopped out
                    if exit_price > group.entry_price.unwrap_or(exit_price) {
                        OrderGroupStatus::StoppedOut
                    } else {
                        OrderGroupStatus::TookProfit
                    }
                };

                // CP-1: Transition group to terminal state
                if let Err(e) = engine_handle
                    .update_group_status(group.id, terminal_status)
                    .await
                {
                    tracing::warn!(
                        group_id = %group.id,
                        error = %e,
                        "REL-03: failed to transition group to terminal"
                    );
                    return;
                }

                let event_type = match terminal_status {
                    OrderGroupStatus::StoppedOut => "stopped_out",
                    OrderGroupStatus::TookProfit => "took_profit",
                    _ => "closed",
                };

                tracing::info!(
                    group_id = %group.id,
                    symbol = %symbol,
                    status = ?terminal_status,
                    "REL-03: group transitioned to terminal"
                );

                // CP-2: Best-effort cancel sibling orders on exchange
                let order_ids = [
                    group.exchange_order_id.clone(),
                    group.exchange_sl_order_id.clone(),
                    group.exchange_tp_order_id.clone(),
                ];
                for order_id in order_ids.into_iter().flatten() {
                    match exchange_api
                        .cancel_order(
                            user_id,
                            &order_id,
                            &symbol_usdt,
                            group.exchange_account_id,
                        )
                        .await
                    {
                        Ok(()) => {
                            tracing::info!(
                                order_id = %order_id,
                                group_id = %group.id,
                                "REL-03: cancelled sibling order"
                            );
                        }
                        Err(ExchangeApiError::OrderNotFound(_)) => {
                            tracing::debug!(
                                order_id = %order_id,
                                group_id = %group.id,
                                "REL-03: sibling order already gone"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                order_id = %order_id,
                                group_id = %group.id,
                                error = %e,
                                "REL-03: failed to cancel sibling order"
                            );
                        }
                    }
                }

                // CP-3: Emit pg_notify for extension UI (toast notification)
                if let Some(pool) = notify_pool {
                    let channel = format!("order.{}", user_id);
                    let payload = serde_json::json!({
                        "stream": channel,
                        "data": {
                            "e": event_type,
                            "s": symbol_usdt,
                            "status": "closed",
                            "group_id": group.id.to_string(),
                        }
                    });
                    if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
                        .bind(&channel)
                        .bind(payload.to_string())
                        .execute(pool)
                        .await
                    {
                        tracing::warn!("REL-03: pg_notify failed: {e}");
                    }
                }
            }
            0 => {
                // Normal for import-only trades or trades placed outside testudo
            }
            count => {
                tracing::warn!(
                    user_id = %user_id,
                    symbol = %symbol,
                    count,
                    "REL-03: ambiguous group match, skipping cleanup"
                );
            }
        }
    }

    /// REL-02: Record a TID as seen. Returns `true` if newly inserted.
    fn record_tid(&mut self, tid: i64) -> bool {
        if self.seen_tids.len() >= MAX_SEEN_TIDS {
            self.seen_tids.clear();
        }
        self.seen_tids.insert(tid)
    }

    /// Translate a Hyperliquid order update into a Testudo `OrderUpdateEvent`.
    fn translate(update: &HlOrderUpdate) -> Option<OrderUpdateEvent> {
        let order = &update.order;

        let status = match update.status.as_str() {
            "filled" => "closed".to_string(),
            other => other.to_string(),
        };

        let side = match order.side.as_str() {
            "A" => "sell".to_string(),
            "B" => "buy".to_string(),
            other => other.to_lowercase(),
        };

        let price = match Decimal::from_str(&order.limit_px) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::error!("HL fill: failed to parse limit_px '{}': {}", order.limit_px, e);
                return None;
            }
        };

        let average = if update.status == "filled" {
            price // Placeholder; enrich_fill_price overwrites with REST avg_px
        } else {
            None
        };

        Some(OrderUpdateEvent {
            id: order.oid.to_string(),
            symbol: AssetUniverse::from_hl_coin(&order.coin),
            status,
            side,
            average,
            timestamp: Some(update.status_timestamp as i64),
            user_id: None,
        })
    }

    /// Record an OID as seen. Returns `true` if newly inserted, `false` if duplicate.
    fn record_oid(&mut self, oid: u64) -> bool {
        if self.seen_oids.len() >= MAX_SEEN_OIDS {
            self.seen_oids.clear();
        }
        self.seen_oids.insert(oid)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// FIX-02: Query REST for actual fill price and update `event.average`.
async fn enrich_fill_price(
    info: &InfoProvider,
    user_address: Address,
    event: &mut OrderUpdateEvent,
    status_timestamp: u64,
) {
    let oid: u64 = match event.id.parse() {
        Ok(v) => v,
        Err(_) => return,
    };

    let start_time = status_timestamp.saturating_sub(FILL_LOOKBACK_MS);

    match info.user_fills_by_time(user_address, start_time, None, None).await {
        Ok(fills) => {
            if let Some(px) = compute_avg_price(oid, &fills) {
                event.average = Some(px);
                tracing::debug!(oid, avg_px = %px, "FIX-02: enriched fill price from REST");
            } else {
                // Retry once after 500ms (stale data mitigation)
                tokio::time::sleep(Duration::from_millis(500)).await;
                match info.user_fills_by_time(user_address, start_time, None, None).await {
                    Ok(fills) => {
                        if let Some(px) = compute_avg_price(oid, &fills) {
                            event.average = Some(px);
                            tracing::debug!(oid, avg_px = %px, "FIX-02: enriched fill price (retry)");
                        } else {
                            tracing::warn!(oid, "FIX-02: no fills found for OID after retry, using limit_px");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(oid, error = %e, "FIX-02: REST retry failed, using limit_px");
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!(oid, error = %e, "FIX-02: fill price reconciliation failed, using limit_px");
        }
    }
}

fn compute_avg_price(oid: u64, fills: &[UserFillByTime]) -> Option<Decimal> {
    let mut total_value = Decimal::ZERO;
    let mut total_sz = Decimal::ZERO;

    for fill in fills {
        if fill.oid != oid {
            continue;
        }
        if let (Ok(px), Ok(sz)) = (
            Decimal::from_str(&fill.px),
            Decimal::from_str(&fill.sz),
        ) {
            total_value += px * sz;
            total_sz += sz;
        }
    }

    if total_sz > Decimal::ZERO {
        Some(total_value / total_sz)
    } else {
        None
    }
}

fn build_event_from_fills(oid: u64, fills: &[&UserFillByTime]) -> Option<OrderUpdateEvent> {
    let first = fills.first()?;

    let side = match first.side.as_str() {
        "A" => "sell".to_string(),
        "B" => "buy".to_string(),
        other => other.to_lowercase(),
    };

    let mut total_value = Decimal::ZERO;
    let mut total_sz = Decimal::ZERO;
    let mut latest_time: u64 = 0;

    for fill in fills {
        if let (Ok(px), Ok(sz)) = (
            Decimal::from_str(&fill.px),
            Decimal::from_str(&fill.sz),
        ) {
            total_value += px * sz;
            total_sz += sz;
        }
        latest_time = latest_time.max(fill.time);
    }

    if total_sz == Decimal::ZERO {
        return None;
    }

    let avg_px = total_value / total_sz;

    Some(OrderUpdateEvent {
        id: oid.to_string(),
        symbol: AssetUniverse::from_hl_coin(&first.coin),
        status: "closed".to_string(),
        side,
        average: Some(avg_px),
        timestamp: Some(latest_time as i64),
        user_id: None,
    })
}

use crate::utils::reconnect::{reconnect_delay, wait_or_cancel};

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_order_update(
        coin: &str,
        side: &str,
        status: &str,
        limit_px: &str,
        orig_sz: &str,
        sz: &str,
        oid: u64,
        cloid: Option<&str>,
    ) -> HlOrderUpdate {
        HlOrderUpdate {
            order: HlBasicOrder {
                coin: coin.to_string(),
                side: side.to_string(),
                limit_px: limit_px.to_string(),
                sz: sz.to_string(),
                oid,
                orig_sz: orig_sz.to_string(),
                cloid: cloid.map(|s| s.to_string()),
            },
            status: status.to_string(),
            status_timestamp: 1710000001000,
        }
    }

    fn make_fill(coin: &str, side: &str, px: &str, sz: &str, oid: u64, time: u64) -> UserFillByTime {
        UserFillByTime {
            closed_pnl: "0".to_string(),
            coin: coin.to_string(),
            crossed: true,
            dir: "Open Long".to_string(),
            hash: "0xabc".to_string(),
            oid,
            px: px.to_string(),
            side: side.to_string(),
            start_position: "0".to_string(),
            sz: sz.to_string(),
            time,
            fee: "0.5".to_string(),
            fee_token: "USDC".to_string(),
            tid: 1,
            cloid: None,
        }
    }

    // ==================== translate() tests ====================

    #[test]
    fn translate_filled_order() {
        let update = make_order_update("BTC", "B", "filled", "65000.0", "0.1", "0.0", 12345, None);
        let event = HyperliquidFillSubscriber::translate(&update).unwrap();

        assert_eq!(event.id, "12345");
        assert_eq!(event.symbol, "BTC_USDT");
        assert_eq!(event.status, "closed");
        assert_eq!(event.side, "buy");
        // FIX-09: price/amount/filled/remaining stripped from OrderUpdateEvent
        assert_eq!(event.average, Some(dec!(65000.0)));
        assert_eq!(event.timestamp, Some(1710000001000));
    }

    #[test]
    fn translate_canceled_order() {
        let update =
            make_order_update("ETH", "A", "canceled", "3500.0", "1.0", "1.0", 67890, None);
        let event = HyperliquidFillSubscriber::translate(&update).unwrap();

        assert_eq!(event.id, "67890");
        assert_eq!(event.symbol, "ETH_USDT");
        assert_eq!(event.status, "canceled");
        assert_eq!(event.side, "sell");
        // FIX-09: price/amount/filled/remaining stripped from OrderUpdateEvent
        assert_eq!(event.average, None);
        assert_eq!(event.timestamp, Some(1710000001000));
    }

    #[test]
    fn translate_open_order() {
        let update =
            make_order_update("SOL", "B", "open", "180.5", "10.0", "10.0", 11111, None);
        let event = HyperliquidFillSubscriber::translate(&update).unwrap();

        assert_eq!(event.id, "11111");
        assert_eq!(event.symbol, "SOL_USDT");
        assert_eq!(event.status, "open");
        assert_eq!(event.side, "buy");
        // FIX-09: price/amount/filled/remaining stripped from OrderUpdateEvent
        assert_eq!(event.average, None);
    }

    #[test]
    fn translate_partial_fill() {
        let update =
            make_order_update("BTC", "A", "open", "64000.0", "0.5", "0.3", 22222, None);
        let event = HyperliquidFillSubscriber::translate(&update).unwrap();

        assert_eq!(event.status, "open");
        assert_eq!(event.side, "sell");
        // FIX-09: amount/filled/remaining stripped from OrderUpdateEvent
        assert_eq!(event.average, None);
    }

    #[test]
    fn translate_with_cloid() {
        let update = make_order_update(
            "DOGE",
            "B",
            "filled",
            "0.15",
            "1000.0",
            "0.0",
            33333,
            Some("testudo:abc:entry"),
        );
        let event = HyperliquidFillSubscriber::translate(&update).unwrap();

        assert_eq!(event.id, "33333");
        assert_eq!(event.symbol, "DOGE_USDT");
        assert_eq!(event.status, "closed");
        // FIX-09: filled stripped from OrderUpdateEvent — verified via average
        assert_eq!(event.average, Some(dec!(0.15)));
    }

    #[test]
    fn translate_side_mapping() {
        let buy = make_order_update("BTC", "B", "open", "1.0", "1.0", "1.0", 1, None);
        assert_eq!(HyperliquidFillSubscriber::translate(&buy).unwrap().side, "buy");

        let sell = make_order_update("BTC", "A", "open", "1.0", "1.0", "1.0", 2, None);
        assert_eq!(HyperliquidFillSubscriber::translate(&sell).unwrap().side, "sell");
    }

    #[test]
    fn translate_symbol_normalization() {
        let update = make_order_update("AVAX", "B", "open", "1.0", "1.0", "1.0", 1, None);
        assert_eq!(
            HyperliquidFillSubscriber::translate(&update).unwrap().symbol,
            "AVAX_USDT"
        );
    }

    #[test]
    fn translate_parse_failure_returns_none() {
        // Only limit_px failure causes None now that orig_sz/sz are no longer parsed
        // (FIX-09: size fields stripped from OrderUpdateEvent).
        let update = make_order_update("BTC", "B", "open", "invalid", "1.0", "1.0", 1, None);
        assert!(HyperliquidFillSubscriber::translate(&update).is_none());

        // These no longer fail — orig_sz/sz are not parsed after FIX-09
        let update = make_order_update("BTC", "B", "open", "1.0", "bad", "1.0", 2, None);
        assert!(HyperliquidFillSubscriber::translate(&update).is_some());

        let update = make_order_update("BTC", "B", "open", "1.0", "1.0", "nope", 3, None);
        assert!(HyperliquidFillSubscriber::translate(&update).is_some());
    }

    // ==================== compute_avg_price tests ====================

    #[test]
    fn compute_avg_price_single_fill() {
        let fills = vec![make_fill("BTC", "B", "65432.10", "0.1", 100, 1710000000)];
        let avg = compute_avg_price(100, &fills).unwrap();
        assert_eq!(avg, dec!(65432.10));
    }

    #[test]
    fn compute_avg_price_multiple_fills_same_oid() {
        let fills = vec![
            make_fill("BTC", "B", "65000", "0.3", 100, 1710000000),
            make_fill("BTC", "B", "65100", "0.2", 100, 1710000001),
        ];
        let avg = compute_avg_price(100, &fills).unwrap();
        assert_eq!(avg, dec!(65040));
    }

    #[test]
    fn compute_avg_price_filters_by_oid() {
        let fills = vec![
            make_fill("BTC", "B", "65000", "0.1", 100, 1710000000),
            make_fill("ETH", "A", "3500", "1.0", 200, 1710000000),
        ];
        let avg = compute_avg_price(100, &fills).unwrap();
        assert_eq!(avg, dec!(65000));
    }

    #[test]
    fn compute_avg_price_no_matching_fills() {
        let fills = vec![make_fill("BTC", "B", "65000", "0.1", 200, 1710000000)];
        assert!(compute_avg_price(100, &fills).is_none());
    }

    #[test]
    fn compute_avg_price_empty_fills() {
        let fills: Vec<UserFillByTime> = vec![];
        assert!(compute_avg_price(100, &fills).is_none());
    }

    // ==================== build_event_from_fills tests ====================

    #[test]
    fn build_event_single_fill() {
        let fill = make_fill("BTC", "B", "65000", "0.1", 100, 1710000000);
        let fills: Vec<&UserFillByTime> = vec![&fill];
        let event = build_event_from_fills(100, &fills).unwrap();

        assert_eq!(event.id, "100");
        assert_eq!(event.symbol, "BTC_USDT");
        assert_eq!(event.status, "closed");
        assert_eq!(event.side, "buy");
        // FIX-09: price/amount/filled/remaining stripped from OrderUpdateEvent
        assert_eq!(event.average, Some(dec!(65000)));
        assert_eq!(event.timestamp, Some(1710000000));
    }

    #[test]
    fn build_event_multiple_fills_weighted_avg() {
        let f1 = make_fill("BTC", "A", "64000", "0.3", 100, 1710000000);
        let f2 = make_fill("BTC", "A", "64200", "0.2", 100, 1710000001);
        let fills: Vec<&UserFillByTime> = vec![&f1, &f2];
        let event = build_event_from_fills(100, &fills).unwrap();

        assert_eq!(event.average, Some(dec!(64080)));
        // FIX-09: amount stripped from OrderUpdateEvent
        assert_eq!(event.side, "sell");
        assert_eq!(event.timestamp, Some(1710000001));
    }

    #[test]
    fn build_event_empty_fills() {
        let fills: Vec<&UserFillByTime> = vec![];
        assert!(build_event_from_fills(100, &fills).is_none());
    }

    // ==================== record_oid (dedup) tests ====================

    #[test]
    fn record_oid_new_returns_true() {
        let mut sub = HyperliquidFillSubscriber::new(
            Network::Testnet,
            Address::ZERO,
            mpsc::channel(1).0,
        );
        assert!(sub.record_oid(1));
        assert!(sub.record_oid(2));
    }

    #[test]
    fn record_oid_duplicate_returns_false() {
        let mut sub = HyperliquidFillSubscriber::new(
            Network::Testnet,
            Address::ZERO,
            mpsc::channel(1).0,
        );
        assert!(sub.record_oid(1));
        assert!(!sub.record_oid(1));
    }

    #[test]
    fn record_oid_clears_at_capacity() {
        let mut sub = HyperliquidFillSubscriber::new(
            Network::Testnet,
            Address::ZERO,
            mpsc::channel(1).0,
        );
        for i in 0..MAX_SEEN_OIDS {
            sub.record_oid(i as u64);
        }
        assert_eq!(sub.seen_oids.len(), MAX_SEEN_OIDS);
        assert!(sub.record_oid(99999));
        assert_eq!(sub.seen_oids.len(), 1);
    }

    // ==================== WS message parsing tests ====================

    #[test]
    fn parse_hl_order_update_json() {
        let json = r#"{"order":{"coin":"BTC","side":"B","limitPx":"65000.0","sz":"0.0","oid":12345,"timestamp":1710000000000,"origSz":"0.1","cloid":null},"status":"filled","statusTimestamp":1710000001000}"#;
        let update: HlOrderUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.order.oid, 12345);
        assert_eq!(update.status, "filled");
        assert_eq!(update.order.coin, "BTC");
    }

    #[test]
    fn parse_hl_ws_message_order_updates() {
        let json = r#"{"channel":"orderUpdates","data":[{"order":{"coin":"ETH","side":"A","limitPx":"3500.0","sz":"1.0","oid":99,"timestamp":1710000000000,"origSz":"1.0","cloid":null},"status":"canceled","statusTimestamp":1710000002000}]}"#;
        let msg: HlWsMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.channel.as_deref(), Some("orderUpdates"));
        let updates: Vec<HlOrderUpdate> = serde_json::from_value(msg.data.unwrap()).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].order.oid, 99);
    }
}
