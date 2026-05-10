# Database Architecture Diagnostic Report

**Target:** testudo-exchange PostgreSQL  
**Engine:** PostgreSQL 12.2 (container), 5 Gi PVC, zero custom runtime parameters  
**Connection Pooling:** sqlx built-in — OLTP pool (max 50, 500ms timeout) + analytics pool (max 10, 5s timeout)  
**Primary Access Patterns:** Financial journal write-through → stats read-back → coach/dignitas snapshot pipeline  
**Tables Evaluated:** 24 tables, 40+ migrations, UNLOGGED cache, 4 queue tables, runtime-created `managed_positions`  
**Date:** 2026-05-10

---

## ⚠️ Critical Reassessment (Read First)

Every query in this system is scoped to `WHERE user_id = $1`. Per-user data volumes are inherently bounded — even a power trader generates hundreds to low thousands of `journal_trades` rows, not millions. PostgreSQL can aggregate, window, and filter 500–5,000 rows in single-digit milliseconds on default settings. The original diagnostic (v1) applied warehouse-scale thinking to what is fundamentally a small-data-per-tenant architecture. Several recommendations were premature optimization. This v2 report corrects that.

**What the v1 report got wrong — and why these recommendations were rejected:**

| Recommendation | Verdict | Why |
|---|---|---|
| UUID v7 migration across all tables | **Rejected** | Requires PG17 or a non-core extension. Existing rows keep v4 UUIDs regardless — the default change only affects new inserts. B-tree depth at 1M rows is 3–4 levels; fragmentation on a table this small is invisible. Revisit only if total row count exceeds 10M. |
| Covering index on `journal_trades` | **Rejected** | A 10-column index on a 25-column table with per-user row counts in the hundreds adds ~40% write overhead (every INSERT updates the covering index too) for zero measurable read gain. The index would be larger than the per-user heap data it "covers." |
| BRIN indexes on `journal_trades` / `raw_fills` | **Rejected** | BRIN exploits physical row-to-key correlation. UUID v4 primary keys scatter rows randomly across pages — a `closed_at` BRIN would cover ~0 correlated rows per range and devolve to a seq scan. Would only help after `CLUSTER ON closed_at_idx`, which is an `ACCESS EXCLUSIVE` lock incompatible with live writes. |
| StatsEngine CTE consolidation | **Deferred** | Merging `aggregate_trades`, `fetch_streaks_sql`, and `fetch_drawdown_sql` into one CTE is architecturally elegant but fragile. For 500-row per-user scans, the latency delta between 3 scans and 1 CTE scan is noise (single-digit ms). Risk of breaking the dashboard outweighs the reward. Revisit if per-user row counts exceed 50K. |
| PgBouncer connection pooling | **Unnecessary** | sqlx already provides built-in pooling with separate OLTP (max 50, 500ms acquire timeout) and analytics (max 10, 5s acquire timeout) pools. Adding PgBouncer would add a network hop without addressing any actual bottleneck. |
| PostgreSQL 12.2 → 16 upgrade | **Deferred to next major release cycle** | The backup cronjob already uses `postgres:16`. PG16 offers real planner and parallel query improvements, but the runtime upgrade requires a full regression pass across all 40+ migrations and query services. Worth doing — just not urgently. Plan it as a scheduled maintenance window, not a hotfix. |

**What survives scrutiny — and is actionable now:**

1. **Engine tuning** — zero downside, prevents silent degradation as data grows
2. **`managed_positions` indexes** — fills a genuine gap; startup rehydration does a full table scan today
3. **`cache_entries` expires_at index** — prevents one table scan per cleanup cycle
4. **Redundant index removal** — reduces write overhead with zero risk
5. **`fetch_rolling_extremes` fix** — trivial code change, reduces wire transfer from N rows to 2

---

## Critical Bottlenecks (Post-Reassessment)

### 1. Zero PostgreSQL Runtime Tuning

The deployment has no `shared_buffers`, `work_mem`, `random_page_cost`, or `effective_cache_size` configuration. Defaults on PG12 are:
- `shared_buffers`: 128 MB (25% of RAM on a typical 2 Gi container = 512 MB unused)
- `work_mem`: 4 MB (aggregates spilling to disk prematurely)
- `random_page_cost`: 4.0 (HDD profile; PVC is on `standard-rwo` SSD)

**Impact:** With growing per-user trade histories, aggregates that currently fit in memory will spill to disk at `work_mem=4MB`. An aggregate over 1,000 trades with 20 columns fits in well under 4 MB of work_mem — so this is not a problem *today*. It becomes one when per-user trade counts hit ~10K or when concurrent dashboard fetches multiply memory pressure.

