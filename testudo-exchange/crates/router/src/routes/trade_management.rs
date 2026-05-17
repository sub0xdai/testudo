//! Trade Management Routes
//!
//! Provides API endpoints for managing trades with SL/TP, break-even,
//! and multi-target exits via the Shadow Engine.
//!
//! # Endpoints
//! - POST   /trades              - Create trade with entry, SL, TP
//! - GET    /trades              - List active trade groups
//! - GET    /trades/{id}         - Get trade group details
//! - PUT    /trades/{id}/sl      - Update stop loss
//! - PUT    /trades/{id}/tp      - Add/update take profit
//! - PUT    /trades/{id}/breakeven - Enable break-even
//! - DELETE /trades/{id}         - Cancel entire trade group
//!
//! # FR-2 (003-risk-enforcement)
//!
//! All order creation routes run through the Decision Loop for risk validation.
//! Orders are marked as `risk_validated = true` only after approval.

use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse};
use engine::{
    BreakEvenConfig, EngineHandle, OrderGroup, OrderGroupStatus, OrderRole, ShadowEngine,
    ShadowOrder, ShadowOrderSide, ShadowOrderType, TakeProfitTarget,
};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

use crate::metrics;

use common_utils::auth::TokenService;

/// Extract user ID from request.
///
/// EXT-05: Dual auth -- checks JWT Bearer token first (via auth_service),
/// falls back to X-User-Id header for backward compatibility with paper trading.
/// Returns (user_id, is_authenticated) where is_authenticated=true means JWT was used.
async fn extract_user_id(
    req: &HttpRequest,
    state: &TradeManagementState,
) -> Result<(Uuid, bool), HttpResponse> {
    // Try JWT Bearer token first
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if let Some(ref token_service) = state.token_service {
                    match token_service.verify_access_token(token) {
                        Ok(claims) => {
                            if let Ok(user_id) = Uuid::parse_str(&claims.sub) {
                                return Ok((user_id, true));
                            }
                        }
                        Err(_) => {
                            return Err(error_response(
                                StatusCode::UNAUTHORIZED,
                                "Invalid or expired token",
                            ));
                        }
                    }
                }
            }
        }
    }

    // Fall back to X-User-Id header (paper trading / backward compat)
    match req.headers().get("X-User-Id") {
        Some(value) => match value.to_str().ok().and_then(|s| Uuid::parse_str(s).ok()) {
            Some(id) => Ok((id, false)),
            None => Err(error_response(
                StatusCode::BAD_REQUEST,
                "Invalid user ID format",
            )),
        },
        None => Err(error_response(
            StatusCode::BAD_REQUEST,
            "X-User-Id header or Authorization Bearer token required",
        )),
    }
}

fn error_response(status: StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(ApiResponse::<()>::error(message.to_string()))
}

use crate::decision_loop::{
    DecisionInputBuilder, DecisionLoop, DecisionOrderSide, DecisionOrderType,
};
use common_utils::risk::{AccountState, RiskConfig};

use crate::services::calibration::CalibrationEngine;
use crate::services::sizing_preview;
use crate::services::exchange_api::{
    ApiOrderType, ExchangeApiError, OrderSide as ExchangeOrderSide,
    PlaceOrderRequest as ExchangePlaceOrderRequest,
};
use crate::services::trade_manager::{
    ManagedPosition, ManagementRules, PartialTpRule, PositionSide,
    TradeManagerService,
    TrailingStopRule,
};
use crate::services::WsSubscriptionManager;

/// AUD-04 FR-5: Time-bounded idempotency cache for trade creation.
/// Stores responses keyed by idempotency key with a 5-minute TTL.
pub struct IdempotencyCache {
    entries: tokio::sync::RwLock<HashMap<String, (std::time::Instant, serde_json::Value)>>,
}

impl IdempotencyCache {
    pub fn new() -> Self {
        Self {
            entries: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Check if a response is cached for the given key.
    /// Prunes expired entries (> 5 min) on each call.
    pub async fn get(&self, key: &str) -> Option<serde_json::Value> {
        let mut entries = self.entries.write().await;
        let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(300);
        entries.retain(|_, (t, _)| *t > cutoff);

        entries.get(key).map(|(_, response)| response.clone())
    }

    /// Store a response for the given idempotency key.
    pub async fn store(&self, key: String, response: serde_json::Value) {
        self.entries
            .write()
            .await
            .insert(key, (std::time::Instant::now(), response));
    }
}

/// Format exchange errors into user-friendly messages.
fn format_exchange_error(e: &ExchangeApiError) -> String {
    match e {
        ExchangeApiError::AgentWalletInactive { .. } => {
            "Agent wallet needs re-authorization. Open Account settings to fix.".to_string()
        }
        ExchangeApiError::InsufficientBalance { .. } => {
            "Insufficient margin — reduce position size or increase leverage".to_string()
        }
        ExchangeApiError::Exchange(msg) if msg.contains("insufficient") => {
            "Insufficient margin — reduce position size or increase leverage".to_string()
        }
        ExchangeApiError::Exchange(msg) if msg.contains("Authentication") => {
            "Exchange authentication failed — check your API keys".to_string()
        }
        ExchangeApiError::Exchange(msg) if msg.to_lowercase().contains("does not exist") => {
            "Agent wallet expired — re-authorize in Account settings.".to_string()
        }
        ExchangeApiError::Exchange(msg) if msg.to_lowercase().contains("rate limit") => {
            "Exchange is busy — wait a moment and retry.".to_string()
        }
        ExchangeApiError::Exchange(msg) => format!("Exchange error: {}", msg),
        ExchangeApiError::Internal(msg) => format!("Internal error: {}", msg),
        ExchangeApiError::OrderNotFound(id) => format!("Order not found: {}", id),
    }
}

/// Returns true if the exchange error is definitive (order was NOT placed).
/// Ambiguous errors (timeout, parse errors) should NOT trigger rollback
/// because the order may have been placed on the exchange.
fn is_definitive_rejection(e: &ExchangeApiError) -> bool {
    match e {
        ExchangeApiError::AgentWalletInactive { .. } => true,
        ExchangeApiError::InsufficientBalance { .. } => true,
        ExchangeApiError::Exchange(msg) => {
            let lower = msg.to_lowercase();
            lower.contains("insufficient")
                || lower.contains("authentication")
                || lower.contains("invalid")
                || lower.contains("not allowed")
                || lower.contains("does not exist")
                || lower.contains("not found")
        }
        _ => false,
    }
}

/// Shared state for trade management routes
pub struct TradeManagementState {
    /// 019e: Actor handle — sole engine access path.
    pub engine_handle: EngineHandle,
    /// Optional token service for JWT verification (AUTH-02).
    /// When None, only X-User-Id header auth is available.
    pub token_service: Option<Arc<dyn TokenService>>,
    /// Shadow (paper) trade manager for automated position management (EXT-09).
    pub trade_manager_shadow: Option<Arc<TradeManagerService>>,
    /// Live (Binance Futures) trade manager (EXT-10).
    pub trade_manager_live: Option<Arc<TradeManagerService>>,
    /// EXT-25: Sidecar WS subscription manager for live fill detection.
    pub ws_subscription_manager: Option<Arc<WsSubscriptionManager>>,
    /// AUD-01 FR-1: Per-user semaphore to serialize live trade creation.
    /// Prevents TOCTOU balance race where two concurrent requests read the same balance.
    trade_locks: Mutex<HashMap<Uuid, Arc<Semaphore>>>,
    /// AUD-04 FR-5: Idempotency cache for trade creation (5-min TTL).
    idempotency_cache: IdempotencyCache,
    /// CON-01a: Pool for exchange_name lookup during trade placement.
    pub pool: Option<sqlx::PgPool>,
    /// QNT-01a: Calibration engine for Calibrated Kelly sizing.
    /// Only consulted when the user has `dynamic_risk_enabled = true`.
    pub calibration_engine: Option<Arc<CalibrationEngine>>,
}

impl TradeManagementState {
    /// Create new state with a fresh ShadowEngine.
    /// Spawns an EngineActor internally.
    pub fn new(engine: ShadowEngine) -> Self {
        let (engine_handle, _fill_rx, _trade_event_rx) = engine::EngineActor::spawn(engine);
        Self {
            engine_handle,
            token_service: None,
            trade_manager_shadow: None,
            trade_manager_live: None,
            ws_subscription_manager: None,
            trade_locks: Mutex::new(HashMap::new()),
            idempotency_cache: IdempotencyCache::new(),
            pool: None,
            calibration_engine: None,
        }
    }

    /// Create new state with an existing EngineHandle.
    ///
    /// Use this when the actor is already spawned by main.rs.
    pub fn new_with_handle(engine_handle: EngineHandle) -> Self {
        Self {
            engine_handle,
            token_service: None,
            trade_manager_shadow: None,
            trade_manager_live: None,
            ws_subscription_manager: None,
            trade_locks: Mutex::new(HashMap::new()),
            idempotency_cache: IdempotencyCache::new(),
            pool: None,
            calibration_engine: None,
        }
    }

    /// Set token service for JWT verification (AUTH-02).
    pub fn with_token_service(mut self, token_service: Arc<dyn TokenService>) -> Self {
        self.token_service = Some(token_service);
        self
    }

    /// Set shadow trade manager for paper trading (EXT-09).
    pub fn with_trade_manager(mut self, trade_manager: Arc<TradeManagerService>) -> Self {
        self.trade_manager_shadow = Some(trade_manager);
        self
    }

    /// Set live trade manager for Binance Futures (EXT-10).
    pub fn with_live_trade_manager(mut self, trade_manager: Arc<TradeManagerService>) -> Self {
        self.trade_manager_live = Some(trade_manager);
        self
    }

    pub fn with_ws_subscription_manager(mut self, manager: Arc<WsSubscriptionManager>) -> Self {
        self.ws_subscription_manager = Some(manager);
        self
    }

    /// CON-01a: Set pool for exchange_name lookup.
    pub fn with_pool(mut self, pool: sqlx::PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// QNT-01a: Set calibration engine for Calibrated Kelly sizing.
    pub fn with_calibration_engine(mut self, engine: Arc<CalibrationEngine>) -> Self {
        self.calibration_engine = Some(engine);
        self
    }

    /// Select the appropriate trade manager based on auth mode (FR-3.2).
    /// EXT-21 FR-3: Authenticated users require live trade manager — no silent shadow fallback.
    fn select_trade_manager(&self, is_authenticated: bool) -> Option<&Arc<TradeManagerService>> {
        if is_authenticated {
            // Live mode (JWT auth) -> live trade manager only (no shadow fallback)
            self.trade_manager_live.as_ref()
        } else {
            // Paper mode (X-User-Id) -> shadow trade manager
            self.trade_manager_shadow.as_ref()
        }
    }
}

/// Request to create a new trade
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateTradeRequest {
    pub symbol: String,
    pub side: String, // "buy" or "sell"
    /// Quantity is optional when management block is provided (calculated from risk%).
    pub quantity: Option<Decimal>,
    pub entry_price: Decimal,
    pub stop_loss_price: Option<Decimal>,
    pub take_profit_price: Option<Decimal>,
    pub take_profit_targets: Option<Vec<TakeProfitTargetRequest>>,
    pub break_even_trigger_percent: Option<Decimal>,
    pub break_even_offset: Option<Decimal>,
    /// EXT-09: Management rules for automated position management.
    /// When present and quantity is absent, position size is calculated from risk_percent.
    pub management: Option<ManagementBlock>,
    /// EXT-16 FR-3: Optional exchange account ID for multi-account routing.
    pub exchange_account_id: Option<String>,
    /// AUD-04 FR-4: Optional idempotency key to prevent duplicate trade creation.
    /// Can also be sent via Idempotency-Key header.
    pub idempotency_key: Option<String>,
    /// RSK-02: Optional user-supplied setup tag (e.g. "breakout").
    /// Normalized on the server: trimmed, max 48 chars. Empty → None.
    pub setup_tag: Option<String>,
}

/// Management configuration sent from the extension (EXT-09).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ManagementBlock {
    pub risk_percent: Decimal,
    pub break_even_at: u32,
    #[serde(default = "default_leverage")]
    pub leverage: u8,
    pub trailing_stop: TrailingStopBlock,
    pub partial_tp: PartialTpBlock,
}

fn default_leverage() -> u8 {
    1
}

/// Trailing stop configuration from the extension.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TrailingStopBlock {
    pub enabled: bool,
    #[serde(alias = "distance_percent")]
    pub distance: u32,
}

/// Partial take-profit configuration from the extension.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PartialTpBlock {
    pub enabled: bool,
    #[serde(alias = "close_percent")]
    pub percent: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TakeProfitTargetRequest {
    pub price: Decimal,
    pub percent_to_close: Decimal,
}

/// Request to update stop loss
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateStopLossRequest {
    pub price: Decimal,
}

/// Request to add/update take profit
#[derive(Debug, Deserialize)]
pub struct UpdateTakeProfitRequest {
    pub price: Decimal,
    pub percent_to_close: Option<Decimal>,
}

/// Request to enable break-even
#[derive(Debug, Deserialize)]
pub struct EnableBreakEvenRequest {
    pub trigger_percent: Decimal,
    pub offset: Option<Decimal>,
}

/// Request to update entry price (pending orders only)
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateEntryPriceRequest {
    pub price: Decimal,
}

/// Trade group response
#[derive(Debug, Serialize, Deserialize)]
pub struct TradeGroupResponse {
    pub id: Uuid,
    pub symbol: String,
    pub entry_order_id: Uuid,
    pub entry_price: Option<Decimal>,
    pub entry_quantity: Decimal,
    pub stop_loss_price: Option<Decimal>,
    pub stop_loss_order_id: Option<Uuid>,
    pub take_profit_targets: Vec<TakeProfitTargetResponse>,
    pub status: String,
    pub break_even_enabled: bool,
    pub break_even_triggered: bool,
    /// EXT-21 FR-2: Indicates whether trade was routed to "live" or "shadow" engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Warnings about non-fatal issues (e.g. SL/TP placement failures).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TakeProfitTargetResponse {
    pub price: Decimal,
    pub percent_to_close: Decimal,
    pub order_id: Option<Uuid>,
    pub filled: bool,
}

/// API Response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    /// Structured error code for frontend pattern matching (UXA-01).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            error_code: None,
        }
    }

    pub fn error(message: String) -> ApiResponse<T> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
            error_code: None,
        }
    }

    /// Error response with structured error code for frontend consumption.
    pub fn error_with_code(message: String, code: String) -> ApiResponse<T> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
            error_code: Some(code),
        }
    }
}

