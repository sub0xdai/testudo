# Implementation Plan — FIX-08 Bybit Fill-Price Reconciliation

**Spec:** `.specify/specs/FIX-08-bybit-fill-reconciliation/spec.md`
**Depends on:** None (code-level fix)
**Strategy:** Four vertical checkpoints from the spec (CP-1..CP-4). Land
the schema + stats-exclusion first (safe for existing writers), then the
sidecar data source, then the "force journaling" change that closes the
missing-trade bug, then the async reconciler that restores price accuracy.
Each CP is independently committable; trades continue to land even if CP-4
fails verification and has to be reworked.

---

## Discoveries

- **Today's skip is literal.** `fill_detector.rs:401-405` emits TradeClosed
  only when `action.exit_price` is `Some`. On Bybit SL/TP fills where safe-cex
  reports `fill.price = null` (stop-market trigger semantics), the sidecar
  forwards `average: null, price: 0-or-triggerPrice`, and the router's
  `event.average.or(event.price)` resolves to `None` or a bogus number.
  Both failure modes land in the journal today — one as a missing row,
  one as a wrong P&L.
- **Reconciliation sweep never writes to the journal.** `reconcile_account`
  updates `OrderGroupStatus` and persists to `managed_positions` via
  `position_repo.mark_closed`, but there is no `trade_event_tx.try_send`
  call anywhere in `reconciliation.rs`. FR-10 is a wiring gap, not a bug.
- **`OrderGroup` has no `side` field.** Position direction is derived at
  emit time from the closing order's side (`sell` → LONG, `buy` → SHORT
  in `emit_trade_closed`). Reconciliation has no close event, so it must
  derive side from `stop_loss_price vs entry_price` (SL < entry → LONG).
  If `entry_price` is missing (shouldn't happen in Active, but guard
  anyway), default LONG — the row is flagged for reconciliation regardless
  and the async reconciler will overwrite derived fields.
- **Dual-emit idempotency is already handled.** WS fill and 30s sweep can
  both produce a TradeClosed for the same group. `insert_journal_trade`
  guards with `SELECT 1 FROM journal_trades WHERE trade_group_id = $1`
  (trade_event_writer.rs:337) before inserting. No additional deduplication
  needed — intentional defence-in-depth, not a race.
- **`needs_reconciliation` has five traversal sites.** Field must land in:
  (1) `emit_trade_closed` payload key, (2) `TradeClosed` JSON schema docs,
  (3) `parse_trade_close_payload` reader + `TradeCloseEvent` struct,
  (4) `JournalTrade` model + `FromRow` derivation, (5) `insert_journal_trade`
  + `record_trade_close` INSERT column lists. Easy to miss one; tasks list
  each explicitly.
- **FR-3 (sidecar `/order/fetch`) is Bybit-only for MVP.** Safe-cex has no
  exchange-agnostic `fetchOrder` method; closed orders are not in `Store`.
  The Bybit path uses `exchange.xhr.get('/v5/order/history', {params:
  {category, orderId}})` directly. Other exchanges return HTTP 501 from the
  endpoint. WOO is follow-up work, out of scope for FIX-08.
- **FR-6 (offline binary) resolution.** The router crate is binary-only
  (no `src/lib.rs`, per AGENTS.md). A second binary under `src/bin/` cannot
  `use router::…` and would need cascading `#[path]` attributes through
  the whole `FillReconciler → CexClient → ExchangeAccountRepository → …`
  tree. The lib-split alternative is a cross-cutting refactor disproportionate
  to a Medium-priority FR. **Resolution:** implement the sweep as a service
  method + PSK-guarded internal admin HTTP endpoint
  (`POST /internal/reconcile-pending-fills`). The spec's literal binary name
  is deferred to a follow-up; operators trigger the sweep with
  `curl -H 'X-Internal-Secret: $SIDECAR_PSK' $ROUTER/internal/reconcile-pending-fills`.
  Documented here so a reviewer isn't blindsided by the missing `[[bin]]`
  section.
