// @anchor exchange:router:app
// @tags api

use crate::config::RouterConfig;
use crate::repositories::exchange_account::ExchangeAccountRepository;
use crate::services::calibration::CalibrationEngine;
use crate::services::coach::CoachService;
use crate::services::hyperliquid::auth::AuthCache;
use crate::services::hyperliquid::universe::AssetUniverse;
use crate::services::{CexClient, ExecutionService};
use common_utils::adapters::CredentialValidator;
use common_utils::auth::TokenService;
use dashmap::DashMap;
use engine::EngineHandle;
use hyperliquid_sdk_rs::Network;
use pg_queue::PgQueueManager;
use sqlx_postgres::PostgresDb;
use std::sync::Arc;
use tokio::sync::Notify;
use uuid::Uuid;

pub struct AppState {
    /// PostgreSQL database connection
    pub postgres_db: PostgresDb,
    /// PostgreSQL connection pool for direct queries
    pub pool: sqlx::Pool<sqlx::Postgres>,
    /// PostgreSQL queue/cache/pubsub manager (replaces Redis)
    pub pg_queue: PgQueueManager,
    pub token_service: Arc<dyn TokenService>,
    pub config: RouterConfig,
    pub credential_validator: CredentialValidator,
    pub execution_service: Arc<ExecutionService>,
    /// 019e: Actor handle — sole engine access path
    pub engine_handle: EngineHandle,
    /// Exchange account repository for credential management
    pub exchange_account_repo: ExchangeAccountRepository,
    /// CEX sidecar client for live multi-exchange trading
    pub cex_client: Arc<CexClient>,
    /// Hyperliquid auth cache for signer invalidation (AW-05)
    pub hl_auth_cache: Option<Arc<AuthCache>>,
    /// Hyperliquid asset universe for symbol → index resolution
    pub hl_universe: Option<Arc<AssetUniverse>>,
    /// FIX-07: Shared HTTP client for Hyperliquid API calls (10s timeout, connection pooling)
    pub hl_http_client: reqwest::Client,
    /// FIX-07: Hyperliquid network resolved once at startup
    pub hl_network: Network,
    /// JNL-12 FR-7: Dedicated pool for journal analytics queries
    pub analytics_pool: sqlx::Pool<sqlx::Postgres>,
    /// RSK-03: Weekly AI trade coach orchestrator.
    pub coach_service: Arc<CoachService>,
    /// Whether the CEX sidecar is in sandbox mode (testnet).
    pub cex_sandbox: bool,
    /// QNT-01a: Calibration engine — loads per-setup + global priors for the
    /// Calibrated Kelly sizing path. Consumed by `create_trade` when the
    /// user has `dynamic_risk_enabled = true` on their `user_settings` blob.
    pub calibration_engine: Arc<CalibrationEngine>,
    /// JNL-SYNC-01: Per-account notify handles for manual "Sync now" triggering.
    /// Key = exchange_account_id. The route POSTs to this to wake the syncer.
    pub journal_syncer_notifiers: Arc<DashMap<Uuid, Arc<Notify>>>,
    /// JNL-SYNC-01: Debounce tracker for manual sync (5s minimum between triggers).
    pub journal_syncer_last_notified: Arc<DashMap<Uuid, std::time::Instant>>,
    /// AGENT-01 CP-4: Idempotency store for signal events.
    /// Key = idempotency_key, value = cached SignalResult JSON.
    pub signal_idempotency: Arc<DashMap<String, serde_json::Value>>,
    /// AGENT-01: Per-user rate limiter for POST /api/v1/signals.
    /// Key = user_id, value = Vec of timestamps.
    pub signal_rate_limiter: Arc<DashMap<Uuid, Vec<std::time::Instant>>>,
    /// AGENT-01: Max requests per window for signal rate limiter.
    pub signal_rate_limit_max: usize,
    /// AGENT-01: Time window for signal rate limiter.
    pub signal_rate_limit_window: std::time::Duration,
}
