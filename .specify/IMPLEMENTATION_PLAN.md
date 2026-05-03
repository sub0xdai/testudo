# Implementation Plan — JNL-SYNC-01 Pull-Based Journal

**Status: COMPLETE — archived 2026-05-03**

**Spec:** `.specify/specs/JNL-SYNC-01-pull-based-journal/spec.md`
**Date:** 2026-05-03
**Replaces:** WS-driven journal pipeline shipped in CON-01 / FIX-08 / FIX-09. The
trader-facing live path (`FillDetector` OCO cancels, broadcasts, engine status)
is **out of scope** and stays untouched.

**Strategy:** Seven vertical checkpoints from the spec, expanded into atomic
tasks. CP-1 lands the pure-function reconstruction (RED test first) so every
later CP has a fixed contract to integrate against. CP-2 adds the data source
(sidecar). CP-3 adds persistence. CP-4 wires the CCXT syncer end-to-end. CP-5
mirrors the same shape for Hyperliquid. CP-6 deletes the old WS-driven journal
write path and the filter clauses guarding stale rows. CP-7 cleans up the
desk and adds the "Sync now" affordance. CP-8 verifies + archives.

**Critical commit ordering:**
- CP-6 (deletion) MUST come AFTER CP-4 + CP-5 are merged and producing rows.
  Reversing the order leaves the journal empty between writer-deletion
  and the syncer turning on.
- Within CP-6, the order is: (a) one-time `UPDATE journal_trades SET
  needs_reconciliation = FALSE WHERE …;` (b) drop the filter SQL clauses;
  (c) delete `fill_reconciler.rs` + `emit_trade_closed`. (a) before (b)
  because pre-existing reconciling rows would otherwise vanish from
  aggregations the moment the filter is dropped.

---

## Discoveries

### Codebase baseline — what exists today

- **`fill_reconciler.rs`** (`testudo-exchange/crates/router/src/services/fill_reconciler.rs`)
  is the WS-driven reconciler we replace. Carries `CloseCandidates`,
  `pick_close_leg`, `apply_close_fill`, `rebuild_daily_stats_for_date`. Spawn
  site at `trade_event_writer.rs:362`. Manual sweep route at
  `routes/internal.rs:88` (`reconcile_pending_fills`) registered in
  `main.rs:1200`. **All deleted in CP-6.**

- **`fill_detector.rs`** emits `TradeCloseEvent` via three near-identical
  blocks at `:389, :426, :483`. `emit_trade_closed` (`:604`) constructs the
  payload via `trade_closed_payload.rs`. **CP-6 deletes the
  `emit_trade_closed` calls + helper.** FillDetector retains its OCO-cancel,
  broadcast, and engine-status responsibilities — those live-trading concerns
  stay WS-driven by design.

- **`hl_fill_journal.rs`** — current Hyperliquid journal write path. Already
  writes `needs_reconciliation: false` (HL fills carry canonical avg_px on
  the `userFills` REST endpoint). **CP-5 supersedes this with the new
  `JournalSyncer` Hyperliquid variant**; the existing call site stays put
  through CP-5 to avoid a gap, and CP-6 removes it.

- **`journal_service.rs::TradeCloseEvent`** carries
  `close_candidates: Option<CloseCandidates>`. No more producers after CP-6;
  the type itself plus `record_trade_close` get deleted with the writer.

- **`needs_reconciliation = FALSE` filter sites (25 SQL clauses):**
  - `coach/digest.rs` (8): `:132, :147, :177, :204, :265, :279, :311, :357, :428`
  - `calibration.rs` (2): `:94, :130`
  - `dignitas/snapshot.rs` (1): `:108`
  - `journal_stats.rs` (3): `:405, :436, :486`
  - `journal_timeseries.rs` (4): `:337, :385, :424, :476`
  - `routes/journal.rs` (3): `:229, :242, :252`
  - `routes/user_settings.rs` (1): `:101`
  - `fill_reconciler.rs` (2 — deleted with the file): `:393, :581`
  All listed clauses are dropped in CP-6. Per AGENTS.md "structural outcomes
  not error-string matching": each filter is removed unconditionally — there
  is no transition window. The pre-flip `UPDATE journal_trades SET
  needs_reconciliation = FALSE` blanket-clears any in-flight reconciling
  rows so aggregates remain consistent through the deploy.

- **`exchange_accounts` schema** (`migrations/20250922173255_exchange_accounts.up.sql`)
  has `last_used_at` but NOT `last_synced_exec_time`. CP-3 adds the column.

- **`journal_trades.exchange_fill_id BIGINT`** with partial unique index
  `idx_unique_import_fill ON (user_id, exchange, exchange_fill_id) WHERE
  exchange_fill_id IS NOT NULL` (`migrations/20260326000000_…`). This is
  the HIST-02 importer's idempotency key. **Distinct from `raw_fills` —
  `raw_fills.exec_id` is `TEXT` and keys per-fill, not per-trade.** CP-3
  adds a separate `source_fills_hash TEXT` column on `journal_trades` for
  the new pull-sync upsert key.

- **`cex_history.rs`** — pre-existing direct-REST CEX history fetcher used
  by HIST-02 imports (`fetch_trade_history`, returns `Vec<CexFill>`). It
  does NOT go through the sidecar; it hits each exchange's REST API
  directly with HMAC signing. **Per spec FR-1 we go through the sidecar
  (`POST /trades/since`) instead** — keeps the exchange adapter surface
  inside `testudo-cex/`. `cex_history.rs` stays in place for HIST-02; a
  follow-up may unify the two paths but that's out of scope here.

- **Sidecar endpoints today** (`testudo-cex/src/server.ts`): `/balance`,
  `/order`, `/order/edit`, `/order/cancel`, `/order/fetch`, `/orders/open`,
  `/orders/cancel-all`, `/position`, `/leverage`, `/trades/by-group`
  (FIX-09 fallback). **No `/trades/since` yet.** CP-2 adds it adjacent in
  `handlers.ts`.