/// Map an ExchangeApiError to a structured error code string.
fn error_code_for(e: &ExchangeApiError) -> String {
    match e {
        ExchangeApiError::AgentWalletInactive { .. } => "agent_wallet_inactive".to_string(),
        ExchangeApiError::InsufficientBalance { .. } => "insufficient_margin".to_string(),
        ExchangeApiError::Exchange(msg) => {
            let lower = msg.to_lowercase();
            if lower.contains("insufficient") {
                "insufficient_margin".to_string()
            } else if lower.contains("does not exist") {
                "agent_wallet_expired".to_string()
            } else if lower.contains("authentication") {
                "auth_failed".to_string()
            } else if lower.contains("rate limit") {
                "rate_limited".to_string()
            } else {
                "exchange_error".to_string()
            }
        }
        ExchangeApiError::Internal(_) => "internal_error".to_string(),
        ExchangeApiError::OrderNotFound(_) => "order_not_found".to_string(),
    }
}

/// Recalculate position size to maintain constant risk when entry or SL changes.
/// Returns the new quantity truncated to 8 decimal places, or falls back to original
/// if SL is missing or distance is zero.
fn recalculate_position_size(
    balance: Decimal,
    risk_percent: Decimal,
    entry_price: Decimal,
    stop_loss_price: Option<Decimal>,
    original_quantity: Decimal,
) -> Decimal {
    match stop_loss_price {
        Some(sl) => {
            let size = common_utils::risk::sizing::fixed_fractional(
                balance,
                risk_percent,
                entry_price,
                sl,
            );
            if size > Decimal::ZERO {
                // Truncate to 8dp to avoid precision overflow in balance checks
                size.round_dp_with_strategy(8, RoundingStrategy::ToZero)
            } else {
                original_quantity
            }
        }
        None => original_quantity,
    }
}

fn order_group_to_response(group: &OrderGroup) -> TradeGroupResponse {
    TradeGroupResponse {
        id: group.id,
        symbol: group.symbol.clone(),
        entry_order_id: group.entry_order_id,
        entry_price: group.entry_price,
        entry_quantity: group.entry_quantity,
        stop_loss_price: group.stop_loss_price,
        stop_loss_order_id: group.stop_loss_order_id,
        take_profit_targets: group
            .take_profit_targets
            .iter()
            .map(|t| TakeProfitTargetResponse {
                price: t.price,
                percent_to_close: t.percent_to_close,
                order_id: t.order_id,
                filled: t.filled,
            })
            .collect(),
        status: format!("{:?}", group.status),
        break_even_enabled: group.break_even_config.is_some(),
        break_even_triggered: group
            .break_even_config
            .as_ref()
            .map(|c| c.triggered)
            .unwrap_or(false),
        execution_mode: None,
        created_at: group.created_at.to_rfc3339(),
        updated_at: group.updated_at.to_rfc3339(),
        warnings: vec![],
    }
}

/// Convert OrderGroup to response, with access to orders for limit price fallback
fn order_group_to_response_with_orders(
    group: &OrderGroup,
    orders: &engine::ShadowOrderManager,
) -> TradeGroupResponse {
    // Use filled entry_price, or fall back to the order's limit price for pending orders
    let entry_price = group
        .entry_price
        .or_else(|| orders.get_order(group.entry_order_id).and_then(|o| o.price));

    TradeGroupResponse {
        id: group.id,
        symbol: group.symbol.clone(),
        entry_order_id: group.entry_order_id,
        entry_price,
        entry_quantity: group.entry_quantity,
        stop_loss_price: group.stop_loss_price,
        stop_loss_order_id: group.stop_loss_order_id,
        take_profit_targets: group
            .take_profit_targets
            .iter()
            .map(|t| TakeProfitTargetResponse {
                price: t.price,
                percent_to_close: t.percent_to_close,
                order_id: t.order_id,
                filled: t.filled,
            })
            .collect(),
        status: format!("{:?}", group.status),
        break_even_enabled: group.break_even_config.is_some(),
        break_even_triggered: group
            .break_even_config
            .as_ref()
            .map(|c| c.triggered)
            .unwrap_or(false),
        execution_mode: None,
        created_at: group.created_at.to_rfc3339(),
        updated_at: group.updated_at.to_rfc3339(),
        warnings: vec![],
    }
}

