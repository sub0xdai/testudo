# Specification: Journal Database Performance Hardening

**Spec ID:** JNL-12-db-performance-hardening
**Date:** 2026-03-19
**Status:** Draft
**Class:** Infrastructure / Database
**Priority:** P1 — Write amplification and unbounded memory allocation will degrade under multi-tenant load (1000 users, 500k trades/year)
**Depends on:** None (first in series)
**Series:** JNL-12 (standalone performance hardening)

---

## Problem Statement

The journal database layer has four scaling issues that will degrade performance as user count grows from single-user development to the target of 1000 concurrent users generating ~500k trades/year.

**Missing index on `trade_group_id` (Critical).** The idempotency check in `journal_service.rs:101-106` executes `SELECT ... FROM journal_trades WHERE trade_group_id = $1` on every trade close event. No index exists on this column — the query performs a sequential scan of the entire `journal_trades` table. At 500k rows, each scan takes 10-50ms. This runs synchronously before every INSERT, directly gating write throughput.

**O(n) CTE recompute in `upsert_daily_stats` (High).** After every trade close, `journal_service.rs:222-244` runs a window function CTE that reads and rewrites every `journal_daily_stats` row for the user+exchange pair to recompute cumulative P&L, peak, and drawdown. A user with 2 years of daily trading (~500 rows) triggers 500 row reads + 500 row updates per trade close. At 10k trade closes/day across all users, this produces ~3.65M row touches/day. The query also has a logical bug: `peak_cumulative_pnl` uses `GREATEST(jds.peak_cumulative_pnl, r.cum_pnl)` where `jds.peak_cumulative_pnl` is the stale value being overwritten — the running peak is not correctly propagated forward through the window.

**Unbounded `fetch_ordered_pnls` (High).** The `risk_stats()` method in `journal_stats.rs:310` calls `fetch_ordered_pnls()` which loads ALL trade `net_pnl` values into a `Vec<Decimal>` with no LIMIT. A user with 500k trades materializes ~12-15MB per request. With 1000 concurrent dashboard loads, this risks 12-15GB of heap allocation. Both streak computation and max drawdown calculation could be expressed as SQL window functions, eliminating the Rust-side memory allocation entirely.

**Shared connection pool (Medium).** A single 50-connection `PgPool` in `sqlx_postgres/src/lib.rs:31-35` is shared between OLTP trading operations (fill detection, order execution, rehydration) and OLAP journal analytics (full-table aggregations, window functions). When analytics routes are wired, long-running analytical queries will compete with latency-sensitive trading operations for connections, risking 500ms acquire timeouts on the trading path.

---

## User Stories

- **As a trader on a multi-tenant instance**, I want trade close ingestion to remain fast regardless of journal size, so that my fill detection pipeline is not bottlenecked by journal writes.
- **As a trader viewing my dashboard**, I want analytics to load without causing out-of-memory errors or starving other users' trading operations, so that the platform remains stable under concurrent load.
- **As the platform operator**, I want the database layer to scale linearly with user count, so that I can grow to 1000 users without re-architecting the journal schema.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add B-tree index on `journal_trades(trade_group_id)` so idempotency lookups are O(log n) | High | Migration |
| FR-2 | Scope the cumulative recompute CTE in `upsert_daily_stats` to only update rows from the affected `stat_date` forward, not the entire history | High | JournalService |
| FR-3 | Fix the `peak_cumulative_pnl` propagation bug by using `MAX(cum_pnl) OVER (ORDER BY stat_date)` in the CTE | High | JournalService |
| FR-4 | Replace `fetch_ordered_pnls` + Rust-side `compute_streaks` with a SQL window function that computes current/best/worst streaks server-side | High | StatsEngine |
| FR-5 | Replace `fetch_ordered_pnls` + Rust-side `compute_max_drawdown` with a SQL window function that computes max drawdown server-side | High | StatsEngine |
| FR-6 | Move `fetch_day_extremes` MIN/MAX computation into SQL (return scalar, not `fetch_all`) | Medium | StatsEngine |
| FR-7 | Create a separate `analytics_pool` with dedicated connections for journal read queries | Medium | sqlx_postgres |
| FR-8 | Pass `analytics_pool` to `StatsEngine` and `TimeSeriesService` constructors | Medium | AppState |

---

## Technical Implementation

### 1. Migration — `trade_group_id` Index (FR-1)

New migration file:

```sql
-- 20260319000000_add_journal_trade_group_index.up.sql
CREATE INDEX CONCURRENTLY idx_journal_trades_group_id
    ON journal_trades(trade_group_id);
```

```sql
-- 20260319000000_add_journal_trade_group_index.down.sql
DROP INDEX IF EXISTS idx_journal_trades_group_id;
```

### 2. Scoped Cumulative Recompute (FR-2, FR-3)

Replace `journal_service.rs:222-244` with a date-bounded CTE that correctly propagates peak:

