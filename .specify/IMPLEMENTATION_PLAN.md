# Implementation Plan — FIX-09 REST-Canonical Fill Reconciliation Protocol

**Status: COMPLETE — archived to `.specify/spec-archive/FIX-09-rest-canonical-fills/` (2026-04-28)**

**Spec:** `.specify/spec-archive/FIX-09-rest-canonical-fills/spec.md`
**Depends on:**
- FIX-02 (HL REST reconciliation precedent — pattern reused).
- FIX-08 (`needs_reconciliation` column at
  `crates/sqlx_postgres/migrations/20260424000000_journal_trades_needs_reconciliation.up.sql`;
  `FillReconciler` skeleton at `crates/router/src/services/fill_reconciler.rs`;
  reconciler dispatch at `crates/router/src/services/trade_event_writer.rs:329`).

**Strategy:** Seven vertical checkpoints. CP-1 lands the RED reproducer
first so the rest of the work has a green-light target. CP-2 (typed
`CloseCandidates`, entry exclusion, qty/time gating) is the
architectural heart and lands before CP-3 (WS contract shrink +
shortcut removal) so that stripping `event.price`/`event.average`
doesn't transiently expose the entry-as-exit bug in production. CP-4
(close_reason + status maturity) layers on top. CP-5 (fetchMyTrades
fallback) covers the Bybit ID-less path; CP-6 ships the
"Reconciling…" UX. CP-7 verifies and archives.

---

## Discoveries

### Backend baseline — what FIX-08 already plumbed

- **`OrderUpdateEvent`** (`crates/router/src/services/cex_client.rs:226–250`)
  carries `id`, `symbol`, `status`, `side`, `price` / `amount` /
  `filled` / `remaining` / `average` (all `Option<Decimal>` via
  `deserialize_decimal_opt`), `timestamp`, `user_id`. Wire source:
  `testudo-cex/src/ws-fills.ts:84–95` (`OrderUpdatePayload`). FR-1
  strips the five middle fields on both ends.

- **`fill_detector.rs` cited regions all match the spec exactly:**
  - `:265` — `let exit_price = event.average.or(event.price);` (read site).
  - `:342–376` — CEX-08 proximity logic
    (`(exit-sl).abs() < (exit-tp).abs()`) when neither SL nor TP IDs
    are tracked. FR-7 deletes this entire branch — classification
    moves into the reconciler.
  - `:430–433`, `:472–475`, `:532–535` — three identical
    `let (price, needs_recon) = match action.exit_price { Some(p) if p > Decimal::ZERO => (p, false), _ => (Decimal::ZERO, true) };`
    blocks (SL / TP / ManualClose). FR-2 collapses each to
    `(Decimal::ZERO, true)` unconditionally.
  - `emit_trade_closed` (`:659`) already takes
    `(group, exit_price, close_side, needs_reconciliation)`. CP-3
    drops the now-dead `exit_price` parameter.

- **`FillReconciler`** (`crates/router/src/services/fill_reconciler.rs`)
  exists with the right shape but is the locus of the FR-3 bug:
  - `reconcile_trade(user_id, trade_group_id, exchange, symbol, order_ids: &[String])`
    receives `order_ids` from
    `trade_closed_payload.rs:29–37`, which **pushes
    `group.exchange_order_id` (entry) FIRST**, then SL, then TP.
    `fetch_real_price` iterates in order and returns the first
    `avg_price > 0` — typically the entry's avg. **Today's reconciler
    happily replaces a placeholder `0` exit_price with the entry's
    avg_price.** This bug is latent today (FIX-08's WS-derived price
    skips reconciliation when nonzero); FIX-09 makes WS price always
    0, which would expose this if not fixed in CP-2 first. The
    commit ordering matters.
  - `apply_price_correction` (`:194–269`) writes
    `exit_price / realized_pnl / realized_pnl_pct / net_pnl / r_multiple`
    and flips `needs_reconciliation = FALSE`. **No `close_reason`
    write, no `OrderGroupStatus` update.** Both added in CP-4.
  - Stats aggregation (`:313`) already excludes
    `needs_reconciliation = TRUE` rows — FR-8's daily-stats
    requirement is already met for that surface.

- **Reconciler dispatch site:**
  `crates/router/src/services/trade_event_writer.rs:329–364` spawns
  `FillReconciler::reconcile_trade` post-journal-write whenever
  `close_event.needs_reconciliation` is true. The flat
  `order_ids: Vec<String>` is `close_event.exchange_order_ids`,
  populated by `trade_closed_payload.rs:29–37` from
  `group.exchange_order_id` / `exchange_sl_order_id` /
  `exchange_tp_order_id`. The dispatch must instead pass typed
  `CloseCandidates` — drives FR-3 / FR-4.

- **`SidecarFetchOrderResponse`**
  (`crates/router/src/services/cex_client.rs:362–375`) returns
  `id, symbol, status, side, avg_price: Option<Decimal>, filled: String, fees: String, timestamp: i64`.
  For FR-4's quantity gating we need a `Decimal`-typed filled qty;
  today it's `String`. CP-2 adds a derived `filled_qty: Decimal`
  field (parsed from `filled` at deser time) without breaking the
  existing wire shape. `timestamp: i64` (millis epoch) is sufficient
  for FR-4's clock-drift slop.

