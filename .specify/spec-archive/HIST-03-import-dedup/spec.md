# Specification: CEX History Import — Idempotent Dedup

**Spec ID:** HIST-03-import-dedup
**Date:** 2026-04-20
**Status:** Draft
**Class:** Fix / Data Integrity
**Priority:** P0 — blocks QNT-01c; any re-run of the Bybit/Binance history importer creates duplicate `journal_trades` rows, poisoning analytics and the Dignitas score inputs.
**Depends on:** HIST-01 (added `exchange_fill_id` + partial unique index — shipped), HIST-02 (CCXT+REST history import — shipped)
**Series:** HIST-03 (standalone fix on top of HIST-01/02)

---

## Problem Statement

HIST-01 added a partial unique index precisely to prevent duplicate imports:

```sql
CREATE UNIQUE INDEX idx_unique_import_fill
    ON journal_trades(user_id, exchange, exchange_fill_id)
    WHERE exchange_fill_id IS NOT NULL;
```

HIST-02 populates `exchange_fill_id` on import (`import_worker.rs:386, 453`) and stamps `source = "import_ccxt"`. The dedup mechanism is **nominally in place**. But observed behavior (2026-04-20): re-running the Bybit history import pulls previously-journaled trades back in as new rows. Every run of the importer for a window that overlaps with prior imports generates duplicates.

Root cause hypotheses (to verify during build — do NOT assume):

1. **Missing `ON CONFLICT DO NOTHING`** — `journal_service.rs:184-224` issues a bare `INSERT INTO journal_trades ... RETURNING ...`. On partial-index collision, Postgres raises `23505` unique-violation. If the import path catches that error and logs-and-continues, that *is* dedup — but the reported behavior is duplicates appearing, implying either the error isn't being raised or the path silently swallows then retries with different inputs.
2. **`exchange_fill_id` fallback produces different keys on re-run** — `import_worker.rs:386`: `exchange_fill_id: Some(fill.id.parse::<i64>().unwrap_or(fill.timestamp as i64))`. If a Bybit fill's ID is non-numeric (e.g., UUID string), the first import falls back to `timestamp`; a later import on the same fill falls back to the same timestamp. Probably OK — but if any fills have parseable IDs in one import run and not another (e.g., Bybit v5 vs legacy endpoints returning different shapes), the key flips and the index misses.
3. **`exchange` column casing/format drift** — partial index is on `(user_id, exchange, exchange_fill_id)`. If imports stamp `exchange` as `"bybit"` in one path and `"Bybit"` in another (CEX-04 normalization was supposed to fix this), the composite key doesn't collide. `FIX-04` in the HL series was explicitly about exchange-name unification; the importer may predate it.
4. **`ON CONFLICT` missing on the INSERT AND the `record_trade_close` path doesn't distinguish import vs live** — the same function is used for both. Live trades have no `exchange_fill_id` (`None`), so they hit the partial index predicate `WHERE exchange_fill_id IS NOT NULL` → never collide. Correct. Imports always set it → should collide. If they don't, it's #2 or #3.

This spec ships a minimal, idempotent import path: INSERT uses explicit `ON CONFLICT DO NOTHING` keyed on the existing partial index, the caller distinguishes "inserted" from "skipped", and the importer's `trades_imported` counter reflects only genuinely new rows. Plus a pre-build diagnostic to confirm which hypothesis is actually firing so the fix is surgical.

---

## User Stories

