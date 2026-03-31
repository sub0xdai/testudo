# Implementation Plan

> Last updated: 2026-04-01
> Current spec: CON-01a-daily-stats-regression
> Phase: COMPLETE

---

## Active Spec: CON-01a-daily-stats-regression

Fix three regressions introduced by CON-01: (1) daily stats not upserted, (2) draft notes not merged, (3) exchange hardcoded to "cex" in TradeClosed payload.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Add daily stats upsert + draft notes merge to TradeEventWriter post-commit (CP-1 + CP-2) | complete | medium | — |
| T2 | Add exchange_name to OrderGroup + wire through configure_group, trade_management, rehydration, fill_detector (CP-3) | complete | high | — |
| T3 | Update tests + fix struct literal breakage from new field | complete | low | T2 |
| T4 | Validate: cargo clippy --all-targets && cargo test, commit | complete | low | T1, T2, T3 |

### Key Decisions

- Daily stats upsert is fire-and-forget after tx.commit() — same SQL as JournalService::upsert_daily_stats
- Draft notes merge is fire-and-forget after tx.commit() — same pattern as JournalService::record_trade_close
- exchange_name added to OrderGroup as Option<String>, populated via configure_group at trade placement and via pool lookup during rehydration
- fill_detector uses group.exchange_name.as_deref().unwrap_or("unknown") — "unknown" fallback makes missing names obvious
- pool added to TradeManagementState as Option<PgPool> for backward compat with test constructors
- pool added to RehydrationService for batch exchange_name lookup during startup
- parse_trade_close_payload default changed from "cex" to "unknown" for consistency

### Discoveries

- TradeManagementState doesn't have direct pool access — had to add `pool: Option<PgPool>` field with `with_pool()` builder
- RehydrationService similarly needed pool added to constructor (2 call sites in main.rs)
- engine::EngineCommand::ConfigureGroup needed exchange_name field added alongside exchange_account_id
- Rehydration uses batch query `SELECT id, exchange_name FROM exchange_accounts WHERE id = ANY($1)` for efficient lookup
- Pre-existing clippy warnings unchanged: useless_conversion (cex_client.rs), unused_variables (actor.rs), manual_contains (evaluator.rs)

---

## Completed Specs

- UX-01-pair-page (COMPLETE)
- UX-02-overview-polish (COMPLETE)
- REL-02-hl-journal-pipeline (COMPLETE)
- REL-03-hl-group-reconciliation (COMPLETE)