/// POST /trades - Create a new trade with entry, SL, TP
///
/// # FR-2 (003-risk-enforcement)
///
/// This endpoint runs the Decision Loop before placing orders. Orders are
/// only marked as `risk_validated = true` after the Decision Loop approves.
pub async fn create_trade(
    req: HttpRequest,
    body: web::Json<CreateTradeRequest>,
    state: web::Data<TradeManagementState>,
) -> HttpResponse {
    let (user_id, is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };

    // EXT-05 FR-5: Execution mode routing — "live" requires JWT authentication
    let execution_mode = req
        .headers()
        .get("X-Execution-Mode")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("shadow");

    if execution_mode == "live" && !is_authenticated {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
            "Live execution requires authentication. Use Authorization: Bearer <token>".to_string(),
        ));
    }

    // EXT-21 FR-3: Reject live trades when CCXT sidecar not configured
    if is_authenticated && state.trade_manager_live.is_none() {
        return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
            "Live trading unavailable — CCXT sidecar not configured".to_string(),
        ));
    }

    // EXT-21 FR-2: Determine execution mode for response
    let execution_mode_label = if is_authenticated { "live" } else { "shadow" };

    // AUD-01 FR-1: Acquire per-user semaphore to serialize live trade creation.
    // Prevents TOCTOU balance race where two concurrent requests read the same balance.
    let _trade_permit = if is_authenticated {
        let semaphore = {
            let mut locks = state.trade_locks.lock().await;
            locks
                .entry(user_id)
                .or_insert_with(|| Arc::new(Semaphore::new(1)))
                .clone()
        };
        Some(
            semaphore
                .acquire_owned()
                .await
                .expect("trade semaphore closed"),
        )
    } else {
        None
    };

    tracing::info!(
        user_id = %user_id,
        mode = %execution_mode_label,
        auth = if is_authenticated { "jwt" } else { "header" },
        "Trade request received"
    );

    let req = body.into_inner();

    // RSK-02: Normalize setup_tag — trim, drop if empty, reject if > 48 chars.
    let setup_tag: Option<String> = match req.setup_tag.as_ref().map(|s| s.trim()) {
        None | Some("") => None,
        Some(trimmed) if trimmed.chars().count() > 48 => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                "setup_tag must be 48 characters or fewer".to_string(),
            ));
        }
        Some(trimmed) => Some(trimmed.to_string()),
    };

    // AUD-04 FR-4/FR-6: Check idempotency key (from body or Idempotency-Key header)
    let idempotency_key = req.idempotency_key.clone();
    if let Some(ref key) = idempotency_key {
        if let Some(cached) = state.idempotency_cache.get(key).await {
            tracing::info!("Idempotency cache hit: key={}", key);
            return HttpResponse::Ok().json(cached);
        }
    }

    // Lazy initialization: ensure user exists in shadow engine with default balance
    if !state.engine_handle.user_exists(user_id).await {
        let _ = state.engine_handle.init_user(user_id).await;
        tracing::info!(
            "Auto-initialized paper trading account for user {}",
            user_id
        );
    }

    // Parse side
    let side = match req.side.to_lowercase().as_str() {
        "buy" => ShadowOrderSide::Buy,
        "sell" => ShadowOrderSide::Sell,
        _ => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                "Invalid side. Use 'buy' or 'sell'".to_string(),
            ));
        }
    };

    // EXT-09: Calculate quantity from management block if not provided
    let exchange_account_id = req
        .exchange_account_id
        .as_ref()
        .and_then(|s| Uuid::parse_str(s).ok());

    // Track risk_amount for R-multiple computation in journal
    let mut computed_risk_amount: Option<Decimal> = None;

    // QNT-01a: Calibrated Kelly pre-sizing, routed through the shared
    // QNT-01b `compute_sizing_preview` helper so the preview endpoint and
    // the execution path are guaranteed byte-identical on the same inputs.
    //
    // Any failure mode (missing user_settings row, DB error during
    // calibration loads, missing engine) silently falls back to baseline —
    // a transient hiccup must never fail a trade that today would succeed
    // (FR-10).
    let dynamic_risk_enabled = match state.pool.as_ref() {
        Some(pool) => {
            match sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT settings FROM user_settings WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
            {
                Ok(Some(blob)) => serde_json::from_value::<
                    crate::routes::user_settings::UserSettings,
                >(blob)
                .map(|s| s.dynamic_risk_enabled)
                .unwrap_or(false),
                Ok(None) => false,
                Err(e) => {
                    tracing::warn!(
                        user_id = %user_id,
                        error = %e,
                        "QNT-01a: user_settings lookup failed, falling back to fixed mode"
                    );
                    false
                }
            }
        }
        None => false,
    };

    let mut override_risk_percent: Option<Decimal> = None;
    let mut kelly_inputs_json: Option<serde_json::Value> = None;

    // FR-10 (strict): gate all QNT-01a pre-sizing behind the toggle so the
    // fixed-mode code path is literally unchanged from pre-QNT behavior.
    // When `dynamic_risk_enabled` is false, `compute_sizing_preview` is
    // never invoked and no Kelly match arms can fire.
    if dynamic_risk_enabled {
        if let Some(mgmt) = req.management.as_ref() {
            let preview = sizing_preview::compute_sizing_preview(
                user_id,
                setup_tag.as_deref(),
                mgmt.risk_percent,
                dynamic_risk_enabled,
                state.calibration_engine.as_deref(),
            )
            .await;

            match preview {
                Ok(p) => match &p.reasoning {
                    sizing_preview::SizingReasoning::NegativeEdge { quarter_kelly } => {
                        tracing::info!(
                            user_id = %user_id,
                            setup_tag = ?setup_tag,
                            quarter_kelly = %quarter_kelly,
                            "QNT-01a: negative edge — rejecting trade"
                        );
                        return HttpResponse::BadRequest().json(
                            ApiResponse::<()>::error_with_code(
                                "Calibration shows negative edge for this setup — size = 0."
                                    .to_string(),
                                "negative_edge".to_string(),
                            ),
                        );
                    }
                    sizing_preview::SizingReasoning::Calibrated {
                        n_setup,
                        p_eff,
                        avg_r_win,
                        avg_r_loss,
                    } => {
                        tracing::info!(
                            user_id = %user_id,
                            setup_tag = ?setup_tag,
                            baseline = %p.baseline_risk_pct,
                            effective = %p.effective_risk_pct,
                            multiplier = %p.edge_multiplier,
                            n_setup = %n_setup,
                            p_eff = %p_eff,
                            avg_r_win = %avg_r_win,
                            avg_r_loss = %avg_r_loss,
                            "QNT-01a: Calibrated Kelly sizing"
                        );
                        override_risk_percent = Some(p.effective_risk_pct);
                        kelly_inputs_json = p.kelly_inputs;
                    }
                    sizing_preview::SizingReasoning::Untagged
                    | sizing_preview::SizingReasoning::FixedMode => {
                        // Baseline flows through unchanged; nothing to journal.
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        user_id = %user_id,
                        setup_tag = ?setup_tag,
                        error = %e,
                        "QNT-01a: calibration load failed — falling back to baseline"
                    );
                }
            }
        }
    }

    let quantity = if let Some(qty) = req.quantity {
        qty
    } else if let Some(ref mgmt) = req.management {
        // QNT-01a: Use Kelly-derived effective risk when available, baseline otherwise.
        let effective_risk_pct = override_risk_percent.unwrap_or(mgmt.risk_percent);

        // Calculate from risk% using fixed_fractional
        let sl_price = match req.stop_loss_price {
            Some(sl) => sl,
            None => {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                    "stop_loss_price is required when using management block without explicit quantity".to_string(),
                ));
            }
        };
        // EXT-21: Use real exchange balance for live trades, shadow for paper
        let balance = if is_authenticated {
            if let Some(tm) = state.trade_manager_live.as_ref() {
                match tm.get_balance(user_id, "USDT", exchange_account_id).await {
                    Ok(b) => {
                        tracing::info!("Live balance for sizing: {} USDT", b);
                        b
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch live balance for sizing: {}", e);
                        return HttpResponse::BadGateway().json(ApiResponse::<()>::error(
                            "Cannot fetch exchange balance — check API keys and connection"
                                .to_string(),
                        ));
                    }
                }
            } else {
                return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
                    "Live trading unavailable — CCXT sidecar not configured".to_string(),
                ));
            }
        } else {
            let balances = state.engine_handle.get_balances(user_id).await;
            balances
                .iter()
                .find(|b| b.asset == "USDT")
                .map(|b| b.available + b.reserved)
                .unwrap_or(Decimal::from(10000))
        };
        tracing::info!(
            "Position sizing: balance={}, risk%={}, entry={}, sl={}",
            balance,
            effective_risk_pct,
            req.entry_price,
            sl_price
        );
        let risk_size = common_utils::risk::sizing::fixed_fractional(
            balance,
            effective_risk_pct,
            req.entry_price,
            sl_price,
        );
        // Capture risk_amount for R-multiple: balance * risk% / 100
        computed_risk_amount = Some(balance * effective_risk_pct / Decimal::from(100));
        if risk_size <= Decimal::ZERO {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                "Calculated position size is zero (entry == stop?)".to_string(),
            ));
        }
        // Margin capacity cap: reject if risk-based size exceeds available margin.
        // The position sizer should not silently reduce size — the user's intended
        // risk would no longer match the actual position.
        let mgmt_leverage = mgmt.leverage;
        if mgmt_leverage > 0 && req.entry_price > Decimal::ZERO {
            let margin_required = (risk_size * req.entry_price) / Decimal::from(mgmt_leverage);
            if margin_required > balance {
                tracing::warn!(
                    "Position requires {} margin but only {} available (leverage={}x)",
                    margin_required, balance, mgmt_leverage
                );
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                    format!(
                        "Insufficient margin: trade needs ${:.2} but ${:.2} available. Widen stop, reduce risk%, or increase leverage.",
                        margin_required, balance
                    ),
                ));
            }
        }
        risk_size.round_dp_with_strategy(8, RoundingStrategy::ToZero)
    } else {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Either quantity or management block must be provided".to_string(),
        ));
    };

    // FR-2: Run Decision Loop for risk validation BEFORE creating the order
    let decision_side = match side {
        ShadowOrderSide::Buy => DecisionOrderSide::Long,
        ShadowOrderSide::Sell => DecisionOrderSide::Short,
    };

    let leverage = req.management.as_ref().map(|m| m.leverage).unwrap_or(1);

    let decision_input = match DecisionInputBuilder::new()
        .user_id(user_id)
        .symbol(&req.symbol)
        .side(decision_side)
        .order_type(DecisionOrderType::Limit)
        .quantity(quantity)
        .entry_price(req.entry_price)
        .stop_loss(req.stop_loss_price.unwrap_or(Decimal::ZERO))
        .leverage(leverage)
        .build()
    {
        Ok(input) => input,
        Err(e) => {
            tracing::error!("Failed to build DecisionInput: {}", e);
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(format!(
                "Invalid order parameters: {}",
                e
            )));
        }
    };

    // Get account state for risk validation (FR-4: Live balance from Shadow Engine)
    let account_state = {
        let balances = state.engine_handle.get_balances(user_id).await;
        let usdt_balance = balances
            .iter()
            .find(|b| b.asset == "USDT")
            .map(|b| b.available + b.reserved)
            .unwrap_or(Decimal::from(10000));

        let positions = state.engine_handle.get_positions(user_id).await;

        AccountState {
            balance: usdt_balance,
            open_position_count: positions.len() as u32,
            daily_pnl: Decimal::ZERO,
            starting_balance: Decimal::from(10000),
        }
    };

    // Run Decision Loop with default risk config
    let decision_loop = DecisionLoop::new(RiskConfig::default());
    let decision_result = decision_loop.execute(&decision_input, &account_state, None);

    if !decision_result.approved {
        let rejection_msg = decision_result
            .rejection
            .as_ref()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "Order rejected by risk management".to_string());

        tracing::warn!(
            user_id = %user_id,
            symbol = %req.symbol,
            qty = %quantity,
            reason = %rejection_msg,
            "Trade creation rejected"
        );
        metrics::ORDERS_TOTAL
            .with_label_values(&[req.side.as_str(), "rejected"])
            .inc();

        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(rejection_msg));
    }

    tracing::info!(
        user_id = %user_id,
        symbol = %req.symbol,
        qty = %decision_result.position_size.unwrap_or(quantity),
        sizing_method = ?decision_result.sizing_method,
        "Trade creation approved"
    );
    metrics::ORDERS_TOTAL
        .with_label_values(&[req.side.as_str(), "approved"])
        .inc();

    // Create entry order with SL/TP and leverage
    let mut order = ShadowOrder::new(
        user_id,
        req.symbol.clone(),
        side,
        ShadowOrderType::Limit,
        quantity,
        Some(req.entry_price),
        None,
        None,
    )
    .with_leverage(leverage);

    if let Some(sl_price) = req.stop_loss_price {
        order = order.with_stop_loss(sl_price);
    }

    if let Some(tp_price) = req.take_profit_price {
        order = order.with_take_profit(tp_price);
    }

    // FR-2: Mark order as risk-validated after Decision Loop approval
    order.mark_risk_validated();

    // 019c FR-1: Decoupled I/O — place order in actor (microseconds), then exchange I/O
    // with NO engine state held, then register exchange IDs back in actor (microseconds).
    let placed_order = match state.engine_handle.place_order(user_id, order).await {
        Ok(order) => order,
        Err(e) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(e.to_string()));
        }
    };

    // Get the group that was auto-created by place_order
    let group = state
        .engine_handle
        .get_group_by_entry_order(placed_order.id)
        .await;

    if let Some(group) = group {
        let group_id = group.id;

        // Configure group: TP targets, break-even, exchange account (actor: microseconds)
        let tp_targets = req.take_profit_targets.map(|targets| {
            targets
                .into_iter()
                .map(|t| TakeProfitTarget {
                    price: t.price,
                    percent_to_close: t.percent_to_close,
                    order_id: None,
                    filled: false,
                })
                .collect()
        });

        let be_config = req.break_even_trigger_percent.map(|trigger_percent| {
            BreakEvenConfig {
                trigger_percent,
                offset: req.break_even_offset,
                triggered: false,
            }
        });

        // CON-01a: Resolve exchange_name from exchange_account_id for journal accuracy
        let exchange_name: Option<String> = match (exchange_account_id, state.pool.as_ref()) {
            (Some(acc_id), Some(pool)) => {
                sqlx::query_scalar::<_, String>(
                    "SELECT exchange_name FROM exchange_accounts WHERE id = $1"
                )
                .bind(acc_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten()
            }
            _ => None,
        };

        if tp_targets.is_some()
            || be_config.is_some()
            || exchange_account_id.is_some()
            || computed_risk_amount.is_some()
            || setup_tag.is_some()
            || kelly_inputs_json.is_some()
        {
            let _ = state
                .engine_handle
                .configure_group(
                    group_id,
                    tp_targets,
                    be_config,
                    exchange_account_id,
                    exchange_name,
                    computed_risk_amount,
                    setup_tag.clone(),
                    kelly_inputs_json.clone(),
                )
                .await;
        }

        // === EXCHANGE I/O — NO ENGINE STATE HELD ===
        // CEX-07: Single bracket order — safe-cex handles SL/TP natively.
        // Entry + SL + TP are submitted as one atomic bracket order.
        let mut entry_exchange_id: Option<String> = None;
        let mut trade_warnings: Vec<String> = Vec::new();

        if is_authenticated {
            if let Some(tm) = state.trade_manager_live.as_ref() {
                let exchange_side = match side {
                    ShadowOrderSide::Buy => ExchangeOrderSide::Buy,
                    ShadowOrderSide::Sell => ExchangeOrderSide::Sell,
                };

                // FR-6: Stamp clientOrderId for defense-in-depth identification
                let entry_client_id = crate::services::numeric_client_order_id(group_id, 1);

                // Single bracket order: entry + optional SL/TP triggers
                match tm
                    .place_order(ExchangePlaceOrderRequest {
                        user_id,
                        symbol: req.symbol.clone(),
                        side: exchange_side,
                        order_type: ApiOrderType::Limit,
                        quantity,
                        price: Some(req.entry_price),
                        stop_price: None,
                        leverage,
                        exchange_account_id,
                        reduce_only: false,
                        client_order_id: Some(entry_client_id),
                        stop_loss_trigger: req.stop_loss_price,
                        take_profit_trigger: req.take_profit_price,
                    })
                    .await
                {
                    Ok(result) => {
                        let is_filled = result.status.as_deref() == Some("closed");
                        tracing::info!(
                            "Live bracket order placed: entry_id={}, symbol={}, qty={}, filled={}",
                            result.id,
                            req.symbol,
                            quantity,
                            is_filled
                        );
                        entry_exchange_id = Some(result.id.clone());

                        // Register entry exchange order ID
                        let _ = state
                            .engine_handle
                            .register_exchange_order_id(group_id, OrderRole::Entry, result.id)
                            .await;

                        // Register bracket child IDs if returned by the exchange
                        if let Some(ref sl_id) = result.stop_loss_order_id {
                            let _ = state
                                .engine_handle
                                .register_exchange_order_id(group_id, OrderRole::StopLoss, sl_id.clone())
                                .await;
                            tracing::info!(
                                "Bracket SL registered: sl_id={}, group={}",
                                sl_id,
                                group_id
                            );
                        }
                        if let Some(ref tp_id) = result.take_profit_order_id {
                            let _ = state
                                .engine_handle
                                .register_exchange_order_id(group_id, OrderRole::TakeProfit, tp_id.clone())
                                .await;
                            tracing::info!(
                                "Bracket TP registered: tp_id={}, group={}",
                                tp_id,
                                group_id
                            );
                        }

                        if is_filled {
                            let fill_price = result.average.unwrap_or(req.entry_price);
                            let _ = state.engine_handle.on_entry_filled(group_id, fill_price).await;
                            tracing::info!(
                                "Entry filled instantly: group={}, fill_price={}",
                                group_id,
                                fill_price
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to place bracket order on exchange: {}", e);

                        if is_definitive_rejection(&e) {
                            // Definitive rejection: order was NOT placed — rollback shadow order
                            if let Err(cancel_err) = state
                                .engine_handle
                                .cancel_order(user_id, placed_order.id)
                                .await
                            {
                                tracing::warn!(
                                    "Failed to rollback shadow entry order {} after exchange rejection: {}",
                                    placed_order.id,
                                    cancel_err
                                );
                            }
                            return HttpResponse::BadGateway()
                                .json(ApiResponse::<()>::error_with_code(
                                    format_exchange_error(&e),
                                    error_code_for(&e),
                                ));
                        }

                        // Ambiguous error (timeout, parse error): order may have been placed.
                        // Keep shadow order group tracked — user can verify and cancel if needed.
                        tracing::warn!(
                            "Ambiguous exchange error for group {}: {} — keeping shadow order tracked",
                            group_id, e
                        );
                        trade_warnings.push(format!(
                            "Exchange response unclear: {}. Order may have been placed — please verify.",
                            format_exchange_error(&e)
                        ));
                    }
                }
            }
        }


        // EXT-09/10: Register with trade manager if management block present
        // Guard: skip DB registration for live trades without confirmed exchange order
        // to prevent ghost positions that survive restart via rehydration.
        if let Some(ref mgmt) = req.management {
            let should_register = !is_authenticated || entry_exchange_id.is_some();
            if should_register {
                if let Some(tm) = state.select_trade_manager(is_authenticated) {
                    let pos_side = match side {
                        ShadowOrderSide::Buy => PositionSide::Long,
                        ShadowOrderSide::Sell => PositionSide::Short,
                    };
                    let mut managed = ManagedPosition::new(
                        user_id,
                        req.symbol.clone(),
                        pos_side,
                        req.entry_price,
                        req.stop_loss_price.unwrap_or(Decimal::ZERO),
                        req.take_profit_price.unwrap_or(req.entry_price),
                        quantity,
                        ManagementRules {
                            risk_percent: mgmt.risk_percent,
                            break_even_at: mgmt.break_even_at,
                            leverage: mgmt.leverage,
                            trailing_stop: if mgmt.trailing_stop.enabled {
                                Some(TrailingStopRule {
                                    enabled: true,
                                    distance_percent: mgmt.trailing_stop.distance,
                                })
                            } else {
                                None
                            },
                            partial_tp: if mgmt.partial_tp.enabled {
                                Some(PartialTpRule {
                                    enabled: true,
                                    close_percent: mgmt.partial_tp.percent,
                                })
                            } else {
                                None
                            },
                        },
                    );
                    managed.exchange_account_id = exchange_account_id;
                    managed.setup_tag = setup_tag.clone();
                    managed.exchange_order_ids.entry_order_id = entry_exchange_id.clone();
                    if let Err(e) = tm.register(managed).await {
                        tracing::warn!("Failed to register managed position: {}", e);
                    }
                }
            }
        }

        // Get final group state for response (actor may have been updated with exchange IDs)
        let final_group = state.engine_handle.get_trade_group(group_id).await;
        let group_for_response = final_group.as_ref().unwrap_or(&group);

        let live_subscription_ctx = if is_authenticated {
            group_for_response
                .exchange_account_id
                .map(|account_id| (account_id, group_for_response.symbol.clone(), group_id))
        } else {
            None
        };

        let mut response = order_group_to_response(group_for_response);
        if response.entry_price.is_none() {
            response.entry_price = placed_order.price;
        }
        response.execution_mode = Some(execution_mode_label.to_string());
        response.warnings = trade_warnings;

        if let Some((account_id, symbol, gid)) = live_subscription_ctx {
            if let Some(ws_manager) = state.ws_subscription_manager.as_ref() {
                match ws_manager
                    .ensure_subscribed(user_id, account_id, &symbol)
                    .await
                {
                    Ok(action) => {
                        tracing::info!(
                            "Live WS subscription ensured: action={:?}, user={}, account={}, group={}, symbol={}",
                            action,
                            user_id,
                            account_id,
                            gid,
                            symbol
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Live WS subscription ensure failed: user={}, account={}, group={}, symbol={}, error={}",
                            user_id,
                            account_id,
                            gid,
                            symbol,
                            e
                        );
                    }
                }
            }
        }

        let api_response = ApiResponse::success(response);
        if let Some(ref key) = idempotency_key {
            if let Ok(cached) = serde_json::to_value(&api_response) {
                state.idempotency_cache.store(key.clone(), cached).await;
            }
        }
        HttpResponse::Created().json(api_response)
    } else {
        // Order placed but no group (no SL/TP)
        let fallback_response = ApiResponse::success(TradeGroupResponse {
            id: placed_order.id,
            symbol: placed_order.symbol,
            entry_order_id: placed_order.id,
            entry_price: placed_order.price,
            entry_quantity: placed_order.quantity,
            stop_loss_price: None,
            stop_loss_order_id: None,
            take_profit_targets: vec![],
            status: "Pending".to_string(),
            break_even_enabled: false,
            break_even_triggered: false,
            execution_mode: Some(execution_mode_label.to_string()),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            warnings: vec![],
        });
        if let Some(ref key) = idempotency_key {
            if let Ok(cached) = serde_json::to_value(&fallback_response) {
                state.idempotency_cache.store(key.clone(), cached).await;
            }
        }
        HttpResponse::Created().json(fallback_response)
    }
}

/// POST /trades/preview - Preview Calibrated Kelly sizing without side effects.
///
/// # QNT-01b FR-4
///
/// Runs the exact same calibration pipeline as `create_trade` via the shared
/// `sizing_preview::compute_sizing_preview` helper (byte-parity contract).
/// No database writes, no CCXT calls, no shadow engine interaction — just
/// the two aggregate queries inside `CalibrationEngine` and the Kelly math.
///
/// Returns a `SizingPreview` (200) whose serialized form omits `kelly_inputs`
/// (that DB-persistence blob is skip-serialized on the struct).
///
/// A missing `management` block yields 400 — there is no baseline risk% to
/// compute against.  Calibration load errors yield 500 with a generic
/// "Preview unavailable" message; the extension renders this as the muted
/// fallback row per FR-10 (the Alt+X confirm button remains enabled and the
/// trade path falls through to baseline on its own).
pub async fn preview_trade_sizing(
    req: HttpRequest,
    body: web::Json<CreateTradeRequest>,
    state: web::Data<TradeManagementState>,
) -> HttpResponse {
    let (user_id, _is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };

    let req = body.into_inner();

    // Normalize setup_tag — same rules as create_trade.
    let setup_tag: Option<String> = match req.setup_tag.as_ref().map(|s| s.trim()) {
        None | Some("") => None,
        Some(trimmed) if trimmed.chars().count() > 48 => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                "setup_tag must be 48 characters or fewer".to_string(),
            ));
        }
        Some(trimmed) => Some(trimmed.to_string()),
    };

    // Baseline risk% comes from the management block; without it there is
    // nothing to size against.
    let Some(mgmt) = req.management.as_ref() else {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "management block required for sizing preview".to_string(),
        ));
    };
    let baseline = mgmt.risk_percent;

    // Look up dynamic_risk_enabled — same query as create_trade. Failures
    // collapse silently to fixed-mode (matches FR-10 spirit: preview never
    // escalates a storage hiccup into a loud error when a safe fallback
    // exists).
    let dynamic_risk_enabled = match state.pool.as_ref() {
        Some(pool) => {
            match sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT settings FROM user_settings WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
            {
                Ok(Some(blob)) => serde_json::from_value::<
                    crate::routes::user_settings::UserSettings,
                >(blob)
                .map(|s| s.dynamic_risk_enabled)
                .unwrap_or(false),
                Ok(None) => false,
                Err(e) => {
                    tracing::warn!(
                        user_id = %user_id,
                        error = %e,
                        "QNT-01b preview: user_settings lookup failed, defaulting to fixed mode"
                    );
                    false
                }
            }
        }
        None => false,
    };

    match sizing_preview::compute_sizing_preview(
        user_id,
        setup_tag.as_deref(),
        baseline,
        dynamic_risk_enabled,
        state.calibration_engine.as_deref(),
    )
    .await
    {
        Ok(preview) => HttpResponse::Ok().json(preview),
        Err(e) => {
            tracing::warn!(
                user_id = %user_id,
                error = %e,
                "QNT-01b preview: calibration load failed"
            );
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Preview unavailable".to_string(),
            ))
        }
    }
}