- **As a user running the Bybit history importer**, I want re-running it to reconcile new trades without creating duplicates of trades already journaled, so that my journal and Dignitas score inputs stay clean.
- **As a developer triaging a stuck/orphaned live trade** (e.g., today's TAO short before the QNT-01a fix), I want to rely on the importer as an idempotent recovery mechanism, so that pulling from exchange history is always safe to invoke.
- **As the Dignitas / coach pipeline**, I want `journal_trades` to contain exactly one row per closed exchange fill, so that P&L, win-rate, and R-multiple aggregates are correct.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `record_trade_close` INSERT appends `ON CONFLICT (user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL DO NOTHING` (matching the partial index shape) | High | `journal_service.rs` |
| FR-2 | `record_trade_close` returns a typed `RecordOutcome::{Inserted(JournalTrade), SkippedDuplicate}` so callers can distinguish new rows from idempotent no-ops | High | `journal_service.rs` |
| FR-3 | `import_worker.rs` increments `trades_imported` only on `Inserted`; `SkippedDuplicate` increments a new `trades_skipped_duplicate` counter | High | `import_worker.rs` |
| FR-4 | `exchange` column is normalized to lowercase at all INSERT sites via a single `canonical_exchange_name()` helper (consistent with CEX-07 / FIX-04 patterns) | High | `import_worker.rs` + live path |
| FR-5 | `exchange_fill_id` fallback (`unwrap_or(timestamp)`) is removed — if the fill has no parseable numeric ID, the importer logs and skips rather than risk a timestamp collision across retries | Medium | `import_worker.rs` |
| FR-6 | Idempotency test: running the importer twice on the same window produces byte-identical `journal_trades` row count after both runs; second run reports `trades_imported = 0` and `trades_skipped_duplicate = N` | High | Integration test |
| FR-7 | Live trades (source != import) remain unaffected — `exchange_fill_id` stays NULL for live trades and the partial index predicate skips them, preserving existing behavior | High | Regression guard |

---

## Technical Implementation

### Vertical Checkpoints

| CP | Scope | Validates |
|----|-------|-----------|
| **CP-0 (diagnostic)** | Query the user's prod DB: `SELECT user_id, exchange, COUNT(*) FROM journal_trades WHERE exchange_fill_id IS NOT NULL GROUP BY user_id, exchange, exchange_fill_id HAVING COUNT(*) > 1 LIMIT 20;`. Inspect duplicate rows for case-mismatched `exchange` values or drifted `exchange_fill_id`. This locks which hypothesis (#1-#4) is actually firing before writing any fix code. | Empirical root cause. |
| **CP-1** | FR-1 + FR-2: INSERT uses `ON CONFLICT DO NOTHING`; `record_trade_close` returns `RecordOutcome`. Live path (`fill_detector.rs`, `trade_event_writer.rs`) updated to match the new return type — trivial because live trades have `exchange_fill_id = None` and always hit `Inserted`. | Duplicates cannot be created at the DB layer regardless of what the importer does. Live trades still journal. |
| **CP-2** | FR-3 + FR-4: importer counters reflect genuinely new inserts; exchange-name normalization applied at all INSERT sites. | Importer reports accurate counts; no casing drift can defeat the partial index. |
| **CP-3** | FR-5: remove the `timestamp as i64` fallback; log+skip non-numeric fill IDs. | No synthetic `exchange_fill_id` values can mask as genuine fill IDs. |
| **CP-4** | FR-6: integration test that re-runs the importer on a fixture window twice; asserts row-count stability + counter shape. | Idempotency is structurally guaranteed, not just expected. |

### Key Types

```rust
// journal_service.rs
pub enum RecordOutcome {
    Inserted(JournalTrade),
    SkippedDuplicate,   // partial unique index collision
}

impl JournalService {
    pub async fn record_trade_close(&self, event: TradeCloseEvent) -> Result<RecordOutcome, Error>;
}
```

### SQL Change

```sql
-- Before
INSERT INTO journal_trades (...) VALUES (...) RETURNING ...;

-- After
INSERT INTO journal_trades (...) VALUES (...)
  ON CONFLICT (user_id, exchange, exchange_fill_id)
    WHERE exchange_fill_id IS NOT NULL
  DO NOTHING
  RETURNING ...;
-- If no row returned → SkippedDuplicate
```

**Important:** the `ON CONFLICT` target must match the partial index predicate exactly, or Postgres rejects the clause. The `WHERE exchange_fill_id IS NOT NULL` qualifier is load-bearing.

### Paved Roads

- **HIST-01 partial unique index** — reused verbatim; no migration needed.
- **`canonical_exchange_name()`** — if one already exists in `common_utils` or `router/services/exchange_api.rs` post-FIX-04, reuse it; otherwise add it in one place (the spec doesn't add competing helpers).
- **`TradeCloseEvent`** — structure stays the same; only the record return type changes.

### Files

- `crates/router/src/services/journal_service.rs` — MODIFIED. Add `ON CONFLICT` clause, add `RecordOutcome` enum, convert handlers to return it, use `fetch_optional` instead of `fetch_one`.
- `crates/router/src/services/import_worker.rs` — MODIFIED. Consume `RecordOutcome` variants, add `trades_skipped_duplicate` counter, remove timestamp fallback on `exchange_fill_id`, apply `canonical_exchange_name()`.
- `crates/router/src/services/fill_detector.rs` — MODIFIED. Minimal: update to the new return type (live trades always land as `Inserted`).
- `crates/router/src/services/trade_event_writer.rs` — MODIFIED. Same as above if it writes closes.
- `crates/router/src/services/cex_history.rs` — MODIFIED if it writes directly; verify during CP-0.
- `crates/router/src/services/tests/journal_service_idempotency.rs` — NEW. Integration test for FR-6.

### No Migration

The partial unique index already exists (HIST-01). No schema change.

---

## Acceptance Criteria

- [ ] CP-0 diagnostic run on the user's DB; dominant failure mode identified and documented in the commit message.
- [ ] Running `POST /api/v1/imports/cex/bybit` twice on the same time window produces:
  - First run: `trades_imported = N, trades_skipped_duplicate = 0`, `COUNT(*) FROM journal_trades WHERE source = 'import_ccxt'` increases by N.
  - Second run: `trades_imported = 0, trades_skipped_duplicate = N`, `COUNT(*)` unchanged.
- [ ] Live Bybit round-trip (submit via Alt+X → fill → close via TP/SL → journal row) still populates `journal_trades` with `source = 'testudo'`, `exchange_fill_id = NULL`. No regression on the live path.
- [ ] A live trade followed by an importer run that includes that trade's time window does NOT create a duplicate — the live row (`exchange_fill_id = NULL`) and any import attempt for the same fill stay distinct because the partial index only applies when `exchange_fill_id IS NOT NULL`. (Open question: should imports check for an existing live row by `trade_group_id` or `exchange_order_ids` and skip? Document the decision during CP-1.)
- [ ] `cargo clippy --all-targets && cargo test` passes (including the new idempotency test).
- [ ] Extension `bun run typecheck` passes (no wire-shape changes expected, but verify).

---

## Risks

1. **`ON CONFLICT` target predicate mismatch.** Postgres requires the `ON CONFLICT` predicate to match the partial index predicate exactly. A syntax slip = runtime error on every import INSERT. *Mitigation:* integration test at CP-1 covers the happy path; also explicit `EXPLAIN` in the test to confirm the partial index is used.
2. **Live path coupling.** `record_trade_close` is shared by the live trade close pathway AND the importer. Changing its return type affects both. *Mitigation:* CP-1 updates all call sites in one commit; the live call sites always land `Inserted` (never duplicate), so behavior is unchanged — the type change is mechanical.
3. **Removing the timestamp fallback (FR-5) could regress live-recovery imports.** If Bybit's REST history sometimes returns fills without numeric IDs, we'd stop importing them. *Mitigation:* log at WARN with the full fill payload so we can see if this happens in the wild; if it does, introduce a separate dedup key (e.g., `timestamp + symbol + qty`) in a follow-up rather than collapsing into `timestamp`.
4. **Existing duplicate rows already in the user's DB.** This spec prevents FUTURE duplicates but does not clean up the current mess. *Mitigation:* include a one-shot SQL script in the commit body (not a migration) for the user to manually dedupe existing rows after deploy: `DELETE FROM journal_trades a USING journal_trades b WHERE a.id > b.id AND a.user_id = b.user_id AND a.exchange = b.exchange AND a.exchange_fill_id = b.exchange_fill_id AND a.exchange_fill_id IS NOT NULL;`.
5. **Exchange-name canonicalization could collide with existing data** if some rows stamp `"bybit"` and others `"Bybit"`. *Mitigation:* CP-0 diagnostic detects this; if present, fix-up SQL included in commit body.

---

## Out of Scope

- Backfilling `exchange_fill_id` for historical live trades (they stay NULL by design).
- Cross-validating imported trades against existing live trades by `exchange_order_id` — tempting but adds complexity; revisit only if FR-7 regression fires.
- Supporting a "re-import" CLI that intentionally overwrites (UPDATE on conflict) — not needed; the importer's job is pure accretion.

---

## Completion Signal

This spec is complete when:
1. CP-0 diagnostic report committed (commit message references which hypothesis fired).
2. All four CPs (1-4) landed on master.
3. All acceptance criteria checked.
4. Empirical verification: user runs Bybit importer twice on the same window; counter shape confirms idempotency.
5. Commit message: `fix(hist-03): idempotent CEX history import — ON CONFLICT DO NOTHING + exchange-name canonicalization`.
