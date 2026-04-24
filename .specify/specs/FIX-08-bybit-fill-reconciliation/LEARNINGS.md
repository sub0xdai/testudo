# LEARNINGS — FIX-08-bybit-fill-reconciliation

## 2026-04-24 — T5+T6+T7 (Sidecar /order/fetch endpoint)

- `testudo-cex/node_modules/safe-cex/dist/` does NOT exist by default — safe-cex-sub0 needs to be built first (`bunx tsc` from safe-cex-sub0/ using typescript@5.1.3), then a manual symlink must be created: `ln -s /home/m0xu/1-projects/testudo/safe-cex-sub0/dist /home/m0xu/1-projects/testudo/testudo-cex/node_modules/safe-cex/dist`. Without this, `handlers.test.ts` fails with "Cannot find package 'safe-cex'".
- `exchange.xhr` is a Bybit-specific property (`Axios` instance) on the safe-cex exchange class; not on `BaseExchange` type. Cast to `(exchange as any).xhr` is required. The route `/v5/order/history` is not in `bybit.types.ts` ENDPOINTS constant — call it directly.
- Two pre-existing test failures exist in `handlers.test.ts` (balance upnl calculation, position leverage field) — unrelated to T5+T7. Verified pre-existing via stash.
- T5+T6+T7 commit goes to the parent monorepo directly (testudo-cex is NOT a submodule).

## 2026-04-24 — T4 (FR-8 stats exclusion sweep)

- Plan's `journal_service.rs:696,735,763,784` EXCLUDE entries are wrong — those lines are all inside `#[cfg(test)] mod hist03_idempotency` (test-verification COUNTs). The actual non-test production code in journal_service.rs has zero aggregation queries; only `record_trade_close` and `upsert_daily_stats` exist there. Skip journal_service.rs for EXCLUDE.
- The pre-existing test `routes::auth::tests::test_me_returns_user_info` was already failing before T4 (stash-verified). Unrelated to FIX-08 changes.
- Exclusion pattern for aliased tables: use `jt.needs_reconciliation = FALSE` when `FROM journal_trades jt` is aliased (digest.rs snapshot); unaliased queries use bare `needs_reconciliation = FALSE`.
- 7 files modified, 17 query sites patched. See plan Discoveries table for full triage.

## 2026-04-24 — T1 (Migration)

- The working directory when Vox runs is **inside `testudo-exchange/`** (the submodule), not the monorepo root. `git add` paths must be relative to the submodule root. After committing in the submodule, `cd /home/m0xu/1-projects/testudo && git add testudo-exchange` bumps the parent pointer.
- Migration naming: `20260424000000_…` — date prefix `YYYYMMDDHHMMSS` per existing convention.
- `exit_price` stays `NUMERIC NOT NULL`; placeholder value `0` satisfies the constraint. No DDL change needed on that column.