- **`TradeCloseEvent`** (`crates/router/src/services/journal_service.rs:17`)
  is the wire-event between fill_detector → trade_event_writer →
  fill_reconciler. Today it carries `exchange_order_ids: Vec<String>`.
  CP-2 replaces with `close_candidates: Option<CloseCandidates>`.
  Per AGENTS.md, add `#[serde(default)]` to defend against legacy
  serialized blobs in pg_queue / WS reconnect paths. CP-4 also adds
  `close_reason: Option<String>` written by reconciler.

- **`OrderGroupStatus`** (`crates/engine/src/shadow/order_group.rs:20–51`)
  has `Pending / Active / StoppedOut / TookProfit / Cancelled / Closed
  / AwaitingReconciliation`. `engine_handle.update_group_status(group_id, status)`
  is the canonical mutation API
  (`fill_detector.rs:289, 363, 378`). Spec's "transient certainty:
  Closed → StoppedOut|TookProfit" is the right protocol. **Decision:**
  do NOT use `AwaitingReconciliation` for live-trade closes — it's
  reserved for rehydration. Live-close transient state stays `Closed`.

- **`JournalTrade` model**
  (`crates/router/src/models/journal.rs:22–57`) has
  `needs_reconciliation: bool` already. **No `close_reason` field.**
  CP-4 adds the column + the model field. The
  `SELECT … FROM journal_trades` column lists in
  `fill_reconciler.rs:109–115` and other call sites must be updated
  (grep target: `realized_pnl, realized_pnl_pct`).

- **Migrations:**
  `20260424000000_journal_trades_needs_reconciliation.up.sql` added the
  bool + a partial index (`WHERE needs_reconciliation = TRUE`). CP-4
  adds `20260427120000_journal_trades_close_reason.up.sql` for the
  `TEXT NULL` column.

### Sidecar baseline

- **`testudo-cex/src/ws-fills.ts:80–115`** — symbol+side-only matching
  loop. Map iteration ambiguity is real: when both SL and TP for one
  group sit in `pendingRemovals`, the first iteration that satisfies
  `symbol === fill.symbol && side === fill.side` wins. After FR-1
  strips price/average from `OrderUpdatePayload`, the matching
  ambiguity remains but its consequences are contained — only `id`
  matters for transition; economics are REST-derived.

- **`testudo-cex/src/server.ts:28–37`** — Express routes today:
  `GET /health`, `POST /balance`, `POST /order`, `POST /order/edit`,
  `POST /order/cancel`, `POST /order/fetch`, `POST /orders/cancel-all`,
  `POST /orders/open`, `POST /position`, `POST /leverage`. Plus
  `WS /ws/orders` via `setupFillStreaming`. **No
  `POST /trades/by-group`** — CP-5 adds it adjacent in `handlers.ts`
  and registers in `server.ts`.

- **`/order/fetch` is Bybit-only today** (`handlers.ts:389–410`,
  hits `/v5/order/history`). CP-5's `/trades/by-group` is
  exchange-agnostic via CCXT
  `fetchMyTrades(symbol, since, undefined, { until })`.

- **safe-cex Bybit `mapOrder` bug** lives in
  `testudo-cex/node_modules/safe-cex/src/exchanges/bybit/bybit.exchange.ts`
  (vendored npm dep, no fork). Per spec risk #5 we explicitly do NOT
  patch it — journal correctness is scope, in-store safe-cex display
  is deferred follow-up.

### Frontend baseline

- **testudo-journal:**
  - `testudo-journal/src/components/trades/TradeRow.tsx:47–54`
    unconditionally renders `formatPrice(t().exit_price)` and
    applies `pnlColor()` / `rColor()`. CP-6 wraps these cells in
    `<Show when={...} fallback={<SkeletonBar />}>`.
  - `JournalTrade` TS type
    (`testudo-journal/src/api/client.ts:259–286`) is missing
    `needs_reconciliation`, `close_reason`, `status`. CP-6 adds them.
  - `SkeletonBar` component from PERF-01 (`src/components/SkeletonBar.tsx`)
    is the reusable shimmer primitive.

- **testudo-extension:**
  - Extension popup does not currently render closed-journal-row UI.
    `MainView.tsx:92` reads `status.startsWith("reconciled_")` but
    the journal-row reconciling UX is testudo-journal's
    responsibility. CP-6's extension touches are limited to schema
    plumbing for forward compat.

- **Daily stats already exclude pending rows** at
  `journal_stats.rs:405` (`AND needs_reconciliation = FALSE`). The
  `list_trades` endpoint at `routes/journal.rs:192–285` does NOT
  filter — by design (FR-9: row must appear in History). CP-6's
  serializer adds the wire-format `status: "reconciling"` /
  `"final"` discriminator; daily stats stay untouched.

### Key drift from the spec text

1. Spec FR-6 references CCXT
   `fetchMyTrades(symbol, since, undefined, { until })`. Confirmed
   implementable on the safe-cex/CCXT layer.
2. The integration test path the spec proposes
   (`router/src/services/integration_tests.rs`) does exist. AGENTS.md
   notes the router crate is binary-only, so the FR-10 reproducer
   lives there as `mod fix09_canonical_fill_tests`,
   `#[tokio::test] #[ignore]`, `DATABASE_URL` from env. A pure-unit
   slice of `pick_close_leg` (no DB) lives inline in
   `fill_reconciler.rs` for fast feedback.
3. `SidecarFetchOrderResponse` lacks `filled_qty: Decimal` — CP-2 (T4)
   adds it as a derived field via custom deserializer; existing
   `filled: String` field stays for backward compat.