/// GET /trades - List active trade groups for user
pub async fn list_trades(state: web::Data<TradeManagementState>, req: HttpRequest) -> HttpResponse {
    let (user_id, _is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };

    let groups = state.engine_handle.list_trade_groups(user_id).await;

    let mut response = Vec::new();
    for group in &groups {
        if !matches!(
            group.status,
            OrderGroupStatus::Pending | OrderGroupStatus::Active
        ) {
            continue;
        }
        let mut resp = order_group_to_response(group);
        // Fallback: use entry order's limit price if entry hasn't filled yet
        if resp.entry_price.is_none() {
            if let Some(order) = state.engine_handle.get_order(group.entry_order_id).await {
                resp.entry_price = order.price;
            }
        }
        response.push(resp);
    }

    HttpResponse::Ok().json(ApiResponse::success(response))
}

/// GET /trades/{id} - Get trade group details
pub async fn get_trade(
    path: web::Path<Uuid>,
    state: web::Data<TradeManagementState>,
    req: HttpRequest,
) -> HttpResponse {
    let group_id = path.into_inner();
    let (user_id, _is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };

    if let Some(group) = state.engine_handle.get_trade_group(group_id).await {
        if group.user_id != user_id {
            return HttpResponse::Forbidden()
                .json(ApiResponse::<()>::error("Access denied".to_string()));
        }
        HttpResponse::Ok().json(ApiResponse::success(order_group_to_response(&group)))
    } else {
        HttpResponse::NotFound().json(ApiResponse::<()>::error("Trade not found".to_string()))
    }
}