- **FR-8 (stats exclusion) is per-query triage.** Aggregations must skip
  flagged rows; user-visible lists/details must show them so traders can
  see "pending" trades. Complete table:

  | File:line | Query purpose | Action |
  |-----------|---------------|--------|
  | `journal_stats.rs:399, 429, 478` | Dashboard aggregations | **EXCLUDE** |
  | `journal_timeseries.rs:331, 378, 416, 467` | Time-series charts | **EXCLUDE** |
  | `coach/digest.rs:130, 144, 173, 199, 259, 272, 303, 348, 420` | Coach aggregations | **EXCLUDE** |
  | `calibration.rs:90, 125` | Dynamic-Risk calibration | **EXCLUDE** |
  | `dignitas/snapshot.rs:115` | Dignitas derivation | **EXCLUDE** |
  | `user_settings.rs:101` | setup_tag coverage COUNT | **EXCLUDE** |
  | `journal_service.rs:696, 735, 763, 784` | Stats/coverage COUNTs | **EXCLUDE** |
  | `journal.rs:143` | Exchange DISTINCT list | **EXCLUDE** (dirty exchanges would clutter the filter chip row) |
  | `journal.rs:155, 165` | Symbol COUNT for filter chips | **EXCLUDE** |
  | `journal.rs:260, 273, 363, 431, 623, 875, 1063, 1148` | List / detail / update | **KEEP** (user must see pending rows) |
  | `journal_service.rs:184` | `trade_group_id` idempotency SELECT | **KEEP** (must still short-circuit on any row) |
  | `trade_event_writer.rs:228, 247, 337` | Draft-merge / auto-tag / idempotency | **KEEP** |
  | `journal_service.rs:616` | `DELETE FROM journal_trades WHERE user_id = $1` (test-teardown) | **KEEP** |

- **`TradeCloseEvent` struct lives in `journal_service.rs`** and is reused
  by `JournalService::record_trade_close` (import path) AND
  `TradeEventWriter::parse_trade_close_payload` (live path). Adding
  `needs_reconciliation: bool` here propagates to both. Imports are never
  flagged (CSV importers carry real fill data); live path sets the flag
  when `exit_price` is missing or zero.
- **Post-commit spawn is already the pattern.** `TradeEventWriter`
  already spawns fire-and-forget balance-snapshot captures
  (trade_event_writer.rs:292). The post-write reconciliation task (FR-1)
  mirrors the same shape — after transaction commit, if the event carries
  `needs_reconciliation: true`, `tokio::spawn(FillReconciler::reconcile_trade(...))`.
- **Exit-price seeding at the reconciliation sweep.** CP-3b has no live
  fill data. Best initial estimate per status transition:
  `StoppedOut → group.stop_loss_price`, `TookProfit →
  group.take_profit_targets[0].price`, `Closed → Decimal::ZERO`. All
  flagged for async reconciliation, which overwrites with the true fill.

---

## Tasks

### CP-1 — Migration + `needs_reconciliation` column plumbing

- [x] **T1** — Migration `20260424000000_journal_trades_needs_reconciliation.up.sql`
  + `.down.sql`.
  `ALTER TABLE journal_trades ADD COLUMN needs_reconciliation BOOLEAN NOT NULL
  DEFAULT FALSE;`
  `CREATE INDEX idx_journal_trades_needs_reconciliation ON journal_trades
  (user_id) WHERE needs_reconciliation = TRUE;` (partial index — backfill
  sweep reads this hot-set cheaply).
  Down migration drops the index then the column. `exit_price` stays
  `NUMERIC NOT NULL`; the 0 placeholder satisfies the constraint.
  *Complexity: simple.*

- [x] **T2** — `models/journal.rs`: add `pub needs_reconciliation: bool`
  to `JournalTrade`. Update every `sqlx::query_as::<_, JournalTrade>` SELECT
  column list across the crate (`journal_service.rs`, `routes/journal.rs`,
  `trade_event_writer.rs` if any, `routes/dignitas/…` if any) to include
  the new column. Compilation will surface the sites — use `cargo check`
  as the driver, do not grep. *Complexity: simple but wide.*

- [x] **T3** — `services/journal_service.rs`: add `needs_reconciliation:
  bool` to `TradeCloseEvent`. Thread into `record_trade_close`
  INSERT. Default `false` at every import-worker and CSV construction
  site (import rows carry real fill data). *Complexity: simple.*

- [x] **T4** — FR-8 stats exclusion sweep. Apply the `AND
  needs_reconciliation = FALSE` clause to every **EXCLUDE** row in the
  Discoveries table. Keep every **KEEP** row untouched. One commit per
  file group so reviewers can audit per-query. Add a one-line justification
  comment above each touched query: `-- FIX-08: exclude unreconciled
  placeholder rows from aggregation`. *Complexity: medium (mechanical but
  touches ~8 files).*

### CP-2 — Sidecar `POST /order/fetch` (Bybit)

