# Specification: Index Remediation — Add Missing, Drop Redundant

**Spec ID:** DBA-02-index-remediation
**Date:** 2026-05-10
**Status:** Draft
**Class:** Operations / Database (DDL)
**Priority:** P2 — Fills genuine gaps in `managed_positions` and `cache_entries`; removes write-path overhead from two redundant indexes
**Depends on:** DBA-01-engine-tuning (not a hard dependency; can run independently)
**Source:** DATABASE_AUDIT.md — Actions 2, 3, 4

---

## Problem Statement

Three index gaps and two redundant indexes were identified in the database audit:

**Gaps (add):**
| Table | Missing Index | Query Impacted |
|---|---|---|
| `managed_positions` | `state` (partial, `WHERE state != 'Closed'`) | `load_active()` at startup rehydration — currently a full table scan |
| `managed_positions` | `exchange_account_id` | Rehydration loop collecting account IDs; exchange verification |
| `cache_entries` | `expires_at` | `cleanup_expired()` — currently a full sequential scan on every cleanup cycle |

**Redundancies (drop):**
| Redundant Index | Subsumed By |
|---|---|
| `idx_exchange_accounts_user_id (user_id)` | `idx_exchange_accounts_user_exchange_active (user_id, exchange_name, is_active)` |
| `idx_dignitas_history_user_date (user_id, date DESC)` | `UNIQUE(user_id, date)` — the unique constraint's btree already supports `ORDER BY date DESC` |

All five operations are `CREATE INDEX CONCURRENTLY` / `DROP INDEX CONCURRENTLY` — they do not block writes. They can be batched into a single migration file and run in one maintenance window.

---

## User Stories

- As a **backend operator restarting the service**, I want rehydration to use an index on active positions rather than scanning all rows in `managed_positions`, so startup time stays fast as position history accumulates.
- As a **backend operator**, I want cache cleanup to target only expired entries via an index, so the `cache_entries` table doesn't accumulate stale rows that degrade read performance.
- As a **database maintainer**, I want to eliminate indexes that duplicate existing constraints, so every INSERT avoids unnecessary btree maintenance overhead.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | Create partial index on `managed_positions(state) WHERE state != 'Closed'` using `CONCURRENTLY` | High |
| FR-2 | Create index on `managed_positions(exchange_account_id)` using `CONCURRENTLY` | High |
| FR-3 | Create index on `cache_entries(expires_at)` using `CONCURRENTLY` | Medium |
| FR-4 | Drop `idx_exchange_accounts_user_id` using `CONCURRENTLY` | Medium |
| FR-5 | Drop `idx_dignitas_history_user_date` using `CONCURRENTLY` | Medium |
| FR-6 | All DDL batched into one migration file named `20260510000000_db_audit_indexes.up.sql` with corresponding `.down.sql` for rollback | High |
| FR-7 | Service restart after migration — rehydration completes without full table scan | Medium |

---

## Acceptance Criteria

- [ ] Migration `20260510000000_db_audit_indexes` exists with `.up.sql` and `.down.sql`
- [ ] `CREATE INDEX CONCURRENTLY` on `managed_positions(state) WHERE state != 'Closed'` succeeds
- [ ] `CREATE INDEX CONCURRENTLY` on `managed_positions(exchange_account_id)` succeeds
- [ ] `CREATE INDEX CONCURRENTLY` on `cache_entries(expires_at)` succeeds
- [ ] `DROP INDEX CONCURRENTLY idx_exchange_accounts_user_id` succeeds
- [ ] `DROP INDEX CONCURRENTLY idx_dignitas_history_user_date` succeeds
- [ ] Down migration restores all dropped indexes and removes all added indexes
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes (migrations are applied during test setup)
- [ ] `pg_stat_user_indexes` confirms the 3 new indexes exist and the 2 dropped indexes are gone
- [ ] `EXPLAIN` on `SELECT * FROM managed_positions WHERE state = 'pending'` shows an index scan (not seq scan) after migration
- [ ] `DATABASE_AUDIT.md` updated to mark Actions 2, 3, 4 as complete

---

## Technical Notes

### Files to Create
- `testudo-exchange/crates/sqlx_postgres/migrations/20260510000000_db_audit_indexes.up.sql`
- `testudo-exchange/crates/sqlx_postgres/migrations/20260510000000_db_audit_indexes.down.sql`

### Migration: `20260510000000_db_audit_indexes.up.sql`