4. The reconciler dispatch (trade_event_writer.rs:329) passes a flat
   `Vec<String>` today. **Plan elects:** replace
   `exchange_order_ids` on `TradeCloseEvent` with
   `close_candidates: Option<CloseCandidates>` cleanly in one commit
   — every call site is in-tree; no deprecation window needed.
5. The latent reconciler bug (entry's avg returned as exit_price) is
   masked today by FIX-08's nonzero-WS-price shortcut. **Once CP-3
   strips that path, the bug becomes the production behavior unless
   CP-2 lands first.** Commit ordering enforced.

---

## Tasks

### CP-1 — RED reproducer test (FR-10)

- [ ] **T1** — Add a router-level integration test reproducing the
  Bybit triggered-TP / SL-trigger-price bug. Location:
  `crates/router/src/services/integration_tests.rs`, new module
  `mod fix09_canonical_fill_tests` gated `#[tokio::test] #[ignore]`,
  `DATABASE_URL` from env.

  Harness:
  - Build a `StatefulMockExchangeApi` (existing helper) configured so
    its REST `fetch_order` mock returns the canonical avg fill price
    for the TP order ID, and the entry-avg for the entry order ID.
  - Seed a `journal_trades` row with
    `needs_reconciliation = TRUE`, `exit_price = 0`,
    `trade_group_id = $g`, `exchange = 'bybit'`, side = short.
  - Inject a synthetic `OrderUpdateEvent` shaped like a Bybit
    triggered-TP fill where `event.price = SL_trigger`,
    `event.average = None`, `event.id = SL_id`. Drive
    `FillDetector::handle_event`.
  - Drive `TradeEventWriter` once. Allow the post-write
    `FillReconciler` spawn to settle.
  - Assert `journal_trades.exit_price == REST_TP_avg_price`,
    NOT `SL_trigger`, NOT `entry_avg`. Today: FAILS (entry_avg wins).

  Cleanup: FK-respecting deletion with `let _ = ...` for idempotent
  cleanups (per AGENTS.md).

  *Complexity: medium.*

- [ ] **T2** — Verify the reproducer FAILS:
  `cd testudo-exchange && DATABASE_URL=… cargo test fix09_canonical_fill -- --ignored`.
  Capture failure output; paste into commit body for baseline audit.
  *Complexity: trivial.*

  Commit (T1, T2): `test(FIX-09): RED reproducer for Bybit canonical fill (CP-1)`.

### CP-2 — Reconciler discrimination (FR-3, FR-4)

- [ ] **T3** — Add types to
  `crates/router/src/services/fill_reconciler.rs`:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct CloseCandidates {
      pub entry_order_id: Option<String>,
      pub sl_order_id: Option<String>,
      pub tp_order_id: Option<String>,
      pub manual_close_order_id: Option<String>,
      pub group_terminalized_at: DateTime<Utc>,
      pub expected_qty: Decimal,
      pub qty_tolerance: Decimal,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum CloseReason { StopLoss, TakeProfit, Manual }

  impl CloseReason {
      pub fn as_str(&self) -> &'static str {
          match self { Self::StopLoss => "sl",
                       Self::TakeProfit => "tp",
                       Self::Manual => "manual" }
      }
  }

  #[derive(Debug, Clone)]
  pub struct CloseFill {
      pub exit_price: Decimal,
      pub close_reason: CloseReason,
      pub matched_order_id: String,
      pub transaction_time: DateTime<Utc>,
  }
  ```
  *Complexity: simple.*

- [ ] **T4** — Add `filled_qty: Decimal` to `SidecarFetchOrderResponse`
  in `crates/router/src/services/cex_client.rs:362–375`. Keep
  existing `filled: String` field untouched. Add via
  `#[serde(default, deserialize_with = "decimal_from_string_field")]`
  parsing the same string at deser time. Unit test asserts:
  `"0.09"` → `dec!(0.09)`, `""` → `Decimal::ZERO`, missing → `Decimal::ZERO`.
  *Complexity: simple.*

- [ ] **T5** — Refactor `fill_reconciler.rs`'s `fetch_real_price` →
  `pick_close_leg`:
  ```rust
  async fn pick_close_leg(
      &self,
      exchange: &str,
      creds: &SidecarCredentials,
      symbol: &str,
      candidates: &CloseCandidates,
  ) -> Option<CloseFill> {
      let backoffs = [Duration::ZERO, Duration::from_secs(1), Duration::from_secs(4)];
      let slop = chrono::Duration::seconds(90);
      let cutoff = candidates.group_terminalized_at - slop;

      for (attempt, backoff) in backoffs.iter().enumerate() {
          if attempt > 0 { tokio::time::sleep(*backoff).await; }

          // Exit-only candidate set. Entry deliberately excluded.
          let exit_candidates: [(Option<&String>, CloseReason); 3] = [
              (candidates.sl_order_id.as_ref(),           CloseReason::StopLoss),
              (candidates.tp_order_id.as_ref(),           CloseReason::TakeProfit),
              (candidates.manual_close_order_id.as_ref(), CloseReason::Manual),
          ];

          for (id_opt, reason) in exit_candidates {
              let Some(id) = id_opt else { continue };
              let Ok(resp) = self.cex_client
                  .fetch_order(exchange, creds, false, symbol, id).await
              else { continue };

              let Some(price) = resp.avg_price else { continue };
              if price <= Decimal::ZERO { continue; }
              if (resp.filled_qty - candidates.expected_qty).abs()
                 > candidates.qty_tolerance { continue; }
              let tx_time = chrono::DateTime::<Utc>::from_timestamp_millis(resp.timestamp)
                  .unwrap_or(candidates.group_terminalized_at);
              if tx_time < cutoff { continue; }

              return Some(CloseFill {
                  exit_price: price,
                  close_reason: reason,
                  matched_order_id: id.clone(),
                  transaction_time: tx_time,
              });
          }
      }
      None
  }
  ```
  Delete old `fetch_real_price`. *Complexity: medium.*

- [ ] **T6** — Update `reconcile_trade` signature in
  `fill_reconciler.rs`:
  - Old: `pub async fn reconcile_trade(&self, user_id, trade_group_id, exchange, symbol, order_ids: &[String])`.
  - New: `pub async fn reconcile_trade(&self, user_id, trade_group_id, exchange, symbol, candidates: CloseCandidates)`.
  - On `Some(close_fill)` from `pick_close_leg`, call new
    `apply_close_fill(&trade, &close_fill)` (T7).
  *Complexity: simple.*

- [ ] **T7** — Refactor `apply_price_correction` →
  `apply_close_fill(&self, trade: &JournalTrade, fill: &CloseFill)`.
  CP-2 scope: write `exit_price = fill.exit_price` and recomputed
  derived fields exactly as `apply_price_correction` does today; keep
  `rebuild_daily_stats_for_date` call. `close_reason` write + status
  upgrade are added in CP-4 (T23, T24). *Complexity: simple.*

- [ ] **T8** — Update `TradeCloseEvent` in
  `crates/router/src/services/journal_service.rs:17`:
  - Replace `exchange_order_ids: Vec<String>` with
    `close_candidates: Option<CloseCandidates>`.
  - Add `#[serde(default)]` per AGENTS.md.
  - `record_trade_close` insert SQL is unchanged (the field is not
    persisted directly on the row).
  *Complexity: simple.*

- [ ] **T9** — Update emitter in
  `crates/router/src/services/trade_closed_payload.rs:13–53`:
  - Drop the flat `exchange_order_ids` Vec construction.
  - Build `CloseCandidates` from `&OrderGroup`:
    - `entry_order_id = group.exchange_order_id.clone()`
    - `sl_order_id = group.exchange_sl_order_id.clone()`
    - `tp_order_id = group.exchange_tp_order_id.clone()`
    - `manual_close_order_id`: new param threaded down from
      `fill_detector::FillKind::ManualClose` (T10).
    - `group_terminalized_at = chrono::Utc::now()`
    - `expected_qty = group.quantity`
    - `qty_tolerance`: pick the stricter of absolute
      `dec!(0.0000001)` (8-decimal precision rule per AGENTS.md) vs
      relative `expected_qty * dec!(0.001)` per call. Document
      choice with `// FR-4` comment.
  *Complexity: medium.*

- [ ] **T10** — Update `fill_detector.rs` to thread
  `manual_close_order_id` for `FillKind::ManualClose` into
  `emit_trade_closed`. Minimal CP-2 wiring; CP-3 collapses the rest
  of the surrounding logic. *Complexity: simple.*

- [ ] **T11** — Update reconciler dispatch in
  `crates/router/src/services/trade_event_writer.rs:329–364`:
  pass `close_event.close_candidates.clone().unwrap_or_else(...)` to
  `FillReconciler::reconcile_trade` instead of the flat
  `exchange_order_ids` list. *Complexity: simple.*

- [ ] **T12** — Inline pure-unit tests at the bottom of
  `fill_reconciler.rs` under `#[cfg(test)] mod pick_close_leg_tests`:
  - **Entry exclusion:** entry_id only set, mock returns avg_price=100;
    assert `pick_close_leg → None`.
  - **Qty mismatch:** TP candidate, expected=0.09, tolerance=0.001,
    fetched filled_qty=0.05; assert `None`.
  - **Time slop:** TP candidate, tx_time = terminalized_at − 91s;
    assert `None`.
  - **Happy SL:** SL candidate, qty + time match; assert
    `Some(CloseFill { close_reason: StopLoss, … })`.
  - **Happy TP:** TP candidate, qty + time match; assert
    `Some(CloseFill { close_reason: TakeProfit, … })`.
  - **Manual:** manual_close_order_id present + qty match; assert
    `Some(CloseFill { close_reason: Manual, … })`.
  - **SL+TP both, only TP filled:** SL fetch returns avg_price=None;
    TP returns valid avg; assert TP wins, close_reason=TakeProfit.
  Use a trait-mocked `CexClient` (existing pattern in the router
  crate). *Complexity: medium.*

- [ ] **T13** — Verify CP-2:
  `cd testudo-exchange && cargo clippy --all-targets && cargo test`.
  All inline unit tests green. Reproducer (T1) remains RED — economics
  still flow through old WS paths until CP-3. *Complexity: trivial.*

  Commit (T3..T13): `feat(FIX-09): typed CloseCandidates + entry-excluded reconciler discrimination (CP-2)`.

### CP-3 — WS contract shrink + shortcut removal (FR-1, FR-2, FR-7)

- [ ] **T14** — Strip economic fields from `OrderUpdateEvent` in
  `crates/router/src/services/cex_client.rs:226–250`:
  - Remove `price`, `amount`, `filled`, `remaining`, `average`.
  - Compile errors will pinpoint every reader. Expected sites:
    `fill_detector.rs:265, 310`. Bundling the field removal with
    reader fixups (T16, T17, T18) is plan-sanctioned per AGENTS.md
    "Don't commit broken intermediate states".
  *Complexity: medium.*

- [ ] **T15** — Mirror in `testudo-cex/src/ws-fills.ts:80–115`:
  - Remove `price`, `amount`, `filled`, `remaining`, `average` from
    both branches (matched-fill at `:84–95` and
    unmatched-cancellation at `:104–115`).
  - Update the `OrderUpdatePayload` TS type to match.
  - Matching loop logic unchanged (still symbol+side); only emitted
    payload shape shrinks.
  *Complexity: simple.*

- [ ] **T16** — Delete `let exit_price = event.average.or(event.price);`
  at `fill_detector.rs:265`. Delete the `exit_price` field from
  `FillAction`. *Complexity: simple — coupled with T17.*

- [ ] **T17** — Collapse the three `Some(p) if p > Decimal::ZERO`
  shortcuts at `fill_detector.rs:430–433, 472–475, 532–535`:
  - Each becomes
    `self.emit_trade_closed(group, side, /* needs_reconciliation = */ true);`
    after dropping the now-dead `exit_price` parameter from
    `emit_trade_closed`.
  - Update `emit_trade_closed` signature at `:659` accordingly.
  - Update `trade_closed_payload::build_trade_closed_payload` to
    write a literal `"exit_price": "0"` placeholder. Confirm
    `record_trade_close`'s INSERT against the `exit_price NOT NULL`
    column (current default is `Decimal::ZERO` — preserves contract).
  *Complexity: medium.*

- [ ] **T18** — Delete the CEX-08 proximity logic at
  `fill_detector.rs:342–376` (FR-7). Replace with:
  `update_group_status(group_id, OrderGroupStatus::Closed)` →
  `FillKind::ManualClose { filled_order_id }`. Reasoning:
  classification (SL vs TP vs manual) now lives in the reconciler's
  `pick_close_leg`. The dispatcher's job is to terminalize the group
  ("something closed") and emit a placeholder; the reconciler
  resolves the leg.

  Behavioral note (`// FR-7` comment): for ID-less Bybit brackets, the
  initial group status is always `Closed` (transient). CP-4's
  reconciler upgrades to `StoppedOut` or `TookProfit` after REST
  resolves.
  *Complexity: medium.*

- [ ] **T19** — Update existing fill_detector tests that asserted on
  `event.price` / `event.average` reads or on the proximity logic.
  Per AGENTS.md, treat breakage as the regression guard doing its
  job. Rewrite to:
  - "On terminal close, `emit_trade_closed` was called with
    `needs_reconciliation = true`."
  - "Group status transitioned to `Closed` (not `StoppedOut` or
    `TookProfit`) when only the WS path runs without reconciler."
  Grep target: `cargo test -p router 2>&1 | grep FAILED`.
  *Complexity: medium.*

