# LEARNINGS — JNL-SYNC-01

## 2026-05-03 — CP-1

**Fee-asset normalization gap (per spec risk 1):** `reconstruct_trades` sums `fee`
values in their native asset. For Bybit (USDT fees), this is correct. For Binance
(BNB fees) or HL (HYPE), `realized_pnl` will be off by the un-converted fee
amount. JNL-SYNC-02 must add a post-sync `UPDATE journal_trades SET fees = …, realized_pnl = …`
migration step keyed on normalized derivation — a syncer re-run will NOT fix existing
`pull_sync` rows because `INSERT … ON CONFLICT DO NOTHING` skips rows whose hash matches.

**Side-flip handling decision:** a fill that crosses zero net is split into
`{exec_id}_close` and `{exec_id}_open` virtual fills with proportional fee
attribution. The test for this (`test_side_flip_produces_two_trades`) requires
a third fill to close the new position, so the test input is three fills
[Buy 1, Sell 2, Buy 1] producing two round-trip trades. If CCXT surfaces
`reduce_only` correctly, the side-flip branch should never fire in normal usage.

**`tracing` added as dependency to `common_utils`:** needed for `tracing::warn!`
in the side-flip path. Added as `tracing = "0.1"` (not workspace ref) since
the workspace root already defines it and common_utils doesn't use the workspace
dependency table pattern.

**Late-arriving fill duplicate-by-design:** if a fill that was part of a prior
round trip arrives in a later poll window (Bybit 7-day pagination boundary),
`reconstruct_trades` will produce a trade with a different `source_fills_hash`
(because the set of exec_ids differs). The DB gets two rows for the "same"
trade. This is documented as MVP acceptable; cross-source dedup is HIST-05.
The `test_late_arriving_fill` test covers independent round trips (not the
hash-divergence case) — the hash-divergence scenario is documented here rather
than tested because `reconstruct_trades` is pure and cannot observe prior calls.

## 2026-05-03 — CP-2

**Stuck-cursor guard logic:** use `nextCursor === cursor` (current cursor), NOT `nextCursor === prevCursor`. The `prevCursor` pattern is incorrect — the comparison must check if the NEXT cursor equals the CURRENT cursor (i.e., no progress made). `!nextCursor || nextCursor === cursor || list.length < 100 → break` is the correct single-line guard.

**Bybit 7-day window × test window:** tests using `since_ms` without a tight `until_ms` will iterate many 7-day windows (1 year back = ~52 windows = ~52 xhr calls per test). Always pass `until_ms = since_ms + small_value` in handler tests that mock a single window. The Bybit window loop is correct and intentional for production; tests must constrain the window explicitly.

**Binance requires symbol per request:** Binance `/fapi/v1/userTrades` has no "all symbols" endpoint. The sidecar handler iterates `exchange.store.markets` when no symbol is given. This is acceptable at MVP scale (single-digit users) but becomes rate-limit-bound for large accounts. If `store.markets` is empty (exchange not yet started), the Binance handler returns an empty array silently — the JournalSyncer must ensure the exchange is started before calling.

**testudo-cex is NOT a submodule:** commits in `testudo-cex/` land in the parent testudo repo directly (unlike `testudo-exchange` which is a submodule). No pointer bump needed for sidecar changes.

## 2026-05-03 — CP-4

**`source_fills_hash` column requires `#[sqlx(default)]`:** `JournalTrade` is returned by many explicit SELECT queries that don't include the new column. Adding `#[sqlx(default)]` on `source_fills_hash: Option<String>` makes the field default to `None` rather than a runtime `ColumnNotFound` error. Any new query returning `JournalTrade` should include `source_fills_hash` explicitly; existing queries are safe due to the default.

**`tick()` made `pub(crate)` for integration tests:** the test module at `services/journal_syncer/integration_tests.rs` is a submodule of `journal_syncer`, so `pub(crate)` is sufficient to access `tick()` from tests. Making it `pub` would expose it unnecessarily to route handlers.