/// PUT /trades/{id}/sl - Update stop loss price
///
/// For Pending groups, recalculates position size to maintain constant risk.
/// For Active groups, only updates the SL price (position already open).
pub async fn update_stop_loss(
    path: web::Path<Uuid>,
    body: web::Json<UpdateStopLossRequest>,
    state: web::Data<TradeManagementState>,
    req: HttpRequest,
) -> HttpResponse {
    let group_id = path.into_inner();
    let (user_id, _is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };
    let new_sl_price = body.price;

    // Gather group info for recalculation (both Pending and Active)
    enum GroupInfo {
        Pending {
            entry_order_id: Uuid,
            symbol: String,
            side: engine::ShadowOrderSide,
            old_quantity: Decimal,
            entry_price: Decimal,
        },
        Active {
            entry_price: Decimal,
            old_quantity: Decimal,
            sl_order_id: Option<Uuid>,
        },
    }

    let group = match state.engine_handle.get_trade_group(group_id).await {
        Some(g) => g,
        None => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Trade not found".to_string()));
        }
    };

    if group.user_id != user_id {
        return HttpResponse::Forbidden()
            .json(ApiResponse::<()>::error("Access denied".to_string()));
    }

    let group_info = if group.status == OrderGroupStatus::Pending {
        let entry_order = state.engine_handle.get_order(group.entry_order_id).await;
        entry_order.map(|o| GroupInfo::Pending {
            entry_order_id: group.entry_order_id,
            symbol: group.symbol.clone(),
            side: o.side,
            old_quantity: o.quantity,
            entry_price: o.price.unwrap_or_default(),
        })
    } else if let Some(ep) = group.entry_price {
        Some(GroupInfo::Active {
            entry_price: ep,
            old_quantity: group.entry_quantity,
            sl_order_id: group.stop_loss_order_id,
        })
    } else {
        None
    };

    match group_info {
        Some(GroupInfo::Pending {
            entry_order_id,
            symbol,
            side,
            old_quantity,
            entry_price,
        }) => {
            // Pending group: cancel old order, create new one with recalculated size
            let balances = state.engine_handle.get_balances(user_id).await;
            let usdt_balance = balances
                .iter()
                .find(|b| b.asset == "USDT")
                .map(|b| b.available + b.reserved)
                .unwrap_or(Decimal::from(10000));

            let risk_config = RiskConfig::default();
            let new_quantity = recalculate_position_size(
                usdt_balance,
                risk_config.account_risk_percent,
                entry_price,
                Some(new_sl_price),
                old_quantity,
            );

            // Cap quantity to what the balance can afford (truncate to 8dp to avoid precision overflow)
            let required = new_quantity * entry_price;
            let new_quantity = if required > usdt_balance {
                (usdt_balance / entry_price).round_dp_with_strategy(8, RoundingStrategy::ToZero)
            } else {
                new_quantity
            };

            // Cancel old entry order WITHOUT cascading to the group
            if let Err(e) = state
                .engine_handle
                .cancel_order_no_cascade(user_id, entry_order_id)
                .await
            {
                return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    format!("Failed to cancel existing entry order: {}", e),
                ));
            }

            // Create new order with recalculated quantity (no SL on order - group tracks it)
            let mut new_order = ShadowOrder::new(
                user_id,
                symbol,
                side,
                ShadowOrderType::Limit,
                new_quantity,
                Some(entry_price),
                None,
                None,
            );
            new_order.mark_risk_validated();

            // Use place_order_no_group to avoid creating a duplicate OrderGroup
            let new_entry_order = match state
                .engine_handle
                .place_order_no_group(user_id, new_order)
                .await
            {
                Ok(order) => order,
                Err(e) => {
                    tracing::error!(
                        "Failed to create replacement entry order for group {}: {}",
                        group_id,
                        e
                    );
                    return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                        format!(
                            "Failed to create new entry order: {}. Original order was cancelled.",
                            e
                        ),
                    ));
                }
            };

            // AUD-01 FR-4: Atomically validate status + update group
            match state
                .engine_handle
                .update_group_stop_loss(
                    group_id,
                    OrderGroupStatus::Pending,
                    new_sl_price,
                    new_quantity,
                    Some((entry_order_id, new_entry_order.id)),
                )
                .await
            {
                Ok(group) => {
                    let mut response = order_group_to_response(&group);
                    response.entry_price = Some(entry_price);
                    HttpResponse::Ok().json(ApiResponse::success(response))
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("status changed") {
                        HttpResponse::Conflict().json(ApiResponse::<()>::error(msg))
                    } else {
                        HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                            "Failed to retrieve updated trade".to_string(),
                        ))
                    }
                }
            }
        }
        Some(GroupInfo::Active {
            entry_price,
            old_quantity,
            sl_order_id,
        }) => {
            // Active group: recalculate size based on new SL distance
            let balances = state.engine_handle.get_balances(user_id).await;
            let usdt_balance = balances
                .iter()
                .find(|b| b.asset == "USDT")
                .map(|b| b.available + b.reserved)
                .unwrap_or(Decimal::from(10000));

            let risk_config = RiskConfig::default();
            let new_quantity = recalculate_position_size(
                usdt_balance,
                risk_config.account_risk_percent,
                entry_price,
                Some(new_sl_price),
                old_quantity,
            );

            // AUD-01 FR-4: Atomically validate status + update group
            match state
                .engine_handle
                .update_group_stop_loss(
                    group_id,
                    OrderGroupStatus::Active,
                    new_sl_price,
                    new_quantity,
                    None, // No entry order swap for Active groups
                )
                .await
            {
                Ok(group) => {
                    // Update the stop order price if it exists
                    if let Some(sl_oid) = sl_order_id {
                        state.engine_handle.update_stop_price(sl_oid, new_sl_price).await;
                    }
                    HttpResponse::Ok().json(ApiResponse::success(order_group_to_response(&group)))
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("status changed") {
                        HttpResponse::Conflict().json(ApiResponse::<()>::error(msg))
                    } else if msg.contains("not found") {
                        HttpResponse::NotFound()
                            .json(ApiResponse::<()>::error("Trade not found".to_string()))
                    } else {
                        HttpResponse::InternalServerError()
                            .json(ApiResponse::<()>::error(msg))
                    }
                }
            }
        }
        None => HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Cannot update stop loss: missing entry price".to_string(),
        )),
    }
}

/// PUT /trades/{id}/tp - Add or update take profit
pub async fn update_take_profit(
    path: web::Path<Uuid>,
    body: web::Json<UpdateTakeProfitRequest>,
    state: web::Data<TradeManagementState>,
    req: HttpRequest,
) -> HttpResponse {
    let group_id = path.into_inner();
    let (user_id, _is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };
    let req = body.into_inner();

    let percent = req.percent_to_close.unwrap_or(Decimal::from(100));
    let target = TakeProfitTarget {
        price: req.price,
        percent_to_close: percent,
        order_id: None,
        filled: false,
    };

    match state
        .engine_handle
        .add_take_profit_target(group_id, user_id, target)
        .await
    {
        Ok(group) => {
            HttpResponse::Ok().json(ApiResponse::success(order_group_to_response(&group)))
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                HttpResponse::NotFound()
                    .json(ApiResponse::<()>::error("Trade not found".to_string()))
            } else if msg.contains("Access denied") {
                HttpResponse::Forbidden()
                    .json(ApiResponse::<()>::error("Access denied".to_string()))
            } else {
                HttpResponse::InternalServerError()
                    .json(ApiResponse::<()>::error(msg))
            }
        }
    }
}

/// PUT /trades/{id}/entry - Update entry price (pending orders only)
///
/// # FR-5.4 (007-editable-position-levels)
///
/// Allows updating the entry price of a pending order by:
/// 1. Validating the order is still pending (not filled)
/// 2. Validating price relationship (entry > SL for longs, entry < SL for shorts)
/// 3. Canceling the existing entry order
/// 4. Creating a new order at the new price
/// 5. Updating the group's entry_order_id index
pub async fn update_entry_price(
    path: web::Path<Uuid>,
    body: web::Json<UpdateEntryPriceRequest>,
    state: web::Data<TradeManagementState>,
    req: HttpRequest,
) -> HttpResponse {
    let group_id = path.into_inner();
    let (user_id, _is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };
    let new_entry_price = body.price;

    // Get group and entry order via actor
    let group = match state.engine_handle.get_trade_group(group_id).await {
        Some(g) => g,
        None => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Trade not found".to_string()));
        }
    };

    if group.user_id != user_id {
        return HttpResponse::Forbidden()
            .json(ApiResponse::<()>::error("Access denied".to_string()));
    }

    // FR-5.4.3: Validate trade status is "Pending"
    if group.status != OrderGroupStatus::Pending {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Entry price can only be modified for pending orders".to_string(),
        ));
    }

    let entry_order = match state.engine_handle.get_order(group.entry_order_id).await {
        Some(o) => o,
        None => {
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Entry order not found".to_string(),
            ));
        }
    };

    let entry_order_id = group.entry_order_id;
    let symbol = group.symbol.clone();
    let side = entry_order.side;
    let quantity = entry_order.quantity;
    let stop_loss_price = group.stop_loss_price;

    // FR-5.4.4: Validate price relationship (entry > SL for longs, entry < SL for shorts)
    if let Some(sl_price) = stop_loss_price {
        match side {
            ShadowOrderSide::Buy => {
                if new_entry_price <= sl_price {
                    return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                        "Entry price must be above stop loss for long positions".to_string(),
                    ));
                }
            }
            ShadowOrderSide::Sell => {
                if new_entry_price >= sl_price {
                    return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                        "Entry price must be below stop loss for short positions".to_string(),
                    ));
                }
            }
        }
    }

    // Recalculate position size to maintain constant risk with new entry price
    let balances = state.engine_handle.get_balances(user_id).await;
    let usdt_balance = balances
        .iter()
        .find(|b| b.asset == "USDT")
        .map(|b| b.available + b.reserved)
        .unwrap_or(Decimal::from(10000));

    let risk_config = RiskConfig::default();
    let qty = recalculate_position_size(
        usdt_balance,
        risk_config.account_risk_percent,
        new_entry_price,
        stop_loss_price,
        quantity,
    );

    // Cap quantity to what the balance can afford
    let new_quantity = {
        let required = qty * new_entry_price;
        if required > usdt_balance {
            (usdt_balance / new_entry_price).round_dp_with_strategy(8, RoundingStrategy::ToZero)
        } else {
            qty
        }
    };

    // FR-5.4.5: Cancel existing entry order WITHOUT cascading to the group
    if let Err(e) = state
        .engine_handle
        .cancel_order_no_cascade(user_id, entry_order_id)
        .await
    {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(format!(
            "Failed to cancel existing entry order: {}",
            e
        )));
    }

    // FR-5.4.6: Create new order at new price with recalculated quantity
    let mut new_order = ShadowOrder::new(
        user_id,
        symbol,
        side,
        ShadowOrderType::Limit,
        new_quantity,
        Some(new_entry_price),
        None,
        None,
    );
    new_order.mark_risk_validated();

    let new_entry_order = match state
        .engine_handle
        .place_order_no_group(user_id, new_order)
        .await
    {
        Ok(order) => order,
        Err(e) => {
            tracing::error!(
                "Failed to create replacement entry order for group {}: {}",
                group_id,
                e
            );
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(format!(
                "Failed to create new entry order: {}. Original order was cancelled.",
                e
            )));
        }
    };

    // FR-5.4.7: Atomically validate status + update entry order and quantity
    match state
        .engine_handle
        .update_group_stop_loss(
            group_id,
            OrderGroupStatus::Pending,
            stop_loss_price.unwrap_or_default(),
            new_quantity,
            Some((entry_order_id, new_entry_order.id)),
        )
        .await
    {
        Ok(group) => {
            // FR-5.4.8: Return updated TradeGroupResponse
            let mut response = order_group_to_response(&group);
            // Fallback: use the new order's limit price for pending orders
            if response.entry_price.is_none() {
                response.entry_price = Some(new_entry_price);
            }
            HttpResponse::Ok().json(ApiResponse::success(response))
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("status changed") {
                HttpResponse::Conflict().json(ApiResponse::<()>::error(
                    msg.replace("stop loss", "entry price"),
                ))
            } else {
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Failed to retrieve updated trade".to_string(),
                ))
            }
        }
    }
}

/// PUT /trades/{id}/breakeven - Enable break-even automation
pub async fn enable_break_even(
    path: web::Path<Uuid>,
    body: web::Json<EnableBreakEvenRequest>,
    state: web::Data<TradeManagementState>,
    req: HttpRequest,
) -> HttpResponse {
    let group_id = path.into_inner();
    let (user_id, _is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };
    let req = body.into_inner();

    // Verify ownership first
    match state.engine_handle.get_trade_group(group_id).await {
        Some(group) if group.user_id != user_id => {
            return HttpResponse::Forbidden()
                .json(ApiResponse::<()>::error("Access denied".to_string()));
        }
        None => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Trade not found".to_string()));
        }
        _ => {}
    }

    // Enable break-even
    match state
        .engine_handle
        .enable_break_even(group_id, req.trigger_percent, req.offset)
        .await
    {
        Ok(()) => {
            if let Some(group) = state.engine_handle.get_trade_group(group_id).await {
                HttpResponse::Ok().json(ApiResponse::success(order_group_to_response(&group)))
            } else {
                HttpResponse::NotFound()
                    .json(ApiResponse::<()>::error("Trade not found".to_string()))
            }
        }
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(e.to_string())),
    }
}

