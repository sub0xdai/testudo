# Implementation Plan

> Last updated: 2026-04-04
> Current spec: JNL-13-true-equity-curve
> Phase: PLANNING COMPLETE

---

## Active Spec: JNL-13-true-equity-curve

True equity curve via balance snapshots — fixes misleading max drawdown percentage and makes equity chart show actual account value.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Migration: create `balance_snapshots` table + add `starting_balance` column to `exchange_accounts` (FR-1, FR-3) | complete | simple | — |
| T2 | Balance snapshot service: insert snapshot, query snapshots by user/date range, resolve exchange_account_id from (user_id, exchange_name) (FR-2) | complete | medium | T1 |
| T3 | Hook snapshot capture into TradeEventWriter on TradeClosed events — spawn background balance fetch after journal co-write (FR-2) | pending | medium | T2 |
| T4 | Update equity curve computation: prefer snapshots → starting_balance fallback → cumulative P&L fallback. Add `is_true_equity` flag to response (FR-4, FR-6) | complete | medium | T2 |
| T5 | Update drawdown calculation: use peak equity as denominator when snapshots exist (FR-5) | complete | medium | T2 |
| T6 | Starting balance PATCH endpoint + exchange account repo update (FR-3, FR-7) | pending | simple | T1 |
| T7 | Frontend: HeroEquityCurve dynamic baseline + EquityPoint type update (FR-8) | complete | simple | T4 |
| T8 | Frontend: starting balance input in Account settings page (FR-7) | pending | simple | T6 |

### Key Decisions

- Snapshot capture is fire-and-forget (`tokio::spawn`) — must not block trade event persistence
- `TradeEvent` lacks `exchange_account_id`; snapshot service resolves from `(user_id, exchange_name)` via exchange_accounts table
- Three-tier fallback for equity: real snapshots → starting_balance + cumulative P&L → raw cumulative P&L
- `is_true_equity` boolean flag in API response tells frontend which data source is active
- exchange constraint on `exchange_accounts` is `UNIQUE(user_id, exchange_name)` — lookup is deterministic

### Discoveries

- `TradeEvent` struct (engine crate) only carries `user_id`, `group_id`, `symbol`, `payload` — no `exchange_account_id`
- TradeClosed payload has `exchange` (string) field, parseable in `parse_trade_close_payload()`
- `trade_event_writer.rs` already has fire-and-forget pattern for daily stats upsert (post-commit block, lines 181-227)
- `exchange_accounts` table has `UNIQUE(user_id, exchange_name)` constraint — can resolve account_id from user+exchange
- `cex_client.fetch_balance()` needs `SidecarCredentials` — requires decrypting the exchange account in the snapshot service
- HeroEquityCurve uses `lightweight-charts` BaselineSeries with baseline at 0 — baseline value is configurable

---

## Completed Specs

- UX-01-pair-page (COMPLETE)
- UX-02-overview-polish (COMPLETE)
- REL-02-hl-journal-pipeline (COMPLETE)
- REL-03-hl-group-reconciliation (COMPLETE)
- CON-01a-daily-stats-regression (COMPLETE)
- UXA-01-agent-wallet-visibility (COMPLETE)
- UXA-02-desk-reauth-ux (COMPLETE)
- UXA-03-extension-error-recovery (COMPLETE)
