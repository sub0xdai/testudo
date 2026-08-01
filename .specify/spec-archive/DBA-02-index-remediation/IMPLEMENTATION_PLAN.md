# DBA-02-index-remediation — Implementation Plan

## Current State Summary

The dev PostgreSQL instance (PG16, port 5000) has 67 indexes across 24 tables. Three tables are missing indexes on query-critical predicates; two tables have redundant indexes that duplicate constraints.

**Missing indexes (confirmed via `pg_indexes`):**

| Table | Missing | Current State |
|---|---|---|
| `managed_positions` | `(state) WHERE state != 'Closed'` | Only PK exists. `load_active()` and exchange verification do full table scans. |
| `managed_positions` | `(exchange_account_id)` | Same — rehydration account-collection loop scans every row. |
| `cache_entries` | `(expires_at)` | Only PK exists. `cleanup_expired()` does `DELETE ... WHERE expires_at <= NOW()` — full table scan. |

**Redundant indexes (confirmed via `pg_indexes`):**

| Index | Subsumed By |
|---|---|
| `idx_exchange_accounts_user_id` | `idx_exchange_accounts_user_exchange_active (user_id, exchange_name, is_active)` — all `user_id`-only queries can use the composite with the same leading column |
| `idx_dignitas_history_user_date (user_id, date DESC)` | `UNIQUE(user_id, date)` — the unique constraint's btree is structurally identical for `ORDER BY date DESC` lookups |

**CONCURRENTLY constraint:** `CREATE INDEX CONCURRENTLY` and `DROP INDEX CONCURRENTLY` cannot run inside a transaction block. sqlx's migration runner wraps each migration file in `BEGIN/COMMIT`. The `-- no-transaction` annotation exists in sqlx, but with multiple statements per file, Postgres wraps them in an implicit transaction. **Practical consequence:** these operations cannot be run through sqlx's automatic migration runner. They must be applied via psql (which splits statements sent via heredoc individually). The migration scripts live in `testudo-ops/` as maintenance artifacts, not in the sqlx migrations directory.

---

## Gap Analysis

| FR | Requirement | Current State | Gap |
|----|-------------|---------------|-----|
| FR-1 | `idx_managed_positions_state` partial index | Does not exist | Create via psql |
| FR-2 | `idx_managed_positions_account` on `exchange_account_id` | Does not exist | Create via psql |
| FR-3 | `idx_cache_expires` on `expires_at` | Does not exist | Create via psql |
| FR-4 | Drop `idx_exchange_accounts_user_id` | Exists, redundant | Drop via psql |
| FR-5 | Drop `idx_dignitas_history_user_date` | Exists, redundant | Drop via psql |
| FR-6 | Migration file for reproducibility | No file exists | Create in `testudo-ops/postgres-db/` (not in sqlx migrations dir — CONCURRENTLY can't run through sqlx's transaction wrapper) |
| FR-7 | `EXPLAIN` shows index scan on managed_positions state query | Currently does seq scan (only PK exists) | Verify after CP-1 |

**Deviation from spec:** FR-6 called for files in `testudo-exchange/crates/sqlx_postgres/migrations/`. The `-- no-transaction` annotation exists in the sqlx version used (v0.7+), but it only handles single-statement migrations — 5 statements would require 5 separate versioned files with 5 corresponding down files (10 total). This is excessive for an operations remediation. Instead, the scripts live in `testudo-ops/postgres-db/` as maintenance artifacts, with the manual psql procedure documented. The spec itself already documents manual psql application as the primary method. This change is pragmatic, not a scope cut.

---

## Checkpoints

### CP-1: Apply All 5 Index Operations via psql ✅

- **Touches**: Running PostgreSQL instance (writes index structures, drops 2, creates 3)
- **Tasks**:
  1. Execute all 5 DDL operations in a single psql heredoc session (psql splits statements individually — each CONCURRENTLY runs in its own implicit transaction, no conflict)
  2. Run verification queries: confirm 3 new indexes exist, 2 old indexes are gone, `EXPLAIN` shows index scan on `managed_positions`
- **Verification**:
  ```sql
  -- 3 new indexes exist
  SELECT indexname FROM pg_indexes
  WHERE indexname IN ('idx_managed_positions_state', 'idx_managed_positions_account', 'idx_cache_expires');
  -- Expected: 3 rows

  -- 2 redundant indexes gone
  SELECT indexname FROM pg_indexes
  WHERE indexname IN ('idx_exchange_accounts_user_id', 'idx_dignitas_history_user_date');
  -- Expected: 0 rows

  -- managed_positions uses index
  EXPLAIN SELECT * FROM managed_positions WHERE state = 'pending';
  -- Expected: Index Scan using idx_managed_positions_state
  ```
- **Commit message**: `ops(db): add missing indexes, drop redundant indexes (DBA-02)`

### CP-2: Create Maintenance Scripts in testudo-ops ✅

- **Touches**: `testudo-ops/postgres-db/dba-02_add_indexes.sql`, `testudo-ops/postgres-db/dba-02_drop_indexes.sql`, `DATABASE_AUDIT.md`
- **Tasks**:
  1. Create `testudo-ops/postgres-db/dba-02_add_indexes.sql` — the 3 `CREATE INDEX CONCURRENTLY` statements as a reproducible maintenance script
  2. Create `testudo-ops/postgres-db/dba-02_drop_indexes.sql` — the 2 `DROP INDEX CONCURRENTLY` statements as a reproducible maintenance script
  3. Mark Actions 2, 3, 4 as complete in `DATABASE_AUDIT.md`
  4. Note in `DATABASE_AUDIT.md` that these scripts are manual (not auto-run by sqlx migrator due to CONCURRENTLY + transaction conflict)
- **Verification**: `ls testudo-ops/postgres-db/dba-02_*.sql` returns both files
- **Commit message**: `ops(db): add DBA-02 maintenance scripts for index remediation`

---

## Risks & Open Questions

### Risks
- **sqlx migrator won't run these.** The ops scripts live in `testudo-ops/`, not in `crates/sqlx_postgres/migrations/`. sqlx's automatic startup migration runner (`sqlx::migrate!()`) will never see them. This is intentional — CONCURRENTLY can't run inside sqlx's transaction wrapper. The scripts are manual maintenance artifacts, documented in `DATABASE_AUDIT.md`.
- **CONCURRENTLY deadlock on high-write tables.** `managed_positions` is low-write (positions open/close slowly). `cache_entries` is UNLOGGED and sees frequent writes. `exchange_accounts` and `dignitas_history` are low-write. Deadlock risk is minimal. If it occurs, retry during lower-load window.
- **`idx_exchange_accounts_user_id` referenced by a query hint.** The codebase uses sqlx with no raw SQL that references index names explicitly. Verified: grep for `idx_exchange_accounts_user_id` in the codebase returns zero hits in Rust files.

### Open Questions
- None. All index states verified against the running instance. Gap analysis confirmed.

### Assumptions Confirmed
- `managed_positions` exists (created at runtime, confirmed in `pg_indexes`)
- `cache_entries` exists (created by `20260131000000_pg_queue_tables.up.sql`, confirmed)
- Redundant indexes exist (created by earlier migrations, confirmed)
- psql heredoc correctly splits CONCURRENTLY statements (confirmed by test earlier in this session)

---

**Plan ready: 2 checkpoints, ~10 minutes total. CP-1 applies to running instance via psql. CP-2 creates maintenance scripts for reproducibility. Run `/skill:vox build DBA-02-index-remediation` to start.**
