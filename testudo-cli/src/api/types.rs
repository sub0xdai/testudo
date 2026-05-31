// @anchor infra:cli:api:types
// @tags api

//! Client-side types mirroring the Testudo backend API JSON shapes.
//!
//! These types are deserialized from backend responses. They are deliberately
//! minimal — only the fields the CLI actually needs. Financial values use
//! `rust_decimal::Decimal` for precision.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Error type ────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Unauthorized — check your agent key in ~/.config/testudo/config.toml")]
    Unauthorized,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Failed to parse response: {0}")]
    Deserialize(String),

    #[error("Unexpected HTTP {0}: {1}")]
    UnexpectedStatus(u16, String),

    #[error("Signal rejected: {0}")]
    SignalRejected(String),
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            ApiError::Network("Request timed out".into())
        } else if e.is_connect() {
            ApiError::Network(format!(
                "Connection refused — is the Testudo backend running? ({})",
                e
            ))
        } else {
            ApiError::Network(e.to_string())
        }
    }
}

// ── Signal types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SignalSide {
    Long,
    Short,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecutionMode {
    Shadow,
    Live,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SignalInput {
    pub symbol: String,
    pub side: SignalSide,
    pub entry_price: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<Decimal>,
    pub take_profit: Vec<TakeProfitTarget>,
    pub execution_mode: ExecutionMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leverage: Option<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TakeProfitTarget {
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct SignalResult {
    pub success: bool,
    #[serde(default)]
    pub trade_group_id: Option<Uuid>,
    #[serde(default)]
    pub entry_order_id: Option<String>,
    #[serde(default)]
    pub position_size: Option<Decimal>,
    pub execution_mode: ExecutionMode,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub agent_key_id: Option<Uuid>,
    #[serde(default)]
    pub rejection: Option<SignalRejection>,
}

#[derive(Debug, Deserialize)]
pub struct SignalRejection {
    pub reason: String,
}

// ── Journal types ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AgentSummary {
    pub timeframe: TimeframeInfo,
    pub overall: OverallStats,
    #[serde(default)]
    pub by_setup: Vec<serde_json::Value>,
    #[serde(default)]
    pub top_trades: Vec<serde_json::Value>,
    #[serde(default)]
    pub equity: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct TimeframeInfo {
    pub label: String,
    #[serde(default)]
    pub from: Option<NaiveDate>,
    #[serde(default)]
    pub to: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct OverallStats {
    pub trade_count: i64,
    pub win_rate: Decimal,
    pub avg_r_multiple: Decimal,
    pub total_pnl: Decimal,
    pub max_drawdown: Decimal,
    pub profit_factor: Decimal,
    #[serde(default)]
    pub sharpe_ratio: Option<Decimal>,
    #[serde(default)]
    pub avg_hold_hours: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct AgentInsight {
    pub pattern: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct CompareRequest {
    pub period_a: String,
    pub period_b: String,
}

#[derive(Debug, Deserialize)]
pub struct CompareResult {
    #[serde(default)]
    pub changes: Vec<serde_json::Value>,
}

// ── Kline types ───────────────────────────────────────────────────────

/// Client-side kline type. The backend returns `common_utils::Candle` for the
/// `/klines` endpoint. We define our own for decoupled deserialization.
#[derive(Debug, Deserialize)]
pub struct KlineData {
    pub timestamp: i64,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    #[serde(default)]
    pub quote_volume: Decimal,
}

// ── Onboarding types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct OnboardingStatus {
    pub is_ready: bool,
    pub next_step: String,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub has_trades: bool,
}

// ── Risk config types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct RiskConfigData {
    pub account_risk_percent: Decimal,
    #[serde(default)]
    pub max_risk_amount: Option<Decimal>,
    #[serde(default)]
    pub max_position_size: Option<Decimal>,
    pub max_leverage: u8,
    #[serde(default)]
    pub daily_max_drawdown_percent: Option<Decimal>,
    #[serde(default)]
    pub max_open_positions: Option<u32>,
    pub require_stop_loss: bool,
    #[serde(default)]
    pub default_stop_atr_multiplier: Option<Decimal>,
    #[serde(default)]
    pub min_risk_reward_ratio: Option<Decimal>,
}
