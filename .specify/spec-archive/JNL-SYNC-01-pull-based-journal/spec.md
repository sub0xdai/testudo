# Specification: Pull-Based Journal — Exchange-History as Source of Truth

**Spec ID:** JNL-SYNC-01-pull-based-journal
**Date:** 2026-05-03
**Status:** Draft
**Class:** Core / Trade Pipeline Architecture
**Priority:** P0 — journal is the desk's product surface and is structurally unreliable on every manual close, every ID-less bracket, every novel exchange edge case. Class of bug recurs per exchange.
**Depends on:** None for green-field work; supersedes the WS-driven journal pipeline introduced in CON-01 / FIX-08 / FIX-09.
**Replaces:** `services/fill_reconciler.rs`, journal-write path in `services/trade_event_writer.rs`, `journal_trades.needs_reconciliation` column, `routes/internal::reconcile_pending_fills`.

---

## Problem Statement

The current journal pipeline is **WebSocket-driven and inverted from how every other journal product on the market works**. The flow is:

`exchange WS → safe-cex sidecar → router FillDetector → emit TradeClosed (placeholder, exit_price = 0) → TradeEventWriter inserts journal_trades row with needs_reconciliation = true → fire-and-forget FillReconciler → REST fetchOrder + fetchMyTrades fallback → UPDATE row to set canonical exit_price`

This architecture has produced four major bug series in three months (FIX-02, FIX-08, FIX-09 CP-1 through CP-7, plus post-deploy patches in `04ca7cf`, `bded31c`, `258cdd2`) and is currently broken on the most common user action: **manually closing a position on the exchange's web UI.** Symptom: row appears in `/desk/trades/` with `status: "reconciling"` and `syncing…` skeleton forever; dashboard / Overview is empty because every aggregate query filters `WHERE needs_reconciliation = FALSE`.

The structural failure modes are unfixable as point fixes:

1. **WS payload semantics are per-exchange.** Bybit's `order` topic emits trigger price for conditionals (FIX-09 root cause). Hyperliquid's `BasicOrder` has no `avg_px` (FIX-02). WOO returns null for `filled`/`remaining`. Every new exchange = new WS-payload audit. Whack-a-mole.
2. **Manual close on exchange UI is invisible to the order-state machine.** The new market-close order has a fresh exchange-assigned ID with no clientOrderId stamp. `groups_by_exchange_order` lookup misses; symbol-based fallback (CEX-08) requires `event.user_id` to be populated and exactly one active group on the symbol. Both can fail.
3. **Reconciler is single-shot.** Post-write reconciliation is fire-and-forget; on final failure it logs `warn!` and leaves the row stuck. There is no scheduled retry. Only a manual `POST /internal/reconcile-pending-fills` admin call re-tries — which means rows can stay "syncing" indefinitely.
4. **Reconciler windows and tolerances are fragile.** `qty_tolerance = 0.0001` against shadow-engine-recorded quantity; any rounding drift kills the match. `since_ms` fallback of `terminate − 1h` misses long-held trades. Bybit's split between `/v5/order/realtime` and `/v5/order/history` can 404 the ID lookup minutes after close.
5. **Backfill is impossible.** A user pairing a new exchange has no way to import their existing trade history. There is no "first sync pulls 90 days" code path because trades are constructed from order events, not fills.

Industry-standard journal products (CoinMarketMan, TraderSync, Edgewonk, TradeZella) avoid every one of these failures by **never trusting WebSocket events for journal economics**. They poll the exchange's REST execution-history endpoint on a fixed cadence, store raw fills keyed on `(exchange, exec_id)`, and reconstruct round-trip trades as a derived projection. A trade "appears in the journal" when the closing fill lands in the next poll — not when an in-process state machine decides it has enough information.

