# Specification: HL Import — Partial-Fill Aggregation

**Spec ID:** HIST-04-hl-partial-fill-aggregation
**Date:** 2026-04-21
**Status:** Draft
**Class:** Fix / Data Model Semantics
**Priority:** P1 — journal analytics (R-multiple, win-rate, Dignitas) over-count Hyperliquid closes proportional to partial-fill fan-out. User-visible on Desk as apparent duplicate trades.
**Depends on:** HIST-01, HIST-02, HIST-03 (shipped — provide the dedup scaffolding this spec builds on)
**Series:** HIST-04 (Hyperliquid-specific follow-up to HIST-03)

---

## Problem Statement

HIST-03 made CEX history import idempotent at the fill-ID level. The Hyperliquid path (`import_worker::import_hl` → `hl_fill_journal::build_trade_close_event`) still emits **one `journal_trades` row per HL closing fill**. When a position closes via N partial fills (standard HL behaviour for any order that matches multiple resting orders), the journal gets N rows — each with a real distinct `tid`, so HIST-01's partial unique index does not catch them. They are legitimately distinct fills, but not legitimately distinct *trades*.

### Evidence (from 2026-04-21 database audit)

Backup of apparent-duplicates pulled from `journal_trades_hist03_dup_backup` on production:

| closed_at (UTC)          | group size | entry | exit prices         | qty/row | fill tids                                                                          |
|--------------------------|------------|-------|---------------------|---------|------------------------------------------------------------------------------------|
| 2026-03-21 07:46:30.583  | 3          | 70798 | 70694, 70693, 70692 | 0.00017 | 333050122367776, 880356009255315, 930185002417954                                  |
| 2026-03-21 07:46:45.632  | 4          | 70668-9| 70668×3, 70669     | 0.00017 | 141519131614366, 454248599633512, 1018021148893471, 1096047518932837               |

All 7 rows were `INSERT`ed within 20 ms of each other during a single import run (`created_at` deltas <10 ms). Each row has a real, distinct `tid` from HL. They are partial fills of two position-close events that Testudo should journal as **two** trades, not **seven**.

### Secondary bug (latent, same module)

`hl_fill_journal::build_trade_close_event` takes `open_time_ms: Option<u64>` that `import_worker` computes from a `HashMap<symbol, open_time>`. The map stores the **first-seen open time** per symbol and never clears across round-trips. Consequence: every closed `BTC_USDT` trade across the import window shares the `opened_at` of the first ever open — duration, opened_at filter queries, and opened-at charts are all wrong. Fixing aggregation (this spec) is the natural place to also correct per-round-trip open tracking.

---

## User Stories

- **As a Hyperliquid user**, I want my journal to show one row per round-trip, so that the Desk trade count matches what I actually traded, not my venue's internal match granularity.
- **As the Dignitas / coach pipeline**, I want aggregate statistics (win rate, avg R, streak detection) to reflect real trading decisions, not exchange match-engine artifacts.
- **As a developer re-running the HL importer**, I want it to be idempotent at the round-trip level — re-imports converge on the same journal shape as the first import.

---

## Functional Requirements

- **FR-1 (Aggregation)**: The HL import path MUST emit one `journal_trades` row per position round-trip (open → net-zero close), not one per fill.
- **FR-2 (Round-trip boundary)**: A round-trip opens when position net qty goes non-zero from zero and closes when it returns to zero. Partial closes of a still-open position do NOT emit a journal row until the position is flat.
- **FR-3 (VWAP entry)**: Aggregated entry_price = Σ(fill.px × fill.sz) / Σ(fill.sz) across all opening fills of the round-trip.
- **FR-4 (VWAP exit)**: Aggregated exit_price = Σ(fill.px × fill.sz) / Σ(fill.sz) across all closing fills of the round-trip.
- **FR-5 (Summed quantities, fees, pnl)**: `quantity = Σ fill.sz` for the opening leg (= Σ fill.sz for closing leg by FR-2); `fees = Σ fill.fee` over all fills in the round-trip; `realized_pnl = Σ fill.closed_pnl` over the closing fills.
- **FR-6 (opened_at / closed_at)**: `opened_at = time of the first opening fill of this round-trip`; `closed_at = time of the last closing fill`. Both per round-trip — not per-symbol shared state.
- **FR-7 (exchange_fill_id)**: Use the `tid` of the **last closing fill** as the aggregated row's `exchange_fill_id`. Re-imports deterministically pick the same last-fill tid so HIST-01's partial unique index dedup's re-imports at the round-trip level.
- **FR-8 (Live poll path uses the same aggregator)**: `hyperliquid::ws_fills::run_rest_poll` currently calls `build_trade_close_event` per fill with `source = "live_poll"`. It must use the same aggregation logic so live-poll reconstructions don't produce partial-fill rows either.
- **FR-9 (source byte-identical)**: `source` strings stay `"import_hl"` and `"live_poll"`. No schema changes.
- **FR-10 (CEX path untouched)**: `source = "import_ccxt"` path is unchanged — HIST-03 already reconstructs positions from raw fills for CCXT.
- **FR-11 (Testudo-native path untouched)**: `source = "testudo"` (live Alt+X trades routed through TradeEventWriter) is unchanged — they already aggregate at the TradeManager layer.
- **FR-12 (Data migration is user-driven, not automated)**: The fix is prospective. Existing `import_hl` and `live_poll` rows remain as-is. The user will, post-deploy, DELETE all `source IN ('import_hl','live_poll')` rows and re-run the importer to rebuild cleanly. Document this in the spec's deploy notes.

