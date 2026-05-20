# Specification: Journal as Agent Memory — Structured Query Interface

**Spec ID:** AGENT-03-journal-memory
**Date:** 2026-05-20
**Status:** Draft
**Class:** Feature / Analytics
**Priority:** P1 — agents need performance feedback to improve; journal data is rich but not agent-queryable
**Depends on:** AGENT-01-signal-endpoint (memory is most useful when there are agent trades to analyze)
**Series:** AGENT-01 through AGENT-03 (Agent Integration Blueprint)

---

## Problem Statement

The Testudo journal records every trade, fill, management decision, tag, and note. The analytics layer (`journal_stats.rs`, `journal_timeseries.rs`) computes equity curves, win rates, R-multiples, symbol breakdowns, time distributions, and duration analyses — 11 chart types, all filterable. The coach pipeline (`services/coach/`) detects behavioral patterns: sizing drift, frequency spikes, session anomalies, setup fatigue, correlation stacking, and streak risk.

But this data is locked behind dashboard UI endpoints designed for human chart rendering. An agent cannot ask a structured question like "what's my win rate on ETH breakout setups in the last 90 days?" or "compare my performance between Q1 and Q2" without either:
1. Calling multiple analytics endpoints and correlating the results manually
2. Parsing raw trade lists and computing stats itself