This spec replaces Testudo's WS-driven journal with the pull-based model. The trader-facing path (`FillDetector` OCO cancels, live position management, extension toasts) is **unchanged** — it remains WS-driven, because that path is latency-critical and lossy is acceptable. The journal becomes a separate, independent consumer of the exchange's REST history.

---

## User Stories

- **As a trader**, when I manually close a position on Bybit's web UI, I want the trade to appear in my Testudo journal with correct entry/exit prices and P&L within the next sync cycle, so I don't have to choose between Testudo's risk discipline and the exchange's UX.
- **As a trader**, when I pair a new exchange account, I want my last 90 days of trades imported automatically, so my dashboard reflects my actual trading history from day one.
- **As a trader**, when I view `/desk/trades/`, every row shown represents a finalized round-trip trade with canonical economics — no placeholders, no skeletons, no "syncing…" rows.
- **As an operator**, when adding a new exchange, the journal pipeline requires zero per-exchange WS-payload archaeology — a single REST `fetchMyTrades` (or HL `userFills`) wrapper is sufficient.
- **As an operator**, when a sync run fails (rate limit, 5xx, network), the watermark stays put and the next run retries from the same cursor — there is no "stuck row" failure mode.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | A new sidecar endpoint `POST /trades/since` MUST accept `{exchange_id, credentials, sandbox, symbol?, since_ms, until_ms?}` and return paginated raw fills via `exchange.fetchMyTrades(symbol, since)`. The endpoint MUST internally walk pagination to exhaustion and return all fills as a single response. | P0 | testudo-cex |
| FR-2 | A new table `raw_fills` MUST exist with columns `(user_id UUID, exchange TEXT, exec_id TEXT, symbol TEXT, side TEXT, price NUMERIC, qty NUMERIC, fee NUMERIC, fee_asset TEXT, exec_time TIMESTAMPTZ, order_id TEXT, raw_json JSONB, created_at TIMESTAMPTZ)`. Primary key `(user_id, exchange, exec_id)`. Idempotent on conflict (upsert no-op). | P0 | sqlx_postgres migration |
| FR-3 | `exchange_accounts` MUST gain a `last_synced_exec_time TIMESTAMPTZ NULL` column. NULL means "never synced; first run pulls 90 days." | P0 | sqlx_postgres migration |
| FR-4 | A new service `JournalSyncer` MUST run a tokio interval task per `(user_id, exchange_account_id)` at `JOURNAL_SYNC_INTERVAL_SECS` (default 30). On each tick: fetch fills since watermark (or `now − 90d` if NULL), upsert to `raw_fills`, advance watermark to `max(exec_time)` of returned fills, then call `reconstruct_trades` over all fills for that account and upsert into `journal_trades`. | P0 | router |
| FR-5 | A pure function `reconstruct_trades(fills: &[RawFill]) -> Vec<JournalTrade>` MUST group fills by symbol, sort chronologically, and emit one `JournalTrade` per round trip (net-position-crosses-zero). Fee-asset normalization, partial fills, side flips, and reduce-only fills MUST be handled. The function MUST be I/O-free and unit-testable in isolation. | P0 | router or common_utils |
| FR-6 | Hyperliquid accounts MUST follow the same architecture using HL's native `userFills` / `info::userFillsByTime` REST endpoint. A parallel `JournalSyncer` variant or a polymorphic adapter MUST cover both CCXT and HL paths. | P0 | router/hyperliquid |
| FR-7 | The desk's `/desk/trades/` route MUST gain a "Sync now" affordance that triggers an out-of-band sync for the active account, debounced 5s. Implementation MAY be: a new POST endpoint `/api/v1/journal/sync` that wakes the syncer for that `(user, account)`. | P1 | router + testudo-journal |
| FR-8 | The first sync after pairing an exchange MUST backfill 90 days of history. Subsequent syncs MUST be incremental from the watermark. Backfill MUST be resumable — if the process restarts mid-backfill, the next run continues from the last-seen `exec_time`. | P0 | router |
| FR-9 | The journal API MUST drop the `status` field and `needs_reconciliation` filter. Every row returned is final. The `WHERE needs_reconciliation = FALSE` clauses at `routes/journal.rs:229,242,252` MUST be removed; aggregation queries become unconditional. | P0 | router/routes/journal |
| FR-10 | `services/fill_reconciler.rs` MUST be deleted. The journal-write path in `services/trade_event_writer.rs` (everything gated on `TradeEventType::TradeClosed`) MUST be deleted; balance-snapshot logic (independent of journal) MUST be preserved by extraction. `routes/internal::reconcile_pending_fills` MUST be deleted. `journal_trades.needs_reconciliation` and `journal_trades.close_reason` columns become `NULL`-able and unwritten — kept for backwards-compat, scheduled for drop in a follow-up migration. | P0 | router |
| FR-11 | `FillDetector` MUST stop emitting `TradeClosed` events. The `emit_trade_closed` function and all `Some(&filled_order_id)` calls at `fill_detector.rs:389,426,483` MUST be removed. FillDetector retains its OCO-cancel responsibility, broadcast events to the extension, and engine status updates — those are live-trading concerns, not journal concerns. | P0 | router/fill_detector |
| FR-12 | Open positions on the desk MUST continue to source from the existing `OrderGroup` engine state and `fetchPositions` REST poll. The `journal_trades` table is closed-trades-only by construction — `reconstruct_trades` cannot emit a row for a position with non-zero net size. | P0 | desk/journal API |
| FR-13 | A regression test MUST exist that simulates a Bybit manual close: synthetic fill rows for an entry + a closing market order with `qty == entry_qty` and opposite side, both upserted to `raw_fills`, and asserts `reconstruct_trades` emits exactly one `JournalTrade` with correct entry/exit prices and P&L, with no dependency on WS events or `OrderGroup` state. | P0 | router (integration test) |
| FR-14 | Sync failures (rate limit, 5xx, deserialization error) MUST log `warn!` and leave the watermark unadvanced. The next tick retries from the same cursor. After N consecutive failures (default 10), the syncer for that account MUST emit a structured error event for the user (mechanism: existing `ManagementEvent` channel) and continue retrying with exponential backoff up to a 5-minute ceiling. | P1 | router |
| FR-15 | The `JOURNAL_SYNCER_ENABLED` env flag MUST exist (default `true`). When false, the syncer task is not spawned; this is the rollback switch. | P1 | router/main |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | RED reproducer + `reconstruct_trades` pure function. Unit tests covering: simple long round trip, simple short round trip, partial entry fills + single close, scaled-in entry + scaled-out exit, manual close (no SL/TP), side flip (close + reverse in one trade), fee-asset variance, multi-symbol interleaving, out-of-order timestamps. RED: a test asserting "given a sequence of fills representing a Bybit manual close, exactly one round-trip JournalTrade is produced with correct economics" — fails because the function doesn't exist yet. | Pure logic is exhaustively tested before any I/O is written. |
| CP-2 | Sidecar `POST /trades/since` endpoint in `testudo-cex/`. Tests against Bybit, WOO, Binance via existing fixture exchanges. Pagination-to-exhaustion verified. | The data source exists and works for all CCXT exchanges. |
| CP-3 | `raw_fills` table migration + `RawFillRepository` (upsert, fetch_since, get_watermark, set_watermark) + `last_synced_exec_time` column on `exchange_accounts`. sqlx integration tests on a clean DB. | Persistence layer is in place; idempotent on `(exchange, exec_id)`. |
| CP-4 | `JournalSyncer` service for CCXT exchanges. Tokio interval task. Spawned from `main.rs` per active account. Wired to `cex_client`, `RawFillRepository`, `JournalTradeRepository`, `reconstruct_trades`. End-to-end integration test: pair a (mocked) account, advance simulated time, assert journal_trades populates correctly. | The CCXT half of the new pipeline runs end-to-end. |
| CP-5 | Hyperliquid syncer variant using HL native SDK `info::userFillsByTime`. Parallel test suite. | Both exchange families work identically from the desk's perspective. |
| CP-6 | Deletion checkpoint. Remove `fill_reconciler.rs`, journal-write path in `trade_event_writer.rs`, `routes/internal::reconcile_pending_fills`, `emit_trade_closed` and call sites, `WHERE needs_reconciliation = FALSE` filters. Update tests. Verify nothing in the FillDetector OCO/cancel/extension-broadcast path regressed. | The old pipeline is gone; the new one is sole authority. |
| CP-7 | Frontend cleanup. Delete `reconciling()` predicate, `<Show>`/`<SkeletonBar>` wrappers around exit/pnl/r-multiple, and `syncing…` badge in `testudo-journal/src/components/trades/TradeRow.tsx`. Drop `needs_reconciliation` and `status` from `JournalTrade` interface in `testudo-journal/src/api/client.ts`. Add "Sync now" button (FR-7) on `/desk/trades/`. Run `bun run build` clean. | UI matches new contract; no dead state branches; manual-sync affordance live. |