- [ ] **T20** — Verify CP-3:
  `cd testudo-exchange && cargo clippy --all-targets && cargo test`.
  Reproducer (T1) STILL RED — close_reason / status not yet
  authoritative until CP-4. *Complexity: trivial.*

  Commit (T14..T20): `refactor(FIX-09): WS contract is transition-only; reconciler is sole exit-price author (CP-3)`.

### CP-4 — close_reason migration + status maturity (FR-5, FR-8)

- [ ] **T21** — New migration
  `testudo-exchange/crates/sqlx_postgres/migrations/20260427120000_journal_trades_close_reason.up.sql`:
  ```sql
  ALTER TABLE journal_trades
      ADD COLUMN close_reason TEXT NULL;
  ```
  Plus `.down.sql` dropping the column. No index needed —
  `close_reason` is filtered/grouped on rarely.
  *Complexity: trivial.*

- [ ] **T22** — Add `close_reason: Option<String>` to `JournalTrade`
  in `crates/router/src/models/journal.rs:22–57`. Update the
  `SELECT … FROM journal_trades` column list in
  `fill_reconciler.rs:109–115` (loader) and any other site that
  hand-rolls the column list (grep target:
  `realized_pnl, realized_pnl_pct`). *Complexity: simple.*

- [ ] **T23** — Extend `apply_close_fill` (T7) to write `close_reason`:
  ```sql
  UPDATE journal_trades SET
      exit_price = $1,
      realized_pnl = $2,
      realized_pnl_pct = $3,
      net_pnl = $4,
      r_multiple = $5,
      close_reason = $6,
      needs_reconciliation = FALSE,
      updated_at = NOW()
  WHERE trade_group_id = $7 AND needs_reconciliation = TRUE
  ```
  Bind `fill.close_reason.as_str()` for `$6`. *Complexity: simple.*

