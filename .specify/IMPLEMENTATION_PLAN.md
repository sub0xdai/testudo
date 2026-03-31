# Implementation Plan

> Last updated: 2026-03-31
> Current spec: REL-02-hl-journal-pipeline
> Phase: COMPLETE

---

## Active Spec: REL-02-hl-journal-pipeline

Fix HL trade data pipeline — closing fills not reaching journal/charts/overview since Mar 27. Lift proven import_worker logic into ws_fills REST poll loop for near-real-time journal writes.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Extract shared `build_trade_close_event()` into `hl_fill_journal.rs`, refactor import_worker to use it | complete | medium | — |
| T2 | Wire JournalService + PgPool + user_id through WsSubscriptionManager → HyperliquidFillSubscriber, write closing fills in `reconcile_since()` with seen_tids dedup | complete | high | T1 |
| T3 | Add `open_times` tracking across poll cycles with 24h startup seed for correct trade duration | complete | medium | T2 |
| T4 | Add `pg_notify` emission after journal write for extension UI update | complete | low | T2 |

### Key Decisions

- T3 and T4 folded into T2 — all three concerns (journal write, open_times, pg_notify) live in the same `reconcile_since()` loop, splitting them would be artificial
- Borrow checker required cloning `Arc<JournalService>` and `Option<PgPool>` before the fill iteration loop — `self.record_tid()` mutably borrows self while journal/pool refs are held
- `journal_service` creation moved earlier in main.rs (before WsSubscriptionManager) to enable wiring via `with_journal()`

### Discoveries

- Clippy `doc_lazy_continuation` triggers when a doc-comment line for a constant is placed immediately after a struct's doc block (even with blank `///` separator) — constants with `///` docs must be separated by structural items or placed in a different region
- `MAX_SEEN_TIDS` placed alongside `MAX_SEEN_OIDS` at top of file to avoid doc comment confusion

---

## Completed Specs

- UX-01-pair-page (COMPLETE)
- UX-02-overview-polish (COMPLETE)