```rust
// journal_service.rs — upsert_daily_stats, second query
sqlx::query(
    "WITH running AS ( \
         SELECT id, \
             SUM(net_pnl) OVER (ORDER BY stat_date) as cum_pnl, \
             MAX(SUM(net_pnl) OVER (ORDER BY stat_date)) \
                 OVER (ORDER BY stat_date) as running_peak \
         FROM journal_daily_stats \
         WHERE user_id = $1 AND exchange = $2 \
     ) \
     UPDATE journal_daily_stats jds SET \
         cumulative_pnl = r.cum_pnl, \
         peak_cumulative_pnl = r.running_peak, \
         drawdown = r.cum_pnl - r.running_peak, \
         drawdown_pct = CASE \
             WHEN r.running_peak > 0 \
             THEN (r.cum_pnl - r.running_peak) / r.running_peak * 100 \
             ELSE 0 END \
     FROM running r \
     WHERE jds.id = r.id \
       AND jds.stat_date >= $3",
)
.bind(trade.user_id)
.bind(&trade.exchange)
.bind(stat_date)  // only update from today forward
```

Note: The CTE still computes the full window (needed for correct cumulative_pnl), but the UPDATE only writes rows from `stat_date` onward. For a new trade on today's date, this typically updates 1 row instead of all ~500.

### 3. SQL-Side Streak Computation (FR-4)

Replace `fetch_ordered_pnls` + `compute_streaks` in `risk_stats()` with a single SQL query:

```sql
WITH outcomes AS (
    SELECT net_pnl,
        CASE WHEN net_pnl > 0 THEN 1 ELSE 0 END as is_win,
        ROW_NUMBER() OVER (ORDER BY closed_at) as rn
    FROM journal_trades
    WHERE user_id = $1
        AND ($2::TEXT IS NULL OR exchange = $2)
        AND ($3::TEXT IS NULL OR symbol = $3)
        AND ($4::DATE IS NULL OR closed_at >= $4)
        AND ($5::DATE IS NULL OR closed_at <= $5)
),
groups AS (
    SELECT is_win,
        rn - ROW_NUMBER() OVER (PARTITION BY is_win ORDER BY rn) as grp,
        rn
    FROM outcomes
),
streaks AS (
    SELECT is_win, COUNT(*) as streak_len,
        MAX(rn) as last_rn
    FROM groups GROUP BY is_win, grp
),
max_rn AS (SELECT MAX(rn) as total FROM outcomes)
SELECT
    COALESCE((SELECT CASE WHEN s.is_win = 1 THEN s.streak_len::INTEGER
                          ELSE -(s.streak_len::INTEGER) END
              FROM streaks s, max_rn m
              WHERE s.last_rn = m.total LIMIT 1), 0) as current_streak,
    COALESCE((SELECT MAX(streak_len)::INTEGER FROM streaks WHERE is_win = 1), 0) as best_streak,
    COALESCE((SELECT -(MAX(streak_len)::INTEGER) FROM streaks WHERE is_win = 0), 0) as worst_streak
```

New row type:

```rust
#[derive(Debug, sqlx::FromRow)]
struct StreakRow {
    current_streak: i32,
    best_streak: i32,
    worst_streak: i32,
}
```

### 4. SQL-Side Max Drawdown (FR-5)

Replace the Rust-side cumulative + `compute_max_drawdown` with:

```sql
WITH cumulative AS (
    SELECT
        SUM(net_pnl) OVER (ORDER BY closed_at) as cum_pnl
    FROM journal_trades
    WHERE user_id = $1
        AND ($2::TEXT IS NULL OR exchange = $2)
        AND ($3::TEXT IS NULL OR symbol = $3)
        AND ($4::DATE IS NULL OR closed_at >= $4)
        AND ($5::DATE IS NULL OR closed_at <= $5)
),
peaks AS (
    SELECT cum_pnl,
        MAX(cum_pnl) OVER (ORDER BY cum_pnl ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) as peak
    FROM cumulative
)
SELECT
    COALESCE(MAX(peak - cum_pnl), 0) as max_drawdown,
    COALESCE(MAX(CASE WHEN peak > 0 THEN (peak - cum_pnl) / peak * 100 ELSE 0 END), 0) as max_drawdown_pct
FROM peaks
```

Note: The `peaks` CTE uses `ORDER BY cum_pnl` which is wrong — it must track the running maximum in insertion order. The correct window is implicit since the CTE preserves row order from `cumulative`. Corrected:

```sql
peaks AS (
    SELECT cum_pnl,
        MAX(cum_pnl) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) as peak
    FROM cumulative
)
```

New row type:

```rust
#[derive(Debug, sqlx::FromRow)]
struct DrawdownRow {
    max_drawdown: Decimal,
    max_drawdown_pct: Decimal,
}
```

### 5. Scalar Day Extremes (FR-6)

Replace `fetch_day_extremes` `fetch_all` + Rust min/max with:

```sql
SELECT
    COALESCE(MIN(net_pnl), 0) as worst_day,
    COALESCE(MAX(net_pnl), 0) as best_day
FROM journal_daily_stats
WHERE user_id = $1
    AND ($2::TEXT IS NULL OR exchange = $2)
    AND ($3::DATE IS NULL OR stat_date >= $3)
    AND ($4::DATE IS NULL OR stat_date <= $4)
```

Returns one row instead of loading all rows into Rust.

