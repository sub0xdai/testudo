# Specification: Push Rolling Extremes MIN/MAX to SQL

**Spec ID:** PERF-03-sql-side-minmax
**Date:** 2026-05-10
**Status:** Draft
**Class:** Refactor / Performance (Backend)
**Priority:** P3 — Trivial code change; removes N-row wire transfer for a 2-row result
**Depends on:** None
**Source:** DATABASE_AUDIT.md — Action 5

---

## Problem Statement

`StatsEngine::fetch_rolling_extremes` in `journal_stats.rs` computes a rolling N-day window sum over `journal_daily_stats`, fetches every intermediate row to Rust, then applies `.min()` and `.max()` client-side:

```rust
let rows = sqlx::query_as::<_, RollingRow>(
    "SELECT SUM(net_pnl) OVER (
        ORDER BY stat_date
        ROWS BETWEEN ($5 - 1) PRECEDING AND CURRENT ROW
    ) as rolling_pnl
    FROM journal_daily_stats
    WHERE user_id = $1 AND ..."
)
.bind(window)
.fetch_all(&self.pool)
.await?;

let worst = rows.iter().filter_map(|r| r.rolling_pnl).min().unwrap_or(Decimal::ZERO);
let best = rows.iter().filter_map(|r| r.rolling_pnl).max().unwrap_or(Decimal::ZERO);
```

For a user with 1,825 days of history (5 years), this ships 1,825 rows of `Decimal` over the wire when only 2 values (min, max) are needed. While the absolute overhead is small (1,825 × 16 bytes ≈ 29 KB), the Rust allocation of `Vec<RollingRow>` and the client-side iteration are unnecessary when SQL can compute MIN/MAX directly.

The fix wraps the existing window-function query in a subquery and applies `MIN()`/`MAX()` in the outer SELECT. The result is a 2-tuple, not an N-row vector. Query plan is identical — the window function still runs once, but the result is aggregated server-side.

---

## User Stories

- As a **user loading the Risk tab**, I want the best/worst rolling period computation to transfer the minimum data over the database wire, even if the absolute latency improvement is small.
- As a **backend maintainer**, I want `fetch_rolling_extremes` to return a scalar tuple rather than a `Vec`, so the function signature matches its semantics and callers don't allocate unnecessarily.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | Wrap existing window-function query in `SELECT MIN(rolling_pnl), MAX(rolling_pnl) FROM (...)` subquery | High |
| FR-2 | Change return type from `(Decimal, Decimal)` via `fetch_one` on a `(Option<Decimal>, Option<Decimal>)` tuple — no more `fetch_all` + `Vec` allocation | High |
| FR-3 | Remove `RollingRow` struct if it is no longer used elsewhere | Medium |
| FR-4 | Existing behavior preserved: `None` rolling values (from incomplete windows at the start of the date range) are ignored by MIN/MAX in SQL, matching the previous Rust `filter_map` behavior | High |
| FR-5 | All existing unit tests pass; no behavioral change | High |

---

## Acceptance Criteria

- [ ] `fetch_rolling_extremes` returns `(Decimal, Decimal)` via `fetch_one` — not `Vec<RollingRow>` via `fetch_all`
- [ ] `MIN(rolling_pnl)` and `MAX(rolling_pnl)` applied in SQL, not in Rust
- [ ] Incomplete windows (NULL rolling_pnl) are correctly ignored by SQL MIN/MAX (NULLs are skipped by aggregate functions)
- [ ] `RollingRow` struct removed if no other code references it
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes
- [ ] `DATABASE_AUDIT.md` updated to mark Action 5 as complete
- [ ] `cargo test` output shows all `journal_stats` tests passing (especially any that exercise risk_stats/rolling extremes)

---

## Technical Notes

### Files to Modify
- `testudo-exchange/crates/router/src/services/journal_stats.rs` — `fetch_rolling_extremes` method
- `DATABASE_AUDIT.md` — mark Action 5 as done

### Before (current code)

