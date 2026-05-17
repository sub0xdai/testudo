# Specification: Statistics Computation Engine

**Spec ID:** JNL-03-stats-engine
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / Computation
**Priority:** P0 — powers all dashboard panels
**Depends on:** JNL-01-schema, JNL-02-ingestion
**Series:** Batch 2 — Backend Computation (JNL-03, JNL-04)

---

## Problem Statement

Raw trade data in `journal_trades` needs to be transformed into the 20+ statistics that populate the dashboard panels (account overview, stats, risk). These computations must be efficient — pre-computed where possible, query-time where freshness matters.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Compute account overview stats: total P&L, balance change, fees paid | High | stats engine |
| FR-2 | Compute performance stats: win rate, profit factor, avg win/loss, expectancy | High | stats engine |
| FR-3 | Compute risk stats: max drawdown, worst day/week/month, avg R-multiple, risk of ruin | High | stats engine |
| FR-4 | Compute streak stats: current streak, best/worst streak | Medium | stats engine |
| FR-5 | Support filtering by: exchange, symbol, date range, tags | High | stats engine |
| FR-6 | All financial math uses `rust_decimal::Decimal` | High | stats engine |

---

## Technical Implementation

### StatsEngine

```rust
// crates/router/src/services/journal_stats.rs

pub struct StatsEngine {
    pool: PgPool,
}

#[derive(Debug, Serialize)]
pub struct AccountOverview {
    pub total_trades: i64,
    pub total_pnl: Decimal,
    pub total_fees: Decimal,
    pub net_pnl: Decimal,
    pub open_pnl: Decimal,           // from live positions if available
    pub deposits: Decimal,            // from exchange balance history
}

#[derive(Debug, Serialize)]
pub struct PerformanceStats {
    pub win_rate: Decimal,            // wins / total * 100
    pub profit_factor: Decimal,       // gross_profit / gross_loss
    pub avg_win: Decimal,
    pub avg_loss: Decimal,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
    pub expectancy: Decimal,          // (win_rate * avg_win) - (loss_rate * avg_loss)
    pub avg_r_multiple: Decimal,
    pub trades_per_day: Decimal,
    pub avg_duration_secs: i64,
    pub total_duration_days: i64,     // first trade to last trade
}

#[derive(Debug, Serialize)]
pub struct RiskStats {
    pub max_drawdown: Decimal,        // absolute $
    pub max_drawdown_pct: Decimal,    // percentage
    pub worst_day: Decimal,
    pub worst_week: Decimal,
    pub worst_month: Decimal,
    pub best_day: Decimal,
    pub best_week: Decimal,
    pub best_month: Decimal,
    pub avg_r_multiple: Decimal,
    pub risk_of_ruin: Decimal,        // simplified: (1 - win_rate) ^ consecutive_losses_to_ruin
    pub current_streak: i32,          // positive = wins, negative = losses
    pub best_streak: i32,
    pub worst_streak: i32,
}

#[derive(Debug, Deserialize)]
pub struct StatsFilter {
    pub exchange: Option<String>,
    pub symbol: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub tags: Option<Vec<String>>,
}

impl StatsEngine {
    pub async fn account_overview(&self, user_id: Uuid, filter: &StatsFilter) -> Result<AccountOverview>;
    pub async fn performance_stats(&self, user_id: Uuid, filter: &StatsFilter) -> Result<PerformanceStats>;
    pub async fn risk_stats(&self, user_id: Uuid, filter: &StatsFilter) -> Result<RiskStats>;
}
```

### Key Queries

**Win rate + profit factor** — single pass over filtered trades:
```sql
SELECT
    COUNT(*) as total,
    COUNT(*) FILTER (WHERE net_pnl > 0) as wins,
    COUNT(*) FILTER (WHERE net_pnl <= 0) as losses,
    COALESCE(SUM(net_pnl) FILTER (WHERE net_pnl > 0), 0) as gross_profit,
    COALESCE(ABS(SUM(net_pnl) FILTER (WHERE net_pnl <= 0)), 0) as gross_loss,
    COALESCE(AVG(net_pnl) FILTER (WHERE net_pnl > 0), 0) as avg_win,
    COALESCE(AVG(net_pnl) FILTER (WHERE net_pnl <= 0), 0) as avg_loss,
    MAX(net_pnl) as largest_win,
    MIN(net_pnl) as largest_loss,
    AVG(r_multiple) FILTER (WHERE r_multiple IS NOT NULL) as avg_r,
    AVG(duration_secs) as avg_duration
FROM journal_trades
WHERE user_id = $1
    AND ($2::TEXT IS NULL OR exchange = $2)
    AND ($3::TEXT IS NULL OR symbol = $3)
    AND ($4::DATE IS NULL OR closed_at >= $4)
    AND ($5::DATE IS NULL OR closed_at <= $5);
```

**Worst day/week/month** — from `journal_daily_stats`:
```sql
-- Worst day
SELECT MIN(net_pnl) FROM journal_daily_stats WHERE user_id = $1;

-- Worst week (rolling 7-day sum)
SELECT MIN(week_pnl) FROM (
    SELECT SUM(net_pnl) OVER (ORDER BY stat_date ROWS BETWEEN 6 PRECEDING AND CURRENT ROW) as week_pnl
    FROM journal_daily_stats WHERE user_id = $1
) sub;
```

**Streak calculation** — ordered scan:
```sql
SELECT net_pnl > 0 as is_win, closed_at
FROM journal_trades
WHERE user_id = $1
ORDER BY closed_at;
```
Compute streaks in Rust by iterating the result set.

### Files

- `testudo-exchange/crates/router/src/services/journal_stats.rs` — new
- `testudo-exchange/crates/router/src/services/mod.rs` — add module

---

## Acceptance Criteria

- [ ] All three stat groups compute correctly from test data
- [ ] Profit factor returns `Decimal::MAX` when gross_loss is zero (no division by zero)
- [ ] Filters narrow results correctly (exchange, symbol, date range)
- [ ] All math uses `Decimal` — no `f64` anywhere
- [ ] Unit tests with known trade data verify expected outputs
- [ ] Edge cases: zero trades returns sensible defaults, single trade works
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Completion Signal

This spec is complete when:
1. StatsEngine computes all 20+ statistics correctly
2. Filters work across all stat groups
3. All acceptance criteria met
4. Code committed to master