- [x] **T5** — `testudo-cex/src/handlers.ts`: add `handleFetchOrder`.
  Reads envelope, extracts `params.orderId` + `params.symbol`.
  For Bybit: `await exchange.xhr.get('/v5/order/history', { params:
  { category: 'linear', orderId } })` → map response row
  (`avgPrice, cumExecQty, cumExecFee, orderStatus, updatedTime`) to wire
  shape `{ id, symbol, status, side, avgPrice: string|null, filled: string,
  fees: string, timestamp: number }`. For exchange ≠ `bybit`, respond
  `501 { error: 'fetchOrder not implemented for <exchange>', code:
  'NotImplemented' }`. All numerics as strings (sidecar contract).
  *Complexity: simple.*

- [x] **T6** — `testudo-cex/src/server.ts`: register
  `app.post("/order/fetch", handlers.handleFetchOrder);` alongside the
  existing `/order`, `/order/edit`, `/order/cancel`. *Complexity: trivial.*

- [x] **T7** — Vitest covering `handleFetchOrder`: Bybit happy path (mock
  xhr returns a filled order), Bybit not-found (returns `404
  OrderNotFound`), non-Bybit exchange returns `501 NotImplemented`. Use
  existing sidecar test harness under `testudo-cex/tests/`. *Complexity:
  simple.*

- [x] **T8** — `crates/router/src/services/cex_client.rs`: add
  `pub async fn fetch_order(&self, exchange_id: &str, creds:
  &SidecarCredentials, sandbox: bool, symbol: &str, order_id: &str)
  -> Result<SidecarFetchOrderResponse, CexClientError>` + a new
  `#[derive(Deserialize)] pub struct SidecarFetchOrderResponse { pub id:
  String, pub status: String, #[serde(default, deserialize_with =
  "deserialize_decimal_opt")] pub avg_price: Option<Decimal>, … }`.
  Map HTTP 501 → `CexClientError::ExchangeError("fetchOrder not
  implemented")`. *Complexity: simple.*

### CP-3 — Force journaling

- [x] **T9** — `services/fill_detector.rs`: change `fn emit_trade_closed`
  signature to `(&self, group: &OrderGroup, exit_price: Decimal,
  close_side: &str, needs_reconciliation: bool)`. Add
  `"needs_reconciliation": needs_reconciliation` to the payload
  `serde_json::json!` block. Replace the three `if let (..., Some(price))`
  guards at lines 401-405, 437-441, and the CEX-08 branch so the method
  is always called:
  ```
  let (price, needs_recon) = match action.exit_price {
      Some(p) if p > Decimal::ZERO => (p, false),
      _ => (Decimal::ZERO, true),
  };
  if let (Some(ref group), Some(ref side)) =
      (&action.group_snapshot, &action.close_event_side) {
      self.emit_trade_closed(group, price, side, needs_recon);
  }
  ```
  Inline tests: new case `test_sl_fill_with_no_price_still_emits_trade_closed`
  (constructs an `OrderUpdateEvent` with both `average=None, price=None` and
  asserts an event with `needs_reconciliation: true` + `exit_price: "0"`
  lands on `trade_event_rx`). *Complexity: medium — lots of tests to
  re-verify.*

- [x] **T10** — `services/trade_event_writer.rs::parse_trade_close_payload`:
  read the new payload key via `payload.get("needs_reconciliation")
  .and_then(|v| v.as_bool()).unwrap_or(false)`. Populate
  `TradeCloseEvent.needs_reconciliation`. Extend `insert_journal_trade`'s
  INSERT column list + bind. Add assertion in existing payload-parse test
  covering both `true` and absent cases. *Complexity: simple.*

- [x] **T11** — `services/reconciliation.rs`: extend `ReconcileAction` with
  `emit_trade_closed_on_terminal: Option<(OrderGroup, Decimal, String)>`
  (group snapshot + seeded exit_price + derived close_side). In
  `determine_reconcile_actions`, for each Active→StoppedOut/TookProfit/Closed
  branch, compute:
  - `seeded_exit_price`: `group.stop_loss_price` (StoppedOut) /
    `group.take_profit_targets[0].price` (TookProfit) / `Decimal::ZERO`
    (Closed).
  - `close_side`: `"sell"` if `group.stop_loss_price < group.entry_price`
    else `"buy"` (long vs short derivation). If either price is None,
    default `"sell"`.
  Populate the new field; leave Pending/AwaitingReconciliation cases as
  `None` (no position ever existed — no journal row).
  *Complexity: medium.*