**Severity:** Low today, medium at 10x current scale.

### 2. `managed_positions` — No Indexes on Runtime-Created Table

The table is created in application code (`trade_manager/repository.rs`) rather than via migration. It has no indexes on `state` or `exchange_account_id`. The rehydration service at startup does a full sequential scan:

```rust
// rehydration.rs: load_active()
let positions = self.repository.load_active().await?;
```

And the Dignitas query iterates over positions to collect `exchange_account_id` values. Without indexes, rehydration time is O(n) on total rows (including historical closed positions, unless filtered out).

**Impact:** Startup latency. With dozens of active positions this is negligible. With hundreds it becomes measurable. The `load_active()` implementation filters `WHERE state != 'Closed'` — without an index that's a full table scan.

**Severity:** Low, but zero-cost to fix.

### 3. `cache_entries` Cleanup Does a Full Sequential Scan

```rust
// pg_queue/src/cache.rs: cleanup_expired()
sqlx::query("DELETE FROM cache_entries WHERE expires_at <= NOW()")
```

The `cache_entries` table is UNLOGGED, meaning it's wiped on crash. Between crashes it accumulates rows from risk config caching, Dignitas weight lookups, and session-related data. The cleanup has no index to find expired rows.

**Impact:** With hundreds of cache entries and frequent TTL expirations, the cleanup scan is trivial. This becomes a concern only if `cache_entries` grows to tens of thousands of rows (which would suggest a cache-purging bug, not a normal operating state).

**Severity:** Low, but the index costs almost nothing.

### 4. Aggregate Over-Fetching in `fetch_rolling_extremes`

```rust
// journal_stats.rs
let rows = sqlx::query_as::<_, RollingRow>(
    "SELECT SUM(net_pnl) OVER (...) as rolling_pnl FROM journal_daily_stats WHERE ..."
).bind(window).fetch_all(&self.pool).await?;

let worst = rows.iter().filter_map(|r| r.rolling_pnl).min().unwrap_or(Decimal::ZERO);
let best = rows.iter().filter_map(|r| r.rolling_pnl).max().unwrap_or(Decimal::ZERO);
```

The query computes a rolling sum for *every row* and ships the entire result to Rust, where min/max is applied client-side. For a user with 90 days of daily stats, that's 90 rows — negligible. For 5 years (1,825 rows), still negligible. The issue is the wire protocol overhead per row, not the data volume.

**Impact:** Trivially fixable with a SQL-side `MIN()/MAX()` wrapper. The fix adds zero complexity and reduces the Rust allocation from `Vec<RollingRow>` to a single tuple.

**Severity:** Low, but the fix is 3 lines of SQL.

---

## Index & Key Deficiencies (Post-Reassessment)

### 1. `managed_positions` — Missing Indexes

| Missing Index | Query Pattern |
|---|---|
| `(state) WHERE state != 'Closed'` | `load_active()` in rehydration + exchange verification |
| `(exchange_account_id)` | Rehydration loop collecting account IDs per position |

**Action:** Add both. These are the only genuinely missing indexes in the schema.

### 2. `cache_entries` — Missing `expires_at` Index

The only query hitting `expires_at` is `cleanup_expired()`. At current cache volumes this is inconsequential, but the btree on `expires_at` is cheap (~8 bytes per row + overhead) and prevents the table from becoming a seq-scan liability if cache usage grows.

**Action:** Add a simple btree.

### 3. Redundant Indexes (Safe to Drop)

| Redundant Index | Subsumed By |
|---|---|
| `idx_exchange_accounts_user_id (user_id)` | `idx_exchange_accounts_user_exchange_active (user_id, exchange_name, is_active)` — all user_id queries can use the composite |
| `idx_dignitas_history_user_date (user_id, date DESC)` | `UNIQUE(user_id, date)` — the unique constraint already creates a btree that supports `ORDER BY date DESC` |

**Impact:** Dropping these saves one btree write per INSERT on each table. Small but zero-risk.

### 4. `idx_journal_trades_user_setup` — Partial Index Mismatch

The partial index is on `(user_id, setup_tag) WHERE setup_tag IS NOT NULL`. The `setup_breakdown` query groups on:

```sql
COALESCE(NULLIF(LOWER(setup_tag), ''), '(untagged)')
```