### Key Types

```rust
// crates/router/src/services/journal_syncer/types.rs

#[derive(Debug, Clone)]
pub struct RawFill {
    pub user_id: Uuid,
    pub exchange: String,
    pub exec_id: String,            // exchange's execution/trade ID, unique per fill
    pub symbol: String,             // canonical: "BTC_USDT"
    pub side: FillSide,             // Buy | Sell
    pub price: Decimal,
    pub qty: Decimal,
    pub fee: Decimal,
    pub fee_asset: String,
    pub exec_time: DateTime<Utc>,
    pub order_id: Option<String>,   // exchange order ID; nullable for some HL fills
    pub raw_json: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillSide { Buy, Sell }

// Output of reconstruct_trades — slots into existing JournalTrade row shape.
#[derive(Debug, Clone)]
pub struct ReconstructedTrade {
    pub user_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: TradeSide,            // Long | Short, derived from opening fill
    pub entry_price: Decimal,       // qty-weighted average of opening fills
    pub exit_price: Decimal,        // qty-weighted average of closing fills
    pub quantity: Decimal,
    pub fees: Decimal,              // sum across all fills, fee-asset-normalized
    pub realized_pnl: Decimal,
    pub opened_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    pub source: TradeSource,        // PullSync (new) | LiveTrade (legacy, kept for FK)
    pub source_fills: Vec<String>,  // exec_ids contributing to this trade
    // exchange_fill_id, kelly_inputs, setup_tag joined later from order_groups
}
```

