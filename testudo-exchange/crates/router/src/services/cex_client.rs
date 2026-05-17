//! CEX Sidecar Client
//!
//! HTTP client for communicating with the CEX Node.js sidecar service.
//! All exchange operations (balance, order, position) are routed through
//! this client to the sidecar at `http://127.0.0.1:3100`.

use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Configuration for the CEX sidecar connection.
#[derive(Debug, Clone)]
pub struct CexSidecarConfig {
    pub base_url: String,
    pub timeout_secs: u64,
    /// PSK for sidecar authentication. When set, injected as `X-Internal-Secret` header.
    pub psk: Option<String>,
}

impl CexSidecarConfig {
    pub fn from_env() -> Self {
        Self {
            base_url: std::env::var("CCXT_SIDECAR_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3100".to_string()),
            timeout_secs: std::env::var("CCXT_SIDECAR_TIMEOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            psk: std::env::var("SIDECAR_PSK").ok(),
        }
    }
}

/// Credentials passed to the sidecar per-request. Never logged or persisted.
pub struct SidecarCredentials {
    pub api_key: String,
    pub secret: String,
    pub password: Option<String>,
}

// Intentionally no Debug impl to prevent accidental credential logging.

/// Serializable credentials for the sidecar request envelope.
#[derive(Serialize)]
struct CredentialsPayload {
    #[serde(rename = "apiKey")]
    api_key: String,
    secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

/// Request envelope sent to every POST endpoint on the sidecar.
#[derive(Serialize)]
struct SidecarEnvelope<P: Serialize> {
    exchange_id: String,
    credentials: CredentialsPayload,
    sandbox: bool,
    params: P,
}

/// Errors from CEX sidecar operations.
#[derive(Debug, Error)]
pub enum CexClientError {
    #[error("Sidecar unavailable: {0}")]
    Unavailable(String),
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Insufficient funds")]
    InsufficientFunds,
    #[error("Order not found: {0}")]
    OrderNotFound(String),
    #[error("Rate limited")]
    RateLimited,
    #[error("Exchange error: {0}")]
    ExchangeError(String),
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
}

/// Sidecar error response body.
#[derive(Deserialize)]
struct SidecarErrorBody {
    error: String,
    #[allow(dead_code)]
    code: String,
}

// ── Response types (string decimals from sidecar) ──

#[derive(Deserialize, Debug)]
pub struct SidecarBalanceEntry {
    pub asset: String,
    pub total: String,
    pub free: String,
    pub used: String,
}

/// HIST-02: A single trade fill from the sidecar's fetchMyTrades response.
#[derive(Deserialize, Debug, Clone)]
pub struct SidecarFill {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub price: String,
    pub amount: String,
    pub cost: String,
    pub fee_cost: String,
    pub fee_currency: String,
    pub timestamp: i64,
}

/// HIST-02: Params for sidecar /trades endpoint.
#[derive(Serialize)]
struct TradesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    since: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<serde_json::Value>,
}

/// Deserialize a value that may be a JSON string or integer into a String.
fn string_or_int<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    struct StringOrInt;
    impl<'de> de::Visitor<'de> for StringOrInt {
        type Value = String;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a string or integer")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<String, E> {
            Ok(v.to_owned())
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<String, E> {
            Ok(v.to_string())
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<String, E> {
            Ok(v.to_string())
        }
    }
    deserializer.deserialize_any(StringOrInt)
}

/// Deserialize an optional value that may be a JSON string, integer, or null.
fn option_string_or_int<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    struct OptionStringOrInt;
    impl<'de> de::Visitor<'de> for OptionStringOrInt {
        type Value = Option<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a string, integer, or null")
        }
        fn visit_none<E: de::Error>(self) -> Result<Option<String>, E> {
            Ok(None)
        }
        fn visit_unit<E: de::Error>(self) -> Result<Option<String>, E> {
            Ok(None)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Option<String>, E> {
            Ok(Some(v.to_owned()))
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Option<String>, E> {
            Ok(Some(v.to_string()))
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Option<String>, E> {
            Ok(Some(v.to_string()))
        }
    }
    deserializer.deserialize_any(OptionStringOrInt)
}

#[derive(Deserialize, Debug)]
pub struct SidecarOrderResponse {
    #[serde(deserialize_with = "string_or_int")]
    pub id: String,
    /// EXT-24 FR-5: Echo back clientOrderId if exchange supports it.
    #[serde(rename = "clientOrderId")]
    pub client_order_id: Option<String>,
    pub status: Option<String>,
    pub symbol: Option<String>,
    pub side: Option<String>,
    #[serde(rename = "type")]
    pub order_type: Option<String>,
    pub amount: Option<String>,
    pub filled: Option<String>,
    pub remaining: Option<String>,
    pub average: Option<String>,
    pub price: Option<String>,
    /// EXT-31: Bracket order child IDs returned by exchange.
    #[serde(default, rename = "stopLossOrderId", deserialize_with = "option_string_or_int")]
    pub stop_loss_order_id: Option<String>,
    #[serde(default, rename = "takeProfitOrderId", deserialize_with = "option_string_or_int")]
    pub take_profit_order_id: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct SidecarPositionResponse {
    pub symbol: String,
    pub side: String,
    pub contracts: String,
    #[serde(rename = "entryPrice")]
    pub entry_price: String,
    #[serde(rename = "unrealizedPnl")]
    pub unrealized_pnl: String,
    /// Per-position configured leverage multiplier (e.g. "8" for 8x). Optional
    /// because some exchanges omit it on spot-like positions.
    #[serde(default)]
    pub leverage: Option<String>,
}

/// EXT-22: Order update event received from the sidecar WebSocket.
/// FIX-09 FR-1: Economics live in REST. WS is transition-only.
/// `average` is kept solely for the entry fill path (limit order avg
/// fill price is accurate for non-conditional orders). Close-leg
/// economics are derived by the JournalSyncer from REST fill history.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderUpdateEvent {
    pub id: String,
    pub symbol: String,
    pub status: String,
    pub side: String,
    #[serde(default, deserialize_with = "deserialize_decimal_opt")]
    pub average: Option<Decimal>,
    pub timestamp: Option<i64>,
    /// CEX-08: User ID tagged by WsSubscriptionManager for symbol-based
    /// fallback matching when exchange order ID is unknown (e.g. Bybit
    /// bracket orders where SL/TP IDs aren't returned at placement time).
    #[serde(skip)]
    pub user_id: Option<uuid::Uuid>,
}

/// EXT-22: Sidecar WebSocket message envelope.
#[derive(Debug, Deserialize)]
struct SidecarWsMessage {
    event: String,
    #[serde(default)]
    data: Option<OrderUpdateEvent>,
    #[serde(default)]
    message: Option<String>,
}

/// EXT-22: Subscribe message sent to sidecar WebSocket.
#[derive(Serialize)]
struct WsSubscribeMessage {
    action: String,
    exchange_id: String,
    credentials: CredentialsPayload,
    sandbox: bool,
    symbols: Vec<String>,
}

// ── Request param types ──

#[derive(Serialize)]
struct BalanceParams {
    #[serde(rename = "type")]
    balance_type: String,
}

#[derive(Serialize)]
struct OrderParams {
    symbol: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    price: Option<String>,
    #[serde(rename = "stopPrice", skip_serializing_if = "Option::is_none")]
    stop_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    leverage: Option<u8>,
    #[serde(rename = "reduceOnly", skip_serializing_if = "Option::is_none")]
    reduce_only: Option<bool>,
    /// EXT-24 FR-5: Optional clientOrderId for defense-in-depth identification.
    #[serde(rename = "clientOrderId", skip_serializing_if = "Option::is_none")]
    client_order_id: Option<String>,
    /// EXT-31: Bracket order — attached stop-loss trigger price.
    #[serde(rename = "stopLoss", skip_serializing_if = "Option::is_none")]
    stop_loss: Option<BracketTrigger>,
    /// EXT-31: Bracket order — attached take-profit trigger price.
    #[serde(rename = "takeProfit", skip_serializing_if = "Option::is_none")]
    take_profit: Option<BracketTrigger>,
}

/// EXT-31: Trigger price for bracket order SL/TP.
#[derive(Serialize)]
struct BracketTrigger {
    #[serde(rename = "triggerPrice")]
    trigger_price: String,
}

#[derive(Serialize)]
struct EditOrderParams {
    #[serde(rename = "orderId")]
    order_id: String,
    symbol: String,
    #[serde(rename = "type")]
    order_type: String,
    side: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    price: Option<String>,
}

#[derive(Serialize)]
struct CancelOrderParams {
    #[serde(rename = "orderId")]
    order_id: String,
    symbol: String,
}

#[derive(Serialize)]
struct CancelAllOrdersParams {
    symbol: String,
}

#[derive(Serialize)]
struct PositionParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
}

/// FIX-08: Params for POST /order/fetch.
#[derive(Serialize)]
struct FetchOrderParams {
    #[serde(rename = "orderId")]
    order_id: String,
    symbol: String,
}

/// EXT-24 FR-3: Params for fetching open orders.
#[derive(Serialize)]
struct OpenOrdersParams {
    symbol: String,
}

/// FIX-08: Fetch-order response from the sidecar POST /order/fetch endpoint.
/// Only Bybit is implemented in MVP; other exchanges return a 501 which maps
/// to `CexClientError::ExchangeError("fetchOrder not implemented for …")`.
#[derive(Deserialize, Debug)]
pub struct SidecarFetchOrderResponse {
    pub id: String,
    pub symbol: String,
    pub status: String,
    pub side: String,
    /// Average fill price. Null when the order hasn't filled yet or the exchange
    /// doesn't return it.
    #[serde(rename = "avgPrice", default, deserialize_with = "deserialize_decimal_opt")]
    pub avg_price: Option<Decimal>,
    pub filled: String,
    pub fees: String,
    pub timestamp: i64,
}

/// FIX-09 CP-5 FR-6: Params for POST /trades/by-group (ID-less bracket fallback).
#[derive(Serialize)]
struct TradesByGroupParams {
    symbol: String,
    since_ms: i64,
    until_ms: i64,
    expected_qty: String,
    qty_tolerance: String,
    entry_side: String,
}

/// FIX-09 CP-5 FR-6: Response from POST /trades/by-group.
#[derive(Deserialize, Debug)]
pub struct SidecarTradesByGroupResponse {
    pub matched: Option<SidecarTradeMatch>,
}

/// A single matched execution from the sidecar's /trades/by-group endpoint.
#[derive(Deserialize, Debug)]
pub struct SidecarTradeMatch {
    pub order_id: String,
    #[serde(default, deserialize_with = "deserialize_decimal_opt")]
    pub avg_price: Option<Decimal>,
    pub filled_qty: String,
    pub transaction_time_ms: i64,
    pub side: String,
}

/// JNL-SYNC-01 FR-1: Params for POST /trades/since.
#[derive(Serialize)]
struct TradesSinceParams {
    since_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    until_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
}

/// JNL-SYNC-01 FR-1: A single normalized fill item from the /trades/since response.
/// All numeric fields are strings to preserve precision (AGENTS.md trading rule).
#[derive(Deserialize, Debug, Clone)]
pub struct SidecarFillSinceItem {
    pub exec_id: String,
    pub symbol: String,
    pub side: String,
    pub price: String,
    pub qty: String,
    pub fee: String,
    pub fee_asset: String,
    pub exec_time_ms: i64,
    pub order_id: Option<String>,
    pub raw_json: serde_json::Value,
}

/// EXT-24 FR-3: Open order response from the sidecar.
#[derive(Deserialize, Debug)]
pub struct SidecarOpenOrderResponse {
    pub id: String,
    #[serde(rename = "clientOrderId")]
    pub client_order_id: Option<String>,
    pub symbol: Option<String>,
    pub status: Option<String>,
    pub side: Option<String>,
    #[serde(rename = "type")]
    pub order_type: Option<String>,
    pub price: Option<String>,
    #[serde(rename = "stopPrice")]
    pub stop_price: Option<String>,
    pub amount: Option<String>,
    pub filled: Option<String>,
    pub remaining: Option<String>,
    pub timestamp: Option<i64>,
}

/// HTTP client for the CEX sidecar service.
pub struct CexClient {
    http: Client,
    base_url: String,
    psk: Option<String>,
}

impl CexClient {
    pub fn new(config: &CexSidecarConfig) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            http,
            base_url: config.base_url.clone(),
            psk: config.psk.clone(),
        }
    }

    /// Health check — GET /health. Returns Ok(()) if sidecar is reachable.
    pub async fn health_check(&self) -> Result<(), CexClientError> {
        self.http
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .map_err(|e| CexClientError::Unavailable(e.to_string()))?;
        Ok(())
    }

    /// Fetch supported exchange IDs — GET /exchanges.
    pub async fn list_exchanges(&self) -> Result<Vec<String>, CexClientError> {
        let mut req = self.http.get(format!("{}/exchanges", self.base_url));
        if let Some(psk) = &self.psk {
            req = req.header("X-Internal-Secret", psk);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| CexClientError::Unavailable(e.to_string()))?;
        resp.json::<Vec<String>>()
            .await
            .map_err(|e| CexClientError::ExchangeError(e.to_string()))
    }

    /// Fetch balance for a user's exchange account.
    pub async fn fetch_balance(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        balance_type: &str,
    ) -> Result<Vec<SidecarBalanceEntry>, CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            BalanceParams {
                balance_type: balance_type.to_string(),
            },
        );
        self.post("/balance", &envelope).await
    }

    /// HIST-02: Fetch user's trade fills for history import.
    pub async fn fetch_trades(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<SidecarFill>, CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            TradesParams { since, limit, symbol: None, extra: None },
        );
        self.post("/trades", &envelope).await
    }

    /// Create an order. Supports optional bracket params (EXT-31).
    pub async fn create_order(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        symbol: &str,
        side: &str,
        order_type: &str,
        amount: Decimal,
        price: Option<Decimal>,
        stop_price: Option<Decimal>,
        leverage: Option<u8>,
        reduce_only: bool,
        client_order_id: Option<String>,
        stop_loss_trigger: Option<Decimal>,
        take_profit_trigger: Option<Decimal>,
    ) -> Result<SidecarOrderResponse, CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            OrderParams {
                symbol: symbol.to_string(),
                side: side.to_string(),
                order_type: order_type.to_string(),
                amount: amount.to_string(),
                price: price.map(|p| p.to_string()),
                stop_price: stop_price.map(|p| p.to_string()),
                leverage,
                reduce_only: if reduce_only { Some(true) } else { None },
                client_order_id,
                stop_loss: stop_loss_trigger.map(|p| BracketTrigger {
                    trigger_price: p.to_string(),
                }),
                take_profit: take_profit_trigger.map(|p| BracketTrigger {
                    trigger_price: p.to_string(),
                }),
            },
        );
        self.post("/order", &envelope).await
    }

    /// Edit an existing order.
    pub async fn edit_order(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        order_id: &str,
        symbol: &str,
        order_type: &str,
        side: &str,
        amount: Option<Decimal>,
        price: Option<Decimal>,
    ) -> Result<SidecarOrderResponse, CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            EditOrderParams {
                order_id: order_id.to_string(),
                symbol: symbol.to_string(),
                order_type: order_type.to_string(),
                side: side.to_string(),
                amount: amount.map(|a| a.to_string()),
                price: price.map(|p| p.to_string()),
            },
        );
        self.post("/order/edit", &envelope).await
    }

    /// Cancel an order.
    pub async fn cancel_order(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        order_id: &str,
        symbol: &str,
    ) -> Result<(), CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            CancelOrderParams {
                order_id: order_id.to_string(),
                symbol: symbol.to_string(),
            },
        );
        let _: serde_json::Value = self.post("/order/cancel", &envelope).await?;
        Ok(())
    }

    /// Cancel ALL open orders for a symbol. Defense-in-depth fallback.
    pub async fn cancel_all_orders(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        symbol: &str,
    ) -> Result<(), CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            CancelAllOrdersParams {
                symbol: symbol.to_string(),
            },
        );
        let _: serde_json::Value = self.post("/orders/cancel-all", &envelope).await?;
        Ok(())
    }

    /// Fetch positions for a symbol (or all).
    pub async fn fetch_positions(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        symbol: Option<&str>,
    ) -> Result<Vec<SidecarPositionResponse>, CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            PositionParams {
                symbol: symbol.map(|s| s.to_string()),
            },
        );
        self.post("/position", &envelope).await
    }

    /// EXT-24 FR-3: Fetch open orders for a symbol from the exchange.
    pub async fn fetch_open_orders(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        symbol: &str,
    ) -> Result<Vec<SidecarOpenOrderResponse>, CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            OpenOrdersParams {
                symbol: symbol.to_string(),
            },
        );
        self.post("/orders/open", &envelope).await
    }

    /// FIX-08: Fetch a single closed order's fill data from the sidecar.
    ///
    /// For Bybit, calls `POST /order/fetch` which hits `/v5/order/history`.
    /// For other exchanges, the sidecar returns HTTP 501 which surfaces as
    /// `CexClientError::ExchangeError("fetchOrder not implemented for …")`.
    pub async fn fetch_order(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        symbol: &str,
        order_id: &str,
    ) -> Result<SidecarFetchOrderResponse, CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            FetchOrderParams {
                order_id: order_id.to_string(),
                symbol: symbol.to_string(),
            },
        );
        self.post("/order/fetch", &envelope).await
    }

    /// FIX-09 CP-5 FR-6: Fetch recent executions for a symbol/time window and return the
    /// matched close leg. Only Bybit is implemented in the sidecar MVP — other exchanges
    /// return 501 which maps to `CexClientError::ExchangeError("not implemented")`.
    pub async fn fetch_trades_by_group(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        symbol: &str,
        since_ms: i64,
        until_ms: i64,
        expected_qty: Decimal,
        qty_tolerance: Decimal,
        entry_side: &str,
    ) -> Result<SidecarTradesByGroupResponse, CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            TradesByGroupParams {
                symbol: symbol.to_string(),
                since_ms,
                until_ms,
                expected_qty: expected_qty.to_string(),
                qty_tolerance: qty_tolerance.to_string(),
                entry_side: entry_side.to_string(),
            },
        );
        self.post("/trades/by-group", &envelope).await
    }

    /// JNL-SYNC-01 FR-1: Fetch all fills since a watermark via POST /trades/since.
    /// The sidecar walks pagination to exhaustion and returns a flat array.
    pub async fn fetch_trades_since(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        since_ms: i64,
        until_ms: Option<i64>,
        symbol: Option<&str>,
    ) -> Result<Vec<SidecarFillSinceItem>, CexClientError> {
        let envelope = self.build_envelope(
            exchange_id,
            creds,
            sandbox,
            TradesSinceParams {
                since_ms,
                until_ms,
                symbol: symbol.map(|s| s.to_string()),
            },
        );
        self.post("/trades/since", &envelope).await
    }

    /// EXT-22: Get the WebSocket URL for the sidecar order stream.
    pub fn ws_url(&self) -> String {
        let base = self
            .base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        format!("{}/ws/orders", base)
    }

    /// EXT-22: Subscribe to order updates via the sidecar WebSocket.
    ///
    /// Opens a WebSocket connection to the sidecar's `/ws/orders` endpoint,
    /// sends credentials and symbol subscriptions, and returns a broadcast
    /// receiver that yields `OrderUpdateEvent`s.
    ///
    /// The WebSocket read loop runs in a spawned task and writes to the
    /// broadcast channel. When the receiver is dropped, the task continues
    /// but events are discarded.
    pub async fn subscribe_orders(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        symbols: Vec<String>,
    ) -> Result<broadcast::Receiver<OrderUpdateEvent>, CexClientError> {
        let ws_url = self.ws_url();

        let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| CexClientError::WebSocketError(format!("Connect failed: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // Send subscribe message
        let sub_msg = WsSubscribeMessage {
            action: "subscribe".to_string(),
            exchange_id: exchange_id.to_string(),
            credentials: CredentialsPayload {
                api_key: creds.api_key.clone(),
                secret: creds.secret.clone(),
                password: creds.password.clone(),
            },
            sandbox,
            symbols,
        };
        let msg_text = serde_json::to_string(&sub_msg)
            .map_err(|e| CexClientError::WebSocketError(e.to_string()))?;
        write
            .send(WsMessage::Text(msg_text.into()))
            .await
            .map_err(|e| CexClientError::WebSocketError(e.to_string()))?;

        // Create broadcast channel (capacity 256 — if consumer is slow, old events are dropped)
        let (tx, rx) = broadcast::channel::<OrderUpdateEvent>(256);

        // Spawn read loop
        tokio::spawn(async move {
            while let Some(result) = read.next().await {
                match result {
                    Ok(WsMessage::Text(text)) => {
                        if let Ok(msg) = serde_json::from_str::<SidecarWsMessage>(&text) {
                            if msg.event == "order_update" {
                                if let Some(event) = msg.data {
                                    // Ignore send errors (no active receivers)
                                    let _ = tx.send(event);
                                }
                            } else if msg.event == "unsupported" {
                                tracing::warn!("Sidecar WS: {}", msg.message.unwrap_or_default());
                                break;
                            } else if msg.event == "error" {
                                tracing::warn!("Sidecar WS error: {}", msg.message.unwrap_or_default());
                            }
                        }
                    }
                    Ok(WsMessage::Close(_)) => {
                        tracing::info!("Sidecar WS connection closed by server");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Sidecar WS read error: {}", e);
                        break;
                    }
                    _ => {} // Ping/Pong handled by tungstenite
                }
            }
            tracing::info!("Sidecar WS read loop exited");
        });

        Ok(rx)
    }

    // ── Private helpers ──

    fn build_envelope<P: Serialize>(
        &self,
        exchange_id: &str,
        creds: &SidecarCredentials,
        sandbox: bool,
        params: P,
    ) -> SidecarEnvelope<P> {
        SidecarEnvelope {
            exchange_id: exchange_id.to_string(),
            credentials: CredentialsPayload {
                api_key: creds.api_key.clone(),
                secret: creds.secret.clone(),
                password: creds.password.clone(),
            },
            sandbox,
            params,
        }
    }

    async fn post<P: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &P,
    ) -> Result<R, CexClientError> {
        let mut req = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .json(body);
        if let Some(psk) = &self.psk {
            req = req.header("X-Internal-Secret", psk);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| CexClientError::Unavailable(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            resp.json::<R>()
                .await
                .map_err(|e| CexClientError::ExchangeError(format!("JSON parse error: {}", e)))
        } else {
            let error_body = resp.json::<SidecarErrorBody>().await.ok();
            let msg = error_body
                .as_ref()
                .map(|b| b.error.clone())
                .unwrap_or_else(|| format!("HTTP {}", status));
            Err(map_status_to_error(status.as_u16(), msg))
        }
    }
}

fn map_status_to_error(status: u16, msg: String) -> CexClientError {
    match status {
        401 => CexClientError::AuthenticationFailed,
        402 => CexClientError::InsufficientFunds,
        404 => CexClientError::OrderNotFound(msg),
        429 => CexClientError::RateLimited,
        502 | 503 => CexClientError::Unavailable(msg),
        _ => CexClientError::ExchangeError(msg),
    }
}

/// FIX-01: Custom deserializer for `Option<Decimal>` that accepts both
/// JSON numbers and strings, converting through string representation
/// to preserve precision.
fn deserialize_decimal_opt<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct DecimalOptVisitor;

    impl<'de> de::Visitor<'de> for DecimalOptVisitor {
        type Value = Option<Decimal>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a number, numeric string, or null")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D2: serde::Deserializer<'de>>(
            self,
            deserializer: D2,
        ) -> Result<Self::Value, D2::Error> {
            deserializer.deserialize_any(DecimalInnerVisitor).map(Some)
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
            Decimal::from_str(&v.to_string())
                .map(Some)
                .map_err(de::Error::custom)
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(Decimal::from(v)))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(Decimal::from(v)))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Decimal::from_str(v).map(Some).map_err(de::Error::custom)
        }
    }

    struct DecimalInnerVisitor;

    impl<'de> de::Visitor<'de> for DecimalInnerVisitor {
        type Value = Decimal;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a number or numeric string")
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
            Decimal::from_str(&v.to_string()).map_err(de::Error::custom)
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Decimal::from(v))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Decimal::from(v))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Decimal::from_str(v).map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_option(DecimalOptVisitor)
}