This expression cannot match the index. Every `setup_breakdown` call force-scans all user trades. However, for per-user trade counts in the hundreds, the force-scan is a non-issue. The index still serves its purpose: `WHERE setup_tag IS NOT NULL` queries (like finding tagged trades) use it correctly.

**Verdict:** Accept the limitation. The index is useful for its intended purpose. The expression-mismatch is a non-problem at current scale.

---

## Tactical Remediation (Corrected — 5 Actions, 3 Specs)

> **Specs created:** [DBA-01-engine-tuning](.specify/specs/DBA-01-engine-tuning/spec.md) · [DBA-02-index-remediation](.specify/specs/DBA-02-index-remediation/spec.md) · [PERF-03-sql-side-minmax](.specify/specs/PERF-03-sql-side-minmax/spec.md)

### Action 1: Engine Tuning → Spec: DBA-01 ✅ (completed 2026-05-10)

Applied to dev Postgres (PG16, port 5000) via `ALTER SYSTEM` + container restart for `shared_buffers`.

| Parameter | Before | After |
|---|---|---|
| `shared_buffers` | 128 MB | **512 MB** |
| `work_mem` | 4 MB | **32 MB** |
| `random_page_cost` | 4.0 | **1.1** |
| `effective_cache_size` | 4 GB | **1 GB** |

Documented in `testudo-ops/postgres-db/config-map.yml` as comments for infra-as-code reference.

### Action 2: Add `managed_positions` Indexes → Spec: DBA-02 ✅ (completed 2026-05-10)

Applied to dev and prod. Maintenance scripts in `testudo-ops/postgres-db/dba-02_add_indexes.sql`.

```sql
CREATE INDEX CONCURRENTLY idx_managed_positions_state
    ON managed_positions(state) WHERE state != 'Closed';

CREATE INDEX CONCURRENTLY idx_managed_positions_account
    ON managed_positions(exchange_account_id);
```

### Action 3: Add `cache_entries` Expiry Index → Spec: DBA-02 ✅ (completed 2026-05-10)

Applied to dev and prod.

```sql
CREATE INDEX CONCURRENTLY idx_cache_expires
    ON cache_entries(expires_at);
```

### Action 4: Drop Redundant Indexes → Spec: DBA-02 ✅ (completed 2026-05-10)

Applied to dev and prod. Maintenance scripts in `testudo-ops/postgres-db/dba-02_drop_indexes.sql`. Verified via `EXPLAIN` — no queries reference these indexes by name.

```sql
DROP INDEX CONCURRENTLY IF EXISTS idx_exchange_accounts_user_id;
DROP INDEX CONCURRENTLY IF EXISTS idx_dignitas_history_user_date;
```

### Action 5: Fix `fetch_rolling_extremes` Wire Transfer → Spec: PERF-03

In `testudo-exchange/crates/router/src/services/journal_stats.rs`, replace the `fetch_rolling_extremes` method:

```rust
// Before: fetches all N rows, min/max in Rust
let rows = sqlx::query_as::<_, RollingRow>("SELECT SUM(net_pnl) OVER (...) ...")
    .fetch_all(&self.pool).await?;
let worst = rows.iter().filter_map(|r| r.rolling_pnl).min()...;
let best = rows.iter().filter_map(|r| r.rolling_pnl).max()...;

// After: push min/max to SQL, return 2 values
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
    ) sub"
)
.bind(user_id).bind(&filter.exchange).bind(filter.date_from)
.bind(filter.date_to).bind(window)
.fetch_one(&self.pool).await?;
let worst = row.0.unwrap_or(Decimal::ZERO);
let best = row.1.unwrap_or(Decimal::ZERO);
```

---

## What Was Rejected and Why (for Future Reference)

These are valid ideas for a different scale. File them under "revisit at 100x growth."

| Idea | Threshold to Revisit |
|---|---|
| UUID v7 PK migration | `journal_trades` exceeds 10M rows OR index bloat > 30% on `pg_stat_user_indexes` |
| Covering index on `journal_trades` | Per-user `journal_trades` exceeds 50K rows AND `EXPLAIN ANALYZE` shows heap fetches > 50% of total time |
| BRIN indexes | After `CLUSTER` on `closed_at` (requires maintenance window) AND table exceeds 1M rows |
| StatsEngine CTE consolidation | Dashboard P99 latency exceeds 200ms AND `EXPLAIN` confirms three separate seq scans are the bottleneck |
| PgBouncer | sqlx pool `acquire_timeout` errors appear in logs OR active connections exceed 80% of pool capacity |
| PG 12→16 upgrade | Scheduled as part of next major release cycle; coordinate with backup image unification |
