//! Wire contract for the coach pipeline.
//!
//! All types are defined here so T3–T6 can implement behavior without
//! churning the type surface. Serialization matches the FR-exposed JSON
//! in `/api/v1/coach/*` responses (decimals as strings, uuids/dates as
//! ISO strings via serde defaults on `rust_decimal::Decimal` + `chrono`).

// @anchor exchange:router:types
// @tags api

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Enum of deterministic behavioral patterns detected in the weekly digest.
///
/// Serialized snake_case so the LLM prompt schema matches the Rust enum
/// (e.g. `"pattern": "sizing_drift"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternKind {
    SizingDrift,
    FrequencySpike,
    SessionAnomaly,
    SetupFatigue,
    CorrelationStack,
    StreakRisk,
}

/// Severity triage for detected patterns. Only `Notable`+ appear in the banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Notable,
    Concerning,
}

/// 30-day rolling user baseline against which the week is compared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBaseline {
    pub avg_trades_per_day: Decimal,
    pub avg_position_size_usd: Decimal,
    /// Top-4 UTC hours by trade count over the baseline window.
    pub typical_session_hours_utc: Vec<u8>,
    pub win_rate: Decimal,
    pub avg_r_multiple: Decimal,
    /// 90th-percentile trade count in any rolling 6h window over the baseline.
    pub p90_trades_per_6h: Decimal,
    /// Keyed by lowercased setup tag (or "(untagged)").
    pub setup_baselines: HashMap<String, SetupBaseline>,
}

/// Per-setup baseline slice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupBaseline {
    pub trade_count: i64,
    pub avg_r_multiple: Decimal,
    pub win_rate: Decimal,
}

/// Aggregate stats for the analyzed week.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekStats {
    pub trade_count: i64,
    pub win_rate: Decimal,
    pub total_pnl: Decimal,
    pub total_r: Decimal,
    /// 24-slot histogram of trades opened per UTC hour (0..=23).
    pub trades_by_hour_utc: [i64; 24],
    /// Keyed by lowercased setup tag (or "(untagged)").
    pub by_setup: HashMap<String, SetupBaseline>,
}

/// One trade's evidence slice — only trades referenced by a flag are serialized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEvidence {
    pub id: Uuid,
    /// First 8 hex chars of `id`, surfaced to the LLM as `[T-xxxxxxxx]` citation tokens.
    pub short_id: String,
    pub symbol: String,
    pub side: String,
    pub opened_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    pub pnl: Decimal,
    pub r_multiple: Option<Decimal>,
    pub setup_tag: Option<String>,
    pub position_size_usd: Decimal,
}

/// One detected pattern with its evidence + pattern-specific metrics JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlaggedPattern {
    pub pattern: PatternKind,
    pub severity: Severity,
    /// Trade IDs — every id MUST be present in the digest's `flagged_trades`.
    pub evidence: Vec<Uuid>,
    /// Free-form per-pattern numbers (e.g. `{"size_multiplier": "2.1"}`).
    pub metrics: serde_json::Value,
}

/// The full compact input handed to the narrator LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachDigest {
    pub user_id: Uuid,
    pub week_start: DateTime<Utc>,
    pub week_end: DateTime<Utc>,
    pub baseline: UserBaseline,
    pub week_stats: WeekStats,
    pub flagged_patterns: Vec<FlaggedPattern>,
    /// Only trades referenced by at least one flagged pattern.
    pub flagged_trades: Vec<TradeEvidence>,
}

/// Structured narrative returned by the LLM (pre-persistence).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarratedReport {
    /// The "top insight" surfaced in the Account banner.
    pub headline: String,
    pub sections: Vec<NarrativeSection>,
    pub model_used: String,
    /// 0..1 — percentage of input tokens served from provider cache. `None`
    /// when the provider does not expose cache metadata.
    pub cache_hit_ratio: Option<Decimal>,
    pub generated_at: DateTime<Utc>,
}

/// One narrative chunk keyed to a pattern. `body` is markdown with `[T-xxx]` tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeSection {
    pub pattern: PatternKind,
    pub body: String,
    /// Full UUIDs of cited trades — MUST be a subset of the digest's `flagged_trades`.
    pub citations: Vec<Uuid>,
}

/// What lands in `coach_reports` + what `GET /latest` / `GET /archive` return.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCoachReport {
    pub id: Uuid,
    pub user_id: Uuid,
    pub week_start: DateTime<Utc>,
    pub week_end: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    /// Provider model name, or `"unavailable"` when the stats-only fallback fired.
    pub model_used: String,
    pub headline: Option<String>,
    /// `None` when the narrator or validator failed — frontend renders
    /// "● coach unavailable this week" in that slot.
    pub narrative_sections: Option<Vec<NarrativeSection>>,
    pub digest: CoachDigest,
    pub cache_hit_ratio: Option<Decimal>,
    pub banner_dismissed_at: Option<DateTime<Utc>>,
}

/// Scheduler + skip-rule configuration pulled from env at startup.
#[derive(Debug, Clone)]
pub struct CoachConfig {
    pub min_lifetime_trades: i64,
    pub min_week_trades: i64,
    /// Global kill-switch (`COACH_ENABLED=false` stops the scheduler entirely).
    pub enabled_global: bool,
}

/// Failure modes for the Narrator trait.
#[derive(Debug, thiserror::Error)]
pub enum NarratorError {
    #[error("narrator request timed out")]
    Timeout,
    #[error("narrator rate limited")]
    RateLimit,
    #[error("narrator response failed to parse: {0}")]
    Parse(String),
    #[error("narrator provider error: {0}")]
    Provider(String),
}

/// Citation-grounding failures. The scheduler converts these into a
/// stats-only fallback row.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("section {section_index}: citation {trade_id} is not present in digest.flagged_trades")]
    UnknownCitation {
        section_index: usize,
        trade_id: Uuid,
    },
    #[error("unknown citation token [T-{token}] in {location}")]
    UnknownToken {
        /// `Some` for section bodies, `None` for the headline.
        section_index: Option<usize>,
        token: String,
        location: String,
    },
}