- [ ] **T24** — Add status maturity in `apply_close_fill`. After the
  UPDATE succeeds and `rows_updated > 0`, call
  `engine_handle.update_group_status(trade_group_id, target_status)`:
  - `CloseReason::StopLoss` → `OrderGroupStatus::StoppedOut`
  - `CloseReason::TakeProfit` → `OrderGroupStatus::TookProfit`
  - `CloseReason::Manual` → no-op (stay `Closed`)

  Wiring: add `engine_handle: EngineHandle` field to
  `FillReconciler`. Today's constructor is
  `FillReconciler::new(pool, cex_client, exchange_repo)`; add a fourth
  `engine_handle` arg. Update the spawn site at
  `trade_event_writer.rs:343`. *Complexity: medium.*

- [ ] **T25** — Surface `status: "reconciling"` in journal API
  responses (FR-8). In `routes/journal.rs:192–285` (`list_trades`),
  add a wrapper struct that re-serializes with a derived `status`
  field (`"reconciling"` when `needs_reconciliation`, else
  `"final"`). For reconciling rows, set `net_pnl = null` and
  `r_multiple = null` (use `Option<Decimal>` in the wrapper).
  Keep `needs_reconciliation` in the payload for backward compat /
  debug, but `status` is the canonical read field.

  Daily stats endpoints already exclude pending rows
  (`journal_stats.rs:405`) — no change there.
  *Complexity: medium.*

