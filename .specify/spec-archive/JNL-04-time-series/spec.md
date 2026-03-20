# Specification: Time-Series Aggregation Queries

**Spec ID:** JNL-04-time-series
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / Computation
**Priority:** P0 — powers all chart components
**Depends on:** JNL-01-schema, JNL-02-ingestion
**Series:** Batch 2 — Backend Computation (JNL-03, JNL-04)

---

## Problem Statement

The dashboard charts require time-series data: equity curves, daily P&L, cumulative profit, symbol distributions, duration/profitability correlations, and loss distribution histograms. These must be computed efficiently from `journal_trades` and `journal_daily_stats`, returning chart-ready data structures.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Equity curve: cumulative P&L over time with drawdown overlay | High | time-series |
| FR-2 | Daily P&L bar chart data | High | time-series |
| FR-3 | Cumulative profit line | High | time-series |
| FR-4 | Symbol distribution: trade count and P&L per symbol | High | time-series |
| FR-5 | Duration vs profitability scatter data | Medium | time-series |
| FR-6 | Return distribution histogram (bucketed daily % returns) | Medium | time-series |
| FR-7 | Market % return breakdown (P&L per symbol as bar chart) | Medium | time-series |
| FR-8 | Trade time distribution (hour-of-day, day-of-week) | Low | time-series |
| FR-9 | Support same StatsFilter for all queries | High | time-series |

---

## Technical Implementation

### TimeSeriesService

```rust
// crates/router/src/services/journal_timeseries.rs

pub struct TimeSeriesService {
    pool: PgPool,
}

#[derive(Debug, Serialize)]
pub struct EquityCurvePoint {
    pub date: NaiveDate,
    pub cumulative_pnl: Decimal,
    pub peak: Decimal,
    pub drawdown: Decimal,
    pub drawdown_pct: Decimal,
}

#[derive(Debug, Serialize)]
pub struct DailyPnlPoint {
    pub date: NaiveDate,
    pub net_pnl: Decimal,
    pub trade_count: i32,
    pub win_count: i32,
}

#[derive(Debug, Serialize)]
pub struct SymbolBreakdown {
    pub symbol: String,
    pub trade_count: i64,
    pub net_pnl: Decimal,
    pub win_rate: Decimal,
    pub avg_r: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct DurationProfitPoint {
    pub duration_minutes: f64,
    pub net_pnl: Decimal,
    pub symbol: String,
    pub side: String,
}

#[derive(Debug, Serialize)]
pub struct ReturnBucket {
    pub bucket_label: String,     // "-5%", "-4%", ..., "0%", "1%", ...
    pub day_count: i64,
}

#[derive(Debug, Serialize)]
pub struct TimeDistribution {
    pub hour: i32,                // 0-23
    pub day_of_week: i32,         // 0=Mon, 6=Sun
    pub trade_count: i64,
    pub net_pnl: Decimal,
}

impl TimeSeriesService {
    pub async fn equity_curve(&self, user_id: Uuid, filter: &StatsFilter) -> Result<Vec<EquityCurvePoint>>;
    pub async fn daily_pnl(&self, user_id: Uuid, filter: &StatsFilter) -> Result<Vec<DailyPnlPoint>>;
    pub async fn symbol_breakdown(&self, user_id: Uuid, filter: &StatsFilter) -> Result<Vec<SymbolBreakdown>>;
    pub async fn duration_profit(&self, user_id: Uuid, filter: &StatsFilter) -> Result<Vec<DurationProfitPoint>>;
    pub async fn return_distribution(&self, user_id: Uuid, filter: &StatsFilter) -> Result<Vec<ReturnBucket>>;
    pub async fn time_distribution(&self, user_id: Uuid, filter: &StatsFilter) -> Result<Vec<TimeDistribution>>;
}
```

### Key Queries

**Equity curve** (from pre-computed daily stats):
```sql
SELECT stat_date, cumulative_pnl, peak_cumulative_pnl, drawdown, drawdown_pct
FROM journal_daily_stats
WHERE user_id = $1
    AND ($2::TEXT IS NULL OR exchange = $2)
    AND ($3::DATE IS NULL OR stat_date >= $3)
    AND ($4::DATE IS NULL OR stat_date <= $4)
ORDER BY stat_date;
```

**Symbol breakdown**:
```sql
SELECT symbol,
    COUNT(*) as trade_count,
    SUM(net_pnl) as net_pnl,
    (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC / GREATEST(COUNT(*), 1) * 100 as win_rate,
    AVG(r_multiple) FILTER (WHERE r_multiple IS NOT NULL) as avg_r
FROM journal_trades
WHERE user_id = $1
GROUP BY symbol
ORDER BY trade_count DESC;
```

**Duration vs profitability**:
```sql
SELECT duration_secs / 60.0 as duration_minutes, net_pnl, symbol, side
FROM journal_trades
WHERE user_id = $1
ORDER BY closed_at;
```

**Return distribution** (bucket daily returns into 1% bands):
```sql
SELECT
    FLOOR(CASE WHEN cumulative_pnl != 0 THEN (net_pnl / ABS(cumulative_pnl - net_pnl + 0.0001)) * 100 ELSE 0 END)::INTEGER as bucket,
    COUNT(*) as day_count
FROM journal_daily_stats
WHERE user_id = $1 AND trade_count > 0
GROUP BY bucket
ORDER BY bucket;
```

**Time distribution**:
```sql
SELECT
    EXTRACT(HOUR FROM opened_at)::INTEGER as hour,
    EXTRACT(DOW FROM opened_at)::INTEGER as day_of_week,
    COUNT(*) as trade_count,
    SUM(net_pnl) as net_pnl
FROM journal_trades
WHERE user_id = $1
GROUP BY hour, day_of_week;
```

### Files

- `testudo-exchange/crates/router/src/services/journal_timeseries.rs` — new
- `testudo-exchange/crates/router/src/services/mod.rs` — add module

---

## Acceptance Criteria

- [ ] Equity curve returns correct running totals with drawdown
- [ ] Daily P&L matches sum of individual trade P&L per day
- [ ] Symbol breakdown percentages sum to 100%
- [ ] Duration scatter returns all trades with correct minute conversion
- [ ] Return distribution buckets cover the full range of observed returns
- [ ] Filters apply consistently across all queries
- [ ] Empty data returns empty arrays (not errors)
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Completion Signal

This spec is complete when:
1. All 6 time-series queries return correct, chart-ready data
2. Unit tests verify each query with known data
3. All acceptance criteria met
4. Code committed to master