The agent needs a **query interface** that:
- Returns consolidated summaries filtered by symbol, timeframe, setup tag, side
- Supports multiple output formats: JSON (for programmatic use) and LLM-ready markdown (for feeding directly into an LLM's context window)
- Surfaces actionable insights from the coach pipeline's pattern detection
- Enables period-over-period comparison for strategy evaluation

All the underlying computation already exists. `StatsEngine`, `TimeSeriesService`, and `CoachDigest` do the heavy lifting. The missing pieces are API endpoints that compose these into agent-friendly responses and a formatter that produces LLM-optimized markdown.

---

## User Stories

- **As a coding agent**, I want to query my trading journal with structured filters, so that I can evaluate my strategy without parsing raw analytics JSON.
- **As a coding agent**, I want an LLM-ready markdown summary of my recent performance, so that I can include it in my reasoning context for the next trade decision.
- **As a strategy developer**, I want to compare my performance between two time periods, so that I can determine if a strategy change improved results.
- **As a user**, I want my agent to know which setups are working and which are failing, so that the agent can adapt its behavior based on real data.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `GET /api/v1/journal/agent/summary` returns a consolidated performance summary filtered by symbol, timeframe, side, setup tag, and exchange | High | Router |
| FR-2 | Summary supports `format=json` (structured JSON) and `format=llm` (markdown optimized for LLM context windows) | High | Router |
| FR-3 | LLM-format summary includes: overall stats, per-setup breakdown table, top-performing trades with citation IDs, actionable insights | High | Router |
| FR-4 | `GET /api/v1/journal/agent/insights` returns detected patterns from the coach pipeline adapted for agent consumption | Medium | Router |
| FR-5 | Insights include: low win-rate setups, stop distance analysis, session timing patterns, sizing consistency checks | Medium | Router |
| FR-6 | `POST /api/v1/journal/agent/compare` compares two time periods or two strategies across key metrics | Medium | Router |
| FR-7 | Compare endpoint returns side-by-side stats: trade count, win rate, avg R, total PnL, max drawdown, Sharpe-like ratio, and per-setup deltas | Medium | Router |
| FR-8 | All agent journal endpoints require authentication (SIWE bearer token) | High | Router |
| FR-9 | Trade citation IDs in LLM output use format `[T-{short_id}]` matching the coach pipeline's citation token convention | Medium | Router |
| FR-10 | Agent journal endpoints support filtering by `source` field (e.g., `?source=agent:hermes_v1.2`) to isolate a specific agent's trades | Medium | Router |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `GET /journal/agent/summary?format=json` — basic stats from StatsEngine | Returns trade_count, win_rate, total_pnl, avg_r |
| CP-2 | Add `format=llm` — same data formatted as markdown with citation tokens | Markdown includes [T-xxxxxxxx] citations, per-setup tables |
| CP-3 | `GET /journal/agent/insights` — wire coach pattern detectors to ad-hoc insights | Returns sizing drift, frequency spike, session anomaly flags |
| CP-4 | `POST /journal/agent/compare` — side-by-side period comparison | Returns delta table with all key metrics |
| CP-5 | Filter by `source` field from AGENT-01 | Agent can isolate its own trades |

### Key Types

```rust
// crates/router/src/models/agent_journal.rs

/// Query parameters for agent journal summary.
#[derive(Debug, Deserialize)]
pub struct AgentSummaryQuery {
    pub timeframe: Option<String>,        // "7d", "30d", "90d", "all"
    pub symbol: Option<String>,           // "ETH_USDT"
    pub side: Option<String>,             // "LONG", "SHORT"
    pub setup_tag: Option<String>,        // "breakout"
    pub exchange: Option<String>,         // "hyperliquid", "binance"
    pub source: Option<String>,           // "agent:hermes_v1.2"
    pub format: Option<SummaryFormat>,    // "json" (default) or "llm"
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SummaryFormat {
    Json,
    Llm,
}

/// Structured summary response (format=json).
#[derive(Debug, Serialize)]
pub struct AgentSummary {
    pub timeframe: TimeframeInfo,
    pub overall: OverallStats,
    pub by_setup: Vec<SetupBreakdown>,
    pub top_trades: Vec<TradeCitation>,
    pub equity: Vec<EquityPoint>,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct SetupBreakdown {
    pub setup: String,
    pub trade_count: i64,
    pub win_rate: Decimal,
    pub avg_r_multiple: Decimal,
    pub total_pnl: Decimal,
}

#[derive(Debug, Serialize)]
pub struct TradeCitation {
    pub id: Uuid,
    pub short_id: String,       // First 8 chars of UUID, used as [T-xxxxxxxx]
    pub symbol: String,
    pub side: String,
    pub opened_at: DateTime<Utc>,
    pub pnl: Decimal,
    pub r_multiple: Option<Decimal>,
    pub setup_tag: Option<String>,
}

/// Insight derived from coach pattern detection.
#[derive(Debug, Serialize)]
pub struct AgentInsight {
    pub pattern: PatternKind,
    pub severity: Severity,
    pub headline: String,
    pub detail: String,
    pub recommendation: Option<String>,
    pub evidence_count: i64,
}

/// Period comparison request.
#[derive(Debug, Deserialize)]
pub struct CompareRequest {
    pub period_a: TimeframeRange,
    pub period_b: TimeframeRange,
    pub filters: Option<AgentSummaryQuery>,  // Shared filters for both periods
}

#[derive(Debug, Deserialize)]
pub struct TimeframeRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

/// Side-by-side comparison result.
#[derive(Debug, Serialize)]
pub struct ComparisonResult {
    pub period_a: PeriodInfo,
    pub period_b: PeriodInfo,
    pub deltas: Vec<MetricDelta>,
    pub by_setup_deltas: Vec<SetupDelta>,
}

#[derive(Debug, Serialize)]
pub struct MetricDelta {
    pub metric: String,
    pub value_a: Decimal,
    pub value_b: Decimal,
    pub delta_pct: Decimal,       // (b - a) / a * 100
    pub direction: DeltaDirection,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeltaDirection {
    Improved,
    Declined,
    Neutral,
}
```

### LLM Markdown Format

When `format=llm`, the response body is a structured markdown string. The router sets `Content-Type: text/markdown`:

```markdown
## Journal Summary: BTC + ETH (Last 90 Days)

### Overall Performance
- Total trades: 112
- Win rate: 54.5%
- Avg R-multiple: 1.72
- Total P&L: +$8,420.50
- Max drawdown: -$1,890.00
- Profit factor: 1.83
- Sharpe ratio: 1.21

### By Setup Tag
| Setup | Trades | Win Rate | Avg R | P&L |
|---|---|---|---|---|
| breakout | 28 | 60.7% | 2.1 | +$3,240 |
| support_bounce | 34 | 55.9% | 1.8 | +$2,850 |
| trend_follow | 22 | 40.9% | 0.9 | -$920 |
| reversal | 28 | 53.6% | 1.5 | +$3,250 |

### Top Performers
- [T-a3f2b1c4] BTC_USDT long — breakout, 4.2R, opened 2026-03-15
- [T-b7c1d2e3] ETH_USDT short — support break, 3.1R, opened 2026-04-02
- [T-c1d2e3f4] BTC_USDT long — trend continuation, 2.8R, opened 2026-04-28
- [T-d2e3f4a5] ETH_USDT long — reversal, 2.5R, opened 2026-05-10
- [T-e3f4a5b6] BTC_USDT short — breakout failure, 2.3R, opened 2026-05-14

### Actionable Insights
- **Breakout setups show edge**: 60.7% WR, 2.1 avg R. Consider increasing allocation.
- **Trend-follow underperforms**: 40.9% WR, losing strategy. Reduce size or pause.
- **Tight stops correlate with losses**: 68% of losing trades had SL < 1.5% from entry.
- **Session timing**: Best performance 14:00–18:00 UTC. Worst: 00:00–04:00 UTC.
```

### Paved Roads

- `journal_stats.rs` `StatsEngine` — already computes `trade_count`, `win_rate`, `total_pnl`, `avg_r`, profit factor, Sharpe ratio, per-symbol, per-setup breakdowns
- `journal_timeseries.rs` `TimeSeriesService` — already computes equity curve, daily PnL, time distributions
- `services/coach/patterns/` — `sizing_drift.rs`, `frequency_spike.rs`, `session_anomaly.rs`, `setup_fatigue.rs`, `correlation_stack.rs`, `streak_risk.rs` already detect patterns against baselines
- `services/coach/types.rs` — `CoachDigest`, `FlaggedPattern`, `TradeEvidence` types already have the structure needed for insights
- `routes/journal.rs` — existing `/journal/trades`, `/journal/analytics/*` endpoints; agent endpoints follow the same patterns
- `middleware/auth.rs` — `AuthenticatedUser` extractor

### Files

- `crates/router/src/routes/agent_journal.rs` — **NEW** — handlers for `/journal/agent/summary`, `/journal/agent/insights`, `/journal/agent/compare`
- `crates/router/src/services/agent_journal.rs` — **NEW** — orchestrates StatsEngine + TimeSeriesService + CoachDigest → AgentSummary, AgentInsight, ComparisonResult
- `crates/router/src/services/agent_journal_formatter.rs` — **NEW** — formats AgentSummary → LLM markdown with citation tokens and per-setup tables
- `crates/router/src/models/agent_journal.rs` — **NEW** — types: AgentSummaryQuery, AgentSummary, AgentInsight, CompareRequest, ComparisonResult, etc.
- `crates/router/src/routes/mod.rs` — route registration for three new endpoints
- `crates/router/src/routes/journal.rs` — no changes needed; agent endpoints are separate routes

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `GET /journal/agent/summary?format=json` returns overall stats, per-setup breakdown, top trades, and equity points
- [ ] `GET /journal/agent/summary?format=llm` returns valid markdown with `[T-xxxxxxxx]` citation tokens
- [ ] LLM markdown includes per-setup table with trade_count, win_rate, avg_r, total_pnl
- [ ] LLM markdown includes actionable insights section (best/worst setups, stop distance analysis)
- [ ] `GET /journal/agent/summary?symbol=ETH_USDT&setup_tag=breakout` filters correctly
- [ ] `GET /journal/agent/summary?source=agent:hermes_v1.2` isolates only that agent's trades
- [ ] `GET /journal/agent/insights` returns detected patterns with severity and recommendations
- [ ] Insights include sizing drift, frequency spike, session anomaly, and setup fatigue patterns
- [ ] `POST /journal/agent/compare` returns side-by-side stats with delta calculations
- [ ] Compare endpoint correctly identifies improved/declined/neutral directions per metric
- [ ] Unauthenticated requests return 401
- [ ] Invalid date ranges or missing required fields return 400 with clear error
- [ ] `cargo clippy --all-targets && cargo test` passes in testudo-exchange
- [ ] Unit tests cover: JSON format, LLM format generation, filtering, comparison deltas

---

## Risks

1. **LLM format token budget** — The markdown response could be large if an agent has hundreds of trades. Mitigation: limit top trades to 10, per-setup table to top 10 setups. Add `limit` query parameter (default 10, max 50) if needed.
2. **Coach pattern freshness** — Insights from the coach pipeline are weekly. Ad-hoc queries may return patterns from the last cached digest, not real-time. Mitigation: document this. Future enhancement: compute patterns on-demand for ad-hoc queries.
3. **Comparison computation cost** — Two full StatsEngine runs per comparison request. Mitigation: cache StatsEngine results with 60s TTL (reuse pg_queue cache). Comparison endpoint is not called at high frequency.

---

## Completion Signal

This spec is complete when:
1. Three agent journal endpoints are implemented and return correct data
2. LLM markdown format renders properly with citation tokens, per-setup tables, and actionable insights
3. Comparison endpoint produces accurate delta calculations
4. All 14 acceptance criteria met
5. `cargo clippy --all-targets && cargo test` passes
6. Code committed to master