### `reconstruct_trades` algorithm (FR-5)

```text
fn reconstruct_trades(fills: &[RawFill]) -> Vec<ReconstructedTrade>:
    by_symbol: HashMap<String, Vec<&RawFill>> = group_by(fills, |f| f.symbol)

    out = vec![]
    for (symbol, fs) in by_symbol:
        fs.sort_by_key(|f| (f.exec_time, f.exec_id))   // tiebreak on exec_id

        net_qty: Decimal = 0
        opening_side: Option<FillSide> = None
        accum: Vec<&RawFill> = vec![]

        for f in fs:
            let signed = if f.side == Buy { +f.qty } else { -f.qty }
            let prev_net = net_qty
            net_qty += signed

            if opening_side.is_none() && net_qty != 0:
                opening_side = Some(f.side)

            accum.push(f)

            // Round trip closes when net_qty crosses or hits exactly zero.
            if prev_net != 0 && net_qty == 0:
                out.push(build_trade_from(accum, opening_side, symbol))
                accum.clear()
                opening_side = None
            else if prev_net.signum() != 0
                 && net_qty.signum() != 0
                 && prev_net.signum() != net_qty.signum():
                // Side flip: split the flipping fill into a closing portion + a new opening
                // portion. Close the trade on the closing portion, start a new accumulator
                // on the opening portion. (Simpler: emit warning + treat as two trades when
                // a single fill flips. CCXT reduce-only flag, when present, prevents this.)
                ...

        // Trailing accum with non-zero net = currently-open position; do NOT emit. (FR-12)

    out
```