```sql
-- DBA-02: Index remediation — add missing, drop redundant.
-- All operations use CONCURRENTLY to avoid blocking writes.
-- Run during low-traffic window; CONCURRENTLY cannot execute inside a transaction.

-- Add missing indexes
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_managed_positions_state
    ON managed_positions(state) WHERE state != 'Closed';

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_managed_positions_account
    ON managed_positions(exchange_account_id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_cache_expires
    ON cache_entries(expires_at);

-- Drop redundant indexes
DROP INDEX CONCURRENTLY IF EXISTS idx_exchange_accounts_user_id;
DROP INDEX CONCURRENTLY IF EXISTS idx_dignitas_history_user_date;
```

### Migration: `20260510000000_db_audit_indexes.down.sql`

```sql
-- Rollback: drop added indexes, restore dropped indexes

DROP INDEX CONCURRENTLY IF EXISTS idx_managed_positions_state;
DROP INDEX CONCURRENTLY IF EXISTS idx_managed_positions_account;
DROP INDEX CONCURRENTLY IF EXISTS idx_cache_expires;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_exchange_accounts_user_id
    ON exchange_accounts(user_id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_dignitas_history_user_date
    ON dignitas_history(user_id, date DESC);
```

### ⚠️ Critical Constraint

`CREATE INDEX CONCURRENTLY` and `DROP INDEX CONCURRENTLY` **cannot run inside a transaction block.** sqlx runs each migration file in an implicit transaction by default. This migration must be applied manually or via a tool that supports non-transactional migrations:

```bash
# Apply manually (each statement independently):
psql $DATABASE_URL -c "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_managed_positions_state ON managed_positions(state) WHERE state != 'Closed';"
psql $DATABASE_URL -c "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_managed_positions_account ON managed_positions(exchange_account_id);"
psql $DATABASE_URL -c "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_cache_expires ON cache_entries(expires_at);"
psql $DATABASE_URL -c "DROP INDEX CONCURRENTLY IF EXISTS idx_exchange_accounts_user_id;"
psql $DATABASE_URL -c "DROP INDEX CONCURRENTLY IF EXISTS idx_dignitas_history_user_date;"
```

Alternatively, if sqlx supports `--no-transaction` for specific migrations, use that flag. If neither path is feasible, split the CONCURRENTLY operations into individual non-transactional scripts in `testudo-ops/` and document the manual apply procedure.

### Verification Queries

```sql
-- Verify new indexes exist
SELECT indexname, tablename FROM pg_indexes
WHERE indexname IN (
    'idx_managed_positions_state',
    'idx_managed_positions_account',
    'idx_cache_expires'
);

-- Verify dropped indexes are gone
SELECT indexname FROM pg_indexes
WHERE indexname IN (
    'idx_exchange_accounts_user_id',
    'idx_dignitas_history_user_date'
);
-- Expected: 0 rows

-- Verify managed_positions uses index for state filter
EXPLAIN SELECT * FROM managed_positions WHERE state = 'pending';
-- Expected: Index Scan using idx_managed_positions_state (not Seq Scan)
```

### Dependencies
- PostgreSQL 12+ (CONCURRENTLY supported since 8.2; partial indexes since 7.4)
- `managed_positions` table must exist (created at startup by `PositionRepository::create_table`)

### Assumptions
- `managed_positions` table exists (it's created by application code, not by migration — see `trade_manager/repository.rs:29`)
- `cache_entries` table exists (created by migration `20260131000000_pg_queue_tables.up.sql`)
- All referenced indexes exist before `DROP INDEX CONCURRENTLY` (they do — they were created in earlier migrations)

### Risks
- **Low.** `CONCURRENTLY` operations read the table twice and may fail with deadlock under very high write load. If deadlocked, retry during lower-traffic window.
- **Low.** Dropping `idx_dignitas_history_user_date` is safe because `UNIQUE(user_id, date)` creates an identical btree. Verify with `EXPLAIN` before dropping.
- **Low.** Dropping `idx_exchange_accounts_user_id` is safe because `idx_exchange_accounts_user_exchange_active(user_id, exchange_name, is_active)` starts with `user_id` as the leading column. Verify no query uses a hint or explicit index reference to `idx_exchange_accounts_user_id`.

---

## Completion Signal

### Implementation Checklist
- [ ] Up and down migration files created
- [ ] CONCURRENTLY constraint documented and manual apply procedure ready
- [ ] All 5 index operations executed successfully (manual or via migration tool)
- [ ] Verification queries confirm 3 new + 2 removed
- [ ] `EXPLAIN` shows index scan on managed_positions state filter
- [ ] Service restarts cleanly; rehydration reads from index
- [ ] `DATABASE_AUDIT.md` updated

### Done Signal
```
<promise>DONE</promise>
```