- [x] **T12** — `services/reconciliation.rs`: add `trade_event_tx:
  Option<mpsc::Sender<TradeEvent>>` + `with_trade_event_sender`
  builder method mirroring `FillDetectorService::with_trade_event_sender`.
  In the `reconcile_account` execution phase, after the `engine_handle.
  update_group_status` call, if `action.emit_trade_closed_on_terminal`
  is Some and the group wasn't already terminal before the sweep, build
  a TradeEvent with `needs_reconciliation: true` and
  `try_send` it on `trade_event_tx`. Reuse the payload builder logic from
  `FillDetectorService::emit_trade_closed` — extract into a `pub(crate)
  fn build_trade_closed_payload(group, exit_price, close_side,
  needs_recon) -> serde_json::Value` free helper in a new
  `services/trade_closed_payload.rs` so both services share it (DRY per
  AGENTS.md "`pub(crate) fn` free helpers over methods when shared").
  *Complexity: medium.*

- [x] **T13** — `main.rs` wiring: clone the existing
  `trade_event_tx` channel sender into `ReconciliationService` via the
  new `.with_trade_event_sender(tx)` call at construction. Verify the
  existing `trade_event_rx` consumer (TradeEventWriter) handles the
  additional load without channel capacity changes — channel size is
  already 1024 per `main.rs`. *Complexity: simple.*

- [x] **T14** — Inline test in `reconciliation.rs`: given an Active
  group with position gone + SL missing, `determine_reconcile_actions`
  produces an action whose `emit_trade_closed_on_terminal.seeded_exit_price`
  equals `group.stop_loss_price` and whose side derivation matches
  long/short orientation. *Complexity: simple.*

### CP-4 — Post-write reconciliation + daily-stats rebuild

- [x] **T15** — New module `services/fill_reconciler.rs`:
  ```rust
  pub struct FillReconciler {
      pool: PgPool,
      cex_client: Arc<CexClient>,
      exchange_repo: Arc<ExchangeAccountRepository>,
  }
  impl FillReconciler {
      pub async fn reconcile_trade(
          &self,
          user_id: Uuid,
          trade_group_id: Uuid,
          exchange: &str,
          symbol: &str,
          order_ids: &[String],
      ) -> Result<(), FillReconcilerError> { … }
  }
  ```
  Logic:
  1. Load `journal_trades` row by `trade_group_id`. Return `Ok(())` if
     `needs_reconciliation = false` (already fixed by a prior attempt or
     the WS event landed with a real price first).
  2. Resolve `ExchangeAccount` for `(user_id, exchange)` via
     `BalanceSnapshotService::resolve_account_id`. Load decrypted creds.
  3. Iterate `order_ids`; for each call
     `cex_client.fetch_order(exchange, &creds, sandbox, symbol, order_id)`
     until one returns an `avg_price > 0`. Retry per ID with `1s → 4s →
     16s` exponential backoff (FR-4; max 3 attempts across the whole
     operation, not per order).
  4. On success: recompute derived fields via
     `compute_derived_fields(&TradeCloseEvent{…})` and
     `UPDATE journal_trades SET exit_price = $1, realized_pnl = $2,
     realized_pnl_pct = $3, net_pnl = $4, r_multiple = $5, fees = $6,
     needs_reconciliation = FALSE, updated_at = NOW() WHERE
     trade_group_id = $7 AND needs_reconciliation = TRUE` (the TRUE
     predicate makes the update idempotent against a concurrent sweep).
  5. On final failure after 3 attempts: log warning with
     `trade_group_id`, keep `needs_reconciliation = TRUE`. A later
     offline sweep (T19) or next restart picks it up.
  Inline tests: pure derived-fields recompute, decision logic around
  the idempotent update predicate. *Complexity: medium.*

- [x] **T16** — FR-5 daily-stats rebuild. After a successful
  `reconcile_trade` UPDATE, call a new helper
  `async fn rebuild_daily_stats_for_date(pool, user_id, exchange, date)`
  that:
  1. Deletes the existing `journal_daily_stats` row for
     `(user_id, stat_date, exchange)`.
  2. Re-aggregates from `journal_trades WHERE user_id = $ AND exchange =
     $ AND DATE(closed_at) = $ AND needs_reconciliation = FALSE`.
  3. Runs the scoped cumulative-pnl/drawdown recompute SQL already
     present in `trade_event_writer.rs:467-492` (refactor that into a
     shared helper first so both sites call the same SQL — AGENTS.md
     DRY pattern). *Complexity: medium.*

