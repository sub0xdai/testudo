use crate::metrics;
use crate::repositories::exchange_account::{ExchangeAccountRepository, RepoError};
use crate::services::cex_client::{CexClient, OrderUpdateEvent, SidecarCredentials};
use crate::services::exchange_api::ExchangeApi;
use crate::services::hyperliquid::auth::{AuthCache, HyperliquidAuth};
use crate::services::hyperliquid::ws_fills::HyperliquidFillSubscriber;
use crate::types::exchange_names::{auth_modes, exchanges};
use engine::EngineHandle;
use hyperliquid_sdk_rs::Network;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch, Mutex};
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SubscriptionKey {
    user_id: Uuid,
    exchange_account_id: Uuid,
}

struct SubscriptionEntry {
    symbols: HashSet<String>,
    stop_tx: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionAction {
    Started,
    Reused,
}

#[derive(Clone)]
pub struct WsSubscriptionManager {
    cex_client: Arc<CexClient>,
    exchange_account_repo: ExchangeAccountRepository,
    /// 017 FR-1: mpsc sender for fill events — backpressure instead of silent drops.
    order_update_sender: mpsc::Sender<OrderUpdateEvent>,
    sandbox: bool,
    entries: Arc<Mutex<HashMap<SubscriptionKey, SubscriptionEntry>>>,
    /// HL-05: Optional Hyperliquid network for native WS subscription.
    hl_network: Option<Network>,
    /// HL-05: Optional auth cache for resolving HL user addresses.
    hl_auth_cache: Option<Arc<AuthCache>>,
    /// REL-02: Optional journal service for direct HL closing fill writes.
    /// REL-02: PgPool for pg_notify after journal writes.
    hl_notify_pool: Option<PgPool>,
    /// REL-03: Optional engine handle for group reconciliation after HL journal writes.
    hl_engine_handle: Option<EngineHandle>,
    /// REL-03: Optional exchange API for cancelling sibling orders after HL position close.
    hl_exchange_api: Option<Arc<dyn ExchangeApi>>,
}

impl WsSubscriptionManager {
    pub fn new(
        cex_client: Arc<CexClient>,
        exchange_account_repo: ExchangeAccountRepository,
        order_update_sender: mpsc::Sender<OrderUpdateEvent>,
        sandbox: bool,
    ) -> Self {
        Self {
            cex_client,
            exchange_account_repo,
            order_update_sender,
            sandbox,
            entries: Arc::new(Mutex::new(HashMap::new())),
            hl_network: None,
            hl_auth_cache: None,
            hl_notify_pool: None,
            hl_engine_handle: None,
            hl_exchange_api: None,
        }
    }

    /// HL-05: Enable Hyperliquid native WS subscriptions.
    pub fn with_hyperliquid(mut self, network: Network, auth_cache: Arc<AuthCache>) -> Self {
        self.hl_network = Some(network);
        self.hl_auth_cache = Some(auth_cache);
        self
    }

    /// JNL-SYNC-01: Enable pg_notify pool for HL closing fill state updates.
    pub fn with_hl_notify_pool(mut self, pool: PgPool) -> Self {
        self.hl_notify_pool = Some(pool);
        self
    }

    /// REL-03: Enable group reconciliation for HL closing fills.
    pub fn with_engine(
        mut self,
        engine_handle: EngineHandle,
        exchange_api: Arc<dyn ExchangeApi>,
    ) -> Self {
        self.hl_engine_handle = Some(engine_handle);
        self.hl_exchange_api = Some(exchange_api);
        self
    }

    /// Remove subscription entries whose background tasks have finished.
    /// Called on each `ensure_subscribed` to prevent unbounded HashMap growth.
    pub async fn prune_finished(&self) {
        let mut entries = self.entries.lock().await;
        let before = entries.len();
        entries.retain(|_key, entry| !entry.handle.is_finished());
        let pruned = before - entries.len();
        if pruned > 0 {
            tracing::info!("prune_finished removed {} stale subscription entries", pruned);
        }
    }