- [ ] **T26** — Verify CP-4:
  `cd testudo-exchange && cargo clippy --all-targets && cargo test`.
  Run reproducer:
  `DATABASE_URL=… cargo test fix09_canonical_fill -- --ignored`.
  **Expected: GREEN.** CP-2 + CP-4 together produce correct
  exit_price + close_reason; CP-3's WS-transition-only contract
  feeds the right zero-price → reconciler path. *Complexity: trivial.*

  Commit (T21..T26): `feat(FIX-09): close_reason + status maturity in reconciler (CP-4)`.

### CP-5 — fetchMyTrades fallback for ID-less brackets (FR-6)

- [ ] **T27** — New sidecar handler `handleTradesByGroup` in
  `testudo-cex/src/handlers.ts` (insert near `handleFetchOrder`,
  ~line 389):
  ```ts
  export async function handleTradesByGroup(req, res) {
    const env = parseEnvelope(req.body);
    const { exchange, credentials, symbol,
            since_ms, until_ms, expected_qty, qty_tolerance,
            entry_side } = env;

    const ex = await gateway.getExchange(exchange, credentials);
    const trades = await ex.fetchMyTrades(symbol, since_ms, undefined, { until: until_ms });

    const closeSide = entry_side === 'buy' ? 'sell' : 'buy';
    const exp = parseFloat(expected_qty);
    const tol = parseFloat(qty_tolerance);

    const candidate = trades
      .filter(t => t.side === closeSide && Math.abs(t.amount - exp) <= tol)
      .sort((a, b) => b.timestamp - a.timestamp)[0];

    if (!candidate) return res.json({ matched: null });
    res.json({
      matched: {
        order_id: candidate.order ?? candidate.id,
        avg_price: String(candidate.price),
        filled_qty: String(candidate.amount),
        transaction_time_ms: candidate.timestamp,
        side: candidate.side,
      },
    });
  }
  ```
  Wire in `server.ts`:
  `app.post("/trades/by-group", handlers.handleTradesByGroup);`.
  *Complexity: medium.*

- [ ] **T28** — Sidecar test
  `testudo-cex/tests/trades-by-group.test.ts` (bun:test):
  - Mock `gateway.getExchange().fetchMyTrades` to return three trades
    (one matching qty + side, one wrong-side, one wrong-qty).
  - Assert handler returns the matching one.
  - Assert `matched: null` when none match.
  *Complexity: simple.*

- [ ] **T29** — Add `fetch_trades_by_group` method to `CexClient` in
  `crates/router/src/services/cex_client.rs`. Wire shape:
  `POST /trades/by-group` with `since_ms`, `until_ms`, `expected_qty`,
  `qty_tolerance`, `entry_side`. Response struct:
  ```rust
  #[derive(Deserialize, Debug)]
  pub struct SidecarTradesByGroupResponse {
      pub matched: Option<SidecarTradeMatch>,
  }
  #[derive(Deserialize, Debug)]
  pub struct SidecarTradeMatch {
      pub order_id: String,
      #[serde(deserialize_with = "deserialize_decimal")]
      pub avg_price: Decimal,
      #[serde(deserialize_with = "deserialize_decimal")]
      pub filled_qty: Decimal,
      pub transaction_time_ms: i64,
      pub side: String,
  }
  ```
  *Complexity: simple.*

- [ ] **T30** — Extend `pick_close_leg` (T5) with the fallback path.
  When `pick_close_leg` returns `None` AND
  `candidates.sl_order_id.is_none() && candidates.tp_order_id.is_none()`
  (Bybit ID-less bracket), call `cex_client.fetch_trades_by_group(...)`.
  On a match, derive `close_reason`:
  - If only `manual_close_order_id` is set → `Manual`.
  - Otherwise classify via price-vs-stop / price-vs-target distance
    on the matched trade's avg_price. Reuse helper
    `classify_by_price(matched_avg, group.stop_price, group.target_price, group.side)`
    that returns `CloseReason::StopLoss` or `CloseReason::TakeProfit`.

  `// FR-6` comment: REST avg is canonical for economics;
  classification is best-effort when IDs are missing.
  *Complexity: medium.*

- [ ] **T31** — Extend `pick_close_leg_tests` (T12):
  - **ID-less + qty match:** `sl/tp/manual = None`, mock
    `fetch_trades_by_group` returns a match. Assert
    `Some(CloseFill { … })` with `exit_price = matched_avg`.
  - **ID-less + no match:** mock returns `matched: None`. Assert
    `None` (reconciler keeps row flagged for next sweep).
  *Complexity: simple.*

