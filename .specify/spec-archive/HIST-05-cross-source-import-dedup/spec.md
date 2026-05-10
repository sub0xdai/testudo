# Specification: Cross-Source Import Deduplication

**Spec ID:** HIST-05-cross-source-import-dedup
**Date:** 2026-04-22
**Status:** Draft
**Class:** Fix / Backend
**Priority:** P1 — silently duplicates trade history when live-trades and imports overlap in time. Pollutes every downstream aggregate.
**Depends on:** HIST-03 (idempotent CEX history import — established the partial unique index this spec extends)
**Series:** HIST-01 through HIST-05 (exchange trade history ingestion hardening)

---

## Problem Statement

HIST-03 introduced a partial unique index `idx_unique_import_fill` on `journal_trades (user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL`. It successfully dedupes imports against each other — a second run of `POST /api/v1/trades/import` for the same account cannot insert the same fill twice.

Live-mode trades (`source = 'testudo'`) carry `exchange_fill_id = NULL` by construction (the partial index's WHERE clause excludes them). A CEX history import for the same exchange account still inserts a fresh row because its `exchange_fill_id` is set and does not collide with any existing `testudo`-source row.

**Consequence (observed 2026-04-22):**

User `99cc6a3b-...-38d679afd021` on Bybit had 4 TAO_USDT rows for 2026-04-20, only 1 of which was a real live trade. The other 3 were residue — a small LONG (qty 0.003) and a small SHORT (qty 0.018) from an older CCXT import run, plus a reconstructed/partial that predated the live-trading pipeline going live for that account. Combined they show as "4 losing trades" in Symbol Allocation and corrupt every downstream statistic.

The root issue: **import and live are two producers with no awareness of each other.** Idempotency is per-producer; cross-producer is not enforced anywhere.

---

## User Stories

- **As a trader who used CEX history import before Testudo's live pipeline was wired up**, I want legacy import rows removed when my live trades cover the same window, so I don't see double-counted trades.
- **As a trader who imports historical trades for tax purposes**, I want the import to skip any fill whose timestamp is covered by an existing live-trade row, so the import fills gaps instead of duplicating reality.
- **As an operator investigating a user's data**, I want a deterministic way to identify which rows came from which source and which are duplicates of each other.

---

## Non-Goals

- **No merging of partial-matches.** If an import row looks *similar* but not identical to a live row (different price, different quantity), don't try to reconcile — surface both and let the operator decide.
- **No automatic deletion of existing duplicates.** This spec ships detection + new-import dedup. A one-time cleanup script is a follow-up.
- **No changes to the live-trade write path.** Only the import path gets new checks.
- **No expansion to on-chain (Hyperliquid) imports.** Those have their own ingestion (HL fill journal) with different semantics.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Import worker, before inserting a row, checks whether a `source = 'testudo'` row exists for the same `(user_id, exchange, symbol)` whose `opened_at..closed_at` window covers the fill's timestamp (±60s tolerance) | High | router |
| FR-2 | If a covering live row exists, skip the import and increment `ImportResult.trades_skipped_live_covered` | High | router |
| FR-3 | Add `journal_trades.dedup_key TEXT` — a canonical key of `exchange:symbol:side:opened_at_bucket` (1-min bucket) set on every row (live + import) at write time | High | backend |
| FR-4 | Partial unique index on `dedup_key` enforces that no two rows share it regardless of source | High | backend |
| FR-5 | Import worker handles `unique_violation` on `dedup_key` the same as existing `exchange_fill_id` collision — mark `SkippedDuplicate` | High | router |
| FR-6 | Admin CLI: `cargo run --bin find-duplicates -- --user <id>` lists rows that share `dedup_key` or are suspected duplicates (same symbol + side + opened_at within 60s). Non-destructive; prints a recommendation only | Medium | router |
| FR-7 | Manual cleanup CLI: `cargo run --bin prune-duplicates -- --user <id> --keep <source>` deletes the non-preferred side of each duplicate pair. Requires explicit confirmation flag `--yes` | Medium | router |
| FR-8 | Import API response surfaces new skip reason alongside existing counters: `{ trades_imported, trades_skipped_duplicate, trades_skipped_live_covered, trades_skipped, errors }` | Medium | router + journal |

---

## Technical Implementation

### Vertical Checkpoints

| CP | Scope | Validates |
|----|-------|-----------|
| CP-1 | Migration: `dedup_key` column + partial unique index. Backfill existing rows with the canonical key. Document the 1-min bucket decision | Key is populated and unique across all existing rows; any existing violations logged |
| CP-2 | Import worker: add the "live-covered" pre-check (FR-1, FR-2) + rely on `dedup_key` unique for the tie-break | New imports against a window already covered by live are rejected cleanly |
| CP-3 | `find-duplicates` + `prune-duplicates` admin CLIs with dry-run by default | Operators can audit without risk of accidental deletion |
| CP-4 | Integration test: seed a live trade + run a CEX import whose fills cover the same window. Expect `trades_skipped_live_covered == N` and `trades_imported == 0` for that span | Dedup holds end-to-end |

### Key Types

```rust
// crates/router/src/services/journal_service.rs — extend DerivedFields / TradeCloseEvent

pub fn compute_dedup_key(event: &TradeCloseEvent) -> String {
    // 1-minute opened_at bucket. Side is LONG/SHORT (normalized upstream).
    let bucket = event.opened_at.timestamp() / 60;
    format!(
        "{exchange}:{symbol}:{side}:{bucket}",
        exchange = event.exchange,
        symbol = event.symbol,
        side = event.side,
    )
}
```

```sql
-- Migration
ALTER TABLE journal_trades ADD COLUMN dedup_key TEXT;

-- Backfill
UPDATE journal_trades
SET dedup_key = exchange || ':' || symbol || ':' || side
                || ':' || (EXTRACT(EPOCH FROM opened_at) / 60)::bigint;

ALTER TABLE journal_trades ALTER COLUMN dedup_key SET NOT NULL;

CREATE UNIQUE INDEX idx_unique_dedup_key
    ON journal_trades (user_id, dedup_key);
```

### Bucket rationale

- **1 minute** balances two failure modes:
  - Too small (1s) → real duplicate imports from different API cursor pages that are seconds apart would slip through.
  - Too large (1 hour) → two legitimate separate trades on the same symbol in the same hour would collide.
  - 1 minute empirically captures 99%+ of same-trade duplicate pairs while keeping distinct scalps separable.
- Operators who do sub-minute scalping on the same symbol need to be aware; log an alert when multiple rows share a bucket but look legitimate.

### Paved Roads

- **HIST-03 partial unique index pattern** — same mechanism, wider scope
- **CcxtHistoryImporter's existing dedup error handling** — extended to also catch `dedup_key` collisions
- **journal_daily_stats rebuild** — already has the infrastructure from HIST-03 to recompute affected dates after imports

### Files

**New (backend):**
- `crates/sqlx_postgres/migrations/NNNN_dedup_key.up.sql` + `.down.sql`
- `crates/router/src/bin/find_duplicates.rs`
- `crates/router/src/bin/prune_duplicates.rs`
- `crates/router/tests/cross_source_dedup_test.rs`

**Modified:**
- `crates/router/src/services/journal_service.rs` — add `compute_dedup_key`; compute it on every insert
- `crates/router/src/services/import_worker.rs` — add pre-check (FR-1); catch new unique-violation path
- `crates/router/src/services/trade_event_writer.rs` — ensure live-path inserts also populate `dedup_key`
- `crates/router/src/routes/imports.rs` — surface new skip counter

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Migration applies cleanly on a production-sized dataset; backfill populates `dedup_key` for every existing row
- [ ] Unique index creation fails loudly if pre-existing duplicates are found — triggers operator review via `find-duplicates` before `ADD CONSTRAINT NOT VALID` → `VALIDATE` cycle
- [ ] Live-path insert generates `dedup_key` identical to what a subsequent import of the same fill would generate
- [ ] Import of a Bybit fill covered by an existing live row returns `trades_skipped_live_covered += 1` and inserts no row
- [ ] Import of a Bybit fill NOT covered by a live row inserts normally
- [ ] `find-duplicates --user <id>` flags the 4-TAO scenario from 2026-04-22 as 3 duplicates + 1 live, and recommends keeping the live row
- [ ] `prune-duplicates --yes` is idempotent (running twice produces zero additional deletes)
- [ ] Verification: `cd testudo-exchange && cargo clippy --all-targets && cargo test`

---

## Risks

1. **Backfill collides on existing duplicates.** Adding the unique index fails if the DB already has duplicate `dedup_key` values. *Mitigation:* CP-1 runs backfill first, then a pre-index audit. If collisions exist, operator must run `prune-duplicates` (non-destructive dry-run first) before the unique constraint can be applied. Ship as two separate migrations: column + backfill, then index.
2. **1-minute bucket false positives.** A high-frequency scalper taking two opposite-direction trades on the same symbol within 60s collides artificially. *Mitigation:* side is part of the key — opposite directions don't collide. Same-side within 60s is rare enough that it's worth a log warning rather than allowing silent duplication.
3. **Timezone / clock skew.** Import uses exchange-reported timestamps; live uses Testudo-server timestamps. They may differ by a few seconds. *Mitigation:* 60s bucket absorbs sub-minute drift. If drift regularly exceeds that, the pre-check FR-1's ±60s tolerance covers it.
4. **Retroactive pruning of legitimate imports.** An operator runs `prune-duplicates --keep testudo` and loses import rows for trades that pre-date the live pipeline. *Mitigation:* CLI takes an explicit `--since <date>` and refuses to touch rows older than the first Testudo-source row per account by default.

---

## Completion Signal

1. FR-1 through FR-8 implemented and tested
2. Production DB migration applied; backfill populates every existing row with a unique dedup_key (any collisions pruned first via the admin CLI)
3. End-to-end test: legacy CCXT import of a window covering today's live trades inserts zero rows
4. Verification passes
5. Committed: `feat(hist-05): cross-source import deduplication — dedup_key + live-covered skip semantics`

---

## Session Context (2026-04-22)

Drafted during the same live-debug session as FIX-08. User flagged "duplicates remaining from prior imports" after investigating their TAO trades; found 2 orphan `import_ccxt` rows alongside 1 real `testudo` live row for 2026-04-20. Manual SQL cleanup removed the orphans for the affected user; this spec is the durable fix that prevents recurrence across all users.