- **HL SDK userFills:** `info.user_fills_by_time(user_address, start_ms,
  end_ms_opt, aggregate_by_time_opt)` is already wired in
  `services/hyperliquid/ws_fills.rs:444, :831, :839`. CP-5 reuses this
  call path inside the new `JournalSyncer` HL variant.

- **`SkeletonBar`** primitive shipped in PERF-01 at
  `testudo-journal/src/components/SkeletonBar.tsx` is the reuse target if
  a transient sync indicator is needed; for JNL-SYNC-01 we don't need it
  on row level (rows only appear when complete) — only as the optional
  full-page "syncing…" toast on the manual "Sync now" button.

- **Verification commands per `.specify/memory/constitution.md` §4:**
  - `cd testudo-exchange && cargo clippy --all-targets && cargo test`
  - `cd testudo-cex && bun test`
  - `cd testudo-journal && bun run build`
  - `cd testudo-extension && bun run typecheck` (NOT `bun run build` per
    AGENTS.md — extension defaults must stay prod-URL).

### Design decisions baked into the plan

1. **`reconstruct_trades` lives in `common_utils`**, not router. Pure
   I/O-free function with rich unit tests (CP-1). Matches the established
   `compute_derived_fields` / `canonical_exchange_name` pattern in
   AGENTS.md. Importable by tests without spinning a router fixture.

2. **`FillSource` trait** abstracts the data source. Two impls:
   `CcxtFillSource` (calls sidecar `/trades/since`) and `HyperliquidFillSource`
   (calls HL SDK `user_fills_by_time` directly — no sidecar in the
   HL trade path today, no reason to add one now). Single `JournalSyncer`
   tokio task body parametrized over `Arc<dyn FillSource>`. Per AGENTS.md
   "Pure / async split": the syncer struct is async-bound; the
   reconstruction is pure.

3. **Idempotency key for `journal_trades` upsert from pull-sync:** new
   column `source_fills_hash TEXT NULL` on `journal_trades`, populated as
   `sha256(exec_ids.sorted().join(":"))`. New partial unique index
   `idx_unique_pull_sync_trade ON (user_id, exchange, source_fills_hash)
   WHERE source_fills_hash IS NOT NULL`. Coexists peacefully with
   `idx_unique_import_fill` (HIST-02) and live-trade rows (both
   `source_fills_hash IS NULL`).

4. **`source` enum string** for `journal_trades.source`: existing values
   are `"live_trade"`, `"import"`. Add `"pull_sync"`. No DB constraint —
   the column is `TEXT` today.

5. **Backfill window** is `90d` per spec. **Implementation:** if
   `last_synced_exec_time IS NULL`, query with
   `since_ms = (now − 90d).as_ms()`. After the first successful poll,
   `last_synced_exec_time = max(exec_time)` of returned fills. Resumability
   is automatic: if the process crashes mid-backfill, the next tick reads
   the watermark (now non-NULL after the first batch upsert) and resumes.

6. **`reconstruct_trades` is run over ALL fills for the (user, exchange,
   account) on every tick** per spec algorithm. Spec calls out this is
   O(N²) but acceptable at current scale. Plan accepts; if benchmarking in
   CP-4 shows >100ms per tick at 10k fills, plan adds an incremental
   variant in CP-7 (only re-reconstruct symbols touched by the new
   batch).

7. **Fee-asset normalization:** per spec risk #1, deferred. CP-1 stores
   fees as-is in their native asset, `ReconstructedTrade.fees` sums only
   quote-denominated fees; non-quote fees go into `raw_json` for later
   conversion in JNL-SYNC-02. Document explicitly in CP-1 LEARNINGS.

8. **Side-flip in a single fill** (CCXT does not always surface
   `reduce_only`): treat as one `Sell` close + one `Buy` open in
   `reconstruct_trades` — emit two trades plus a `tracing::warn!` with
   the fill's exec_id. Tests cover the path (CP-1 T3).

