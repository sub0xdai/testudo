# Specification: PostgreSQL Engine Tuning

**Spec ID:** DBA-01-engine-tuning
**Date:** 2026-05-10
**Status:** Draft
**Class:** Operations / Infrastructure (Database)
**Priority:** P1 — Zero-risk, zero-code, prevents silent degradation as data grows
**Depends on:** None
**Source:** DATABASE_AUDIT.md — Action 1

---

## Problem Statement

The PostgreSQL 12.2 deployment has zero custom runtime parameters. It runs on container defaults:

| Parameter | Default | Optimal (2 Gi container, SSD PVC) |
|---|---|---|
| `shared_buffers` | 128 MB | 512 MB (25% of RAM) |
| `work_mem` | 4 MB | 32 MB |
| `random_page_cost` | 4.0 (HDD) | 1.1 (SSD) |
| `effective_cache_size` | 4 GB | 1 GB |

With default `work_mem=4MB`, aggregate queries that exceed 4 MB of working memory spill to disk. Per-user `journal_trades` data is small today (hundreds of rows), so this is not a current bottleneck. However, as trade histories grow and concurrent dashboard loads increase, the disk-spill threshold will be hit silently. `random_page_cost=4.0` causes the planner to prefer sequential scans when index scans would be faster on SSD-backed PVC — this is a latent planner bias present on every query today.

These four parameters can be set via `ALTER SYSTEM` with a `pg_reload_conf()` — no restart required, no migration, no code change. Rollback is instant.

---

## User Stories

- As a **user loading the journal dashboard**, I want aggregate queries to stay in memory rather than spilling to disk as my trade history grows, so dashboard load times remain consistent over years of use.
- As a **backend operator**, I want the PostgreSQL planner to make cost decisions appropriate for SSD storage, so index scans are chosen when they're actually faster than sequential scans.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | Set `shared_buffers = '512MB'` (25% of container RAM) | High |
| FR-2 | Set `work_mem = '32MB'` (8× default; prevents disk spills for per-user aggregates up to ~5K rows) | High |
| FR-3 | Set `random_page_cost = 1.1` (SSD-appropriate; was 4.0 = HDD) | High |
| FR-4 | Set `effective_cache_size = '1GB'` (let planner account for OS page cache) | Medium |
| FR-5 | Apply via `ALTER SYSTEM` + `pg_reload_conf()` — no restart | High |
| FR-6 | Document the parameters in `testudo-ops/postgres-db/config-map.yml` as comments for infrastructure-as-code reproducibility | Medium |

---

## Acceptance Criteria

- [ ] `SHOW shared_buffers` returns `512MB` after apply
- [ ] `SHOW work_mem` returns `32MB` after apply
- [ ] `SHOW random_page_cost` returns `1.1` after apply
- [ ] `SHOW effective_cache_size` returns `1GB` after apply
- [ ] `pg_reload_conf()` completes without error
- [ ] Journal dashboard loads with no regression in response time (subjective check)
- [ ] `DATABASE_AUDIT.md` updated to mark Action 1 as complete

---

## Technical Notes

### Files to Modify
- `testudo-ops/postgres-db/config-map.yml` — add comments documenting the tuned parameters (the actual values live in `postgresql.auto.conf` after `ALTER SYSTEM`, not in the ConfigMap)
- `DATABASE_AUDIT.md` — mark Action 1 as done

### No Files to Create
- No migration file — these are runtime parameters, not schema changes

### Implementation

Connect to the running PostgreSQL instance and execute:

```sql
ALTER SYSTEM SET shared_buffers = '512MB';
ALTER SYSTEM SET work_mem = '32MB';
ALTER SYSTEM SET random_page_cost = 1.1;
ALTER SYSTEM SET effective_cache_size = '1GB';
SELECT pg_reload_conf();
```

Verify:

```sql
SELECT name, setting, unit, context
FROM pg_settings
WHERE name IN ('shared_buffers', 'work_mem', 'random_page_cost', 'effective_cache_size');
```

### Rollback

```sql
ALTER SYSTEM RESET shared_buffers;
ALTER SYSTEM RESET work_mem;
ALTER SYSTEM RESET random_page_cost;
ALTER SYSTEM RESET effective_cache_size;
SELECT pg_reload_conf();
```

### Dependencies
- Access to the running PostgreSQL instance (kubectl exec or equivalent)

### Assumptions
- Container has ≥ 2 Gi RAM (shared_buffers=512MB is 25%)
- PVC backing storage is SSD (random_page_cost=1.1 is SSD-appropriate; if spinning disk, keep at 4.0)
- The `ALTER SYSTEM` write path (`$PGDATA/postgresql.auto.conf`) is writable in the container

### Risks
- **None.** These are safe, conservative values. `shared_buffers=512MB` is well within typical container memory limits. `work_mem=32MB` is 8× default but still conservative (each query node can use up to `work_mem`; with 4 parallel workers, peak is 4 × 32 = 128 MB per query). If container memory is below 1 Gi, scale `shared_buffers` down proportionally.

---

## Completion Signal

### Implementation Checklist
- [ ] All 4 parameters applied via `ALTER SYSTEM`
- [ ] `pg_reload_conf()` executed
- [ ] `SHOW` verification confirms all 4 values
- [ ] Journal dashboard loads normally
- [ ] ConfigMap documentation updated

### Done Signal
```
<promise>DONE</promise>
```
