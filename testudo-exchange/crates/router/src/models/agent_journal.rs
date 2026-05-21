//! AGENT-03: Agent journal models — wire types for the three agent memory endpoints.
//!
//! Types are designed to be serialized as JSON (for programmatic consumption) and
//! formatted as LLM-optimized markdown (for direct context-window injection).
//! All financial values use `rust_decimal::Decimal`.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Query types ───────────────────────────────────────────────────────

/// Query parameters for GET /journal/agent/summary.
#[derive(Debug, Deserialize)]
pub struct AgentSummaryQuery {
    pub timeframe: Option<String>,        // "7d", "30d", "90d", "all" (default: "90d")
    pub symbol: Option<String>,           // "ETH_USDT"
    pub side: Option<String>,             // "LONG", "SHORT"
    pub setup_tag: Option<String>,        // "breakout"
    pub exchange: Option<String>,         // "hyperliquid", "binance"
    pub source: Option<String>,           // "agent:hermes_v1.2"
    #[serde(default)]
    pub format: SummaryFormat,            // "json" (default) or "llm"
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SummaryFormat {
    #[default]
    Json,
    Llm,
}

// ── Summary response ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AgentSummary {
    pub timeframe: TimeframeInfo,
    pub overall: OverallStats,
    pub by_setup: Vec<SetupBreakdown>,
    pub top_trades: Vec<TradeCitation>,
    pub equity: Vec<EquityPoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeframeInfo {
    pub label: String,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallStats {
    pub trade_count: i64,
    pub win_rate: Decimal,
    pub avg_r_multiple: Decimal,
    pub total_pnl: Decimal,
    pub max_drawdown: Decimal,
    pub profit_factor: Decimal,
    pub sharpe_ratio: Option<Decimal>,
    pub avg_hold_hours: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupBreakdown {
    pub setup: String,
    pub trade_count: i64,
    pub win_rate: Decimal,
    pub avg_r_multiple: Decimal,
    pub total_pnl: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeCitation {
    pub id: Uuid,
    pub short_id: String,
    pub symbol: String,
    pub side: String,
    pub opened_at: DateTime<Utc>,
    pub pnl: Decimal,
    pub r_multiple: Option<Decimal>,
    pub setup_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EquityPoint {
    pub date: NaiveDate,
    pub cumulative_pnl: Decimal,
    pub equity: Option<Decimal>,
}

// ── Insight types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AgentInsight {
    pub pattern: PatternKind,
    pub severity: Severity,
    pub headline: String,
    pub detail: String,
    pub recommendation: Option<String>,
    pub evidence_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternKind {
    SizingDrift,
    FrequencySpike,
    SessionAnomaly,
    SetupFatigue,
    CorrelationStack,
    StreakRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Notable,
    Concerning,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginatedInsights {
    pub insights: Vec<AgentInsight>,
    pub total: i64,
}

// ── Comparison types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CompareRequest {
    pub period_a: TimeframeRange,
    pub period_b: TimeframeRange,
    #[serde(default)]
    pub filters: Option<CompareFilters>,
}

#[derive(Debug, Deserialize)]
pub struct TimeframeRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct CompareFilters {
    pub symbol: Option<String>,
    pub side: Option<String>,
    pub setup_tag: Option<String>,
    pub exchange: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonResult {
    pub period_a: PeriodInfo,
    pub period_b: PeriodInfo,
    pub deltas: Vec<MetricDelta>,
    pub by_setup_deltas: Vec<SetupDelta>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodInfo {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub trade_count: i64,
    pub win_rate: Decimal,
    pub avg_r_multiple: Decimal,
    pub total_pnl: Decimal,
    pub max_drawdown: Decimal,
    pub profit_factor: Decimal,
    pub sharpe_ratio: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricDelta {
    pub metric: String,
    pub value_a: Decimal,
    pub value_b: Decimal,
    pub delta_pct: Decimal,
    pub direction: DeltaDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeltaDirection {
    Improved,
    Declined,
    Neutral,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupDelta {
    pub setup: String,
    pub trade_count_a: i64,
    pub trade_count_b: i64,
    pub win_rate_a: Decimal,
    pub win_rate_b: Decimal,
    pub total_pnl_a: Decimal,
    pub total_pnl_b: Decimal,
    pub avg_r_a: Option<Decimal>,
    pub avg_r_b: Option<Decimal>,
}

// ── Unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_format_default_is_json() {
        assert_eq!(SummaryFormat::default(), SummaryFormat::Json);
    }

    #[test]
    fn test_summary_format_deser_lowercase() {
        let json: SummaryFormat = serde_json::from_str("\"json\"").unwrap();
        assert_eq!(json, SummaryFormat::Json);

        let llm: SummaryFormat = serde_json::from_str("\"llm\"").unwrap();
        assert_eq!(llm, SummaryFormat::Llm);
    }

    #[test]
    fn test_pattern_kind_serde_roundtrip() {
        let variants = [
            PatternKind::SizingDrift,
            PatternKind::FrequencySpike,
            PatternKind::SessionAnomaly,
            PatternKind::SetupFatigue,
            PatternKind::CorrelationStack,
            PatternKind::StreakRisk,
        ];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            let back: PatternKind = serde_json::from_str(&json).unwrap();
            assert_eq!(*v, back);
        }
    }

    #[test]
    fn test_severity_serde_roundtrip() {
        let variants = [Severity::Info, Severity::Notable, Severity::Concerning];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            let back: Severity = serde_json::from_str(&json).unwrap();
            assert_eq!(*v, back);
        }
    }

    #[test]
    fn test_delta_direction_serde() {
        assert_eq!(
            serde_json::to_string(&DeltaDirection::Improved).unwrap(),
            "\"improved\""
        );
        assert_eq!(
            serde_json::to_string(&DeltaDirection::Declined).unwrap(),
            "\"declined\""
        );
        assert_eq!(
            serde_json::to_string(&DeltaDirection::Neutral).unwrap(),
            "\"neutral\""
        );
    }

    #[test]
    fn test_compare_request_deser() {
        let json = r#"{
            "period_a": {"from": "2026-01-01", "to": "2026-03-31"},
            "period_b": {"from": "2026-04-01", "to": "2026-06-30"},
            "filters": {"symbol": "ETH_USDT", "source": "agent:hermes_v1.2"}
        }"#;
        let req: CompareRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.period_a.from.to_string(), "2026-01-01");
        assert_eq!(req.period_b.to.to_string(), "2026-06-30");
        let f = req.filters.unwrap();
        assert_eq!(f.symbol.unwrap(), "ETH_USDT");
        assert_eq!(f.source.unwrap(), "agent:hermes_v1.2");
    }

    #[test]
    fn test_agent_summary_query_deser_defaults() {
        // JSON with empty/missing fields should produce defaults
        let q: AgentSummaryQuery = serde_json::from_str("{}").unwrap();
        assert!(q.timeframe.is_none());
        assert_eq!(q.format, SummaryFormat::Json);
    }

    #[test]
    fn test_agent_summary_query_deser_full() {
        let json = r#"{
            "timeframe": "30d",
            "symbol": "ETH_USDT",
            "side": "LONG",
            "setup_tag": "breakout",
            "exchange": "hyperliquid",
            "source": "agent:hermes_v1.2",
            "format": "llm"
        }"#;
        let q: AgentSummaryQuery = serde_json::from_str(json).unwrap();
        assert_eq!(q.timeframe.unwrap(), "30d");
        assert_eq!(q.symbol.unwrap(), "ETH_USDT");
        assert_eq!(q.side.unwrap(), "LONG");
        assert_eq!(q.setup_tag.unwrap(), "breakout");
        assert_eq!(q.exchange.unwrap(), "hyperliquid");
        assert_eq!(q.source.unwrap(), "agent:hermes_v1.2");
        assert_eq!(q.format, SummaryFormat::Llm);
    }
}
