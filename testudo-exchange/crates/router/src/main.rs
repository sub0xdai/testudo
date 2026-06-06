#![allow(clippy::type_complexity)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::borrowed_box)]
#![allow(clippy::new_without_default)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::unnecessary_lazy_evaluations)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::for_kv_map)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_assignments)]

// @anchor exchange:router:main
// @tags api

use actix_cors::Cors;
use actix_web::{
    http::header,
    web::{self, scope},
    App, HttpResponse, HttpServer,
};
use common_utils::{
    adapters::CredentialValidator,
    auth::{JwtTokenService, TokenService},
};
use confik::{Configuration as _, EnvSource};
use dotenvy::dotenv;
use engine::{EngineActor, ShadowEngine};
use routes::{
    agent_journal, agent_keys, auth, coach, depth, dignitas, exchanges, imports, internal, journal, klines,
    market_data, onboarding, order, paper_balance, public_profile, risk, risk_config, signal, sync, tickers,
    trade, trade_events, trade_management, user_settings,
};
use dashmap::DashMap;
use sqlx_postgres::PostgresDb;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

// EXT-16 FR-1: Sidecar health monitoring
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarHealth {
    Healthy,
    Unreachable,
}

/// Shared sidecar health state for the health endpoint and background task.
pub struct SidecarHealthState {
    pub status: tokio::sync::RwLock<SidecarHealth>,
}

pub mod adapters; // Exchange adapters (Shadow, Binance)
pub mod config;
pub mod decision_loop; // Decision Loop for risk validation and position sizing
pub mod exchange; // Add exchange module
pub mod metrics; // AUD-05: Prometheus metrics
pub mod middleware;
pub mod policy; // AUTH-03: centralized permission policy engine
pub mod models; // JNL-01: Journal data models
pub mod repositories;
pub mod routes;
pub mod services; // Sync service and background tasks
pub mod types;
pub mod utils; // Universal validation and response abstractions
use crate::config::RouterConfig;
use crate::middleware::JwtMiddleware;
use crate::types::app::AppState;

/// SEC-02: Check if an origin is allowed by the CORS policy.
/// Web app origins checked against ALLOWED_ORIGINS.
/// Chrome extension origins pinned to ALLOWED_EXTENSION_ORIGINS (exact match).
/// Firefox moz-extension:// uses prefix check (UUIDs are per-install, not pinnable).
fn is_origin_allowed(origin: &str, allowed_origins: &str) -> bool {
    // Web app origins
    if allowed_origins.split(',').any(|o| o.trim() == origin) {
        return true;
    }

    // Chrome: pin to specific extension ID(s) via env var
    if origin.starts_with("chrome-extension://") {
        let ext_origins = std::env::var("ALLOWED_EXTENSION_ORIGINS").unwrap_or_default();
        if ext_origins.is_empty() {
            // No pinning configured — allow any (dev mode)
            return true;
        }
        return ext_origins.split(',').any(|o| o.trim() == origin);
    }

    // Firefox: per-install UUID, prefix check only (MDN: random UUID per instance)
    if origin.starts_with("moz-extension://") {
        return true;
    }

    false
}

/// AUD-05 FR-4: GET /api/v1/metrics — Prometheus metrics endpoint.
async fn prometheus_metrics() -> HttpResponse {
    let body = metrics::encode_metrics();
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(body)
}

/// EXT-16 FR-1.5: GET /api/v1/health/sidecar — returns current sidecar health state.
async fn get_sidecar_health(state: web::Data<SidecarHealthState>) -> HttpResponse {
    let status = state.status.read().await.clone();
    HttpResponse::Ok().json(serde_json::json!({ "status": status }))
}