9. **HL fills lack `order_id` symmetry with CCXT.** The `RawFill` adapter
   in CP-5 normalizes: `coin → symbol` (`"BTC" → "BTC_USDT"` via
   `canonical_exchange_name`'s sibling `canonicalize_hl_coin`), `dir`
   (`"B"|"A"`) → `FillSide`. Tests pin the mapping.

10. **Frontend cleanup is decoupled from backend deletion** — CP-6
    deletes the writer; CP-7 deletes the UI's reconciling-state
    branches. After CP-6, no rows are written without canonical
    economics, so the `<Show when={t().status !== 'reconciling'}>`
    guards in `TradeRow.tsx` simply become straight cells.

11. **Manual "Sync now" implementation:** new POST endpoint
    `/api/v1/journal/sync` keyed on the authenticated user's currently
    active exchange account. Endpoint sends a notify on a per-account
    `tokio::sync::Notify`; the syncer's interval loop is
    `tokio::select!` on `interval.tick()` vs `notify.notified()` so the
    user-triggered run advances the same watermark. Debounce is
    server-side: 5s minimum between Notify-driven runs per account.

12. **Rollback flag:** `JOURNAL_SYNCER_ENABLED` env (default `true`).
    When `false`, the spawn loop in `main.rs` skips creating syncer
    tasks — falls back to ZERO journal writes. Production-safety: if
    the new pipeline misbehaves, flip the flag and redeploy; rows just
    stop arriving until investigation completes. Old WS-driven writes
    are deleted by then so there is no "old path also runs" failure
    mode.

### Constraints not yet examined and deferred to CP execution

- Exact CCXT `fetchMyTrades` pagination semantics per exchange (Bybit
  has the 7-day window quirk that `cex_history.rs` already handles).
  CP-2 implements pagination by advancing `since` to
  `last_fill_exec_time + 1ms` until an empty page is returned, with a
  per-exchange max-window override map for known restrictions. Specifics
  pinned during CP-2.
- Whether `info.user_fills_by_time` paginates HL fills past the default
  page size — CP-5 verifies, walks if needed.
- Rate-limit headroom against Bybit's 600 req / 5s when 30s × N accounts
  poll concurrently. CP-4 measures; if approaching limits, lengthens the
  interval per-exchange via env overrides (`JOURNAL_SYNC_INTERVAL_BYBIT_SECS`).

---

## Tasks

### CP-1 — Pure `reconstruct_trades` + RED reproducer (FR-5, FR-13)

- [ ] **T1** — Define types in
  `testudo-exchange/crates/common_utils/src/journal/mod.rs` (new module):
  `RawFill { user_id, exchange, exec_id, symbol, side: FillSide,
  price, qty, fee, fee_asset, exec_time, order_id, raw_json }`,
  `enum FillSide { Buy, Sell }`,
  `ReconstructedTrade { user_id, exchange, symbol, side: TradeSide,
  entry_price, exit_price, quantity, fees, realized_pnl, opened_at,
  closed_at, source_fills: Vec<String>, source_fills_hash: String }`.
  Re-export from `common_utils` lib root. *Complexity: simple.*

- [ ] **T2** — RED reproducer: integration-style unit test in
  `common_utils/src/journal/tests.rs`. Ten cases:
  - Long round trip (Buy 1 @100, Sell 1 @110 → one Long trade).
  - Short round trip.
  - **FR-13 case — Bybit manual close:** Buy 0.05 @50_000, Sell 0.05
    @51_000 (no SL/TP IDs, no clientOrderId). Assert exactly one
    Long trade with correct economics — no dependency on WS or
    `OrderGroup` state.
  - Partial entry, single close (Buy 0.5, Buy 0.5, Sell 1).
  - Scaled in / scaled out (Buy 0.3, Buy 0.7, Sell 0.4, Sell 0.6).
  - Side flip in one fill (Buy 1 @100, Sell 2 @110) → two trades.
  - Multi-symbol interleaving — one trade per symbol.
  - Out-of-order timestamps — sort-then-walk produces stable output.
  - Open position not emitted (Buy 1 only → empty Vec).
  - Duplicate exec_id idempotency — same hash on second run.
  - **Late-arriving fill (Gate 1 amendment):** tick 1 sees `[A, B]`
    (round trip closes, hash `H1`); tick 2 sees `[A, B, C]` where `C`
    arrived late on a paginated boundary (Bybit 7-day window quirk).
    `reconstruct_trades` emits a *new* round trip on tick 2 with hash
    `H2 != H1`. Both rows persist (DO NOTHING on conflict but keys
    differ). **Plan elects: document as duplicate-by-design for MVP.**
    `reconstruct_trades` MUST emit `tracing::warn!` (with the new
    hash + fill exec_ids) when a closing fill arrives whose preceding
    accumulator already produced an emitted trade in a prior call —
    observability without semantic change. Cross-source dedup is
    HIST-05 territory; T44 LEARNINGS captures the gap.

  Tests reference `reconstruct_trades` which doesn't exist yet →
  compile fails → tests FAIL. *Complexity: medium.*

- [ ] **T3** — Implement `reconstruct_trades(fills: &[RawFill]) ->
  Vec<ReconstructedTrade>`. Group by symbol, sort by `(exec_time,
  exec_id)`, walk fills accumulating signed net qty, emit a trade
  when `prev_net != 0 && net == 0`. Side-flip mid-fill: split + warn.
  Trailing non-zero accumulator: no emit. Pure function — no clock,
  no DB, no I/O. Verify T2 GREEN. *Complexity: medium.*

- [ ] **T4** — Add `pub fn hash_source_fills(exec_ids: &[String]) ->
  String` (sha256 of sorted, colon-joined IDs). Unit test asserts
  determinism + order-independence. *Complexity: trivial.*

- [ ] **T5** — Verify CP-1: `cd testudo-exchange && cargo clippy
  --all-targets && cargo test`. No new warnings. *Complexity: trivial.*

  Commit (T1–T5): `feat(JNL-SYNC-01): pure reconstruct_trades + 10-case test suite (CP-1)`.

### CP-2 — Sidecar `POST /trades/since` (FR-1)

- [ ] **T6** — Add handler `handleTradesSince` in
  `testudo-cex/src/handlers.ts` (insert near `handleTradesByGroup`).
  Body: `{exchange, credentials, sandbox, symbol?, since_ms,
  until_ms?}`. Walk pagination by advancing `cursor = max(timestamps)
  + 1` until empty page or partial page (<100). Safety ceiling 50
  pages. Stuck-cursor guard. Return wire shape `{exec_id, symbol,
  side, price, qty, fee, fee_asset, exec_time_ms, order_id,
  raw_json}` — all numerics as strings (AGENTS.md trading rule).
  *Complexity: medium.*

- [ ] **T7** — Wire route in `testudo-cex/src/server.ts`:
  `app.post("/trades/since", handlers.handleTradesSince);`.
  *Complexity: trivial.*

- [ ] **T8** — Bun test `testudo-cex/tests/trades-since.test.ts`:
  three pages aggregation, empty-page early-out, stuck-cursor guard.
  *Complexity: simple.*

- [ ] **T9** — Verify CP-2: `cd testudo-cex && bun test`. Green.
  *Complexity: trivial.*

  Commit (T6–T9): `feat(JNL-SYNC-01): sidecar POST /trades/since with pagination-to-exhaustion (CP-2)`.

### CP-3 — `raw_fills` table + watermark column + repository (FR-2, FR-3)

- [ ] **T10** — Migration
  `crates/sqlx_postgres/migrations/20260503000000_raw_fills.up.sql`:
  ```sql
  CREATE TABLE raw_fills (
      user_id        UUID         NOT NULL,
      exchange       TEXT         NOT NULL,
      exec_id        TEXT         NOT NULL,
      symbol         TEXT         NOT NULL,
      side           TEXT         NOT NULL,
      price          NUMERIC(40,18) NOT NULL,
      qty            NUMERIC(40,18) NOT NULL,
      fee            NUMERIC(40,18) NOT NULL DEFAULT 0,
      fee_asset      TEXT         NOT NULL DEFAULT '',
      exec_time      TIMESTAMPTZ  NOT NULL,
      order_id       TEXT         NULL,
      raw_json       JSONB        NOT NULL DEFAULT '{}'::jsonb,
      created_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
      PRIMARY KEY (user_id, exchange, exec_id),
      FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
  );
  CREATE INDEX idx_raw_fills_user_exchange_time
      ON raw_fills(user_id, exchange, exec_time DESC);
  CREATE INDEX idx_raw_fills_symbol
      ON raw_fills(user_id, exchange, symbol);
  ```
  Plus matching `.down.sql`. *Complexity: simple.*

- [ ] **T11** — Migration
  `20260503000001_exchange_accounts_last_synced.up.sql`:
  `ALTER TABLE exchange_accounts ADD COLUMN last_synced_exec_time
  TIMESTAMPTZ NULL;`. Plus `.down.sql`. *Complexity: trivial.*

- [ ] **T12** — Migration
  `20260503000002_journal_trades_source_fills_hash.up.sql`:
  ```sql
  ALTER TABLE journal_trades ADD COLUMN source_fills_hash TEXT NULL;
  CREATE UNIQUE INDEX idx_unique_pull_sync_trade
      ON journal_trades(user_id, exchange, source_fills_hash)
      WHERE source_fills_hash IS NOT NULL;
  ```
  Plus `.down.sql`. *Complexity: trivial.*

- [ ] **T13** — `RawFillRepository` in
  `crates/router/src/repositories/raw_fills.rs`:
  - `upsert_many(&self, fills: &[RawFill]) -> Result<usize, sqlx::Error>` —
    `INSERT … ON CONFLICT (user_id, exchange, exec_id) DO NOTHING`.
  - `fetch_for_account(&self, user_id, exchange) -> Result<Vec<RawFill>>`.
  - `count_for_account(&self, user_id, exchange) -> Result<i64>`.
  Returns count of NEW inserts (idempotency check via
  `RowsAffected`). *Complexity: medium.*

- [ ] **T14** — Watermark accessors on
  `repositories/exchange_account.rs`:
  - `get_last_synced_exec_time(account_id) -> Option<DateTime<Utc>>`.
  - `set_last_synced_exec_time(account_id, ts) -> ()`.
  *Complexity: simple.*

- [ ] **T15** — Integration test in
  `repositories/raw_fills.rs` (`#[cfg(test)] #[tokio::test] #[ignore]`):
  insert 5 fills; re-insert same 5 + 3 new → assert 3 new added;
  `fetch_for_account` returns 8. FK-respecting cleanup with
  `let _ = …`. *Complexity: simple.*

- [ ] **T16** — Verify CP-3: `cargo clippy --all-targets && cargo
  test`. Migration tests run; integration tests gated `#[ignore]`
  runnable manually with `DATABASE_URL=… cargo test -- --ignored`.
  *Complexity: trivial.*

  Commit (T10–T16): `feat(JNL-SYNC-01): raw_fills table + watermark column + repository (CP-3)`.

### CP-4 — `JournalSyncer` for CCXT exchanges (FR-4, FR-7, FR-8, FR-14, FR-15)

- [ ] **T17** — Define `FillSource` trait in
  `services/journal_syncer/mod.rs`:
  ```rust
  #[async_trait]
  pub trait FillSource: Send + Sync {
      async fn fetch_since(&self, user_id: Uuid, account_id: Uuid,
                           since: DateTime<Utc>)
          -> Result<Vec<RawFill>, SyncError>;
      fn exchange_label(&self) -> &str;
  }
  ```
  Plus `SyncError` enum (network / deser / rate_limit / other).
  *Complexity: simple.*

- [ ] **T18** — Implement `CcxtFillSource` in
  `services/journal_syncer/ccxt.rs`. Holds `Arc<CexClient>` +
  `Arc<ExchangeAccountRepository>`. `fetch_since` decrypts credentials
  for the account, calls sidecar `POST /trades/since`, deserializes,
  normalizes to `RawFill` (canonical exchange name + symbol).
  *Complexity: medium.*

- [ ] **T19** — Implement `JournalSyncer` task body in
  `services/journal_syncer/syncer.rs`:
  ```rust
  pub struct JournalSyncer { /* pool, repos, source, notify, interval_secs */ }
  impl JournalSyncer {
      pub async fn run(self, mut shutdown: Receiver<()>) {
          // loop { tokio::select!{ tick | notify | shutdown }; tick().await }
      }
      async fn tick(&self) -> Result<(), SyncError> {
          // 1. read watermark or default to now-90d
          // 2. fetch_since
          // 3. raw_fills.upsert_many
          // 4. set watermark = max(exec_time)
          // 5. fetch ALL fills for account
          // 6. reconstruct_trades
          // 7. journal_trades.upsert_many_pull_sync
      }
  }
  ```
  *Complexity: complex.*

- [ ] **T20** — Add
  `JournalTradeRepository::upsert_many_pull_sync(&[ReconstructedTrade])
  -> Result<usize, sqlx::Error>`:
  ```sql
  INSERT INTO journal_trades (..., source, source_fills_hash,
                              needs_reconciliation, ...)
  VALUES (...)
  ON CONFLICT (user_id, exchange, source_fills_hash)
  WHERE source_fills_hash IS NOT NULL
  DO NOTHING;
  ```
  Per AGENTS.md "ON CONFLICT must repeat the WHERE predicate
  verbatim" for partial unique indexes. Always sets
  `needs_reconciliation = FALSE`, `source = 'pull_sync'`. Idempotent.

  **Gate-1.5 amendment:** when `INSERT … ON CONFLICT DO NOTHING`
  returns `rows_affected = 0` for a trade whose `source_fills_hash`
  is NOT present in the prior tick's reconstruction output (i.e. a
  freshly reconstructed trade that nonetheless conflicts), emit
  `tracing::warn!(target: "journal_syncer", hash, exec_ids,
  "duplicate_hash_emitted")`. This is the observability signal for
  the late-arriving-fill duplicate-by-design semantic that the
  pure `reconstruct_trades` cannot itself detect. Compare against
  a small `HashSet<String>` of hashes from the prior tick stored on
  the `JournalSyncer` struct. *Complexity: small addition.*
  *Complexity: medium.*

