# LEARNINGS — FIX-08-bybit-fill-reconciliation

## 2026-04-24 — T5+T6+T7 (Sidecar /order/fetch endpoint)

- `testudo-cex/node_modules/safe-cex/dist/` does NOT exist by default — safe-cex-sub0 needs to be built first (`bunx tsc` from safe-cex-sub0/ using typescript@5.1.3), then a manual symlink must be created: `ln -s /home/m0xu/1-projects/testudo/safe-cex-sub0/dist /home/m0xu/1-projects/testudo/testudo-cex/node_modules/safe-cex/dist`. Without this, `handlers.test.ts` fails with "Cannot find package 'safe-cex'".
- `exchange.xhr` is a Bybit-specific property (`Axios` instance) on the safe-cex exchange class; not on `BaseExchange` type. Cast to `(exchange as any).xhr` is required. The route `/v5/order/history` is not in `bybit.types.ts` ENDPOINTS constant — call it directly.
- Two pre-existing test failures exist in `handlers.test.ts` (balance upnl calculation, position leverage field) — unrelated to T5+T7. Verified pre-existing via stash.
- T5+T6+T7 commit goes to the parent monorepo directly (testudo-cex is NOT a submodule).

## 2026-04-24 — T15+T16 (FillReconciler + daily-stats rebuild)

- `JournalTrade` (FromRow) does NOT include `exchange_order_ids` (TEXT[] in DB) — the SELECT column list in all query_as sites omits it. Callers provide order_ids as parameters to `reconcile_trade`.
- `rebuild_daily_stats_for_date` uses `closed_at::date = $3` to filter by date in SQL (NaiveDate bound maps to PostgreSQL DATE); consistent with Rust's `closed_at.date_naive()` (both UTC).
- The cumulative recompute SQL (`WITH running AS ...`) was duplicated in `journal_service.rs::upsert_daily_stats` AND `trade_event_writer.rs::upsert_daily_stats`. Extracted to `recompute_cumulative_pnl_from` in `journal_service.rs` (pub(crate)); all three callers (JournalService, TradeEventWriter, FillReconciler) now share it.
- Idempotent UPDATE predicate `AND needs_reconciliation = TRUE` in `apply_price_correction` prevents concurrent sweeps from overwriting a real price with a duplicate fix.
- `HAVING COUNT(*) > 0` on the INSERT SELECT in `rebuild_daily_stats_for_date` prevents inserting a zero-count row if a day's only trades are still pending reconciliation.

## 2026-04-24 — T4 (FR-8 stats exclusion sweep)

- Plan's `journal_service.rs:696,735,763,784` EXCLUDE entries are wrong — those lines are all inside `#[cfg(test)] mod hist03_idempotency` (test-verification COUNTs). The actual non-test production code in journal_service.rs has zero aggregation queries; only `record_trade_close` and `upsert_daily_stats` exist there. Skip journal_service.rs for EXCLUDE.
- The pre-existing test `routes::auth::tests::test_me_returns_user_info` was already failing before T4 (stash-verified). Unrelated to FIX-08 changes.
- Exclusion pattern for aliased tables: use `jt.needs_reconciliation = FALSE` when `FROM journal_trades jt` is aliased (digest.rs snapshot); unaliased queries use bare `needs_reconciliation = FALSE`.
- 7 files modified, 17 query sites patched. See plan Discoveries table for full triage.

## 2026-04-24 — T11+T12+T13+T14 (Reconciliation TradeClosed emit path)

- Extracted `build_trade_closed_payload()` into `services/trade_closed_payload.rs` — both `FillDetectorService` and `ReconciliationService` call it; zero duplication.
- `derive_close_side()` uses `stop_loss_price < entry_price` to identify LONG (exits via "sell") vs SHORT (exits via "buy"). This works even for `Closed` branch (both brackets gone) because `stop_loss_price` is a price target, not an order ID, and persists on `OrderGroup` after the SL order fills.
- `ReconcileAction.emit_trade_closed_on_terminal` carries a full `OrderGroup` clone — acceptable at 30s cadence; avoid temptation to hold a reference.
- The idempotency guard at `trade_event_writer.rs:337` has NO `needs_reconciliation` predicate, so a WS-originated journal row (real price) correctly prevents the reconciliation sweep from double-inserting. Defense-in-depth as designed.
- Always set `needs_reconciliation = true` in the reconciliation emit path — seeded prices are estimates that the async reconciler (T15) must overwrite.

## 2026-04-24 — T20 (FR-6 binary deferral)

- The literal `cargo run --bin reconcile-fills` invocation from FR-6 is deferred. The router crate is binary-only (no `src/lib.rs`); a second `[[bin]]` cannot `use router::…` without cascading `#[path]` attributes through the FillReconciler → CexClient → ExchangeAccountRepository call tree. A lib+bin split is a cross-cutting refactor disproportionate to this Medium-priority FR.
- Operators trigger the pending-fills sweep with: `curl -X POST -H "X-Internal-Secret: $SIDECAR_PSK" http://router/internal/reconcile-pending-fills`. The endpoint was implemented as T19.
- A follow-up spec can promote the router to a lib+bin layout if a CLI interface becomes necessary.
- Manual QA deferral: "kill WS connection + fill Bybit trade" requires a live Bybit account; verified design correctness via inline unit tests only. Deferred to live session by operator.

## 2026-04-24 — T23 (Verification Summary)

- **Dual-emit idempotency**: WS fill path and 30s reconciliation sweep can both fire for the same trade group. `trade_event_writer.rs` guards at the `SELECT 1 ... WHERE trade_group_id = $1` level before INSERT — first writer wins, second is a no-op. No explicit lock needed.
- **Side derivation without a side field**: `OrderGroup` carries no explicit `side` field. Close direction is inferred from `stop_loss_price < entry_price` (LONG exits via "sell"; SHORT exits via "buy"). Both `FillDetector` and `ReconciliationService` derive side this way via shared `derive_close_side()` in `trade_closed_payload.rs`.
- **Stats-query triage pattern**: classify every `journal_trades` query as EXCLUDE (aggregations feeding stats/charts) vs KEEP (user-facing lists, idempotency guards, imports). Only EXCLUDE queries gain `AND needs_reconciliation = FALSE`. Wrong classification causes either silent P&L distortion (forgot to exclude) or missing "pending" rows in the UI (excluded a list query).
- **Verification**: `cargo clippy --all-targets` clean; `cargo test` 736 pass / 1 pre-existing fail (`test_me_returns_user_info`); `bun test` in testudo-cex 118 pass / 2 pre-existing fail (balance/position leverage — unrelated to FIX-08).
- **Live QA deferred**: "Kill WS connection, fill trade on Bybit, verify in desk after 30s poll" requires a live Bybit account. Correctness verified via inline unit tests (T9, T14, T15). Operator should run this against a sandbox account before enabling in production.

## 2026-04-24 — T1 (Migration)

- The working directory when Vox runs is **inside `testudo-exchange/`** (the submodule), not the monorepo root. `git add` paths must be relative to the submodule root. After committing in the submodule, `cd /home/m0xu/1-projects/testudo && git add testudo-exchange` bumps the parent pointer.
- Migration naming: `20260424000000_…` — date prefix `YYYYMMDDHHMMSS` per existing convention.
- `exit_price` stays `NUMERIC NOT NULL`; placeholder value `0` satisfies the constraint. No DDL change needed on that column.