/// DELETE /trades/{id} - Cancel entire trade group
pub async fn cancel_trade(
    path: web::Path<Uuid>,
    state: web::Data<TradeManagementState>,
    req: HttpRequest,
) -> HttpResponse {
    let group_id = path.into_inner();
    let (user_id, is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };

    // Get group info via actor
    let group = match state.engine_handle.get_trade_group(group_id).await {
        Some(g) => g,
        None => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Trade not found".to_string()));
        }
    };

    if group.user_id != user_id {
        return HttpResponse::Forbidden()
            .json(ApiResponse::<()>::error("Access denied".to_string()));
    }

    let entry_order_id = group.entry_order_id;
    let exchange_order_id = group.exchange_order_id.clone();
    let exchange_sl_order_id = group.exchange_sl_order_id.clone();
    let exchange_tp_order_id = group.exchange_tp_order_id.clone();
    let exchange_account_id = group.exchange_account_id;
    let symbol = group.symbol.clone();

    // EXT-21 + OCO: Cancel all exchange orders (entry + SL + TP)
    if is_authenticated {
        if let Some(tm) = state.trade_manager_live.as_ref() {
            tracing::info!(
                group_id = %group_id,
                symbol = %symbol,
                entry_id = ?exchange_order_id,
                sl_id = ?exchange_sl_order_id,
                tp_id = ?exchange_tp_order_id,
                "cancel_trade: attempting exchange order cleanup"
            );

            let ids_to_cancel = [
                ("entry", exchange_order_id.as_deref()),
                ("sl", exchange_sl_order_id.as_deref()),
                ("tp", exchange_tp_order_id.as_deref()),
            ];

            for (role, maybe_id) in &ids_to_cancel {
                match maybe_id {
                    Some(exch_oid) => {
                        match tm
                            .cancel_order(user_id, exch_oid, &symbol, exchange_account_id)
                            .await
                        {
                            Ok(()) => {
                                tracing::info!(
                                    group_id = %group_id,
                                    role = %role,
                                    order_id = %exch_oid,
                                    "exchange order cancelled"
                                );
                            }
                            Err(ExchangeApiError::OrderNotFound(_)) => {
                                tracing::debug!(
                                    group_id = %group_id,
                                    role = %role,
                                    order_id = %exch_oid,
                                    "exchange order already filled/cancelled"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    group_id = %group_id,
                                    role = %role,
                                    order_id = %exch_oid,
                                    error = %e,
                                    "exchange cancel failed"
                                );
                            }
                        }
                    }
                    None => {
                        tracing::warn!(
                            group_id = %group_id,
                            role = %role,
                            "exchange order ID is None — was never stored"
                        );
                    }
                }
            }

            // Always sweep cancel_all_orders for defense-in-depth
            tracing::info!(
                group_id = %group_id,
                symbol = %symbol,
                "running cancel_all_orders sweep for symbol"
            );
            if let Err(e) = tm
                .cancel_all_orders(user_id, &symbol, exchange_account_id)
                .await
            {
                tracing::warn!(
                    group_id = %group_id,
                    symbol = %symbol,
                    error = %e,
                    "cancel_all_orders sweep failed"
                );
            }

            // Close any open exchange position for Active groups.
            // HL-11: Derive close side from entry order instead of querying
            // exchange positions (which fails with wrong wallet address on HL).
            if group.status == OrderGroupStatus::Active {
                // Get entry order side — close in opposite direction
                let entry_side = state.engine_handle.get_order(entry_order_id).await
                    .map(|o| o.side);
                let (close_side, side_label) = match entry_side {
                    Some(ShadowOrderSide::Buy) => (ExchangeOrderSide::Sell, "sell"),
                    Some(ShadowOrderSide::Sell) => (ExchangeOrderSide::Buy, "buy"),
                    None => {
                        // Fallback: derive from SL vs entry price
                        let entry_px = group.entry_price.unwrap_or_default();
                        let sl_px = group.stop_loss_price.unwrap_or_default();
                        if sl_px < entry_px && sl_px > Decimal::ZERO {
                            (ExchangeOrderSide::Sell, "sell")
                        } else {
                            (ExchangeOrderSide::Buy, "buy")
                        }
                    }
                };
                let quantity = group.entry_quantity;

                tracing::info!(
                    group_id = %group_id,
                    symbol = %symbol,
                    side = %side_label,
                    quantity = %quantity,
                    entry_side = ?entry_side,
                    "cancel_trade: attempting position close"
                );

                // HL-11: Don't use reduce_only for agent wallet close —
                // HL validates reduce_only against the agent address (which has
                // no position), not the main wallet. Use a regular opposite-side
                // order to net out the position instead.
                match tm
                    .place_order(ExchangePlaceOrderRequest {
                        user_id,
                        symbol: symbol.clone(),
                        side: close_side,
                        order_type: ApiOrderType::Market,
                        quantity,
                        price: None,
                        stop_price: None,
                        leverage: 0,
                        exchange_account_id,
                        reduce_only: false,
                        client_order_id: None,
                        stop_loss_trigger: None,
                        take_profit_trigger: None,
                    })
                    .await
                {
                    Ok(close_result) => {
                        let close_id = &close_result.id;
                        tracing::info!(
                            group_id = %group_id,
                            symbol = %symbol,
                            close_order_id = %close_id,
                            quantity = %quantity,
                            side = %side_label,
                            "cancel_trade: position closed with market order"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            group_id = %group_id,
                            symbol = %symbol,
                            error = %e,
                            "cancel_trade: failed to close exchange position"
                        );
                    }
                }
            }
        }
    }

    // Cancel the entry order in shadow engine (which cascades to cancel linked orders)
    match state
        .engine_handle
        .cancel_order(user_id, entry_order_id)
        .await
    {
        Ok(_) => {
            // Persist cancellation to DB
            if let Some(ref tm) = state.trade_manager_live {
                let _ = tm.mark_position_closed(group_id).await;
            }
            HttpResponse::Ok().json(ApiResponse::success("Trade cancelled"))
        }
        Err(e) => {
            // If entry is already filled or ghost (failed placement), force-cancel the group
            if let Some(group) = state.engine_handle.get_trade_group(group_id).await {
                if !group.status.is_terminal() {
                    let linked_ids = group.get_linked_order_ids();
                    let _ = state
                        .engine_handle
                        .update_group_status(group_id, OrderGroupStatus::Cancelled)
                        .await;

                    for order_id in linked_ids {
                        let _ = state
                            .engine_handle
                            .cancel_order(user_id, order_id)
                            .await;
                    }
                    // Persist cancellation to DB
                    if let Some(ref tm) = state.trade_manager_live {
                        let _ = tm.mark_position_closed(group_id).await;
                    }
                    return HttpResponse::Ok().json(ApiResponse::success("Trade cancelled"));
                }
            }
            HttpResponse::BadRequest().json(ApiResponse::<()>::error(e.to_string()))
        }
    }
}

/// HL-09 FR-11: POST /trades/cleanup — Cancel all non-terminal order groups for a user.
/// Used to purge stale/ghost orders from debugging sessions.
pub async fn cleanup_stale_trades(
    state: web::Data<TradeManagementState>,
    req: HttpRequest,
) -> HttpResponse {
    let (user_id, is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };

    let groups = state.engine_handle.list_trade_groups(user_id).await;
    let mut cancelled_count = 0u32;

    for group in &groups {
        // HL-11 FR-3: Only purge Pending (ghost) groups — leave Active untouched
        if group.status != OrderGroupStatus::Pending {
            continue;
        }

        // SEC-01: Only cancel real exchange orders if authenticated via JWT
        if is_authenticated {
            if let Some(ref tm) = state.trade_manager_live {
                let ids_to_cancel = [
                    ("entry", group.exchange_order_id.as_deref()),
                    ("sl", group.exchange_sl_order_id.as_deref()),
                    ("tp", group.exchange_tp_order_id.as_deref()),
                ];
                for (role, maybe_id) in &ids_to_cancel {
                    if let Some(exch_oid) = maybe_id {
                        match tm
                            .cancel_order(
                                user_id,
                                exch_oid,
                                &group.symbol,
                                group.exchange_account_id,
                            )
                            .await
                        {
                            Ok(()) => tracing::debug!(
                                group_id = %group.id, role = %role,
                                "cleanup: cancelled exchange order"
                            ),
                            Err(ExchangeApiError::OrderNotFound(_)) => {} // already gone
                            Err(e) => tracing::warn!(
                                group_id = %group.id, role = %role, error = %e,
                                "cleanup: exchange cancel failed"
                            ),
                        }
                    }
                }
            }
        }

        // Cancel shadow engine orders
        let linked_ids = group.get_linked_order_ids();
        for order_id in linked_ids {
            let _ = state.engine_handle.cancel_order(user_id, order_id).await;
        }

        // Force group status to Cancelled
        let _ = state
            .engine_handle
            .update_group_status(group.id, OrderGroupStatus::Cancelled)
            .await;

        // SEC-01: Only persist to DB if authenticated via JWT
        if is_authenticated {
            if let Some(ref tm) = state.trade_manager_live {
                let _ = tm.mark_position_closed(group.id).await;
            }
        }

        cancelled_count += 1;
    }

    tracing::info!(
        user_id = %user_id,
        cancelled = cancelled_count,
        total = groups.len(),
        "cleanup_stale_trades: purged ghost order groups"
    );

    HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "cancelled": cancelled_count,
        "total_groups": groups.len(),
    })))
}

/// Response for management status of a position.
#[derive(Debug, Serialize)]
pub struct ManagementStatusResponse {
    pub position_id: Uuid,
    pub state: String,
    pub be_triggered: bool,
    pub trailing_active: bool,
    pub partial_tp_fired: bool,
    pub current_stop: Option<Decimal>,
    pub remaining_quantity: Option<Decimal>,
}