- [ ] **T32** — Verify CP-5:
  `cd testudo-exchange && cargo clippy --all-targets && cargo test`,
  `cd testudo-cex && bun test`. Both green. *Complexity: trivial.*

  Commit (T27..T32): `feat(FIX-09): fetchMyTrades fallback for ID-less brackets (CP-5)`.

### CP-6 — UX reconciling state (FR-9)

- [ ] **T33** — Extend `JournalTrade` TS interface in
  `testudo-journal/src/api/client.ts:259–286`:
  ```ts
  needs_reconciliation?: boolean
  close_reason?: 'sl' | 'tp' | 'manual' | null
  status?: 'reconciling' | 'final'
  ```
  All optional (legacy rows default to `final`). *Complexity: simple.*

- [ ] **T34** — Update `testudo-journal/src/components/trades/TradeRow.tsx`
  rendering at lines 47–54. Each economic cell becomes:
  ```tsx
  <Show
    when={t().status !== 'reconciling'}
    fallback={<SkeletonBar w="3rem" h="0.875rem" />}
  >
    <span class={pnlColor(t().net_pnl)}>{formatCurrency(t().net_pnl)}</span>
  </Show>
  ```
  Apply to `exit_price`, `net_pnl`, `r_multiple` cells. When
  reconciling, omit win/loss color class on the row container —
  use a neutral border. *Complexity: medium.*

- [ ] **T35** — Reuse `SkeletonBar` from PERF-01
  (`src/components/SkeletonBar.tsx`). If it doesn't accept `w`/`h`
  props, add a tiny `<SyncingCell />` wrapper colocated with
  TradeRow. *Complexity: trivial.*

- [ ] **T36** — Extension forward-compat: add
  `needs_reconciliation: z.boolean().optional()` and
  `close_reason: z.enum(['sl', 'tp', 'manual']).optional().nullable()`
  to the appropriate Zod schema in `testudo-extension/src/schemas.ts`
  for closed-trade rows. No visual change. *Complexity: simple.*

- [ ] **T37** — Manual frontend verification:
  `cd testudo-journal && bun run typecheck && bun run build`. Visual:
  load the desk on a journal with a known reconciling row (or
  simulate by toggling `needs_reconciliation = true` for one row in
  the DB) — confirm skeleton bars render in place of $0.00 / -1.5R /
  red coloring; row remains in the list, neutrally styled.
  *Complexity: simple.*

- [ ] **T38** — Verify the extension build:
  `cd testudo-extension && bun run typecheck` (per AGENTS.md, NOT
  `bun run build` for the extension during verification).
  *Complexity: trivial.*

  Commit (T33..T38): `feat(FIX-09): journal renders reconciling rows with skeleton state (CP-6)`.

### CP-7 — Verify + backfill + archive

- [ ] **T39** — Re-run reproducer to confirm GREEN:
  `cd testudo-exchange && DATABASE_URL=… cargo test fix09_canonical_fill -- --ignored`.
  *Complexity: trivial.*

- [ ] **T40** — Full verification matrix per
  `.specify/memory/constitution.md` §4:
  ```
  cd testudo-exchange && cargo clippy --all-targets && cargo test
  cd testudo-cex && bun test
  cd testudo-journal && bun run build
  cd testudo-extension && bun run typecheck
  ```
  All green. *Complexity: simple.*

- [ ] **T41** — One-off backfill for the 2026-04-27 ETHUSDT trade
  per spec §"Completion Signal":
  - Identify the row:
    `SELECT id, trade_group_id, exit_price, close_reason FROM
     journal_trades WHERE exit_price = 2419 AND closed_at::date =
     '2026-04-27' …`.
  - Prefer (a) flip `needs_reconciliation = TRUE` and let the new
    sweep correct it via the production code path — exercises the
    fix on a real row as a smoke test.
  - Fallback (b) one-off UPDATE: `exit_price = 2369.78`,
    recompute derived fields, set `close_reason = 'tp'`, then
    `rebuild_daily_stats_for_date` for that (user, exchange, date).

  Document the backfill in the commit body. *Complexity: simple.*

- [ ] **T42** — Write
  `.specify/specs/FIX-09-rest-canonical-fills/LEARNINGS.md`:
  - Real measurements: `pick_close_leg` first-attempt success rate
    vs after retry — informs backoff tuning.
  - Whether `/trades/by-group` fallback ever fired in soak window —
    informs whether Bybit ID resolution is broken or rare.
  - Surprising finding: the pre-FIX-09 reconciler silently used
    entry's avg_price as exit_price for every exchange (latent FR-3
    bug). Note for future auditors.
  - Deferred: safe-cex Bybit `mapOrder` in-store display drift.
  *Complexity: simple.*

- [ ] **T43** — Update root `MEMORY.md` with one-liner: "FIX-09
  (Apr 27 2026): WS is transition-only; REST `fetchOrder` /
  `fetchMyTrades` is canonical for journal economics + close_reason.
  `CloseCandidates` excludes entry from candidate set by construction."
  Strip any stale references to `exchange_order_ids: Vec<String>` if
  present. *Complexity: trivial.*

- [ ] **T44** — Archive spec per repo convention:
  `mv .specify/specs/FIX-09-rest-canonical-fills/
       .specify/spec-archive/FIX-09-rest-canonical-fills/`.
  Final `IMPLEMENTATION_PLAN.md` status flip to "COMPLETE — archived".
  *Complexity: trivial.*

  Commit (T39..T44): `docs(FIX-09): LEARNINGS, MEMORY update, spec archive (CP-7)`.