    pub async fn ensure_subscribed(
        &self,
        user_id: Uuid,
        exchange_account_id: Uuid,
        symbol: &str,
    ) -> Result<SubscriptionAction, String> {
        // FIX-07: Prune finished entries to prevent unbounded growth
        self.prune_finished().await;

        let key = SubscriptionKey {
            user_id,
            exchange_account_id,
        };

        let normalized_symbol = symbol.trim().to_string();
        if normalized_symbol.is_empty() {
            return Err("symbol cannot be empty".to_string());
        }

        tracing::info!(
            "subscribe_requested user_id={} exchange_account_id={} symbol={}",
            user_id,
            exchange_account_id,
            normalized_symbol
        );

        let mut entries = self.entries.lock().await;
        if let Some(entry) = entries.get_mut(&key) {
            if entry.handle.is_finished() {
                let symbols: Vec<String> = entry.symbols.iter().cloned().collect();
                let (stop_tx, stop_rx) = watch::channel(false);
                let handle = self.spawn_subscription_task(key, symbols, stop_rx);
                entry.stop_tx = stop_tx;
                entry.handle = handle;

                tracing::info!(
                    "subscribe_started user_id={} exchange_account_id={} reason=task_restarted",
                    user_id,
                    exchange_account_id,
                );
            }

            if entry.symbols.contains(&normalized_symbol) {
                tracing::info!(
                    "subscribe_reused user_id={} exchange_account_id={} symbol={} reason=already_subscribed",
                    user_id,
                    exchange_account_id,
                    normalized_symbol
                );
                return Ok(SubscriptionAction::Reused);
            }

            entry.symbols.insert(normalized_symbol.clone());
            let symbols: Vec<String> = entry.symbols.iter().cloned().collect();

            let _ = entry.stop_tx.send(true);
            entry.handle.abort();

            let (stop_tx, stop_rx) = watch::channel(false);
            let handle = self.spawn_subscription_task(key, symbols, stop_rx);
            entry.stop_tx = stop_tx;
            entry.handle = handle;

            tracing::info!(
                "subscribe_reused user_id={} exchange_account_id={} symbol={} reason=symbol_fan_in",
                user_id,
                exchange_account_id,
                normalized_symbol
            );
            return Ok(SubscriptionAction::Reused);
        }

        let symbols = vec![normalized_symbol.clone()];
        let symbol_set = symbols.iter().cloned().collect::<HashSet<_>>();
        let (stop_tx, stop_rx) = watch::channel(false);
        let handle = self.spawn_subscription_task(key, symbols, stop_rx);

        entries.insert(
            key,
            SubscriptionEntry {
                symbols: symbol_set,
                stop_tx,
                handle,
            },
        );

        tracing::info!(
            "subscribe_started user_id={} exchange_account_id={} symbols=1",
            user_id,
            exchange_account_id
        );

        Ok(SubscriptionAction::Started)
    }