Key design constraints:
- **Closed-only.** Open positions never emit a `ReconstructedTrade`. The desk shows them via existing live-state channels.
- **Idempotent.** Re-running over the same fills produces the same trades. Upsert on `journal_trades` keys on `(user_id, exchange, source_fills_hash)` or similar deterministic ID.
- **Pure.** No DB, no clock, no rand. Time comes from `exec_time` in the fills.
- **Fee normalization.** Fees in BNB / HYPE / USDC-PERP-collateral need converting to the trade's quote currency at the fill timestamp. CP-1 may stub this with "fees stored as-is, conversion deferred" and a follow-up checkpoint adds the conversion. Document the gap explicitly.

### `JournalSyncer` task shape (FR-4)

```text
loop:
    sleep JOURNAL_SYNC_INTERVAL_SECS
    for each active (user, exchange_account):
        watermark = exchange_accounts.last_synced_exec_time
                    .unwrap_or(now - 90d)
        fills = sidecar.POST /trades/since {since_ms: watermark.to_millis()}
        if fills.is_empty(): continue
        raw_fills.upsert_all(fills)
        new_watermark = fills.iter().map(|f| f.exec_time).max()
        exchange_accounts.set_watermark(account_id, new_watermark)
        all_fills = raw_fills.fetch_for_account(user, account)
        trades = reconstruct_trades(&all_fills)
        journal_trades.upsert_many(trades)
```

Per-account isolation, exponential backoff on failure, no shared state with `FillDetector`.

### Frontend impact (FR-9, CP-7)

`testudo-journal/src/api/client.ts:286-288` — drop `needs_reconciliation` and `status` from the `JournalTrade` interface. (Optional in CP-6; deletion in CP-7.)

`testudo-journal/src/components/trades/TradeRow.tsx:13,54-66,73-77` — remove `reconciling()` predicate, `<Show fallback={<SkeletonBar/>}>` wrappers around exit/pnl/r-multiple, and the `syncing…` badge. The row becomes a straight render.

No layout changes. No new components. No chart wiring changes. The "Sync now" button is the only additive UI element — a small inline button in the `/desk/trades/` toolbar that POSTs to `/api/v1/journal/sync` and shows a brief spinner while the syncer runs out-of-band. Dashboard / Overview repopulates automatically once the `WHERE needs_reconciliation = FALSE` filter is dropped on the backend (CP-6) — the rows the dashboard was already filtering out simply stop existing.

---

## Acceptance Criteria

1. **Manual close on Bybit:** open a position via Testudo, close it manually on Bybit's web UI, wait ≤30s, see the trade appear on `/desk/trades/` with correct entry/exit/PnL — no `syncing…`, no skeleton, no placeholder.
2. **Backfill on connect:** pair a new Bybit account that has 60 days of prior history. Within one sync cycle the journal contains all reconstructable round trips from the last 90 days.
3. **Idempotency:** restart the router process. The next sync produces zero new rows in `journal_trades` (same data → same outputs) and no errors.
4. **Hyperliquid parity:** the same flows pass on a Hyperliquid account.
5. **No legacy paths reachable:** `grep -r "needs_reconciliation\|FillReconciler\|emit_trade_closed\|reconcile_pending_fills" crates/` returns zero hits in `src/` (only in archived migrations / changelog).
6. **Dashboard non-empty:** with at least one closed trade, the Overview hero stats, equity curve, and per-exchange breakdown all render real values.
7. **Regression test green:** the FR-13 reconstruction test runs in CI and passes.
8. **`cargo clippy --all-targets && cargo test` clean** in `testudo-exchange/`. `bun run build` clean in `testudo-cex/`. `bun run build` clean in `testudo-journal/`.

