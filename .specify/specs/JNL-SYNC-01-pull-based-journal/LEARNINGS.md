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
