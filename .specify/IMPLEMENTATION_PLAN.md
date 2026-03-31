# Implementation Plan

> Last updated: 2026-03-31
> Current spec: REL-03-hl-group-reconciliation
> Phase: COMPLETE

---

## Active Spec: REL-03-hl-group-reconciliation

After REL-02 writes an HL closing fill to the journal, the corresponding OrderGroup remains Active (FillDetector never matched). Add reconciliation: find matching group by (user_id, symbol), transition to terminal, cancel siblings, emit notify.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Wire EngineHandle + ExchangeApi into WsSubscriptionManager → HyperliquidFillSubscriber, add group reconciliation after journal write (CP-1 + CP-2 + CP-3) | complete | high | — |
| T2 | Validate: cargo clippy --all-targets && cargo test, commit | complete | low | T1 |

### Key Decisions

- All 3 checkpoints (group transition, cancel siblings, pg_notify) implemented in single task — they're a single code path after journal write
- Cardinality check: only act on exactly 1 matching active group per (user_id, symbol)
- Best-effort cleanup: journal entry is never rolled back on reconciliation failure
- Symbol matching handles both formats: HL coin name ("BTC") and group format ("BTC_USDT")
- When REL-03 engine_handle is present, `reconcile_group` emits pg_notify with specific event_type (stopped_out/took_profit); when absent, REL-02 fallback emits generic "trade_closed"

### Discoveries

- `reconcile_group` is a static async method (takes explicit params) rather than `&self` method to avoid borrow checker issues with the mutable `record_tid()` loop
- `fill.dir` field contains "Close Long"/"Close Short" — used directly as `fill_side` parameter for terminal status determination
- HL OrderGroups use "BTC_USDT" symbol format while HL fills use bare coin name "BTC" — reconciler matches both

---

## Completed Specs

- UX-01-pair-page (COMPLETE)
- UX-02-overview-polish (COMPLETE)
- REL-02-hl-journal-pipeline (COMPLETE)