**`exchange_label` is canonical name in `raw_fills` query:** `raw_fill_repo.fetch_for_account(user_id, &self.exchange_label)` uses the label string directly. The `exchange_label` in `JournalSyncer` must be the canonical (lowercase) exchange name to match what `raw_fills` stores. `CcxtFillSource.fetch_since` applies `canonical_exchange_name` before setting `fill.exchange`, so as long as the syncer's `exchange_label` matches, the fetch is correct.

**Pre-existing test failure:** `routes::auth::tests::test_me_returns_user_info` was already failing before CP-4 (the `me` handler returns `body["user"]["id"]` but the test asserts `body["user_id"]`). Not introduced by this checkpoint.

## 2026-05-03 — CP-5

**Symbol convention confirmed:** `format!("{}_USDT", fill.coin)` is the correct HL symbol format, consistent with `ws_fills.rs:555` (`"s": format!("{}_USDT", fill.coin)`) and `AssetUniverse::from_hl_coin` (`universe.rs:213`). The `convert_hl_fill` implementation matches.

**HL SDK pagination:** `info.user_fills_by_time(address, since_ms, None, None)` returns all fills in one call — the SDK has no documented page-size cap in the current version. Consistent with how `ws_fills.rs:444` uses it in production. No pagination loop needed at this time.

**`hl_fill_journal.rs` still exists and must NOT be deleted:** the file exists and is used by `import_worker.rs` (HIST-02 batch HL import). CP-6 removed its usage from `ws_fills.rs` (the WS-driven live path) but kept the file. `build_trade_close_event` and `timestamp_to_datetime` remain for the importer.

**Spawn gated on both `syncer_enabled && hl_enabled`:** HL syncer requires both flags to avoid spawning a live-data poller when HL integration is disabled. Mirrors the CCXT pattern (`syncer_enabled && ccxt_enabled`).

## 2026-05-03 — CP-6

**`reconciliation.rs` also emitted TradeClosed (FIX-08 FR-10 path):** plan said to delete `fill_reconciler.rs` + `emit_trade_closed` in `fill_detector.rs`, but `reconciliation.rs` had its own `emit_trade_closed_on_terminal: Option<(OrderGroup, Decimal, String)>` field on `ReconcileAction` and a corresponding block that sent TradeClosed events for missed WS fills. This was also deleted in CP-6; if missed in future similar specs, grep for `TradeClosed` and `build_trade_closed_payload` across all services.

**`WsSubscriptionManager.with_journal` renamed to `with_hl_notify_pool`:** the journal reference from `with_journal(journal, pool)` was forwarded to `HyperliquidFillSubscriber.with_journal(journal, user_id, pool)`. After CP-6, the subscriber only needs `user_id` + `pool` for group reconciliation. `WsSubscriptionManager` was updated to `with_hl_notify_pool(pool)` and `HyperliquidFillSubscriber` to `with_user(user_id, pool)`. The call site in `main.rs` changed from `with_journal(journal_service.clone(), pg_pool.clone())` to `with_hl_notify_pool(pg_pool.clone())`.

**Balance snapshot extraction wired to JournalSyncer:** `JournalSyncer` gained a `pool: PgPool` field and `cex_client: Option<Arc<CexClient>>` field. `spawn_balance_snapshot` free function added to `balance_snapshot.rs`. In `tick()`, fires after `new_count > 0`. HL syncers use `cex_client = None` (no CCXT for HL balance). Balance snapshot delay is ≤30s (syncer cadence) rather than immediate — acceptable for fire-and-forget.

**Pre-existing test failure confirmed:** `routes::auth::tests::test_me_returns_user_info` still failing after CP-6 (same root cause as CP-4: handler returns `body["user"]["id"]` but test asserts `body["user_id"]`). Not introduced by this spec.