---

## Risks

| Risk | Mitigation |
|------|------------|
| **Fee-asset normalization across exchanges is messy** (BNB fees on Binance, USDC-margin fees on HL, native-token fees on WOO). Naive sum gives wrong P&L. | CP-1 stores fees as-is and emits trades with `fees = sum_of_quote_denominated_fees_only`, flagging non-quote-denominated fees in `raw_json`. A follow-up spec (`JNL-SYNC-02-fee-normalization`) handles conversion. Document the gap in CP-1's LEARNINGS. |
| **Hyperliquid fills lack symbol-side parity with the CCXT model** (HL uses `coin` not `symbol`, dirs are `B`/`A`). | The `RawFill` adapter in CP-5 normalizes to canonical form before upsert. Tests pin the mapping. |
| **`fetchMyTrades` rate limits** vary per exchange; 30s × N accounts could exceed. | Add per-exchange rate-limit headroom budget; if approaching limits, lengthen the interval for that exchange. The sidecar already has a connection pool. |
| **Re-running `reconstruct_trades` over all fills every tick is O(N²)** for users with many fills. | Acceptable at current scale (single-digit users, ≤10k fills each). Future optimization: incremental reconstruction keyed on symbols touched by the latest poll. Out of scope. |
| **Open positions whose closing fill is delayed** (e.g. exchange returns partial pages stale): the trade appears later than expected. | This is a feature, not a bug — the trade appears when the exchange knows it closed. Document the ≤30s + sync-cadence latency expectation in user-facing copy. |
| **Removing `WHERE needs_reconciliation = FALSE` filters before deleting the writer path could surface stale legacy `needs_reconciliation = true` rows.** | CP-6 ordering: delete the writer path FIRST, then in the same checkpoint drop the filter. Or run a one-time `UPDATE journal_trades SET needs_reconciliation = FALSE` before flipping the filter (acceptable since there are no users; written once, not "manual DB surgery"). |
| **Side flips in a single fill** (rare but possible on `reduce_only=false` market reversals). | Algorithm splits the fill; CP-1 unit-tests cover the case. If CCXT doesn't surface `reduce_only` reliably, fall back to "treat as close + new open" with a logged warning. |

---

## Completion Signal

JNL-SYNC-01 is complete when:

1. All 7 checkpoints land on master with green CI.
2. The acceptance criteria above all pass against a live Bybit testnet account and a Hyperliquid testnet account.
3. `services/fill_reconciler.rs` no longer exists in the tree.
4. The desk's `TradeRow.tsx` has no `reconciling` branch.
5. A clean clone + `cargo test` produces zero failures in router, engine, and sqlx_postgres crates.
6. LEARNINGS.md captures: fee-normalization gap, side-flip handling decisions, observed Bybit/HL pagination quirks, and any rate-limit headroom adjustments made during CP-4/CP-5.
7. The spec is moved to `.specify/spec-archive/JNL-SYNC-01-pull-based-journal/`.

---

## Out of Scope

- **Fee-asset → quote-currency conversion at fill timestamp** (deferred to JNL-SYNC-02).
- **Cross-source dedup** between pull-sync fills and any other ingest (e.g. CSV import, manual entry) — handled separately by HIST-05 if/when relevant.
- **Tier gating of backfill window** (90d for everyone in v1; tiered backfill is a MON-* concern).
- **Multi-account reconciliation** (one user with two Bybit accounts under different API keys) — each `(user, exchange_account)` syncs independently; merging is a UX concern, not a data concern.
- **Open-position economics in the journal table.** Open positions remain on the live engine + REST poll surface (`journal_trades` is closed-only).
- **The trader-facing live order management path.** `FillDetector` OCO cancels, extension toasts, engine status transitions — all unchanged.