- [ ] **T21** — Spawn syncers from `main.rs`:
  - On startup, query
    `SELECT id, user_id, exchange_name FROM exchange_accounts WHERE
    is_active = TRUE AND exchange_name != 'hyperliquid'`.
  - For each row, instantiate `CcxtFillSource` + `JournalSyncer`
    with a fresh `Arc<Notify>`. Spawn
    `tokio::spawn(syncer.run(shutdown_rx))`.
  - Store `(account_id → notify)` map in `AppState` for the manual
    "Sync now" route.
  - Gate the entire spawn block on `JOURNAL_SYNCER_ENABLED` env
    (default `true`).
  - Wire into the existing `account_added` flow: when a new exchange
    account is paired post-startup, the same spawn helper instantiates
    its syncer immediately (so a click on "Sync now" right after
    pairing succeeds).
  *Complexity: medium.*

- [ ] **T22** — Manual sync route `POST /api/v1/journal/sync` in
  `routes/journal.rs`:
  - Auth required (existing JWT middleware).
  - Body `{ exchange_account_id: Uuid }` (optional — defaults to
    user's most-recently-used active account).
  - Server-side debounce: track `(account_id → last_notify_at)` in
    `AppState`; reject 409 if `now - last < 5s`.
  - On success, fire `notify.notify_one()`. Returns 202 Accepted.
  *Complexity: medium.*

- [ ] **T23** — Failure-mode handling per FR-14:
  - Sync errors → `warn!` log, watermark stays put, exponential
    backoff (30s → 60s → 120s → 240s, cap 300s).
  - On 10 consecutive failures, emit a structured `ManagementEvent`
    for the user (existing channel) and continue retrying capped at
    5min. No persistent failure state — recovery is automatic on
    next success.
  *Complexity: medium.*

- [ ] **T24** — Integration test
  `services/journal_syncer/integration_tests.rs`,
  `#[tokio::test] #[ignore]`, `DATABASE_URL` from env:
  - Mock `FillSource` returning a deterministic fill stream.
  - First call: 5 fills = 2 round trips + 1 open.
  - `tick()` once → `raw_fills` has 5 rows; `journal_trades` has 2;
    watermark advanced.
  - Second call: 0 new fills → `tick()` again → no new rows;
    watermark unchanged.
  - **Gate-1.5 amendment — late-arriving-close case:** tick 1
    returns `[Buy 1 @100, Sell 0.5 @110]` (open net 0.5; no trade
    emitted). Tick 2 returns the same two PLUS
    `[Sell 0.5 @120]` (now net 0; trade emitted with both sells).
    Assert exactly ONE trade in `journal_trades` with `entry_price
    = 100`, `exit_price = (110*0.5 + 120*0.5) / 1.0 = 115`. No
    orphan rows. This is the narrower scenario the CP-1 T11 test
    didn't pin — verifies pure-fn correctness end-to-end through
    the syncer.
  - FK-respecting cleanup. *Complexity: medium.*

- [ ] **T25** — Verify CP-4:
  `cd testudo-exchange && cargo clippy --all-targets && cargo test`.
  Manual: `DATABASE_URL=… cargo test --ignored journal_syncer`.
  Both green. *Complexity: trivial.*

  Commit (T17–T25): `feat(JNL-SYNC-01): JournalSyncer for CCXT with manual-sync route + backoff (CP-4)`.

### CP-5 — Hyperliquid syncer variant (FR-6)

- [ ] **T26** — Implement `HyperliquidFillSource` in
  `services/journal_syncer/hyperliquid.rs`:
  - `fetch_since` calls `info.user_fills_by_time(user_addr, since_ms,
    None, None)` per existing `services/hyperliquid/ws_fills.rs:444`.
  - Walk pagination if HL returns >limit fills (verify SDK behavior
    in this task; default behavior pinned by SDK source).
  - Normalize `userFill` → `RawFill`:
    - `coin` → canonical symbol (helper `canonicalize_hl_coin("BTC")
      → "BTC_USDT"` matching existing `ws_fills.rs` mapping).
    - `dir` → `FillSide` (`"B"` → Buy, `"A"` → Sell, etc — pin
      against SDK enum).
    - `tid` → `exec_id`.
    - `closed_pnl` informational only — `reconstruct_trades` derives
      PnL from prices.
  - Tests pin the mapping with synthetic SDK responses.
  *Complexity: medium.*

- [ ] **T27** — Spawn HL syncers in `main.rs`:
  query active HL agent wallets (existing `hl_agent_wallets` schema);
  for each, instantiate `HyperliquidFillSource` + `JournalSyncer`.
  Same `Arc<Notify>` map; same shutdown wiring. *Complexity: simple.*

- [ ] **T28** — Integration test mirroring T24 for HL path
  (`services/journal_syncer/hyperliquid_tests.rs`). Wrap the SDK
  call behind a trait so it's mockable; no live HL hits.
  *Complexity: medium.*

- [ ] **T29** — Verify CP-5: `cargo clippy --all-targets && cargo
  test`. All green. *Complexity: trivial.*

  Commit (T26–T29): `feat(JNL-SYNC-01): JournalSyncer Hyperliquid variant via userFills (CP-5)`.

### CP-6 — Delete WS-driven journal write path + filter clauses (FR-9, FR-10, FR-11)

- [ ] **T30** — Pre-flip data heal migration
  `20260503000003_journal_trades_clear_reconciling.up.sql`:
  ```sql
  UPDATE journal_trades
     SET needs_reconciliation = FALSE
   WHERE needs_reconciliation = TRUE;
  ```
  Per AGENTS.md and Risk #6: prevents reconciling rows from
  suddenly appearing in aggregates the moment T31 drops the filter.
  *Complexity: trivial.*

- [ ] **T31** — Drop the 25 `WHERE needs_reconciliation = FALSE`
  clauses catalogued in Discoveries. For each file, remove the `AND
  needs_reconciliation = FALSE` lines, leave surrounding query
  intact. Files: `coach/digest.rs` (8), `calibration.rs` (2),
  `dignitas/snapshot.rs` (1), `journal_stats.rs` (3),
  `journal_timeseries.rs` (4), `routes/journal.rs` (3),
  `routes/user_settings.rs` (1). Bundle in one commit since they're
  trivially independent edits but must land atomically. Touch ZERO
  behavior other than removing the filter. *Complexity: simple
  (mechanical) but high line count.*

- [ ] **T32** — Delete FillDetector journal-emission path:
  - `services/fill_detector.rs:389, :426, :483` — remove the three
    `self.emit_trade_closed(group, side, true, …)` calls. The
    surrounding terminalization logic (status transitions, OCO
    cancels, extension broadcasts) STAYS.
  - Delete `emit_trade_closed` method (`:604`) and call sites.
  - Delete `services/trade_closed_payload.rs` entirely.
  - Update tests in fill_detector that assert on journal payload.
    **Gate 1 mandate:** any rewrite MUST preserve assertions on
    OCO-cancel paths and extension-broadcast events — those test the
    live-trader responsibilities the spec explicitly keeps untouched.
    Deletion is acceptable ONLY if the OCO-cancel + broadcast
    assertions exist elsewhere in the suite. Document the
    grep-verified location of each preserved assertion in the commit
    message. *Complexity: medium.*

- [ ] **T33** — Delete `services/fill_reconciler.rs` entirely. Plus:
  - Remove `pub mod fill_reconciler` line from `services/mod.rs`.
  - Remove `routes/internal::reconcile_pending_fills`
    (`routes/internal.rs:88`), pub re-exports, and route
    registration in `main.rs:1200`.
  - Delete the FillReconciler spawn block in
    `services/trade_event_writer.rs:362` and surrounding imports.
  - Delete `services/journal_service.rs::TradeCloseEvent`,
    `record_trade_close`, and any `TradeEventType::TradeClosed`
    handling in `trade_event_writer`. Balance-snapshot logic stays —
    extract any coupled balance-snapshot writes into a small
    `record_balance_snapshot` free function before deleting the
    wrapper.
  - Delete `services/hl_fill_journal.rs` (replaced by HL syncer)
    and remove its registration from `services/mod.rs`.
  *Complexity: complex.*

- [ ] **T34** — Strip `close_candidates` field from `TradeCloseEvent`
  if any references survive — should be zero after T33. Compiler
  errors are the gate. *Complexity: trivial.*

- [ ] **T35** — Verify CP-6: `cd testudo-exchange && cargo clippy
  --all-targets && cargo test` green. Manual grep gates:
  ```
  grep -rn "needs_reconciliation\s*=\s*FALSE" testudo-exchange/crates/router/src/  # zero hits
  grep -rn "FillReconciler\|emit_trade_closed\|fill_reconciler" testudo-exchange/crates/router/src/  # zero hits
  grep -rn "reconcile_pending_fills" testudo-exchange/crates/router/src/  # zero hits
  ```
  All three return empty. *Complexity: trivial.*

  Commit (T30–T35): `refactor(JNL-SYNC-01): delete WS-driven journal writer + needs_reconciliation filters (CP-6)`.

### CP-7 — Frontend cleanup + "Sync now" button (FR-7, FR-9)

- [ ] **T36** — Strip reconciling state from
  `testudo-journal/src/components/trades/TradeRow.tsx`:
  - Remove `reconciling()` predicate (~line 13).
  - Remove `<Show … fallback={<SkeletonBar/>}>` wrappers around
    `exit_price`, `pnl`, `r_multiple` cells (~lines 54–66, 73–77).
  - Remove `syncing…` badge JSX.
  - Result: row renders straight columns, no conditionals.
  *Complexity: simple.*

- [ ] **T37** — Drop `needs_reconciliation`, `close_reason`, `status`
  from `JournalTrade` TS interface in
  `testudo-journal/src/api/client.ts:259–286`. *Complexity: trivial.*

- [ ] **T38** — Add "Sync now" button to `/desk/trades/` toolbar:
  - Inline button next to existing controls (locate parent
    component during implementation — likely `pages/Trades.tsx` or
    similar).
  - On click: POST `/api/v1/journal/sync` with active account ID;
    show 1.5s spinner via existing toast/loading primitive; disable
    button for 5s post-click (matches server-side debounce).
  - On 409, show "Sync already running" toast.
  *Complexity: medium.*

- [ ] **T39** — Extension forward-compat: drop optional
  `needs_reconciliation` / `close_reason` / `status` from any popup
  schema in `testudo-extension/src/schemas.ts` if present. Visual
  unchanged. *Complexity: trivial.*

- [ ] **T40** — Verify CP-7:
  - `cd testudo-journal && bun run build` clean.
  - `cd testudo-extension && bun run typecheck` clean (NOT `bun run
    build` per AGENTS.md).
  - Visual smoke: load `/desk/trades/` against a journal with closed
    trades; rows render straight. Click "Sync now"; confirm POST
    fires (Network tab) and spinner shows + clears.
  *Complexity: simple.*

  Commit (T36–T40): `feat(JNL-SYNC-01): desk Sync-Now button + drop reconciling-state UI (CP-7)`.

### CP-8 — Acceptance, backfill, archive

- [ ] **T41** — Acceptance pass against live testnet (manual;
  deferrable per AGENTS.md):
  - **A1 — Manual close on Bybit:** open a position via Testudo,
    close on Bybit web UI, wait ≤30s. Confirm row appears with
    correct economics.
  - **A2 — Backfill on connect:** pair fresh Bybit account with ≥30d
    of prior history. Wait one cycle. Confirm prior round trips
    populate.
  - **A3 — Idempotency:** restart router. Next tick produces zero
    new rows; logs show no errors.
  - **A4 — HL parity:** A1 + A3 against an HL agent wallet.

  If testnet access unavailable, document as "deferred to live
  session" and proceed. *Complexity: simple (operational).*

- [ ] **T42** — Verification matrix (constitution §4):
  - `cd testudo-exchange && cargo clippy --all-targets && cargo test`
  - `cd testudo-cex && bun test`
  - `cd testudo-journal && bun run build`
  - `cd testudo-extension && bun run typecheck`
  All green. Re-grep gates from T35 still empty. *Complexity: trivial.*

- [ ] **T43** — Acceptance grep (FR-9 / spec §"Acceptance Criteria" #5):
  ```
  grep -rn "needs_reconciliation\|FillReconciler\|emit_trade_closed\|reconcile_pending_fills" \
       testudo-exchange/crates/router/src/
  ```
  Zero hits in `src/`. (Old migrations + spec-archive may keep
  references — historical, doesn't fail acceptance.) *Complexity: trivial.*

- [ ] **T44** — Write
  `.specify/specs/JNL-SYNC-01-pull-based-journal/LEARNINGS.md`:
  - Per-tick latency at 1k, 5k, 10k fills (informs incremental-
    reconstruction follow-up).
  - Whether the side-flip case fired (informs whether `reduce_only`
    surfacing is needed earlier).
  - Bybit / HL pagination quirks observed.
  - Fee-asset normalization gap (carries to JNL-SYNC-02).
  - Rate-limit headroom adjustments per exchange.
  - **Gate-1.5 amendment — FR-1 divergence:** spec FR-1 says
    "`exchange.fetchMyTrades`" but CP-2 implements per-exchange
    direct REST in `handlers.ts` (`(exchange as any).xhr.get(…)`)
    because safe-cex's wrapper doesn't expose pagination cursors,
    time-window splitting (Bybit 7d), or fromId continuation
    (Binance) — silent loss past page 1 was unacceptable. Adapter
    surface still inside `testudo-cex/` per the architectural
    goal. New exchanges = new switch arm in `handleTradesSince`.
    Spec FR-1 wording will be updated in a follow-up to match.
  - **Gate-1.5 amendment — perf flag:** `RawFillRepository::upsert_many`
    does N single-row inserts in a loop. At 1000-fill backfill that's
    1000 round trips. Acceptable for one-time pair-account events; if
    A2 acceptance feels slow, batch via `INSERT … VALUES (...), (...), …`.
  - **Gate-1.5 amendment — `String` → `sqlx::Error::Protocol` smell**
    in `RawFillRepository::try_from`. Cosmetic; future cleanup.
  - **Gate 1 amendment:** explicit note that T20's `INSERT … ON
    CONFLICT … DO NOTHING` semantics mean re-running the syncer
    after JNL-SYNC-02 ships fee-normalization will NOT update
    existing `pull_sync` rows. JNL-SYNC-02 must include an explicit
    `UPDATE journal_trades SET fees = …, realized_pnl = … WHERE
    source = 'pull_sync'` migration step keyed on the normalized
    derivation, not a syncer re-run. Document so future-you doesn't
    burn an hour debugging "why didn't fees update."
  - **Late-fill duplicate-row observation:** if T2 case 11's
    `tracing::warn!` ever fires in production, capture frequency
    here — informs whether HIST-05 cross-source dedup needs to ship
    sooner.
  *Complexity: simple.*

- [ ] **T45** — Update root `MEMORY.md` with one-liner: "JNL-SYNC-01
  (May 3 2026): Journal is pull-based — `JournalSyncer` polls
  exchange REST `fetchMyTrades` (CCXT) / `userFills` (HL) every 30s,
  upserts to `raw_fills`, then `reconstruct_trades` (pure) projects
  round trips into `journal_trades`. WS-driven writer + `FillReconciler`
  + `needs_reconciliation` column (kept nullable for back-compat) all
  retired." Strip stale `FillReconciler` references. *Complexity: trivial.*

- [ ] **T46** — Archive spec:
  `mv .specify/specs/JNL-SYNC-01-pull-based-journal/
      .specify/spec-archive/JNL-SYNC-01-pull-based-journal/`. Flip
  this plan's status to "COMPLETE — archived". *Complexity: trivial.*

  Commit (T41–T46): `docs(JNL-SYNC-01): acceptance, LEARNINGS, MEMORY update, spec archive (CP-8)`.

---

## Commit strategy

Per AGENTS.md: NO `Co-Authored-By: Claude` trailers in this repo.
Stage specific files; never `git add -A`.

| # | Tasks | Title |
|---|-------|-------|
| 1 | T1–T5 | `feat(JNL-SYNC-01): pure reconstruct_trades + 10-case test suite (CP-1)` |
| 2 | T6–T9 | `feat(JNL-SYNC-01): sidecar POST /trades/since with pagination-to-exhaustion (CP-2)` |
| 3 | T10–T16 | `feat(JNL-SYNC-01): raw_fills table + watermark column + repository (CP-3)` |
| 4 | T17–T25 | `feat(JNL-SYNC-01): JournalSyncer for CCXT with manual-sync route + backoff (CP-4)` |
| 5 | T26–T29 | `feat(JNL-SYNC-01): JournalSyncer Hyperliquid variant via userFills (CP-5)` |
| 6 | T30–T35 | `refactor(JNL-SYNC-01): delete WS-driven journal writer + needs_reconciliation filters (CP-6)` |
| 7 | T36–T40 | `feat(JNL-SYNC-01): desk Sync-Now button + drop reconciling-state UI (CP-7)` |
| 8 | T41–T46 | `docs(JNL-SYNC-01): acceptance, LEARNINGS, MEMORY update, spec archive (CP-8)` |

Bundling rationale (AGENTS.md "Don't commit broken intermediate states"):
- **CP-3** ships three migrations, repository methods, and watermark
  accessors as one commit — splitting leaves the schema half-evolved.
- **CP-4** ships syncer trait, impl, spawn wiring, manual route, AND
  backoff in one commit. Each piece is unused without the others;
  shipping any subset leaves dead code in `main.rs`.
- **CP-6** ships the heal migration, filter drops, FillDetector
  emission deletion, and FillReconciler module deletion in ONE commit.
  Splitting any of these creates an interim state where the journal
  silently drops rows or fails to compile (compiler-enforced after
  T33).
- **CP-1 must merge before CP-4** — `reconstruct_trades` is the
  syncer's core dependency.
- **CP-4 + CP-5 must both merge before CP-6** — the deletion CP
  removes the only journal-write path before the new path runs would
  produce zero journal rows in production.

---

## Risks (from spec, with mitigations or escalations)

1. **Fee-asset normalization** (spec risk 1) — deferred to JNL-SYNC-02.
   CP-1 stores fees as-is; sums only quote-denominated. T44 LEARNINGS
   captures the gap explicitly. **Acceptance impact:** P&L is correct
   to within fees-paid-in-quote precision; non-quote-denominated fees
   show on raw_fills.raw_json but not subtracted from `realized_pnl`.
   Acceptable for MVP given spec's explicit out-of-scope.
2. **HL fills lack symbol-side parity with CCXT** (spec risk 2) — the
   `HyperliquidFillSource` adapter normalizes (T26). Tests pin the
   mapping.
3. **`fetchMyTrades` rate limits** (spec risk 3) — measured in CP-4
   (T25 manual). Per-exchange interval override env vars exist as
   escape valve. T44 captures the measurement.
4. **`reconstruct_trades` is O(N²)** at 10k fills/account (spec risk 4)
   — accepted at current scale. CP-7 follow-up tracked in LEARNINGS if
   T25 measurement crosses 100ms/tick.
5. **Closing-fill latency** (spec risk 5) — feature not bug. UX surfaces
   it via the manual "Sync now" button in CP-7.
6. **Filter-drop ordering risk** (spec risk 6) — addressed by T30
   (heal migration) before T31 (filter removal) within CP-6.

### Plan-specific risks

7. **Existing HIST-02 importer (`cex_history.rs`) overlap.** Two REST
   history paths now exist — direct (HIST-02) and sidecar
   (`/trades/since`). They write to different tables (`journal_trades`
   directly via importer vs `raw_fills` → reconstruction). The two
   upsert keys are different (`exchange_fill_id` vs
   `source_fills_hash`) so no DB-level conflict. UX may show
   duplicates IFF a user runs both pipelines — cross-source dedup is
   HIST-05 territory per spec "Out of Scope". Document in LEARNINGS.

8. **`hl_fill_journal.rs` removal timing.** This file currently
   writes HL journal rows on WS fill events. CP-5 stands up the new
   HL syncer; CP-6 deletes the old path. Between CP-5 merge and CP-6
   merge, both paths run — risk of duplicate rows. **Mitigation:**
   the new pull-sync upsert keys on `source_fills_hash` (NULL on
   `hl_fill_journal` writes); old WS rows have NULL hash. Different
   key spaces → no DB conflict. UX may show two rows per HL trade
   between commits. **Plan elects:** ship CP-5 and CP-6 as
   back-to-back commits in the same deploy window; the live gap is
   minutes, not hours. If gap is unavoidable, add a one-time
   post-CP-5 SQL hotfix to suppress `hl_fill_journal` writes via env
   flag.

9. **`JOURNAL_SYNCER_ENABLED=false` rollback behavior:** flipping to
   false post-CP-6 means ZERO journal writes happen — the entire
   journal stops accumulating new closed trades. **This is the
   intended rollback semantic** per spec FR-15. Operationally, a
   "stop the bleeding" tool, not a long-term state. T44 LEARNINGS
   documents the behavior so future operators don't misinterpret.

10. **First-tick timing on a freshly-paired account.** A user pairs
    an exchange, then immediately clicks "Sync now". The syncer task
    must be spawned on pair, not just on startup. **Mitigation:**
    T21 wires the spawn helper into the existing `account_added`
    flow. Verified during execution.

---

## Blockers

None. All dependencies in place:
- `common_utils` crate exists; new `journal` submodule is purely additive.
- `testudo-cex` sidecar follows the FIX-09 `/trades/by-group` precedent
  for new endpoints.
- `RawFillRepository` follows the existing `JournalTradeRepository`
  pattern.
- HL SDK `info.user_fills_by_time` already wired in current `ws_fills.rs`.
- Migration tooling (sqlx) handles the four new migrations cleanly.
- The 25-clause filter-drop is mechanical, not architecturally risky
  — caught by `cargo test` regression suite.

---

## PLANNING COMPLETE

Spec: JNL-SYNC-01-pull-based-journal
Total Tasks: 46 (T1–T46)
Ready for BUILD mode.

Next task: **T1** — define `RawFill` / `FillSide` / `ReconstructedTrade`
types in `testudo-exchange/crates/common_utils/src/journal/mod.rs` and
re-export from the crate root. This unblocks T2 (RED reproducer) and
T3 (`reconstruct_trades` implementation), the core of CP-1.