/// AUD-06 FR-2: GET /api/v1/health/ready — readiness probe (can I serve traffic?).
/// Checks DB pool connectivity and CEX sidecar health.
async fn health_ready(
    app_state: web::Data<AppState>,
    sidecar_state: web::Data<SidecarHealthState>,
) -> HttpResponse {
    let db_ok = sqlx::query("SELECT 1")
        .execute(&app_state.pool)
        .await
        .is_ok();
    let sidecar_ok = *sidecar_state.status.read().await == SidecarHealth::Healthy;

    if db_ok && sidecar_ok {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "ready", "db": true, "sidecar": true
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready", "db": db_ok, "sidecar": sidecar_ok
        }))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    // AUD-05 FR-1: Structured JSON logging via tracing-subscriber
    // Falls back to RUST_LOG env var; defaults to "debug" if unset.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug"));
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .init();

    // AUD-05 FR-4/FR-5: Register Prometheus metrics
    metrics::register_metrics();

    // Get JWT secrets from environment - fail fast if not provided
    let jwt_access_secret = std::env::var("JWT_ACCESS_SECRET")
        .expect("JWT_ACCESS_SECRET environment variable must be provided");
    let jwt_refresh_secret = std::env::var("JWT_REFRESH_SECRET")
        .expect("JWT_REFRESH_SECRET environment variable must be provided");

    let mut config = RouterConfig::builder()
        .override_with(EnvSource::new())
        .try_build()
        .unwrap();

    // Set the JWT secrets and other defaults
    config.jwt_access_secret = jwt_access_secret;
    config.jwt_refresh_secret = jwt_refresh_secret;
    config.jwt_access_expires_seconds = 3600; // 1 hour
    config.jwt_refresh_expires_seconds = 2592000; // 30 days
    config.rate_limit_requests_per_minute = 100;
    config.rate_limit_burst_capacity = 10;

    // Validate critical security configuration
    if let Err(err) = config.validate() {
        tracing::error!(error = %err, "Configuration validation failed");
        std::process::exit(1);
    }

    // Database pool for repositories
    let postgres_db = PostgresDb::new().await.unwrap();
    let pg_pool = postgres_db
        .get_pg_connection()
        .expect("Failed to get PG pool");

    // Apply pending schema migrations on startup — deploy + migrate become one step.
    // Migrations are embedded at compile time via the macro; no runtime filesystem access.
    tracing::info!("Applying database migrations");
    if let Err(err) = sqlx::migrate!("../sqlx_postgres/migrations")
        .run(&pg_pool)
        .await
    {
        tracing::error!(error = %err, "Migration failed");
        std::process::exit(1);
    }
    tracing::info!("Database migrations up to date");

    // AUTH-02: Create TokenService (replaces email/password AuthService)
    let token_service: Arc<dyn TokenService> = Arc::new(JwtTokenService::new(
        config.jwt_access_secret.clone(),
        config.jwt_refresh_secret.clone(),
    ));

    // 019e: Create shadow engine — owned by the EngineActor (no Arc/RwLock).
    let engine = ShadowEngine::new();

    // 019e: Spawn EngineActor — takes ownership of the engine.
    let (engine_handle, fill_event_rx, trade_event_rx, _trade_event_tx) = EngineActor::spawn_shared(engine);
    tracing::info!("EngineActor spawned");

    // Create ExecutionService with ShadowEngineAdapter (shadow-only for legacy /order route)
    let shadow_adapter = Arc::new(adapters::ShadowEngineAdapter::new(engine_handle.clone()));
    let execution_service = Arc::new(services::ExecutionService::shadow_only(shadow_adapter));

    // AUD-04 FR-3: Exchange account repository with AES-256-GCM encryption — fail fast
    let exchange_account_repo = {
        use repositories::exchange_account::{AesGcmVault, ExchangeAccountRepository};
        let vault = AesGcmVault::from_env().expect(
            "ENCRYPTION_KEY environment variable is required. \
             Exchange credentials cannot be stored without it.",
        );
        ExchangeAccountRepository::new(pg_pool.clone(), vault)
    };

    // 012: Create CEX sidecar client for live trading
    let cex_config = services::CexSidecarConfig::from_env();
    let cex_client = Arc::new(services::CexClient::new(&cex_config));

    // FR-4.5: Health check sidecar on startup (warning if unreachable, not fatal)
    let initial_health = match cex_client.health_check().await {
        Ok(()) => {
            tracing::info!("CEX sidecar reachable at {}", cex_config.base_url);
            SidecarHealth::Healthy
        }
        Err(e) => {
            tracing::warn!(
                "CEX sidecar not reachable ({}). Live trading unavailable, paper mode OK.",
                e
            );
            SidecarHealth::Unreachable
        }
    };

    // EXT-16 FR-1: Shared sidecar health state
    let sidecar_health_state = Arc::new(SidecarHealthState {
        status: tokio::sync::RwLock::new(initial_health),
    });

    // AUD-03 FR-1: Create shutdown cancellation token
    let shutdown = CancellationToken::new();

    // AUD-03 FR-2: Wire SIGTERM/SIGINT to cancel the token
    {
        let shutdown_signal = shutdown.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for ctrl_c");
            tracing::info!("Shutdown signal received, initiating graceful shutdown...");
            shutdown_signal.cancel();
        });
    }

    // 019f: Spawn TradeEventWriter — single-writer persistence for trade event audit log
    {
        let writer = services::trade_event_writer::TradeEventWriter::new(
            trade_event_rx,
            pg_pool.clone(),
        );
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("TradeEventWriter shutting down");
                }
                _ = writer.run() => {}
            }
        });
        tracing::info!("TradeEventWriter spawned (batch=50, flush=100ms)");
    }

    // EXT-16 FR-1.1: Spawn background health check task (30s interval)
    {
        let client = cex_client.clone();
        let health_state = sidecar_health_state.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            interval.tick().await; // skip first immediate tick (already checked above)
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        tracing::info!("Sidecar health monitor shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        let new_status = match client.health_check().await {
                            Ok(()) => SidecarHealth::Healthy,
                            Err(_) => SidecarHealth::Unreachable,
                        };
                        let old_status = health_state.status.read().await.clone();
                        if new_status != old_status {
                            match (&old_status, &new_status) {
                                (SidecarHealth::Healthy, SidecarHealth::Unreachable) => {
                                    tracing::warn!("Sidecar health: healthy -> unreachable");
                                }
                                (SidecarHealth::Unreachable, SidecarHealth::Healthy) => {
                                    tracing::info!("Sidecar health: unreachable -> healthy");
                                }
                                _ => {}
                            }
                            *health_state.status.write().await = new_status;
                        }
                    }
                }
            }
        });
        tracing::info!("Sidecar health monitor spawned (30s interval)");
    }

    // EXT-24: Position repository for managed position persistence + schema migration
    let position_repository =
        services::trade_manager::repository::PositionRepository::new(pg_pool.clone());
    position_repository
        .create_table()
        .await
        .expect("Failed to create/migrate managed_positions table");

    // Create market data state for Binance integration
    let market_data_state = web::Data::new(market_data::MarketDataState::new());

    // EXT-16 FR-4 / AUD-03 FR-6: Bounded management event channel (backpressure at 1024)
    let (mgmt_event_tx, mut mgmt_event_rx) =
        tokio::sync::mpsc::channel::<services::ManagementEvent>(1024);

    // EXT-22/EXT-25 / 017 FR-1: Shared order update mpsc channel.
    // WS subscription manager forwards sidecar events into this channel,
    // and FillDetectorService is the sole consumer. Using mpsc instead of
    // broadcast so the producer applies backpressure instead of dropping events.
    let (order_update_tx, order_update_rx) =
        tokio::sync::mpsc::channel::<services::OrderUpdateEvent>(1024);

    // EXT-09: Create shadow trade manager service (019b: uses EngineHandle)
    let shadow_exchange_api = Arc::new(services::ShadowExchangeApi::new(engine_handle.clone()));
    let trade_manager_shadow = Arc::new(
        services::TradeManagerService::new(shadow_exchange_api, None)
            .with_event_sender(mgmt_event_tx.clone()),
    );

    // 012: Create live trade manager with CexExchangeApi via sidecar
    let ccxt_enabled = std::env::var("CCXT_ENABLED").unwrap_or_default() == "true"
        || std::env::var("CCXT_SIDECAR_URL").is_ok();
    let sandbox = std::env::var("CCXT_SANDBOX").unwrap_or_else(|_| "true".to_string()) != "false";

    // HL-05: Check for Hyperliquid native integration
    let hl_enabled = std::env::var("HYPERLIQUID_ENABLED").unwrap_or_default() == "true";
    let hl_network = if std::env::var("HYPERLIQUID_TESTNET").unwrap_or_default() == "true" {
        hyperliquid_sdk_rs::Network::Testnet
    } else {
        hyperliquid_sdk_rs::Network::Mainnet
    };

    // FIX-07: Shared HTTP client for all Hyperliquid API calls (connection pooling + 10s timeout)
    let hl_http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to build Hyperliquid HTTP client");

    // HL-05: Build optional Hyperliquid components
    let hl_auth_cache: Option<Arc<services::hyperliquid::auth::AuthCache>> = if hl_enabled {
        Some(Arc::new(services::hyperliquid::auth::AuthCache::new()))
    } else {
        None
    };

    // JNL-12 FR-7: Dedicated analytics pool for journal read queries
    let analytics_pool = postgres_db.analytics_pool().clone();

    // HL-01: Fetch asset universe before AppState so it's available to route handlers
    let hl_universe: Option<Arc<services::hyperliquid::AssetUniverse>> = if hl_enabled {
        match services::hyperliquid::AssetUniverse::fetch(hl_network).await {
            Ok(u) => Some(Arc::new(u)),
            Err(e) => {
                tracing::error!("Failed to fetch Hyperliquid asset universe: {}", e);
                None
            }
        }
    } else {
        None
    };

    // RSK-03: Build the weekly AI trade coach pipeline.
    // Narrator uses an OpenAI-compatible endpoint so DeepSeek/GLM/OpenRouter
    // all work via config. API key is fetched from env (fail-fast in prod).
    let coach_service = {
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
            tracing::warn!(
                "OPENAI_API_KEY not set — coach narrator will fail and stats-only \
                 fallback rows will be persisted for every generated report."
            );
            String::new()
        });
        let narrator: std::sync::Arc<dyn services::coach::narrator::Narrator> =
            std::sync::Arc::new(services::coach::narrator::OpenAiNarrator::new(
                config.llm_base_url.clone(),
                api_key,
                config.llm_model.clone(),
            ));
        let coach_config = services::coach::CoachConfig {
            min_lifetime_trades: config.coach_min_lifetime_trades,
            min_week_trades: config.coach_min_week_trades,
            enabled_global: config.coach_enabled_global,
        };
        std::sync::Arc::new(services::coach::CoachService::new(
            pg_pool.clone(),
            analytics_pool.clone(),
            narrator,
            coach_config,
        ))
    };

    // RSK-03: Spawn the weekly scheduler (hourly wake-up, fires at Sun 18:00 UTC).
    if config.coach_enabled_global {
        services::coach::schedule::spawn_weekly_task(
            coach_service.clone(),
            pg_pool.clone(),
            shutdown.clone(),
        );
        tracing::info!("Coach weekly scheduler spawned (Sun 18:00 UTC)");
    } else {
        tracing::info!("Coach globally disabled (COACH_ENABLED_GLOBAL=false)");
    }

    // ENG-01a: Spawn the daily dignitas snapshot scheduler (UTC 00:xx).
    services::dignitas::schedule::spawn_daily_task(pg_pool.clone(), shutdown.clone());
    tracing::info!("Dignitas daily scheduler spawned (UTC 00:xx)");

    // QNT-01a: Calibration engine — shared across HTTP handlers and the
    // trade-management state.
    let calibration_engine =
        Arc::new(services::calibration::CalibrationEngine::new(pg_pool.clone()));

    let journal_syncer_notifiers: Arc<dashmap::DashMap<uuid::Uuid, Arc<tokio::sync::Notify>>> =
        Arc::new(dashmap::DashMap::new());
    let journal_syncer_last_notified: Arc<dashmap::DashMap<uuid::Uuid, std::time::Instant>> =
        Arc::new(dashmap::DashMap::new());

    let signal_idempotency: Arc<DashMap<String, serde_json::Value>> =
        Arc::new(DashMap::new());

    // AGENT-01: Per-user rate limiter for signals (30 req / 60s by default).
    let signal_rate_limit_max: usize = std::env::var("SIGNAL_RATE_LIMIT_MAX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let signal_rate_limit_window_secs: u64 = std::env::var("SIGNAL_RATE_LIMIT_WINDOW_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let signal_rate_limiter: Arc<DashMap<uuid::Uuid, Vec<std::time::Instant>>> =
        Arc::new(DashMap::new());
    let signal_rate_limit_max = Arc::new(signal_rate_limit_max);
    let signal_rate_limit_window = std::time::Duration::from_secs(signal_rate_limit_window_secs);
    let signal_rate_limit_window = Arc::new(signal_rate_limit_window);

    let app_state = web::Data::new(AppState {
        postgres_db,
        pool: pg_pool.clone(),
        pg_queue: pg_queue::PgQueueManager::new(pg_pool.clone()),
        token_service: token_service.clone(),
        config: config.clone(),
        credential_validator: CredentialValidator::new(),
        execution_service: execution_service.clone(),
        engine_handle: engine_handle.clone(),
        exchange_account_repo: exchange_account_repo.clone(),
        cex_client: cex_client.clone(),
        hl_auth_cache: hl_auth_cache.clone(),
        hl_universe: hl_universe.clone(),
        hl_http_client: hl_http_client.clone(),
        hl_network,
        analytics_pool,
        cex_sandbox: sandbox,
        coach_service: coach_service.clone(),
        calibration_engine: calibration_engine.clone(),
        journal_syncer_notifiers: journal_syncer_notifiers.clone(),
        journal_syncer_last_notified: journal_syncer_last_notified.clone(),
        signal_idempotency: signal_idempotency.clone(),
        signal_rate_limiter: signal_rate_limiter.clone(),
        signal_rate_limit_max: *signal_rate_limit_max,
        signal_rate_limit_window: *signal_rate_limit_window,
    });

    let cex_exchange_api: Option<Arc<services::CexExchangeApi>> = if ccxt_enabled {
        let api = Arc::new(services::CexExchangeApi::new(
            cex_client.clone(),
            exchange_account_repo.clone(),
            sandbox,
        ));
        tracing::info!(
            "CexExchangeApi enabled (sandbox={}, sidecar={})",
            sandbox,
            cex_config.base_url
        );
        Some(api)
    } else {
        tracing::warn!(
            "CEX sidecar not enabled — JWT-authenticated trades will return 503. \
             Set CCXT_ENABLED=true or CCXT_SIDECAR_URL to enable live trading."
        );
        None
    };

    // HL-05: Build the live exchange API — routing, HL-only, or CEX-only
    let live_exchange_api: Option<Arc<dyn services::ExchangeApi>> =
        if let (Some(cex_api), Some(universe), Some(auth_cache)) =
            (cex_exchange_api.as_ref(), hl_universe.as_ref(), hl_auth_cache.as_ref())
        {
            // Both CEX sidecar and Hyperliquid available — use routing layer
            let hl_api = Arc::new(services::hyperliquid::HyperliquidExchangeApi::new(
                universe.clone(),
                auth_cache.clone(),
                exchange_account_repo.clone(),
                hl_network,
            ));
            let routing_api = Arc::new(services::RoutingExchangeApi::new(
                cex_api.clone(),
                hl_api,
                exchange_account_repo.clone(),
            ));
            tracing::info!("RoutingExchangeApi enabled (Hyperliquid + CEX sidecar)");
            Some(routing_api)
        } else if let (Some(universe), Some(auth_cache)) =
            (hl_universe.as_ref(), hl_auth_cache.as_ref())
        {
            // Hyperliquid only — no CEX sidecar needed
            let hl_api = Arc::new(services::hyperliquid::HyperliquidExchangeApi::new(
                universe.clone(),
                auth_cache.clone(),
                exchange_account_repo.clone(),
                hl_network,
            ));
            tracing::info!("HyperliquidExchangeApi enabled (Hyperliquid only, no CEX sidecar)");
            Some(hl_api)
        } else {
            cex_exchange_api.clone().map(|api| api as Arc<dyn services::ExchangeApi>)
        };

    let trade_manager_live: Option<Arc<services::TradeManagerService>> =
        live_exchange_api.as_ref().map(|api| {
            Arc::new(
                services::TradeManagerService::new(api.clone(), Some(position_repository.clone()))
                    .with_event_sender(mgmt_event_tx.clone())
                    .with_engine_handle(engine_handle.clone()),
            )
        });

    // JNL-13: Instantiate journal service for trade close recording
    let journal_service = Arc::new(services::journal_service::JournalService::new(
        pg_pool.clone(),
    ));

    // HL-05: WsSubscriptionManager with optional Hyperliquid native WS
    // REL-02: Also wires JournalService for direct HL closing fill writes
    let ws_subscription_manager: Option<Arc<services::WsSubscriptionManager>> =
        if live_exchange_api.is_some() {
            let mut mgr = services::WsSubscriptionManager::new(
                cex_client.clone(),
                exchange_account_repo.clone(),
                order_update_tx.clone(),
                sandbox,
            );
            if let Some(auth_cache) = hl_auth_cache.as_ref() {
                mgr = mgr.with_hyperliquid(hl_network, auth_cache.clone());
                tracing::info!("WsSubscriptionManager: Hyperliquid native WS enabled");
            }
            mgr = mgr.with_hl_notify_pool(pg_pool.clone());
            // REL-03: Wire engine handle + exchange API for group reconciliation
            if let Some(ref api) = live_exchange_api {
                mgr = mgr.with_engine(engine_handle.clone(), api.clone());
            }
            Some(Arc::new(mgr))
        } else {
            None
        };

    // EXT-09: Create price feed with broadcast channel for trade manager
    // 019d: Uses fire-and-forget push_price() — fills go to fill_event channel
    // 016 FR-2: Include live trade manager for polling live-only symbols
    let price_feed = {
        let mut pf = services::PriceFeedService::with_defaults(engine_handle.clone());
        if let Some(ref live_tm) = trade_manager_live {
            pf = pf.with_live_trade_manager(live_tm.clone());
        }
        pf
    };
    let price_rx = price_feed.subscribe();
    // Subscribe live trade manager from the SAME price feed (not a new orphan instance)
    let price_rx_live = price_feed.subscribe();

    // EXT-16 FR-4 / AUD-03 FR-5: Forward management events to WS via pg_queue NOTIFY
    {
        let pool = app_state.pool.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        tracing::info!("Management event forwarder shutting down");
                        break;
                    }
                    event = mgmt_event_rx.recv() => {
                        let Some(event) = event else { break };
                        let channel = format!("order.{}", event.user_id);
                        let payload = serde_json::json!({
                            "stream": channel,
                            "data": {
                                "e": event.detail,
                                "s": event.symbol,
                                "status": event.event_type,
                            }
                        });
                        if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
                            .bind(&channel)
                            .bind(payload.to_string())
                            .execute(&pool)
                            .await
                        {
                            tracing::warn!("Failed to publish management event: {}", e);
                        }
                    }
                }
            }
        });
        tracing::info!("Management event forwarder spawned");
    }

    // Create trade management state using the shared shadow engine (EXT-05: with auth_service for dual auth)
    let mut trade_state_builder =
        trade_management::TradeManagementState::new_with_handle(engine_handle.clone())
            .with_token_service(token_service.clone())
            .with_trade_manager(trade_manager_shadow.clone())
            .with_pool(app_state.pool.clone())
            .with_calibration_engine(calibration_engine.clone());

    if let Some(ref live_tm) = trade_manager_live {
        trade_state_builder = trade_state_builder.with_live_trade_manager(live_tm.clone());
    }

    if let Some(ref ws_manager) = ws_subscription_manager {
        trade_state_builder = trade_state_builder.with_ws_subscription_manager(ws_manager.clone());
    }

    let trade_state = web::Data::new(trade_state_builder);

    // EXT-24 / 019d: Rehydrate OrderGroups from persisted ManagedPositions.
    // Must run BEFORE spawning PriceFeed, TradeManager, FillDetector, and HTTP server.
    {
        let rehydration_service =
            services::rehydration::RehydrationService::new(position_repository.clone(), engine_handle.clone(), app_state.pool.clone());
        match rehydration_service.rehydrate().await {
            Ok(summary) => {
                tracing::info!(
                    "Startup rehydration complete: {} positions, {} exchange IDs",
                    summary.positions_loaded,
                    summary.exchange_ids_registered,
                );

                if let Some(ref ws_manager) = ws_subscription_manager {
                    match rehydration_service.collect_live_subscription_tuples().await {
                        Ok(tuples) => {
                            for tuple in tuples {
                                if let Err(e) = ws_manager
                                    .ensure_subscribed(
                                        tuple.user_id,
                                        tuple.exchange_account_id,
                                        &tuple.symbol,
                                    )
                                    .await
                                {
                                    tracing::warn!(
                                        "Startup live resubscribe failed: user={} account={} symbol={} error={}",
                                        tuple.user_id,
                                        tuple.exchange_account_id,
                                        tuple.symbol,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to collect startup live subscription tuples: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    "Startup rehydration failed: {}. Positions will not be restored.",
                    e
                );
            }
        }
    }

    // EXT-24 FR-3: Exchange verification on startup (disable with REHYDRATION_VERIFY_EXCHANGE=false)
    if std::env::var("REHYDRATION_VERIFY_EXCHANGE").unwrap_or_default() != "false" {
        let verify_service =
            services::rehydration::RehydrationService::new(position_repository.clone(), engine_handle.clone(), app_state.pool.clone());
        let summary = verify_service
            .verify_exchange(&cex_client, &exchange_account_repo, sandbox)
            .await;
        tracing::info!(
            "Exchange verification: {} verified, {} stale, {} errors",
            summary.verified,
            summary.stale_detected,
            summary.errors,
        );
    }

    // 017 FR-3 / 019d: Post-rehydration reconciliation log via EngineHandle.
    {
        let active_groups = engine_handle.active_group_count().await;
        tracing::info!(
            active_groups = active_groups,
            "Post-rehydration reconciliation: shadow engine state"
        );
    }

    // EXT-24: Load persisted positions into the live trade manager for price tick evaluation
    if let Some(ref live_tm) = trade_manager_live {
        match live_tm.load_from_db().await {
            Ok(count) => tracing::info!("Live trade manager loaded {} positions from DB", count),
            Err(e) => tracing::error!("Live trade manager failed to load positions: {}", e),
        }
    }

    // Spawn the price feed background service (008-shadow-fill-engine FR-1)
    {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            price_feed.run(shutdown).await;
        });
        tracing::info!("PriceFeedService spawned (2s poll interval)");
    }

    // EXT-09: Spawn shadow trade manager service
    {
        let tm = trade_manager_shadow.clone();
        let rx = price_rx;
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("TradeManagerService (shadow) shutting down");
                }
                _ = tm.run(rx) => {}
            }
        });
        tracing::info!("TradeManagerService (shadow) spawned");
    }

    // AUD-02 FR-5: Clone trade managers for the GC task before they're consumed
    let gc_trade_manager_shadow = trade_manager_shadow.clone();
    let gc_trade_manager_live = trade_manager_live.clone();

    // EXT-10: Spawn live trade manager service — uses same price feed broadcast channel
    if let Some(live_tm) = trade_manager_live {
        let tm = live_tm.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("TradeManagerService (live) shutting down");
                }
                _ = tm.run(price_rx_live) => {}
            }
        });
        tracing::info!("TradeManagerService (live) spawned");
    }

    // EXT-22 / 019d: Spawn FillDetectorService — listens to both WS and actor fill channels
    if let Some(ref api) = cex_exchange_api {
        let fill_detector = services::fill_detector::FillDetectorService::new(
            engine_handle.clone(),
            api.clone(),
        )
        .with_event_sender(mgmt_event_tx.clone());

        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("FillDetectorService shutting down");
                }
                _ = fill_detector.run(order_update_rx, fill_event_rx) => {}
            }
        });
        tracing::info!("FillDetectorService spawned (dual-channel)");
    }

    // HIST-01: Spawn trade history import worker
    {
        let import_queue = pg_queue::QueueRepository::new(pg_pool.clone());
        let import_worker = services::import_worker::ImportWorker::new(
            import_queue,
            exchange_account_repo.clone(),
            journal_service.clone(),
            hl_network,
            pg_pool.clone(),
        );
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            import_worker.run(shutdown).await;
        });
        tracing::info!("ImportWorker spawned");
    }

    // AUD-02 FR-5 / 019d: GC task — uses EngineHandle.prune_terminal()
    {
        let gc_handle = engine_handle.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            interval.tick().await; // skip first immediate tick
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        tracing::info!("GC task shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        let cutoff =
                            std::time::Instant::now() - std::time::Duration::from_secs(3600);

                        // Prune shadow engine collections via actor
                        let pruned = gc_handle.prune_terminal(cutoff).await;

                        // Prune trade manager closed positions
                        let tm_pruned = gc_trade_manager_shadow.prune_closed().await;
                        let live_pruned = if let Some(ref live_tm) = gc_trade_manager_live {
                            live_tm.prune_closed().await
                        } else {
                            0
                        };

                        let total = pruned + tm_pruned + live_pruned;
                        if total > 0 {
                            tracing::info!("GC: pruned {} terminal entries (engine={}, trade_mgr={})", total, pruned, tm_pruned + live_pruned);
                        }
                    }
                }
            }
        });
        tracing::info!("GC task spawned (5min interval, 1h TTL)");
    }

    // 018 / 019d: Spawn ReconciliationService if CEX is enabled — uses EngineHandle
    if let Some(ref api) = cex_exchange_api {
        let reconciliation_service = services::reconciliation::ReconciliationService::new(
            engine_handle.clone(),
            cex_client.clone(),
            exchange_account_repo.clone(),
            api.clone(),
            sandbox,
        )
        .with_position_repo(position_repository.clone())
        .with_event_sender(mgmt_event_tx.clone());

        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            interval.tick().await; // skip first immediate tick
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        tracing::info!("ReconciliationService shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        reconciliation_service.sweep().await;
                    }
                }
            }
        });
        tracing::info!("ReconciliationService spawned (30s interval)");
    }

    // EXT-22: Store the order update sender for use by WebSocket subscription tasks.
    // When a live trade is created and a sidecar WS connection is established,
    // order update events are pushed through this channel to the fill detector.
    let order_update_sender = web::Data::new(order_update_tx);

    let sidecar_health_data = web::Data::from(sidecar_health_state.clone());

    // AUTH-02 T5: Auth dependencies — injected via web::Data, not embedded in AppState
    let nonce_store = web::Data::new(services::auth::NonceStore::new());
    let pairing_store = web::Data::new(services::auth::PairingStore::new());
    // Rate limit extension-pair: 5 attempts per 60 seconds per IP
    let pair_rate_limiter = web::Data::new(middleware::RateLimiter::new(5, std::time::Duration::from_secs(60)));
    // ENG-01b: Rate limit public profile endpoint: 60 req/min per IP (dedicated instance)
    let pub_profile_rate_limiter = web::Data::new(middleware::RateLimiter::new(60, std::time::Duration::from_secs(60)));
    let session_repo = web::Data::new(repositories::session::SessionRepository::new(pg_pool.clone()));
    let user_repo = web::Data::new(repositories::user::PostgresUserRepository::new(pg_pool.clone()));

    // JNL-SYNC-01 FR-4/FR-15: Spawn JournalSyncer per active CCXT exchange account.
    let syncer_enabled = std::env::var("JOURNAL_SYNCER_ENABLED")
        .map(|v| v.to_lowercase() != "false")
        .unwrap_or(true);

    if syncer_enabled && ccxt_enabled {
        let interval_secs: u64 = std::env::var("JOURNAL_SYNC_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let journal_service_for_sync = Arc::new(services::journal_service::JournalService::new(
            pg_pool.clone(),
        ));

        // Query all active non-HL accounts at startup.
        let accounts: Vec<(uuid::Uuid, uuid::Uuid, String)> = sqlx::query_as(
            "SELECT id, user_id, exchange_name FROM exchange_accounts \
             WHERE is_active = TRUE AND exchange_name != 'hyperliquid'",
        )
        .fetch_all(&pg_pool)
        .await
        .unwrap_or_default();

        for (account_id, user_id, exchange_name) in accounts {
            let notify = Arc::new(tokio::sync::Notify::new());
            journal_syncer_notifiers.insert(account_id, notify.clone());

            let source = Arc::new(services::journal_syncer::ccxt::CcxtFillSource::new(
                cex_client.clone(),
                exchange_account_repo.clone(),
                sandbox,
                exchange_name.clone(),
            ));

            let syncer = services::journal_syncer::syncer::JournalSyncerBuilder {
                user_id,
                account_id,
                exchange_label: exchange_name,
                interval_secs,
                source,
                pool: pg_pool.clone(),
                exchange_account_repo: exchange_account_repo.clone(),
                journal_service: journal_service_for_sync.clone(),
                notify,
                event_tx: Some(mgmt_event_tx.clone()),
                cex_client: Some(cex_client.clone()),
            }
            .build();

            let token = shutdown.clone();
            tokio::spawn(async move { syncer.run(token).await });
        }

        tracing::info!(
            accounts = journal_syncer_notifiers.len(),
            interval_secs,
            "JournalSyncer spawned for CCXT accounts"
        );
    } else if !syncer_enabled {
        tracing::warn!("JOURNAL_SYNCER_ENABLED=false — journal pull-sync disabled");
    }

    // JNL-SYNC-01 FR-6: Spawn JournalSyncer per active Hyperliquid account.
    if syncer_enabled && hl_enabled {
        let interval_secs: u64 = std::env::var("JOURNAL_SYNC_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let journal_service_for_hl_sync =
            Arc::new(services::journal_service::JournalService::new(pg_pool.clone()));

        let hl_accounts: Vec<(uuid::Uuid, uuid::Uuid)> = sqlx::query_as(
            "SELECT id, user_id FROM exchange_accounts \
             WHERE is_active = TRUE AND exchange_name = 'hyperliquid'",
        )
        .fetch_all(&pg_pool)
        .await
        .unwrap_or_default();

        let hl_account_count = hl_accounts.len();
        for (account_id, user_id) in hl_accounts {
            let notify = Arc::new(tokio::sync::Notify::new());
            journal_syncer_notifiers.insert(account_id, notify.clone());

            let source = Arc::new(services::journal_syncer::hyperliquid::HyperliquidFillSource::new(
                hl_network,
                exchange_account_repo.clone(),
            ));

            let syncer = services::journal_syncer::syncer::JournalSyncerBuilder {
                user_id,
                account_id,
                exchange_label: "hyperliquid".to_string(),
                interval_secs,
                source,
                pool: pg_pool.clone(),
                exchange_account_repo: exchange_account_repo.clone(),
                journal_service: journal_service_for_hl_sync.clone(),
                notify,
                event_tx: Some(mgmt_event_tx.clone()),
                cex_client: None,
            }
            .build();

            let token = shutdown.clone();
            tokio::spawn(async move { syncer.run(token).await });
        }

        if hl_account_count > 0 {
            tracing::info!(
                accounts = hl_account_count,
                interval_secs,
                "JournalSyncer spawned for Hyperliquid accounts"
            );
        }
    }

    // AUD-03 FR-11: Log startup completion
    tracing::info!("All background tasks spawned, starting HTTP server");

    // AUD-04 FR-1/FR-2: CORS restricted to known origins + extension patterns
    let allowed_origins = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "https://testudo.app".to_string());

    let server = HttpServer::new(move || {
        let origins = allowed_origins.clone();
        let cors = Cors::default()
            .allowed_origin_fn(move |origin, _req_head| {
                let origin_str = origin.to_str().unwrap_or("");
                is_origin_allowed(origin_str, &origins)
            })
            .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                header::HeaderName::from_static("x-user-id"),
                header::HeaderName::from_static("x-execution-mode"),
                header::HeaderName::from_static("idempotency-key"),
                header::HeaderName::from_static("x-request-id"),
            ])
            .expose_headers(vec![header::HeaderName::from_static("x-request-id")])
            .supports_credentials() // AUTH-02 FR-18: Allow HttpOnly cookie auth
            .max_age(3600);

        App::new()
            .wrap(cors)
            // AUD-05 FR-3: X-Request-Id correlation
            .wrap(middleware::RequestIdMiddleware)
            // AUD-05 FR-2: Automatic request/response tracing spans
            .wrap(tracing_actix_web::TracingLogger::default())
            .service(
                scope("/api/v1")
                    .app_data(app_state.clone())
                    .app_data(market_data_state.clone())
                    .app_data(sidecar_health_data.clone())
                    .app_data(order_update_sender.clone())
                    .service(
                        web::scope("/health")
                            .route("", web::get().to(HttpResponse::Ok)) // GET /health (liveness)
                            .route("/ready", web::get().to(health_ready)) // GET /health/ready (AUD-06 FR-2: readiness)
                            .route("/sidecar", web::get().to(get_sidecar_health)), // GET /health/sidecar (EXT-16 FR-1.5)
                    )
                    // AUD-05 FR-4: Prometheus metrics endpoint
                    .route("/metrics", web::get().to(prometheus_metrics))
                    // JNL-11: JSON-LD context document
                    .route("/context.jsonld", web::get().to(routes::context::get_context))
                    // AUTH-02: Auth dependencies
                    .app_data(nonce_store.clone())
                    .app_data(pairing_store.clone())
                    .app_data(pair_rate_limiter.clone())
                    .app_data(pub_profile_rate_limiter.clone())
                    .app_data(session_repo.clone())
                    .app_data(user_repo.clone())
                    // AUTH-02: Public auth routes (no JWT required)
                    .service(
                        web::scope("/auth")
                            .route("/nonce", web::get().to(auth::get_nonce))
                            .route("/verify-siwe", web::post().to(auth::verify_siwe))
                            .route("/verify-siws", web::post().to(auth::verify_siws))
                            .route("/refresh", web::post().to(auth::refresh))
                            .route("/extension-pair", web::post().to(auth::extension_pair))
                            .route("/extension-refresh", web::post().to(auth::extension_refresh))
                            // Authenticated auth routes (JWT required)
                            .service(
                                web::scope("")
                                    .wrap(JwtMiddleware::new(token_service.clone()))
                                    .route("/logout", web::post().to(auth::logout))
                                    .route("/revoke-all", web::post().to(auth::revoke_all))
                                    .route("/me", web::get().to(auth::me))
                                    .route("/pair-extension", web::post().to(auth::pair_extension))
                                    .route("/pair-status", web::get().to(auth::pair_status)),
                            ),
                    )
                    .service(web::scope("/depth").route("", web::get().to(depth::get_depth))) // GET /depth?symbol=SOL_USDC
                    .service(
                        web::scope("/trade-history").route("", web::get().to(trade::get_trades)),
                    ) // GET /trade-history?symbol=SOL_USDC (executed trades)
                    .service(web::scope("/klines").route("", web::get().to(klines::get_klines))) // GET /klines?symbol=SOL_USDC&interval=1m&startTime=1727022600
                    .service(web::scope("/tickers").route("", web::get().to(tickers::get_tickers))) // GET /tickers
                    // Market data routes - live data from Binance
                    .service(
                        web::scope("/market-data")
                            .route("/ticker", web::get().to(market_data::get_ticker)) // GET /market-data/ticker?symbol=BTC_USDC
                            .route("/orderbook", web::get().to(market_data::get_orderbook)) // GET /market-data/orderbook?symbol=BTC_USDC&limit=20
                            .route("/klines", web::get().to(market_data::get_klines)) // GET /market-data/klines?symbol=BTC_USDC&interval=1h&limit=100
                            .route("/markets", web::get().to(market_data::get_markets)), // GET /market-data/markets
                    )
                    // Position sync routes - E.4
                    .service(
                        web::scope("/sync")
                            .route("", web::post().to(sync::trigger_sync)) // POST /sync - Trigger manual sync
                            .route("/status", web::get().to(sync::get_sync_status)) // GET /sync/status - Get last sync result
                            .route("/diff", web::get().to(sync::get_sync_diff)), // GET /sync/diff - Get position differences
                    )
                    .service(
                        web::scope("/order")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("", web::get().to(order::get_open_order)) // GET /order
                            .route("", web::post().to(order::execute_order)) // POST /order
                            .route("", web::delete().to(order::cancel_order)), // DELETE /order
                    )
                    .service(
                        web::scope("/orders")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("", web::get().to(order::get_open_orders)) // GET /orders
                            .route("", web::delete().to(order::cancel_all_orders)), // DELETE /orders
                    )
                    .service({
                        let agent_wallet_enabled = std::env::var("HYPERLIQUID_AGENT_WALLET_ENABLED")
                            .unwrap_or_default() == "true";

                        let mut exchanges_scope = web::scope("/exchanges")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("", web::get().to(exchanges::list_exchanges)) // GET /exchanges
                            .route(
                                "/supported",
                                web::get().to(exchanges::list_supported_exchanges),
                            ); // GET /exchanges/supported (FR-5.3)

                        if agent_wallet_enabled {
                            exchanges_scope = exchanges_scope.service(
                                web::scope("/agent-wallet")
                                    .route("/init", web::post().to(exchanges::init_agent_wallet))
                                    .route("/approve-data", web::post().to(exchanges::approve_data))
                                    .route("/approve", web::post().to(exchanges::approve_agent))
                                    .route("/migrate", web::post().to(exchanges::migrate_to_agent_wallet))
                                    .route("/{id}/revoke", web::delete().to(exchanges::revoke_agent)),
                            ); // POST /exchanges/agent-wallet/* (AW-01, AW-02, AW-05)
                            tracing::info!("Agent wallet routes enabled (HYPERLIQUID_AGENT_WALLET_ENABLED=true)");
                        }

                        exchanges_scope.service(
                                web::scope("/accounts")
                                    .route("", web::get().to(exchanges::get_user_exchange_accounts)) // GET /exchanges/accounts
                                    .route("", web::post().to(exchanges::add_exchange_account)) // POST /exchanges/accounts
                                    .route(
                                        "/{id}",
                                        web::delete().to(exchanges::delete_exchange_account),
                                    ) // DELETE /exchanges/accounts/{id}
                                    .route(
                                        "/{id}/test",
                                        web::post().to(exchanges::test_exchange_connection),
                                    ) // POST /exchanges/accounts/{id}/test
                                    .route(
                                        "/{id}/balance",
                                        web::get().to(exchanges::get_exchange_balance),
                                    ) // GET /exchanges/accounts/{id}/balance (EXT-17)
                                    .route(
                                        "/{id}/positions",
                                        web::get().to(exchanges::get_exchange_positions),
                                    ) // GET /exchanges/accounts/{id}/positions
                                    .route(
                                        "/{id}/close-position",
                                        web::post().to(exchanges::close_exchange_position),
                                    ), // POST /exchanges/accounts/{id}/close-position (EXT-34)
                            )
                    },
                    )
                    .service(
                        web::scope("/risk-config")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("", web::get().to(risk_config::get_risk_config)) // GET /risk-config
                            .route("", web::put().to(risk_config::update_risk_config)), // PUT /risk-config
                    )
                    .service(
                        // QNT-01a: User settings (Dynamic Risk toggle + unlock gate)
                        web::scope("/user")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("/settings", web::get().to(user_settings::get_user_settings))
                            .route("/settings", web::patch().to(user_settings::patch_user_settings)),
                    )
                    .service(
                        // RSK-01: Unified risk snapshot aggregating across all venues
                        web::scope("/risk")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("/snapshot", web::get().to(risk::get_snapshot)), // GET /risk/snapshot
                    )
                    .service(
                        // RSK-03: AI trade coach — weekly report + banner state
                        web::scope("/coach")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("/latest", web::get().to(coach::get_latest)) // GET /coach/latest
                            .route("/archive", web::get().to(coach::get_archive)) // GET /coach/archive
                            .route("/preference", web::get().to(coach::get_preference)) // GET /coach/preference
                            .route("/preference", web::patch().to(coach::update_preference)) // PATCH /coach/preference
                            .route("/mark-viewed", web::post().to(coach::mark_viewed)) // POST /coach/mark-viewed
                            .route(
                                "/{report_id}/dismiss-banner",
                                web::patch().to(coach::dismiss_banner),
                            ), // PATCH /coach/{id}/dismiss-banner
                    )
                    .service(
                        // ENG-01a/ENG-01b: Dignitas score + public identity
                        web::scope("/dignitas")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("/me", web::get().to(dignitas::get_me))
                            .route("/history", web::get().to(dignitas::get_history))
                            .route("/preferences", web::patch().to(dignitas::patch_preferences))
                            .route("/handle", web::post().to(dignitas::post_handle))
                            .route("/handle", web::patch().to(dignitas::patch_handle))
                            .route("/handle", web::delete().to(dignitas::delete_handle))
                            .route("/visibility", web::patch().to(dignitas::patch_visibility))
                            .route("/identity", web::get().to(dignitas::get_identity)),
                    )
                    .service(
                        // ENG-01b: Public profile — no JWT required, per-IP rate limited
                        web::scope("/public").service(
                            web::scope("/profile")
                                .route("/{handle}", web::get().to(public_profile::get_profile)),
                        ),
                    )
                    .service(
                        web::scope("/journal")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            // AGENT-03: Agent journal memory — nested scope, no ordering conflict
                            .service(
                                web::scope("/agent")
                                    .route("/summary", web::get().to(agent_journal::get_summary))
                                    .route("/insights", web::get().to(agent_journal::get_insights))
                                    .route("/compare", web::post().to(agent_journal::post_compare)),
                            )
                            .route("/sync", web::post().to(journal::trigger_manual_sync))
                            .route("/analytics/filter-options", web::get().to(journal::filter_options))
                            .route("/analytics/overview", web::get().to(journal::overview))
                            .route("/analytics/equity-curve", web::get().to(journal::equity_curve))
                            .route("/analytics/daily-pnl", web::get().to(journal::daily_pnl))
                            .route("/analytics/symbol-breakdown", web::get().to(journal::symbol_breakdown))
                            .route("/analytics/setup-breakdown", web::get().to(journal::setup_breakdown))
                            .route("/analytics/duration-profit", web::get().to(journal::duration_profit))
                            .route("/analytics/return-distribution", web::get().to(journal::return_distribution))
                            .route("/analytics/time-distribution", web::get().to(journal::time_distribution))
                            .route("/analytics/batch", web::post().to(journal::analytics_batch))
                            .route("/trades", web::get().to(journal::list_trades))
                            .route("/trades/{id}", web::get().to(journal::get_trade))
                            .route("/trades/{id}/notes", web::patch().to(journal::update_trade_notes))
                            .route("/trades/{id}/tags", web::post().to(journal::add_trade_tags))
                            .route("/trades/{id}/tags/{tag_id}", web::delete().to(journal::remove_trade_tag))
                            .route("/drafts/{id}", web::get().to(journal::get_draft_notes))
                            .route("/drafts/{id}/notes", web::patch().to(journal::save_draft_notes))
                            .route("/entries", web::get().to(journal::list_entries))
                            .route("/entries", web::post().to(journal::create_entry))
                            .route("/entries/{id}", web::get().to(journal::get_entry))
                            .route("/entries/{id}", web::put().to(journal::update_entry))
                            .route("/entries/{id}", web::delete().to(journal::delete_entry))
                            .route("/tags", web::get().to(journal::list_tags))
                            .route("/tags", web::post().to(journal::create_tag))
                            .route("/tags/{id}", web::put().to(journal::update_tag))
                            .route("/tags/{id}", web::delete().to(journal::delete_tag))
                            .route("/setup-tags", web::get().to(journal::list_setup_tags))
                            .route("/upload", web::post().to(journal::upload_journal_image))
                            .route("/storage", web::get().to(journal::storage_usage))
                            .route("/images/{id}", web::delete().to(journal::delete_image)),
                    )
                    // HIST-01: Trade history import routes
                    .service(
                        web::scope("/trades/import")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("", web::post().to(imports::start_import)) // POST /trades/import
                            .route("/status", web::get().to(imports::import_status)), // GET /trades/import/status
                    )
                    // Trade management routes — JWT required (SEC-01)
                    .service(
                        web::scope("/trades")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .app_data(trade_state.clone())
                            .route("", web::post().to(trade_management::create_trade)) // POST /trades - Create trade with SL/TP
                            .route("", web::get().to(trade_management::list_trades)) // GET /trades - List active trades
                            .route(
                                "/preview",
                                web::post().to(trade_management::preview_trade_sizing),
                            ) // POST /trades/preview (QNT-01b)
                            .route("/{id}", web::get().to(trade_management::get_trade)) // GET /trades/{id}
                            .route(
                                "/{id}/sl",
                                web::put().to(trade_management::update_stop_loss),
                            ) // PUT /trades/{id}/sl
                            .route(
                                "/{id}/tp",
                                web::put().to(trade_management::update_take_profit),
                            ) // PUT /trades/{id}/tp
                            .route(
                                "/{id}/entry",
                                web::put().to(trade_management::update_entry_price),
                            ) // PUT /trades/{id}/entry (pending only)
                            .route(
                                "/{id}/breakeven",
                                web::put().to(trade_management::enable_break_even),
                            ) // PUT /trades/{id}/breakeven
                            .route(
                                "/{id}/management",
                                web::get().to(trade_management::get_trade_management),
                            ) // GET /trades/{id}/management (EXT-09)
                            .route(
                                "/{id}/events",
                                web::get().to(trade_events::get_trade_events),
                            ) // GET /trades/{id}/events (019f)
                            .route("/{id}", web::delete().to(trade_management::cancel_trade)) // DELETE /trades/{id}
                            .route("/cleanup", web::post().to(trade_management::cleanup_stale_trades)), // POST /trades/cleanup (HL-09)
                    )
                    // Agent API keys — scoped credentials for autonomous agents (AGENT-07)
                    .service(
                        web::scope("/agent-keys")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("", web::post().to(agent_keys::create_key))
                            .route("", web::get().to(agent_keys::list_keys))
                            .route("/{key_id}", web::delete().to(agent_keys::revoke_key))
                            .route("/{key_id}", web::patch().to(agent_keys::update_key)),
                    )
                    // Onboarding status — single-call agent readiness check (AGENT-06)
                    .service(
                        web::scope("/onboarding")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("/status", web::get().to(onboarding::get_status)),
                    )
                    // Agent signal endpoint — programmatic trade execution
                    .service(
                        web::scope("/signals")
                            .wrap(JwtMiddleware::new(token_service.clone()))
                            .route("", web::post().to(signal::create_signal)),
                    )
                    .service(
                        web::scope("/paper")
                            .app_data(trade_state.clone())
                            .route(
                                "/balances",
                                web::get().to(paper_balance::get_paper_balances),
                            ) // GET /paper/balances
                            .route("/reset", web::post().to(paper_balance::reset_paper_balance)), // POST /paper/reset
                    ),
            )
            // API v2 - Columnar wire format for efficiency
            .service(
                scope("/api/v2")
                    .app_data(market_data_state.clone())
                    .service(
                        web::scope("/market-data").route(
                            "/orderbook",
                            web::get().to(market_data::get_orderbook_columnar),
                        ), // GET /v2/market-data/orderbook - Columnar format (~25% smaller)
                    ),
            )
            // JNL-SYNC-01 CP-6: /internal/reconcile-pending-fills removed
            // Serve uploaded journal images (authenticated)
            .service(
                web::scope("/uploads/journal")
                    .wrap(JwtMiddleware::new(token_service.clone()))
                    .service(actix_files::Files::new("", "./uploads/journal")),
            )
    })
    .bind(config.server_addr.clone())?
    .run();
    tracing::info!("Server running at http://{}/", config.server_addr);

    // Wait for either server completion or shutdown signal
    tokio::select! {
        result = server => {
            tracing::info!("HTTP server stopped");
            result
        }
        _ = shutdown.cancelled() => {
            tracing::info!("Shutdown complete — all background tasks stopped");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CORS rejects requests from unknown origins
    #[test]
    fn test_cors_rejects_unknown_origin() {
        let allowed = "https://testudo.app,https://staging.testudo.app";
        assert!(!is_origin_allowed("https://evil.com", allowed));
        assert!(!is_origin_allowed("https://testudo.app.evil.com", allowed));
        assert!(!is_origin_allowed("http://localhost:3000", allowed));
        assert!(!is_origin_allowed("", allowed));
    }

    /// CORS allows configured web domains
    #[test]
    fn test_cors_allows_configured_origins() {
        let allowed = "https://testudo.app,https://staging.testudo.app";
        assert!(is_origin_allowed("https://testudo.app", allowed));
        assert!(is_origin_allowed("https://staging.testudo.app", allowed));
    }

    /// SEC-02: Chrome extension allowed when no pinning configured (dev mode)
    #[test]
    fn test_cors_allows_chrome_extension_unpinned() {
        // Ensure env var is unset for this test
        std::env::remove_var("ALLOWED_EXTENSION_ORIGINS");
        let allowed = "https://testudo.app";
        assert!(is_origin_allowed(
            "chrome-extension://abcdefghijklmnop",
            allowed
        ));
    }

    /// SEC-02: Chrome extension pinned — only allowlisted ID passes
    #[test]
    fn test_cors_chrome_extension_pinned() {
        std::env::set_var(
            "ALLOWED_EXTENSION_ORIGINS",
            "chrome-extension://testudo1234567890",
        );
        let allowed = "https://testudo.app";
        // Allowlisted ID passes
        assert!(is_origin_allowed(
            "chrome-extension://testudo1234567890",
            allowed
        ));
        // Non-allowlisted ID rejected
        assert!(!is_origin_allowed(
            "chrome-extension://malicious9999999",
            allowed
        ));
        // Cleanup
        std::env::remove_var("ALLOWED_EXTENSION_ORIGINS");
    }

    /// SEC-02: Firefox extension uses prefix check (per-install UUID, not pinnable)
    #[test]
    fn test_cors_allows_firefox_extension() {
        let allowed = "https://testudo.app";
        assert!(is_origin_allowed(
            "moz-extension://abcdef-1234-5678",
            allowed
        ));
        // Any Firefox extension UUID passes (by design — UUIDs are per-install)
        assert!(is_origin_allowed(
            "moz-extension://ffffffff-0000-1111",
            allowed
        ));
    }

    /// FR-8: Startup fails without CREDENTIAL_ENCRYPTION_KEY
    /// This test verifies that AesGcmVault::from_env() returns Err when env var is missing.
    /// FIX-07: Removed unsafe env::remove_var — test env doesn't set the var.
    #[test]
    fn test_encryption_key_required() {
        use repositories::exchange_account::AesGcmVault;
        let result = AesGcmVault::from_env();
        assert!(
            result.is_err(),
            "AesGcmVault::from_env() should fail without CREDENTIAL_ENCRYPTION_KEY"
        );
    }
}