**`hyperliquid.rs` was untracked after CP-5 commit:** `pub mod hyperliquid;` was declared in `mod.rs` and the file existed on disk (so compilation succeeded) but the file was never staged. A fresh clone would have failed to compile. Bundled into CP-6 commit. When staging journal_syncer modules, always `git status --short | grep journal_syncer` to catch any untracked additions.

**Stale comment cleanup required for acceptance criteria:** 7 files had comment references to `FillReconciler`, `emit_trade_closed`, or `needs_reconciliation=TRUE` even after functional deletion. The acceptance criterion `grep -r "FillReconciler|emit_trade_closed|fill_reconciler|reconcile_pending_fills"` would have failed on these. Cleaned in the same CP-6 commit: `fill_detector.rs`, `cex_client.rs`, `journal_service.rs`, `journal_syncer/mod.rs`, `routes/internal.rs`, `routes/journal.rs`, `models/journal.rs`.

## 2026-05-03 — CP-8

**`needs_reconciliation` acceptance grep caveat:** The T43 acceptance grep covers `needs_reconciliation|FillReconciler|emit_trade_closed|reconcile_pending_fills`. `FillReconciler`, `emit_trade_closed`, and `reconcile_pending_fills` return zero hits. `needs_reconciliation` still appears as a struct field in `models/journal.rs:54`, `journal_service.rs`, and `routes/journal.rs` — this is intentional backwards-compat (spec FR-10: "kept for backwards-compat, scheduled for drop in a follow-up migration"). The functional behavior (filter clauses, reconciler service, live write path) is fully gone. The acceptance criterion should be read as "no legacy pipeline behavior" not "no column name in source".

**Pre-existing test failures (not introduced by JNL-SYNC-01):**
- `testudo-cex`: `POST /balance` (expected total "10000", got "10150") and `POST /position` (unexpected `leverage` field) — both introduced by commit `7a32dee` (`fix(risk): propagate position leverage`) which predates CP-2.
- `testudo-extension bun run typecheck`: Multiple errors in `modal.tsx` (`Cannot find name 'chrome'`), `AuthSection.tsx`, `utils.ts`, `types.test.ts` — pre-existing before JNL-SYNC-01 started. None of the failing files were modified by this spec.
- `testudo-exchange cargo test`: `routes::auth::tests::test_me_returns_user_info` — pre-existing (documented in CP-4 and CP-6 LEARNINGS).

**Verification commands as accepted by spec acceptance criteria #8:**
- `cargo clippy --all-targets` — clean (one pre-existing unused variable warning)
- `cargo test` — 741 passed, 1 pre-existing failure
- `bun run build` (testudo-cex) — clean exit 0
- `bun run build` (testudo-journal) — clean exit 0
- `bun run typecheck` (testudo-extension) — pre-existing type errors, not introduced by this spec

**Fee-asset normalization gap carries to JNL-SYNC-02:** as documented in CP-1. `ON CONFLICT DO NOTHING` means a syncer re-run will NOT update existing `pull_sync` rows after JNL-SYNC-02 ships. JNL-SYNC-02 must include an explicit `UPDATE journal_trades SET fees = …, realized_pnl = … WHERE source = 'pull_sync'` migration, not just a new sync run.

**Performance: `upsert_many` is N single-row inserts.** At 1000-fill backfill that's 1000 round trips. Acceptable for now (single-digit users). If the A2 acceptance test (backfill on connect) feels slow in production, batch via `INSERT … VALUES (…), (…), …` — this is the primary optimization target for JNL-SYNC-01 follow-up.

**Rate-limit headroom not measured:** CP-4/CP-5 integration tests run against mock sources. Per-exchange rate-limit behaviour at 30s × N accounts is deferred to first production deploy observation. `JOURNAL_SYNC_INTERVAL_BYBIT_SECS` env override exists as escape valve.

**Side-flip branch did not fire in test suite:** no live exchange data was used, so we cannot confirm the side-flip case in production. If `tracing::warn!` fires for side-flip during live testing, check whether CCXT surfaces `reduce_only` on the exchange in question — if so, the branch should never fire for that exchange.