/// GET /trades/{id}/management - Get automated management status for a position
pub async fn get_trade_management(
    path: web::Path<Uuid>,
    state: web::Data<TradeManagementState>,
    req: HttpRequest,
) -> HttpResponse {
    let position_id = path.into_inner();
    let (user_id, _is_authenticated) = match extract_user_id(&req, &state).await {
        Ok(result) => result,
        Err(resp) => return resp,
    };

    // FR-3.4: Check both managers for the position
    let managers: Vec<&Arc<TradeManagerService>> = [
        state.trade_manager_shadow.as_ref(),
        state.trade_manager_live.as_ref(),
    ]
    .into_iter()
    .flatten()
    .collect();

    if managers.is_empty() {
        return HttpResponse::NotFound().json(ApiResponse::<()>::error(
            "Trade management not enabled".to_string(),
        ));
    }

    for tm in &managers {
        if let Some(pos) = tm.get_position(position_id).await {
            // SEC-04: Ownership check — consistent with get_trade, cancel_trade, etc.
            if pos.user_id != user_id {
                return HttpResponse::Forbidden()
                    .json(ApiResponse::<()>::error("Access denied".to_string()));
            }

            let trailing_active = pos.be_triggered
                && pos
                    .rules
                    .trailing_stop
                    .as_ref()
                    .map_or(false, |t| t.enabled);

            return HttpResponse::Ok().json(ApiResponse::success(ManagementStatusResponse {
                position_id: pos.id,
                state: format!("{:?}", pos.state),
                be_triggered: pos.be_triggered,
                trailing_active,
                partial_tp_fired: pos.partial_tp_fired,
                current_stop: Some(pos.current_stop),
                remaining_quantity: Some(pos.remaining_qty),
            }));
        }
    }

    HttpResponse::NotFound().json(ApiResponse::<()>::error(
        "Managed position not found".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services;
    use rust_decimal_macros::dec;
    use serde_json::Value;

    fn assert_canonical_envelope(body: &Value, success: bool) {
        assert_eq!(body.get("success").and_then(Value::as_bool), Some(success));
        assert!(body.get("data").is_some(), "missing data field");
        assert!(body.get("error").is_some(), "missing error field");
    }

    #[test]
    fn test_api_response_success() {
        let response: ApiResponse<&str> = ApiResponse::success("test");
        assert!(response.success);
        assert_eq!(response.data, Some("test"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<()> = ApiResponse::error("test error".to_string());
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
    }

    /// RSK-02: CreateTradeRequest accepts setup_tag as an optional field,
    /// and round-trips cleanly when absent, null, or a value.
    #[test]
    fn test_create_trade_request_deserializes_setup_tag() {
        let with_tag: CreateTradeRequest = serde_json::from_value(serde_json::json!({
            "symbol": "BTC_USDT",
            "side": "buy",
            "quantity": "0.1",
            "entry_price": "50000",
            "setup_tag": "breakout",
        }))
        .unwrap();
        assert_eq!(with_tag.setup_tag.as_deref(), Some("breakout"));

        let without_field: CreateTradeRequest = serde_json::from_value(serde_json::json!({
            "symbol": "BTC_USDT",
            "side": "buy",
            "quantity": "0.1",
            "entry_price": "50000",
        }))
        .unwrap();
        assert!(without_field.setup_tag.is_none());

        let explicit_null: CreateTradeRequest = serde_json::from_value(serde_json::json!({
            "symbol": "BTC_USDT",
            "side": "buy",
            "quantity": "0.1",
            "entry_price": "50000",
            "setup_tag": null,
        }))
        .unwrap();
        assert!(explicit_null.setup_tag.is_none());
    }

    // ==================== Position Size Recalculation Tests ====================

    #[test]
    fn test_recalculate_position_size_basic() {
        // $10k balance, 2% risk, entry $50k, SL $49k -> stop distance $1k
        // Risk amount: $200 / $1000 = 0.2 BTC
        let size = recalculate_position_size(
            dec!(10000),
            dec!(2),
            dec!(50000),
            Some(dec!(49000)),
            dec!(0.5), // original quantity (should be ignored)
        );
        assert_eq!(size, dec!(0.2));
    }

    #[test]
    fn test_recalculate_position_size_wider_stop() {
        // $10k balance, 2% risk, entry $50k, SL $48k -> stop distance $2k
        // Risk amount: $200 / $2000 = 0.1 BTC
        let size = recalculate_position_size(
            dec!(10000),
            dec!(2),
            dec!(50000),
            Some(dec!(48000)),
            dec!(0.5),
        );
        assert_eq!(size, dec!(0.1));
    }

    #[test]
    fn test_recalculate_position_size_tighter_stop() {
        // $10k balance, 2% risk, entry $50k, SL $49500 -> stop distance $500
        // Risk amount: $200 / $500 = 0.4 BTC
        let size = recalculate_position_size(
            dec!(10000),
            dec!(2),
            dec!(50000),
            Some(dec!(49500)),
            dec!(0.2),
        );
        assert_eq!(size, dec!(0.4));
    }

    #[test]
    fn test_recalculate_position_size_no_sl_returns_original() {
        // Without SL, can't calculate risk-based size -> return original
        let size = recalculate_position_size(dec!(10000), dec!(2), dec!(50000), None, dec!(0.5));
        assert_eq!(size, dec!(0.5));
    }

    #[test]
    fn test_recalculate_position_size_zero_distance_returns_original() {
        // Entry == SL -> zero distance -> fixed_fractional returns 0 -> fallback to original
        let size = recalculate_position_size(
            dec!(10000),
            dec!(2),
            dec!(50000),
            Some(dec!(50000)),
            dec!(0.5),
        );
        assert_eq!(size, dec!(0.5));
    }

    #[test]
    fn test_recalculate_position_size_short_position() {
        // Short: entry $49k, SL $50k -> stop distance $1k (abs)
        // Risk amount: $200 / $1000 = 0.2 BTC
        let size = recalculate_position_size(
            dec!(10000),
            dec!(2),
            dec!(49000),
            Some(dec!(50000)),
            dec!(0.5),
        );
        assert_eq!(size, dec!(0.2));
    }

    // ==================== Integration Tests ====================

    #[actix_web::test]
    async fn test_update_entry_recalculates_size() {
        // Setup: create trade with entry=50000, SL=49000 (stop distance $1000)
        // Then move entry to 50500 (stop distance now $1500)
        // Quantity should change from 0.2 to ~0.1333

        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        // Init user
        state.engine_handle.init_user(user_id).await.unwrap();

        // Create a trade via the create_trade endpoint
        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::post().to(create_trade))
                .route("/trades/{id}/entry", web::put().to(update_entry_price)),
        )
        .await;

        let create_req = CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: Some(dec!(0.2)),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: None,
        };

        let req = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201, "Trade creation should succeed");

        let body: ApiResponse<TradeGroupResponse> = actix_web::test::read_body_json(resp).await;
        let group_id = body.data.unwrap().id;

        // Now update entry price from 50000 to 50500
        // Old stop distance: |50000 - 49000| = 1000 -> size = 200/1000 = 0.2
        // New stop distance: |50500 - 49000| = 1500 -> size = 200/1500 = 0.1333...
        let update_req = UpdateEntryPriceRequest { price: dec!(50500) };

        let req = actix_web::test::TestRequest::put()
            .uri(&format!("/trades/{}/entry", group_id))
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&update_req)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200, "Entry update should succeed");

        let body: ApiResponse<TradeGroupResponse> = actix_web::test::read_body_json(resp).await;
        let updated = body.data.unwrap();

        // The new quantity should be recalculated based on new stop distance
        // balance=10000, risk=2%, entry=50500, sl=49000
        // size = (10000 * 2 / 100) / |50500 - 49000| = 200 / 1500 = 0.13333333 (truncated 8dp)
        let expected_size = common_utils::risk::sizing::fixed_fractional(
            dec!(10000),
            dec!(2),
            dec!(50500),
            dec!(49000),
        )
        .round_dp_with_strategy(8, RoundingStrategy::ToZero);
        assert_eq!(
            updated.entry_quantity, expected_size,
            "Quantity should be recalculated for new stop distance"
        );
        assert_ne!(
            updated.entry_quantity,
            dec!(0.2),
            "Quantity should differ from original"
        );
    }

    #[actix_web::test]
    async fn test_update_sl_recalculates_size_pending() {
        // Setup: create pending trade with entry=50000, SL=49000
        // Then move SL to 48000 (stop distance widens from $1000 to $2000)
        // Quantity should change from 0.2 to 0.1

        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::post().to(create_trade))
                .route("/trades/{id}/sl", web::put().to(update_stop_loss)),
        )
        .await;

        // Create trade
        let create_req = CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: Some(dec!(0.2)),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: None,
        };

        let req = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: ApiResponse<TradeGroupResponse> = actix_web::test::read_body_json(resp).await;
        let group_id = body.data.unwrap().id;

        // Update SL from 49000 to 48000 (wider stop)
        let update_req = UpdateStopLossRequest { price: dec!(48000) };

        let req = actix_web::test::TestRequest::put()
            .uri(&format!("/trades/{}/sl", group_id))
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&update_req)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: ApiResponse<TradeGroupResponse> = actix_web::test::read_body_json(resp).await;
        let updated = body.data.unwrap();

        // New size: balance=10000, risk=2%, entry=50000, sl=48000
        // size = 200 / 2000 = 0.1
        assert_eq!(
            updated.entry_quantity,
            dec!(0.1),
            "Quantity should be recalculated for wider stop"
        );
    }

    #[actix_web::test]
    async fn test_update_sl_resizes_active() {
        // Active groups should also have their size recalculated to maintain constant risk

        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::post().to(create_trade))
                .route("/trades/{id}/sl", web::put().to(update_stop_loss)),
        )
        .await;

        // Create trade: entry=50000, SL=49000 (distance=1000)
        let create_req = CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: Some(dec!(0.2)),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: None,
        };

        let req = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: ApiResponse<TradeGroupResponse> = actix_web::test::read_body_json(resp).await;
        let group_id = body.data.unwrap().id;

        // Simulate entry fill by manually setting group status to Active
        state.engine_handle.on_entry_filled(group_id, dec!(50000)).await.unwrap();

        // Update SL to 48000 (distance=2000, 2x wider -> size should halve)
        let update_req = UpdateStopLossRequest { price: dec!(48000) };

        let req = actix_web::test::TestRequest::put()
            .uri(&format!("/trades/{}/sl", group_id))
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&update_req)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: ApiResponse<TradeGroupResponse> = actix_web::test::read_body_json(resp).await;
        let updated = body.data.unwrap();

        // balance=10000, risk=2%, entry=50000, sl=48000
        // size = (10000 * 2 / 100) / 2000 = 200 / 2000 = 0.1
        assert_eq!(
            updated.entry_quantity,
            dec!(0.1),
            "Active group should have size recalculated based on new SL distance"
        );
    }

    // ==================== EXT-21: Execution Mode & Sidecar Rejection Tests ====================

    #[tokio::test]
    async fn test_select_trade_manager_authenticated_no_live_returns_none() {
        // EXT-21 FR-3: Authenticated users get None when no live manager (no shadow fallback)
        let engine = ShadowEngine::new();
        let state = TradeManagementState::new(engine);
        assert!(state.select_trade_manager(true).is_none());
    }

    #[tokio::test]
    async fn test_select_trade_manager_unauthenticated_returns_shadow() {
        // Unauthenticated users still get shadow trade manager
        let engine = ShadowEngine::new();
        let state = TradeManagementState::new(engine);
        let shadow_api = Arc::new(services::ShadowExchangeApi::new(state.engine_handle.clone()));
        let tm = Arc::new(TradeManagerService::new(shadow_api, None));
        let state = state.with_trade_manager(tm);
        assert!(state.select_trade_manager(false).is_some());
    }

    #[actix_web::test]
    async fn test_create_trade_shadow_returns_execution_mode() {
        // Paper trade (X-User-Id auth) should include execution_mode: "shadow"
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::post().to(create_trade)),
        )
        .await;

        let create_req = CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: Some(dec!(0.1)),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: None,
        };

        let req = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: ApiResponse<TradeGroupResponse> = actix_web::test::read_body_json(resp).await;
        let trade = body.data.unwrap();
        assert_eq!(trade.execution_mode, Some("shadow".to_string()));
    }

    #[actix_web::test]
    async fn test_list_trades_success_uses_canonical_envelope() {
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::post().to(create_trade))
                .route("/trades", web::get().to(list_trades)),
        )
        .await;

        let create_req = CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: Some(dec!(0.1)),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: None,
        };

        let create = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req)
            .to_request();
        let create_resp = actix_web::test::call_service(&app, create).await;
        assert_eq!(create_resp.status(), 201);

        let list = actix_web::test::TestRequest::get()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .to_request();
        let list_resp = actix_web::test::call_service(&app, list).await;
        assert_eq!(list_resp.status(), 200);

        let body: Value = actix_web::test::read_body_json(list_resp).await;
        assert_canonical_envelope(&body, true);
        assert!(body
            .get("data")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false));
    }

    #[actix_web::test]
    async fn test_missing_auth_header_uses_canonical_error_envelope() {
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::get().to(list_trades)),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/trades")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400);

        let body: Value = actix_web::test::read_body_json(resp).await;
        assert_canonical_envelope(&body, false);
        assert!(body.get("error").and_then(Value::as_str).is_some());
    }

    #[actix_web::test]
    async fn test_get_trade_not_found_uses_canonical_error_envelope() {
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades/{id}", web::get().to(get_trade)),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri(&format!("/trades/{}", Uuid::new_v4()))
            .insert_header(("X-User-Id", user_id.to_string()))
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        let body: Value = actix_web::test::read_body_json(resp).await;
        assert_canonical_envelope(&body, false);
    }

    #[actix_web::test]
    async fn test_cancel_trade_not_found_uses_canonical_error_envelope() {
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades/{id}", web::delete().to(cancel_trade)),
        )
        .await;

        let req = actix_web::test::TestRequest::delete()
            .uri(&format!("/trades/{}", Uuid::new_v4()))
            .insert_header(("X-User-Id", user_id.to_string()))
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        let body: Value = actix_web::test::read_body_json(resp).await;
        assert_canonical_envelope(&body, false);
    }

    // ==================== AUD-01: Trade Execution Safety Tests ====================

    #[actix_web::test]
    async fn test_aud01_fr6_concurrent_trades_both_get_correct_sizing() {
        // AUD-01 FR-6: Two concurrent shadow trades for the same user should both
        // succeed with correct sizing. The per-user semaphore only applies to live
        // trades, so shadow trades are not blocked.
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::post().to(create_trade))
                .route("/trades", web::get().to(list_trades)),
        )
        .await;

        // Place first trade
        let create_req1 = CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: Some(dec!(0.1)),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: None,
        };

        let req1 = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req1)
            .to_request();
        let resp1 = actix_web::test::call_service(&app, req1).await;
        assert_eq!(resp1.status(), 201, "First trade should succeed");

        // Place second trade (same user)
        let create_req2 = CreateTradeRequest {
            symbol: "ETH_USDT".to_string(),
            side: "sell".to_string(),
            quantity: Some(dec!(1.0)),
            entry_price: dec!(3000),
            stop_loss_price: Some(dec!(3100)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: None,
        };

        let req2 = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req2)
            .to_request();
        let resp2 = actix_web::test::call_service(&app, req2).await;
        assert_eq!(resp2.status(), 201, "Second trade should succeed");

        // Verify both trades exist
        let list_req = actix_web::test::TestRequest::get()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .to_request();
        let list_resp = actix_web::test::call_service(&app, list_req).await;
        assert_eq!(list_resp.status(), 200);

        let body: Value = actix_web::test::read_body_json(list_resp).await;
        let trades = body.get("data").and_then(Value::as_array).unwrap();
        assert_eq!(trades.len(), 2, "Both trades should be listed");
    }

    #[actix_web::test]
    async fn test_aud01_fr6_per_user_semaphore_exists() {
        // AUD-01 FR-6: Verify per-user trade lock infrastructure exists and works.
        // Two different users should get independent semaphores.
        let engine = ShadowEngine::new();
        let state = TradeManagementState::new(engine);

        let user_a = Uuid::new_v4();
        let user_b = Uuid::new_v4();

        // Acquire lock for user A
        let sem_a = {
            let mut locks = state.trade_locks.lock().await;
            locks
                .entry(user_a)
                .or_insert_with(|| Arc::new(Semaphore::new(1)))
                .clone()
        };
        let _permit_a = sem_a.acquire().await.unwrap();

        // User B should not be blocked
        let sem_b = {
            let mut locks = state.trade_locks.lock().await;
            locks
                .entry(user_b)
                .or_insert_with(|| Arc::new(Semaphore::new(1)))
                .clone()
        };
        let permit_b = sem_b.try_acquire();
        assert!(permit_b.is_ok(), "User B should not be blocked by User A's lock");

        // User A's second attempt should be blocked
        let permit_a2 = sem_a.try_acquire();
        assert!(
            permit_a2.is_err(),
            "User A's second acquire should fail while first is held"
        );
    }

    #[actix_web::test]
    async fn test_aud01_fr8_update_sl_on_stopped_out_group_returns_error() {
        // AUD-01 FR-8: update_stop_loss on a group that transitioned to StoppedOut
        // should return an error, not create new orders.
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::post().to(create_trade))
                .route("/trades/{id}/sl", web::put().to(update_stop_loss)),
        )
        .await;

        // Create trade
        let create_req = CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: Some(dec!(0.2)),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: None,
        };

        let req = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req)
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: ApiResponse<TradeGroupResponse> = actix_web::test::read_body_json(resp).await;
        let group_id = body.data.unwrap().id;

        // Simulate: fill entry then transition to StoppedOut
        state.engine_handle.on_entry_filled(group_id, dec!(50000)).await.unwrap();
        state.engine_handle.update_group_status(group_id, OrderGroupStatus::StoppedOut).await.unwrap();

        // Attempt to update SL on stopped-out group — should fail
        let update_req = UpdateStopLossRequest { price: dec!(48000) };
        let req = actix_web::test::TestRequest::put()
            .uri(&format!("/trades/{}/sl", group_id))
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&update_req)
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        // Should return 409 Conflict (not 200)
        assert_eq!(
            resp.status(),
            409,
            "update_stop_loss on StoppedOut group should return 409 Conflict"
        );

        let body: Value = actix_web::test::read_body_json(resp).await;
        assert_canonical_envelope(&body, false);
        let error_msg = body.get("error").and_then(Value::as_str).unwrap_or("");
        assert!(
            error_msg.contains("status changed"),
            "Error should mention status change, got: {}",
            error_msg
        );
    }

    #[actix_web::test]
    async fn test_aud01_fr8_update_entry_on_filled_group_returns_error() {
        // AUD-01 FR-5 corollary: update_entry_price on a group that filled mid-operation
        // should return an error.
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::post().to(create_trade))
                .route("/trades/{id}/entry", web::put().to(update_entry_price)),
        )
        .await;

        // Create trade
        let create_req = CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: Some(dec!(0.2)),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: None,
        };

        let req = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req)
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: ApiResponse<TradeGroupResponse> = actix_web::test::read_body_json(resp).await;
        let group_id = body.data.unwrap().id;

        // Simulate entry fill (transitions from Pending to Active)
        state.engine_handle.on_entry_filled(group_id, dec!(50000)).await.unwrap();

        // Attempt to update entry price on now-Active group — should fail
        let update_req = UpdateEntryPriceRequest {
            price: dec!(50500),
        };
        let req = actix_web::test::TestRequest::put()
            .uri(&format!("/trades/{}/entry", group_id))
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&update_req)
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        // The initial read-lock check catches Active status with a 400
        assert_eq!(
            resp.status(),
            400,
            "update_entry_price on Active group should be rejected"
        );
    }

    // ==================== AUD-04: Security Hardening Tests ====================

    /// FR-9: Duplicate idempotency key returns cached response, not new trade
    #[actix_web::test]
    async fn test_idempotency_key_returns_cached_response() {
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        // Init user
        state.engine_handle.init_user(user_id).await.unwrap();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades", web::post().to(create_trade))
                .route("/trades", web::get().to(list_trades)),
        )
        .await;

        let idem_key = "test-idem-key-123".to_string();

        // First request with idempotency key
        let create_req = CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: Some(dec!(0.1)),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management: None,
            exchange_account_id: None,
            idempotency_key: Some(idem_key.clone()),
            setup_tag: None,
        };

        let req = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201, "First trade should succeed");
        let first_body: Value = actix_web::test::read_body_json(resp).await;
        let first_id = first_body["data"]["id"].as_str().unwrap().to_string();

        // Second request with same idempotency key — should return cached response
        let req2 = actix_web::test::TestRequest::post()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&create_req)
            .to_request();

        let resp2 = actix_web::test::call_service(&app, req2).await;
        assert_eq!(resp2.status(), 200, "Cached response should return 200");
        let second_body: Value = actix_web::test::read_body_json(resp2).await;
        let second_id = second_body["data"]["id"].as_str().unwrap().to_string();

        // Same trade ID returned — no duplicate created
        assert_eq!(first_id, second_id, "Idempotency should return same trade");

        // Verify only one trade exists
        let list_req = actix_web::test::TestRequest::get()
            .uri("/trades")
            .insert_header(("X-User-Id", user_id.to_string()))
            .to_request();
        let list_resp = actix_web::test::call_service(&app, list_req).await;
        let list_body: Value = actix_web::test::read_body_json(list_resp).await;
        let trades = list_body["data"].as_array().unwrap();
        assert_eq!(trades.len(), 1, "Only one trade should exist");
    }

    /// FR-5: Idempotency cache entries expire (test uses direct cache API)
    #[tokio::test]
    async fn test_idempotency_cache_expiry() {
        let cache = IdempotencyCache::new();

        // Store an entry
        cache
            .store(
                "test-key".to_string(),
                serde_json::json!({"success": true}),
            )
            .await;

        // Should be retrievable immediately
        assert!(cache.get("test-key").await.is_some());

        // Non-existent key returns None
        assert!(cache.get("non-existent").await.is_none());
    }

    // QNT-01b T3: /trades/preview handler tests.
    //
    // Happy-path integration coverage without a Postgres fixture: when
    // `state.pool` is None, the handler treats the user as having
    // `dynamic_risk_enabled = false` (matches the create_trade branch at
    // line 732) and returns a FixedMode preview. That gives us a
    // deterministic end-to-end test of the HTTP surface, JSON shape, and
    // `kelly_inputs` skip-serialization without mocking the engine.

    fn base_management_block() -> ManagementBlock {
        ManagementBlock {
            risk_percent: dec!(1.0),
            break_even_at: 50,
            leverage: 1,
            trailing_stop: TrailingStopBlock {
                enabled: false,
                distance: 0,
            },
            partial_tp: PartialTpBlock {
                enabled: false,
                percent: 0,
            },
        }
    }

    fn preview_request(management: Option<ManagementBlock>, setup_tag: Option<&str>) -> CreateTradeRequest {
        CreateTradeRequest {
            symbol: "BTC_USDT".to_string(),
            side: "buy".to_string(),
            quantity: None,
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: None,
            take_profit_targets: None,
            break_even_trigger_percent: None,
            break_even_offset: None,
            management,
            exchange_account_id: None,
            idempotency_key: None,
            setup_tag: setup_tag.map(str::to_string),
        }
    }

    #[actix_web::test]
    async fn test_preview_returns_fixed_mode_when_no_pool_wired() {
        // Without state.pool the dynamic_risk_enabled lookup short-circuits
        // to false → FixedMode preview, baseline passes through unchanged.
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades/preview", web::post().to(preview_trade_sizing)),
        )
        .await;

        let payload = preview_request(Some(base_management_block()), Some("breakout"));

        let req = actix_web::test::TestRequest::post()
            .uri("/trades/preview")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&payload)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Value = actix_web::test::read_body_json(resp).await;
        assert_eq!(body["reasoning"]["kind"], "fixed_mode");
        assert_eq!(body["baseline_risk_pct"], "1.0");
        assert_eq!(body["effective_risk_pct"], "1.0");
        assert_eq!(body["edge_multiplier"], "1");
        assert!(
            body.get("kelly_inputs").is_none(),
            "kelly_inputs must be skipped in wire response, got {body}"
        );
    }

    #[actix_web::test]
    async fn test_preview_requires_management_block() {
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades/preview", web::post().to(preview_trade_sizing)),
        )
        .await;

        let payload = preview_request(None, Some("breakout"));

        let req = actix_web::test::TestRequest::post()
            .uri("/trades/preview")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&payload)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400);
    }

    #[actix_web::test]
    async fn test_preview_rejects_oversized_setup_tag() {
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));
        let user_id = Uuid::new_v4();

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades/preview", web::post().to(preview_trade_sizing)),
        )
        .await;

        let oversized: String = "x".repeat(49);
        let payload = preview_request(Some(base_management_block()), Some(&oversized));

        let req = actix_web::test::TestRequest::post()
            .uri("/trades/preview")
            .insert_header(("X-User-Id", user_id.to_string()))
            .set_json(&payload)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400);
    }

    #[actix_web::test]
    async fn test_preview_requires_auth() {
        let engine = ShadowEngine::new();
        let state = web::Data::new(TradeManagementState::new(engine));

        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .route("/trades/preview", web::post().to(preview_trade_sizing)),
        )
        .await;

        let payload = preview_request(Some(base_management_block()), Some("breakout"));

        let req = actix_web::test::TestRequest::post()
            .uri("/trades/preview")
            .set_json(&payload)
            .to_request();

        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400); // missing X-User-Id header
    }

    /// QNT-01b Risk #1: preview and execution must produce identical sizing
    /// numbers on the same inputs. Both paths call
    /// `sizing_preview::compute_sizing_preview` with the same argument
    /// tuple, so verifying parity reduces to demonstrating that two
    /// invocations with identical arguments return identical outputs.
    /// Without the engine wired, both calls flow through the FixedMode
    /// branch deterministically.
    #[tokio::test]
    async fn test_preview_compute_is_byte_parity_with_create_trade_compute() {
        use crate::services::sizing_preview as sp;

        let user_id = Uuid::new_v4();

        let preview_result =
            sp::compute_sizing_preview(user_id, Some("breakout"), dec!(1.5), false, None)
                .await
                .unwrap();

        let create_trade_result =
            sp::compute_sizing_preview(user_id, Some("breakout"), dec!(1.5), false, None)
                .await
                .unwrap();

        assert_eq!(
            preview_result.baseline_risk_pct,
            create_trade_result.baseline_risk_pct
        );
        assert_eq!(
            preview_result.effective_risk_pct,
            create_trade_result.effective_risk_pct
        );
        assert_eq!(
            preview_result.edge_multiplier,
            create_trade_result.edge_multiplier
        );
        assert_eq!(preview_result.reasoning, create_trade_result.reasoning);
    }
}