- [x] **T17** — `trade_event_writer.rs::flush_transaction` post-commit:
  after the existing balance-snapshot spawn block, if
  `close_event.needs_reconciliation == true`, `tokio::spawn(async move {
  FillReconciler::new(pool, client, repo).reconcile_trade(user_id,
  group_id, &exchange, &symbol, &order_ids).await })`. Construct the
  reconciler with the same `cex_client` / `exchange_repo` handles the
  writer already holds. `trade_event_writer.rs` gains a new
  `fill_reconciler_enabled: bool` toggle from `AppState` so tests can
  disable the spawn. *Complexity: simple.*

- [x] **T18** — `main.rs` wiring: pass `cex_client` and `exchange_repo`
  into `TradeEventWriter::new` (they are already `Arc`-cloned in the
  `AppState`). Confirm no ordering issue — writer runs after
  `main()` already constructs both services. *Complexity: trivial.*

- [x] **T19** — FR-6 internal admin endpoint.
  `POST /internal/reconcile-pending-fills` guarded by the same
  `X-Internal-Secret` header the sidecar already uses. Handler queries
  `SELECT trade_group_id, user_id, exchange, symbol, exchange_order_ids
  FROM journal_trades WHERE needs_reconciliation = TRUE LIMIT 500` and
  fans out `FillReconciler::reconcile_trade` calls (bounded by a
  `futures::stream::iter(…).buffer_unordered(8)` semaphore). Returns
  `{ attempted, succeeded, still_pending }`. Register in `main.rs`
  under a new `web::scope("/internal")` with the internal-secret
  middleware. *Complexity: medium.*

- [x] **T20** — Document the deferral of the literal
  `cargo run --bin reconcile-fills` invocation in LEARNINGS.md and
  `.specify/AGENTS.md`. Operators run the sweep via
  `curl -X POST -H "X-Internal-Secret: $SIDECAR_PSK"
  http://router/internal/reconcile-pending-fills`. A follow-up spec
  can lift the router into a lib+bin layout if the curl surface
  becomes insufficient. *Complexity: trivial.*

### CP-5 — Verification

- [x] **T21** — `cd testudo-exchange && cargo clippy --all-targets &&
  cargo test`. Fix any clippy regressions from the new helper
  extractions. *Complexity: simple.*

- [x] **T22** — `cd testudo-cex && bun run build && bun test`. Confirm
  the new `/order/fetch` endpoint compiles and vitest passes.
  *Complexity: simple.*

- [x] **T23** — Append to `.specify/specs/FIX-08-bybit-fill-reconciliation/
  LEARNINGS.md`:
  - Gotchas: dual-emit idempotency, side derivation without a side
    field, FR-6 deferral rationale, stats-query triage pattern.
  - Manual QA deferral: kill WS connection + fill Bybit trade requires
    a live exchange account; document as "deferred to live session".
  Final commit: `fix(FIX-08): Bybit fill-price reconciliation — force
  journaling, post-write reconcile, stats exclude`. *Complexity: trivial.*

---

## Commit strategy

- **T1 alone** — migration ships reversibly, no code depends on it yet.
- **T2 + T3 bundled** — model field + TradeCloseEvent field touch many
  call sites; unusable individually.
- **T4 alone** — stats-exclusion sweep is self-contained, large diff,
  reviewable as one change.
- **T5 + T6 + T7 bundled** — sidecar endpoint + server wiring + vitest
  ship atomically.
- **T8 alone** — `CexClient::fetch_order` is callable surface, no
  callers yet.
- **T9 + T10 bundled** — FillDetector emit-always depends on
  parse_trade_close_payload handling the new key; one atomic change.
- **T11 + T12 + T13 + T14 bundled** — reconciliation emit path is
  useless without the wiring; all four land together.
- **T15 + T16 bundled** — reconciler module + daily-stats rebuild
  helper; T16 reuses T15's post-update hook.
- **T17 + T18 bundled** — post-commit spawn + main.rs wiring.
- **T19 alone** — admin endpoint is independently valuable (backfill
  tool even before T17 lands).
- **T20 alone** — docs.
- **T21, T22, T23** — verification commits in order.

---

## Blockers

None. All infrastructure in place: `trade_event_tx` mpsc channel exists,
`TradeEventWriter` already has `cex_client` + `exchange_repo` handles
threaded through `AppState`, sidecar handler pattern is established,
`RateLimiter` / internal-secret middleware conventions exist.

---

## PLANNING COMPLETE

Spec: FIX-08-bybit-fill-reconciliation
Total Tasks: 23 (T1–T23)
Ready for BUILD mode.

Next task: T1 — Migration `20260424000000_journal_trades_needs_reconciliation.{up,down}.sql`.