### 6. Analytics Connection Pool (FR-7, FR-8)

```rust
// sqlx_postgres/src/lib.rs
pub struct PostgresDb {
    pool: sqlx::Pool<sqlx::Postgres>,
    analytics_pool: sqlx::Pool<sqlx::Postgres>,
}

impl PostgresDb {
    pub async fn new() -> Result<Self, sqlx::Error> {
        // ... existing pool setup ...

        let analytics_max: u32 = std::env::var("DB_ANALYTICS_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let analytics_pool = PgPoolOptions::new()
            .max_connections(analytics_max)
            .acquire_timeout(Duration::from_secs(5))  // analytics can wait longer
            .connect(&db_url)
            .await?;

        Ok(Self { pool, analytics_pool })
    }

    pub fn analytics_pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        &self.analytics_pool
    }
}
```

```rust
// types/app.rs — add field
pub struct AppState {
    // ... existing fields ...
    /// Dedicated pool for journal analytics queries (FR-7)
    pub analytics_pool: sqlx::Pool<sqlx::Postgres>,
}
```

`StatsEngine::new(analytics_pool)` and `TimeSeriesService::new(analytics_pool)` receive the dedicated pool. `JournalService::new(pool)` keeps the OLTP pool since its writes are latency-sensitive and small.

### Files

- `testudo-exchange/crates/sqlx_postgres/migrations/20260319000000_add_journal_trade_group_index.up.sql` — new migration (FR-1)
- `testudo-exchange/crates/sqlx_postgres/migrations/20260319000000_add_journal_trade_group_index.down.sql` — down migration (FR-1)
- `testudo-exchange/crates/router/src/services/journal_service.rs` — fix CTE scope and peak bug (FR-2, FR-3)
- `testudo-exchange/crates/router/src/services/journal_stats.rs` — SQL-side streaks, drawdown, day extremes (FR-4, FR-5, FR-6)
- `testudo-exchange/crates/sqlx_postgres/src/lib.rs` — analytics pool (FR-7)
- `testudo-exchange/crates/router/src/types/app.rs` — analytics_pool field (FR-8)
- `testudo-exchange/crates/router/src/main.rs` — wire analytics_pool into AppState (FR-8)

### Dependencies Added

None — all changes use existing `sqlx`, `rust_decimal` capabilities.

---

## Acceptance Criteria

- [ ] `EXPLAIN ANALYZE` on `SELECT ... FROM journal_trades WHERE trade_group_id = $1` shows Index Scan, not Seq Scan (FR-1)
- [ ] `upsert_daily_stats` UPDATE touches only rows with `stat_date >= trade.closed_at.date()`, verified by `EXPLAIN ANALYZE` showing row count = 1 for today's trades (FR-2)
- [ ] `peak_cumulative_pnl` is monotonically non-decreasing when queried `ORDER BY stat_date` for any user+exchange (FR-3)
- [ ] `risk_stats()` returns correct `current_streak`, `best_streak`, `worst_streak` without loading trade rows into Rust memory (FR-4)
- [ ] `risk_stats()` returns correct `max_drawdown` and `max_drawdown_pct` without loading trade rows into Rust memory (FR-5)
- [ ] `fetch_day_extremes` returns a single-row scalar result, not `Vec<DayPnlRow>` (FR-6)
- [ ] `DB_ANALYTICS_MAX_CONNECTIONS` env var configures a separate pool used by `StatsEngine` and `TimeSeriesService` (FR-7)
- [ ] `JournalService` continues using the primary OLTP pool (FR-8)
- [ ] All existing `compute_streaks` and `compute_max_drawdown` unit tests still pass (pure functions retained for use in non-DB contexts)
- [ ] Error path: analytics pool exhaustion returns 503 without affecting trade execution path
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **SQL streak query correctness** — The gaps-and-islands SQL pattern for streaks is non-trivial. Mitigation: Keep the existing `compute_streaks()` pure function and its 7 unit tests. Add integration tests that compare SQL results against `compute_streaks()` output for known datasets.

2. **Migration on live data** — `CREATE INDEX CONCURRENTLY` requires no table lock but can fail if there are concurrent schema changes. Mitigation: Run migration during low-traffic window. The `CONCURRENTLY` keyword ensures zero downtime.

3. **CTE scope boundary** — Scoping the UPDATE to `stat_date >= $3` assumes no backdated trades arrive. If a trade with `closed_at` in the past is ingested, only that date onward is recomputed, which is correct. But if trades arrive out of order for the same day, the cumulative window still recomputes correctly because `SUM OVER (ORDER BY stat_date)` covers the full history in the CTE. Risk is low.

---

## Completion Signal

This spec is complete when:
1. `trade_group_id` index migration applied and verified via EXPLAIN
2. `upsert_daily_stats` scoped to affected date with correct peak propagation
3. Streak and drawdown computed in SQL, zero unbounded `fetch_all` in `risk_stats()`
4. Day extremes returned as scalar SQL result
5. Analytics pool created and wired to read-path services
6. All acceptance criteria met
7. `cargo clippy --all-targets && cargo test` passes
8. Code committed to master