/// Parse a string decimal from the sidecar into `rust_decimal::Decimal`.
/// FIX-01: Returns `Result` — parse failures are errors, never silent zeros.
pub fn parse_decimal(s: &str) -> Result<Decimal, String> {
    Decimal::from_str(s).map_err(|e| format!("Failed to parse decimal '{}': {}", s, e))
}

/// Parse an optional string decimal.
/// FIX-01: Returns `Result` — parse failures are errors, never silent zeros.
pub fn parse_decimal_opt(s: &Option<String>) -> Result<Option<Decimal>, String> {
    match s {
        Some(v) => Decimal::from_str(v)
            .map(Some)
            .map_err(|e| format!("Failed to parse decimal '{}': {}", v, e)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_parse_decimal() {
        assert_eq!(parse_decimal("123.456").unwrap(), dec!(123.456));
        assert_eq!(parse_decimal("0").unwrap(), Decimal::ZERO);
        assert!(parse_decimal("invalid").is_err(), "Invalid input must return Err");
    }

    #[test]
    fn test_parse_decimal_opt() {
        assert_eq!(
            parse_decimal_opt(&Some("42.5".to_string())).unwrap(),
            Some(dec!(42.5))
        );
        assert_eq!(parse_decimal_opt(&None).unwrap(), None);
        assert!(
            parse_decimal_opt(&Some("bad".to_string())).is_err(),
            "Invalid input must return Err"
        );
    }

    #[test]
    fn test_config_defaults() {
        // Don't set env vars — test default values
        let config = CexSidecarConfig {
            base_url: "http://127.0.0.1:3100".to_string(),
            timeout_secs: 10,
            psk: None,
        };
        assert_eq!(config.base_url, "http://127.0.0.1:3100");
        assert_eq!(config.timeout_secs, 10);
    }

    #[test]
    fn test_map_status_to_error() {
        assert!(matches!(
            map_status_to_error(401, "bad".into()),
            CexClientError::AuthenticationFailed
        ));
        assert!(matches!(
            map_status_to_error(402, "funds".into()),
            CexClientError::InsufficientFunds
        ));
        assert!(matches!(
            map_status_to_error(404, "order123".into()),
            CexClientError::OrderNotFound(_)
        ));
        assert!(matches!(
            map_status_to_error(429, "slow".into()),
            CexClientError::RateLimited
        ));
        assert!(matches!(
            map_status_to_error(503, "down".into()),
            CexClientError::Unavailable(_)
        ));
        assert!(matches!(
            map_status_to_error(500, "err".into()),
            CexClientError::ExchangeError(_)
        ));
    }

    #[test]
    fn test_ws_url_conversion() {
        let config = CexSidecarConfig {
            base_url: "http://127.0.0.1:3100".to_string(),
            timeout_secs: 10,
            psk: None,
        };
        let client = CexClient::new(&config);
        assert_eq!(client.ws_url(), "ws://127.0.0.1:3100/ws/orders");

        let config_https = CexSidecarConfig {
            base_url: "https://sidecar.example.com".to_string(),
            timeout_secs: 10,
            psk: None,
        };
        let client_https = CexClient::new(&config_https);
        assert_eq!(client_https.ws_url(), "wss://sidecar.example.com/ws/orders");
    }

    #[test]
    fn test_sidecar_fetch_order_response_deserialization() {
        let json = r#"{
            "id": "ord123",
            "symbol": "BTC/USDT:USDT",
            "status": "closed",
            "side": "sell",
            "avgPrice": "65000.50",
            "filled": "0.01",
            "fees": "0.65",
            "timestamp": 1713000000000
        }"#;
        let resp: SidecarFetchOrderResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "ord123");
        assert_eq!(resp.avg_price, Some(dec!(65000.50)));
        assert_eq!(resp.status, "closed");
        assert_eq!(resp.timestamp, 1713000000000i64);

        // null avgPrice (unfilled / exchange withholds it)
        let json_null = r#"{
            "id": "ord456",
            "symbol": "ETH/USDT:USDT",
            "status": "open",
            "side": "buy",
            "avgPrice": null,
            "filled": "0",
            "fees": "0",
            "timestamp": 1713000000001
        }"#;
        let resp_null: SidecarFetchOrderResponse = serde_json::from_str(json_null).unwrap();
        assert_eq!(resp_null.avg_price, None);
    }

    #[test]
    fn test_sidecar_credentials_no_debug() {
        // SidecarCredentials intentionally does not implement Debug.
        // This test verifies the struct can be constructed but not accidentally printed.
        let _creds = SidecarCredentials {
            api_key: "key".to_string(),
            secret: "secret".to_string(),
            password: None,
        };
        // If Debug were derived, `format!("{:?}", _creds)` would compile.
        // We can't negative-test at runtime, but the absence of `#[derive(Debug)]` is the point.
    }
}