    fn spawn_subscription_task(
        &self,
        key: SubscriptionKey,
        symbols: Vec<String>,
        mut stop_rx: watch::Receiver<bool>,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            manager
                .run_subscription_task(key, symbols, &mut stop_rx)
                .await;
        })
    }

    async fn run_subscription_task(
        &self,
        key: SubscriptionKey,
        symbols: Vec<String>,
        stop_rx: &mut watch::Receiver<bool>,
    ) {
        let mut attempt: u32 = 0;
        let mut consecutive_empty: u32 = 0;
        const MAX_CONSECUTIVE_EMPTY: u32 = 3;

        loop {
            if *stop_rx.borrow() {
                return;
            }

            let creds = match self
                .exchange_account_repo
                .load_credentials(key.exchange_account_id, key.user_id)
                .await
            {
                Ok(c) => c,
                Err(RepoError::NotFound) => {
                    // Account deactivated or deleted — stop retrying permanently
                    tracing::warn!(
                        "stream_terminated user_id={} exchange_account_id={} reason=account_not_found_or_deactivated",
                        key.user_id,
                        key.exchange_account_id,
                    );
                    return;
                }
                Err(e) => {
                    tracing::warn!(
                        "stream_disconnected user_id={} exchange_account_id={} reason=credential_load_error error={}",
                        key.user_id,
                        key.exchange_account_id,
                        e
                    );
                    let delay = reconnect_delay(attempt);
                    tracing::info!(
                        "stream_reconnect_scheduled user_id={} exchange_account_id={} delay_ms={}",
                        key.user_id,
                        key.exchange_account_id,
                        delay.as_millis()
                    );
                    if wait_or_cancel(delay, stop_rx).await {
                        return;
                    }
                    attempt = attempt.saturating_add(1);
                    continue;
                }
            };

            // HL-05: Route Hyperliquid accounts to native WS subscriber
            if creds.exchange_name.eq_ignore_ascii_case(exchanges::HYPERLIQUID) {
                if let (Some(network), Some(auth_cache)) =
                    (self.hl_network, self.hl_auth_cache.as_ref())
                {
                    let auth_result = match creds.auth_mode.as_str() {
                        auth_modes::AGENT_WALLET => {
                            if let Some(ref wallet_addr) = creds.wallet_address {
                                auth_cache
                                    .get_or_insert_agent(
                                        key.exchange_account_id,
                                        &creds.api_secret,
                                        wallet_addr,
                                    )
                                    .await
                            } else {
                                Err(super::hyperliquid::auth::AuthError::MissingWalletAddress)
                            }
                        }
                        _ => {
                            auth_cache
                                .get_or_insert(
                                    key.exchange_account_id,
                                    &creds.api_key,
                                    &creds.api_secret,
                                )
                                .await
                        }
                    };
                    match auth_result {
                        Ok(auth) => {
                            tracing::info!(
                                "subscribe_started user_id={} exchange_account_id={} backend=hyperliquid",
                                key.user_id,
                                key.exchange_account_id,
                            );
                            let mut subscriber = HyperliquidFillSubscriber::new(
                                network,
                                auth.query_address(),
                                self.order_update_sender.clone(),
                            );
                            // JNL-SYNC-01: Wire user_id + pool for group reconciliation / pg_notify.
                            if let Some(pool) = self.hl_notify_pool.as_ref() {
                                subscriber = subscriber.with_user(key.user_id, pool.clone());
                            }
                            // REL-03: Wire engine + exchange API for group reconciliation
                            if let (Some(engine_handle), Some(exchange_api)) =
                                (self.hl_engine_handle.as_ref(), self.hl_exchange_api.as_ref())
                            {
                                subscriber = subscriber.with_engine(
                                    engine_handle.clone(),
                                    exchange_api.clone(),
                                );
                            }
                            // Run the native HL subscriber — it handles its own reconnection.
                            // FIX-02: run() is now &mut self for watermark/dedup state.
                            subscriber.run(stop_rx.clone()).await;
                            return;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "stream_disconnected user_id={} exchange_account_id={} reason=hl_auth_error error={}",
                                key.user_id,
                                key.exchange_account_id,
                                e
                            );
                            let delay = reconnect_delay(attempt);
                            if wait_or_cancel(delay, stop_rx).await {
                                return;
                            }
                            attempt = attempt.saturating_add(1);
                            continue;
                        }
                    }
                } else {
                    tracing::warn!(
                        "stream_disconnected user_id={} exchange_account_id={} reason=hyperliquid_not_configured",
                        key.user_id,
                        key.exchange_account_id,
                    );
                    return;
                }
            }

            // Default path: sidecar WS via CexClient
            let sidecar_creds = SidecarCredentials {
                api_key: creds.api_key,
                secret: creds.api_secret,
                password: creds.passphrase,
            };

            let mut order_updates = match self
                .cex_client
                .subscribe_orders(
                    &creds.exchange_name,
                    &sidecar_creds,
                    self.sandbox,
                    symbols.clone(),
                )
                .await
            {
                Ok(rx) => rx,
                Err(e) => {
                    tracing::warn!(
                        "stream_disconnected user_id={} exchange_account_id={} reason=connect_failed error={}",
                        key.user_id,
                        key.exchange_account_id,
                        e
                    );
                    let delay = reconnect_delay(attempt);
                    tracing::info!(
                        "stream_reconnect_scheduled user_id={} exchange_account_id={} delay_ms={}",
                        key.user_id,
                        key.exchange_account_id,
                        delay.as_millis()
                    );
                    if wait_or_cancel(delay, stop_rx).await {
                        return;
                    }
                    attempt = attempt.saturating_add(1);
                    continue;
                }
            };

            metrics::WS_CONNECTIONS.inc();
            if attempt == 0 {
                tracing::info!(
                    "subscribe_started user_id={} exchange_account_id={} symbol_count={}",
                    key.user_id,
                    key.exchange_account_id,
                    symbols.len()
                );
            } else {
                tracing::info!(
                    "stream_reconnected user_id={} exchange_account_id={} attempts={}",
                    key.user_id,
                    key.exchange_account_id,
                    attempt
                );
            }

            attempt = 0;
            let mut received_events = false;

            loop {
                tokio::select! {
                    changed = stop_rx.changed() => {
                        if changed.is_ok() && *stop_rx.borrow() {
                            metrics::WS_CONNECTIONS.dec();
                            return;
                        }
                    }
                    recv = order_updates.recv() => {
                        match recv {
                            Ok(mut event) => {
                                received_events = true;
                                // CEX-08: Tag event with user_id for symbol-based fallback
                                // matching in fill detector (Bybit bracket orders).
                                event.user_id = Some(key.user_id);
                                // 017 FR-1: mpsc send applies backpressure when channel
                                // is full instead of silently dropping fill events.
                                if let Err(e) = self.order_update_sender.send(event).await {
                                    tracing::warn!(
                                        "forward_error user_id={} exchange_account_id={} reason={}",
                                        key.user_id,
                                        key.exchange_account_id,
                                        e
                                    );
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                tracing::warn!(
                                    "forward_error user_id={} exchange_account_id={} reason=lagged skipped={}",
                                    key.user_id,
                                    key.exchange_account_id,
                                    skipped
                                );
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                metrics::WS_CONNECTIONS.dec();
                                tracing::warn!(
                                    "stream_disconnected user_id={} exchange_account_id={} reason=stream_closed",
                                    key.user_id,
                                    key.exchange_account_id,
                                );
                                break;
                            }
                        }
                    }
                }
            }

            // If stream closed without delivering any events, the exchange likely
            // doesn't support watchOrders. Give up after repeated empty sessions.
            if received_events {
                consecutive_empty = 0;
            } else {
                consecutive_empty += 1;
                if consecutive_empty >= MAX_CONSECUTIVE_EMPTY {
                    metrics::WS_CONNECTIONS.dec();
                    tracing::warn!(
                        "stream_abandoned user_id={} exchange_account_id={} reason=consecutive_empty_sessions count={}",
                        key.user_id,
                        key.exchange_account_id,
                        consecutive_empty
                    );
                    return;
                }
            }

            let delay = reconnect_delay(consecutive_empty.max(1));
            tracing::info!(
                "stream_reconnect_scheduled user_id={} exchange_account_id={} delay_ms={}",
                key.user_id,
                key.exchange_account_id,
                delay.as_millis()
            );
            if wait_or_cancel(delay, stop_rx).await {
                return;
            }
            attempt = 1;
        }
    }
}

use crate::utils::reconnect::{reconnect_delay, wait_or_cancel};
