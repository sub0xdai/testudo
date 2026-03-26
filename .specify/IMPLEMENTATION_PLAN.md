# Implementation Plan

> Last updated: 2026-03-26
> Current spec: ONBOARD-01-stepper-onboarding
> Phase: COMPLETE

---

## Active Spec: HIST-01-exchange-history-import

Import exchange trade history (Phase 1: Hyperliquid) — pg_queue async jobs, closing fills → journal_trades, auto-trigger on exchange credential save, dedup on `(user_id, exchange, exchange_fill_id)`.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Schema migration — add `source TEXT NOT NULL DEFAULT 'testudo'` and `exchange_fill_id BIGINT` columns to `journal_trades`. Add partial unique index `(user_id, exchange, exchange_fill_id) WHERE exchange_fill_id IS NOT NULL`. | complete | low | — |
| T2 | pg_queue migration — create `queue_imports` table (same structure as `queue_orders`). Add `TradeImports` variant to `QueueName` enum. Wire up LISTEN/NOTIFY trigger. | complete | low | — |
| T3 | Update JournalTrade model — add `source` and `exchange_fill_id` fields to `JournalTrade` struct in `models/journal.rs`. Update `record_trade_close()` INSERT query in `journal_service.rs` to include new columns. Update `TradeCloseEvent` to accept optional `source` and `exchange_fill_id`. | complete | medium | T1 |
| T4 | Import worker — create `services/import_worker.rs`. Implement HL fill fetcher: paginate `user_fills_by_time()` across 90-day window, filter to closing perp fills (`closedPnl != "0.0"`, no `@` prefix coins), map each fill to `TradeCloseEvent`, call `record_trade_close()`. Handle dedup via ON CONFLICT or pre-check. | complete | complex | T1, T2, T3 |
| T5 | Import routes — create `routes/imports.rs`. `POST /api/v1/trades/import` (enqueue job, return job_id). `GET /api/v1/trades/import/status` (list user's import jobs with status/counts). Register routes in `routes/mod.rs`. | complete | medium | T2 |
| T6 | Auto-trigger on exchange add — modify `routes/exchanges.rs` POST handler to enqueue an import job after credentials are saved. | complete | low | T2, T5 |
| T7 | Spawn import worker — add worker loop startup to `main.rs` as a Tokio task alongside existing workers. | complete | low | T4 |
| T8 | WebSocket notification — send `import_complete` event to user via pg_notify → ws-stream pipeline on job completion. | complete | medium | T4, T7 |
| T9 | Verification — `cargo clippy --all-targets` clean (only pre-existing warnings), `cargo test` 1025 pass / 0 fail. | complete | low | T1–T8 |

### Key Decisions

- **Credentials not in job payload**: Worker loads credentials from `exchange_account_repo.load_credentials(account_id, user_id)` at execution time. Only `account_id` stored in queue payload.
- **Entry price derivation**: `entry = exit - (closedPnl / sz)` for longs, `entry = exit + (closedPnl / sz)` for shorts. P&L is exact from HL; entry is derived.
- **opened_at = closed_at**: HL closing fills don't include open time. Duration will show 0s for imports. Acceptable.
- **Leverage defaults to 1**: HL fills don't include leverage. P&L is already correct from `closedPnl`.
- **Route through existing journal pipeline**: `record_trade_close()` handles P&L computation, daily stats, drawdown. Imported trades get the same treatment.
- **Phase 2 (CCXT) is a separate spec**: This spec covers Hyperliquid only.

### Discoveries

(populated during build iterations)

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| EXT-41-desk-dashboard | 2026-03-24 |
| EXT-40-smart-card-grid | 2026-03-24 |
| EXT-39-pair-ux | 2026-03-24 |
| AUTH-03-frontend-auth | 2026-03-24 |
| AUTH-02-backend-auth | 2026-03-24 |
| AUTH-01-infra-hardening | 2026-03-24 |
| ANL-01-bloomberg-charts (Phase 1) | 2026-03-23 |
| JNL-18-storage-quotas | 2026-03-22 |
| JNL-17-nested-collections | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