---

## Commit strategy

Per AGENTS.md: NO `Co-Authored-By: Claude` trailers in this repo.
Stage specific files; never `git add -A`.

| # | Tasks | Title |
|---|-------|-------|
| 1 | T1, T2 | `test(FIX-09): RED reproducer for Bybit canonical fill (CP-1)` |
| 2 | T3–T13 | `feat(FIX-09): typed CloseCandidates + entry-excluded reconciler discrimination (CP-2)` |
| 3 | T14–T20 | `refactor(FIX-09): WS contract is transition-only; reconciler is sole exit-price author (CP-3)` |
| 4 | T21–T26 | `feat(FIX-09): close_reason + status maturity in reconciler (CP-4)` |
| 5 | T27–T32 | `feat(FIX-09): fetchMyTrades fallback for ID-less brackets (CP-5)` |
| 6 | T33–T38 | `feat(FIX-09): journal renders reconciling rows with skeleton state (CP-6)` |
| 7 | T39–T44 | `docs(FIX-09): LEARNINGS, MEMORY update, spec archive (CP-7)` |

Bundling rationale (AGENTS.md "Don't commit broken intermediate states"):
- CP-2 contains the type definitions, the reconciler logic, the
  dispatch propagation, and the in-tree tests in one commit because
  the typed `CloseCandidates` is unused without the dispatch update
  at T11; splitting them leaves master with a typed field nothing
  reads.
- CP-3 bundles the WS field strip with the shortcut removal because
  removing the fields without removing the readers breaks compile.
- **Strict ordering: CP-2 must merge before CP-3.** CP-3 makes the
  reconciler the sole exit-price author; CP-2 fixes the
  reconciler's entry-as-exit bug. Reversing exposes the latent bug
  in production.

---

## Risks (with mitigations)

1. **Reconciler latency under exchange REST slowness** (spec risk #1)
   — `fetchOrder` may take 100–500ms. Mitigation: 3-attempt retry
   with backoff already exists; UX explicitly designed for the wait
   in CP-6 (skeleton, not phantom-loss). T42 captures real retry-rate
   for follow-up tuning.
2. **`fetchMyTrades` rate limit on Bybit** (spec risk #2) — V5 allows
   600 req/5s. Mitigation: tight `since` window (`entry_time_ms − 1000`);
   only invoke fallback when ID path fails. T31 covers no-match case.
3. **Clock-drift slop false positives** (spec risk #3) — 90s. The
   combined gating (id-priority + qty match + side match + time
   slop) makes accidental match probability negligible.
4. **Existing tests break** (spec risk #4) — addressed by T19; treat
   breakage as the regression guard.
5. **safe-cex fork drift** (spec risk #5) — vendored bug stays in
   `node_modules/safe-cex`. Documented in T42 as deferred follow-up.
6. **Migration ordering** (spec risk #6) — `close_reason` column must
   exist before reconciler writes it. T21 lands the migration; sqlx
   migration-at-startup pattern handles deploy ordering.

### Plan-specific risks

7. **Latent reconciler bug exposure.** Today's reconciler iterates
   `[entry, sl, tp]` and returns the first non-zero `avg_price` —
   typically entry. FIX-08 masks this because nonzero WS price skips
   reconciliation. **Once CP-3 lands** (`exit_price` always 0 →
   reconciler always invoked), this latent path becomes the
   production path. **Mitigation:** strict commit ordering — CP-2
   merges before CP-3.
8. **`OrderGroupStatus::AwaitingReconciliation` exists but is unused
   here.** Per Discoveries: reserved for rehydration. Live-trade
   transient state is `Closed`; reconciler upgrades to
   `StoppedOut|TookProfit`.
9. **`EngineHandle` injection into `FillReconciler` (T24).** Couples
   the reconciler to the engine actor, but only via a single async
   call (`update_group_status`). Acceptable per AGENTS.md "Pure /
   async split"; documented in code.
10. **Tolerance constant choice** in T9 — qty tolerance balances
    "exchange precision step" vs "false-negative rejection". Plan
    elects: stricter of absolute `dec!(0.0000001)` vs relative
    `expected_qty * dec!(0.001)`. Revisit if T42 shows false
    negatives.
11. **PnlCalendar / Overview cache invalidation.** When the
    reconciler upgrades a row from `reconciling → final`, the journal
    SWR cache learns of the change on next mount (TTL 30s).
    Acceptable for MVP. Follow-up: emit a WS event on reconciliation
    completion that bumps the cache. Out of scope for FIX-09.

---

## Blockers

None. All dependencies are in place: FIX-02 reconciliation pattern,
FIX-08 `needs_reconciliation` schema, existing `FillReconciler`
skeleton, `SidecarFetchOrderResponse` shape, `OrderGroupStatus`
variants, `EngineHandle::update_group_status` API.

---

## PLANNING COMPLETE

Spec: FIX-09-rest-canonical-fills
Total Tasks: 44 (T1–T44)
Ready for BUILD mode.

Next task: T1 — write the RED reproducer integration test in
`crates/router/src/services/integration_tests.rs` documenting the
Bybit triggered-TP / SL-trigger-price bug. Without this, every
subsequent checkpoint lacks a green-light target.