```rust
/// Fetch best/worst rolling N-day window from daily stats.
async fn fetch_rolling_extremes(
    &self,
    user_id: Uuid,
    filter: &StatsFilter,
    window: i32,
) -> Result<(Decimal, Decimal), sqlx::Error> {
    let rows = sqlx::query_as::<_, RollingRow>(
        "SELECT SUM(net_pnl) OVER ( \
            ORDER BY stat_date \
            ROWS BETWEEN ($5 - 1) PRECEDING AND CURRENT ROW \
        ) as rolling_pnl \
        FROM journal_daily_stats \
        WHERE user_id = $1 \
            AND ($2::TEXT IS NULL OR exchange = $2) \
            AND ($3::DATE IS NULL OR stat_date >= $3) \
            AND ($4::DATE IS NULL OR stat_date <= $4)",
    )
    .bind(user_id)
    .bind(&filter.exchange)
    .bind(filter.date_from)
    .bind(filter.date_to)
    .bind(window)
    .fetch_all(&self.pool)
    .await?;

    let worst = rows
        .iter()
        .filter_map(|r| r.rolling_pnl)
        .min()
        .unwrap_or(Decimal::ZERO);
    let best = rows
        .iter()
        .filter_map(|r| r.rolling_pnl)
        .max()
        .unwrap_or(Decimal::ZERO);

    Ok((worst, best))
}
```

### After (replacement)

```rust
/// Fetch best/worst rolling N-day window from daily stats.
/// MIN/MAX applied in SQL — returns a 2-tuple, not a Vec.
async fn fetch_rolling_extremes(
    &self,
    user_id: Uuid,
    filter: &StatsFilter,
    window: i32,
) -> Result<(Decimal, Decimal), sqlx::Error> {
    let row: (Option<Decimal>, Option<Decimal>) = sqlx::query_as(
        "SELECT MIN(rolling_pnl), MAX(rolling_pnl) FROM ( \
            SELECT SUM(net_pnl) OVER ( \
                ORDER BY stat_date \
                ROWS BETWEEN ($5 - 1) PRECEDING AND CURRENT ROW \
            ) AS rolling_pnl \
            FROM journal_daily_stats \
            WHERE user_id = $1 \
                AND ($2::TEXT IS NULL OR exchange = $2) \
                AND ($3::DATE IS NULL OR stat_date >= $3) \
                AND ($4::DATE IS NULL OR stat_date <= $4) \
        ) sub",
    )
    .bind(user_id)
    .bind(&filter.exchange)
    .bind(filter.date_from)
    .bind(filter.date_to)
    .bind(window)
    .fetch_one(&self.pool)
    .await?;

    let worst = row.0.unwrap_or(Decimal::ZERO);
    let best = row.1.unwrap_or(Decimal::ZERO);

    Ok((worst, best))
}
```

### Cleanup

If `RollingRow` is defined only for `fetch_rolling_extremes`, remove it:

```rust
// Remove this struct if no other query uses it:
#[derive(Debug, sqlx::FromRow)]
struct RollingRow {
    rolling_pnl: Option<Decimal>,
}
```

Check with: `grep -rn "RollingRow" testudo-exchange/crates/router/src/services/journal_stats.rs`

### Compatibility

- **Schema:** No change. Query hits the same table (`journal_daily_stats`), same WHERE clause, same window function.
- **API:** No change. Return type is still `(Decimal, Decimal)`. Callers in `risk_stats()` are unaffected.
- **NULL behavior:** SQL `MIN()`/`MAX()` skip NULLs, matching the previous `filter_map(|r| r.rolling_pnl)` which skipped `None`. First few rows of a rolling window may produce NULL (fewer than N preceding rows exist); these are correctly ignored by both approaches.

### Dependencies
- None

### Assumptions
- `journal_daily_stats` is populated with enough rows for rolling windows to produce meaningful MIN/MAX (true for any user with ≥ 1 day of stats)

### Risks
- **None.** Query semantics are identical. The subquery wrapping is transparent to the query planner — the window function runs the same way. The only difference is where the final MIN/MAX aggregation happens (server vs client). SQL aggregate functions skip NULLs, matching the Rust `filter_map` behavior exactly.

---

## Completion Signal

### Implementation Checklist
- [ ] `fetch_rolling_extremes` replaced with SQL-side MIN/MAX version
- [ ] `RollingRow` struct removed if no other references exist
- [ ] `cargo clippy --all-targets` passes with no new warnings
- [ ] `cargo test` passes all tests (unit tests in `journal_stats.rs` are all pure Rust, no DB — they test `compute_streaks`, `compute_max_drawdown`, etc., not `fetch_rolling_extremes`)
- [ ] `DATABASE_AUDIT.md` updated

### Done Signal
```
<promise>DONE</promise>
```