---

## Non-Goals

- Rewriting the CEX import path (HIST-03 territory).
- Changing how live Testudo trades are journaled (different architectural layer entirely).
- Back-filling fees for pre-HIST-04 rows.
- Cross-exchange aggregation semantics (one spec, one exchange family).

---

## Files

### Backend (Rust) — `testudo-exchange`

- `crates/router/src/services/hl_fill_journal.rs` — replace per-fill `build_trade_close_event` with a round-trip aggregator. Keep the module's public surface stable; the caller simply receives `Vec<TradeCloseEvent>` (zero or one per invocation cycle) instead of `Option<TradeCloseEvent>`.
- `crates/router/src/services/import_worker.rs` — `import_hl` loop: instead of iterating fills and calling `process_hl_fill` per fill, buffer all fills for the window and emit aggregated round-trips. Fix the `open_times: HashMap<symbol, u64>` pattern — it becomes an internal detail of the aggregator.
- `crates/router/src/services/hyperliquid/ws_fills.rs` — `run_rest_poll` path: same aggregator, incremental over each poll interval. Handle mid-poll partial state (open round-trips not yet closed must NOT emit; carry over to the next poll tick).

### Tests

- `crates/router/tests/` — property/unit tests for the aggregator covering: single full-fill close (degenerate = 1 fill → 1 row), N-partial close (→ 1 row), pyramid add then full close (→ 1 row), flip (long → short through zero = 2 rows).

---

## Acceptance Criteria

- [ ] Unit tests for aggregator cover all 4 scenarios above. Baseline 655 router tests still pass; count goes up.
- [ ] Fresh HL re-import on staging: `SELECT COUNT(*) FROM journal_trades WHERE source = 'import_hl'` produces fewer rows than the same window produces pre-HIST-04, and content matches externally-verified position round-trips from HL's own UI.
- [ ] `opened_at` is distinct per round-trip (no longer symbol-shared).
- [ ] `exchange_fill_id` for each aggregated row equals the `tid` of the last closing fill (verifiable by comparing to HL's `/info fills` response).
- [ ] Re-running the import idempotently: second run INSERT's zero rows, HIST-01 partial index catches on `tid` of last fill.
- [ ] Live-poll path produces the same aggregation shape (verified by a test that simulates two partial-fill poll cycles).

---

## Task Sketch (for planning)

1. **T1** — Diagnostic: dump current `import_hl` row distribution by `(symbol, closed_at::date, exit_price)` to confirm scale of over-count on prod before fix.
2. **T2** — Pure aggregator module: `fn aggregate_round_trips(fills: &[UserFillByTime]) -> Vec<TradeCloseEvent>`. 100% pure, no IO, unit-testable. Tests: single-fill, N-partial, pyramid, flip.
3. **T3** — Refactor `import_worker::import_hl` to use the aggregator. Delete `open_times` map + `process_hl_fill` per-fill path.
4. **T4** — Refactor `ws_fills::run_rest_poll` to use the aggregator, with carry-over state for mid-poll open round-trips.
5. **T5** — Fix `opened_at` correctness (falls out of T2; T5 verifies via unit test).
6. **T6** — Integration test: mock HL fills fixture (extend the HIST-03 test harness) exercises the import loop end-to-end, asserts aggregated row count and shape.
7. **T7** — Deploy notes: document the `DELETE FROM journal_trades WHERE source IN ('import_hl','live_poll')` + re-import step. Add to runbook.
8. **T8** — Spec archive + baseline verify.

---

## Migration / Deploy Notes

Post-deploy operational steps (user-invoked, not automated by the spec):

```sql
-- 1. Backup existing HL-sourced rows
CREATE TABLE journal_trades_hist04_backup AS
SELECT * FROM journal_trades WHERE source IN ('import_hl', 'live_poll');

-- 2. Clear HL-sourced rows
DELETE FROM journal_trades WHERE source IN ('import_hl', 'live_poll');

-- 3. Re-run the HL importer for the full history window via the extension or API
-- 4. Verify counts
SELECT source, COUNT(*) FROM journal_trades WHERE exchange = 'hyperliquid' GROUP BY source;

-- 5. Keep backup for a week, then drop
-- DROP TABLE journal_trades_hist04_backup;
```

---

## Open Questions

- Does HL ever report a "closing" fill that brings the position *past* zero (e.g., a stop triggered on a flipped position)? If so, aggregator must split at zero-crossings. T2 test: "flip through zero" covers this.
- Does the live-poll path share any state with websocket-native fill detection (`hyperliquid/ws_fills.rs` WS subscriber)? If yes, aggregation must not double-count when both paths observe the same fill. Verify during T4.
