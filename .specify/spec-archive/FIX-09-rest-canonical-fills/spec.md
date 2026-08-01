# Specification: REST-Canonical Fill Reconciliation Protocol

**Spec ID:** FIX-09-rest-canonical-fills
**Date:** 2026-04-27
**Status:** Draft
**Class:** Core / Trade Pipeline Integrity
**Priority:** P0 — journal exit prices are silently wrong on Bybit live closes; user-visible PnL is incorrect; class of bug recurs per exchange.
**Depends on:** FIX-02 (HL REST reconciliation precedent), FIX-08 (`needs_reconciliation` schema + reconciler service).
**Series:** FIX-01 through FIX-09 (Hyperliquid Audit Fix Series + this spec)

---

## Problem Statement

The journal recorded an ETHUSDT short on 2026-04-27 with `exit_price = 2,419` (the SL trigger price) when the trade actually closed at `2,369.78` (TP filled, market close per Bybit Trade History). PnL was reported as `−$2.34 / −1.5R` when the trade was a winner. Investigation revealed three layered causes:

1. **safe-cex Bybit `mapOrder` (`safe-cex-sub0/.../bybit.exchange.ts:723–742`)** sets `Order.price = triggerPrice` for any conditional order. The actual avg fill (`avgPrice` / `cumExecValue ÷ cumExecQty`) is never read.
2. **Sidecar fill matching (`testudo-cex/src/ws-fills.ts:80–100`)** matches WS fills to removals by `symbol + side` only. SL and TP for the same group share both — Map iteration order decides which order ID and snapshot price are emitted as the "closed" event. When TP fires and Bybit OCO-cancels SL, the SL snapshot can win, emitting `id = SL_id` and `price = SL_trigger`.
3. **Router (`fill_detector.rs:265, 430–474, 532`)** trusts `event.average.or(event.price)` for journal economics and only triggers `FillReconciler` when WS price is exactly `0`. A non-zero-but-wrong price (the SL trigger) writes a final, "trustworthy" journal row that never gets reconciled.

The root architectural issue: `OrderUpdateEvent` carries `price` and `average` fields whose semantics vary per exchange. Bybit's `order` topic emits trigger prices for conditionals, HL's `BasicOrder` has no avg at all, WOO/Binance each have their own conventions. We've patched HL (FIX-02) and now Bybit; WOO is next. This is whack-a-mole.

This spec replaces the per-exchange WS-trust pattern with a deterministic protocol: **WebSocket events drive state transitions and OCO logic; REST `fetchOrder` / `fetchMyTrades` is the canonical source for journal economics.** Any exchange that supports REST order lookup is supported uniformly; no per-exchange WS-payload archaeology is required.

---

## User Stories

- **As a trader**, I want my journal exit prices to match what the exchange filled at, every time, regardless of which exchange I use, so that my P&L, R-multiple, and win-rate metrics reflect reality.
- **As a trader**, I want the journal to show a "Reconciling…" state during the brief verification window (rather than `$0.00` or a wrong number), so that I trust the system is working and don't panic-react to phantom losses.
- **As an operator**, I want the canonical-fill code path to be exchange-agnostic, so that adding a new exchange does not require auditing WS-payload semantics for every conditional-order edge case.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `OrderUpdateEvent` MUST NOT carry `price` or `average` fields. The WS contract is transition-only: `id`, `symbol`, `status`, `side`, `timestamp`, `user_id`. | P0 | router/cex_client + sidecar/ws-fills |
| FR-2 | On any live group terminalization (StoppedOut/TookProfit/Closed/ManualClose), `FillDetector` MUST emit `TradeClosed` with `exit_price = 0` and `needs_reconciliation = true`. The `Some(p) if p > Decimal::ZERO` shortcut at `fill_detector.rs:430,472,532` MUST be removed. | P0 | router/fill_detector |
| FR-3 | `FillReconciler` MUST exclude the entry order from exit-leg candidates. The entry's `avg_price` MUST never be returned as `exit_price`. | P0 | router/fill_reconciler |
| FR-4 | `FillReconciler` MUST gate candidate selection by: (a) `filled_qty == group.quantity` (within an exchange precision-step tolerance), AND (b) `transaction_time >= group_terminalized_at − 90s` clock-drift slop. | P0 | router/fill_reconciler |
| FR-5 | `FillReconciler` MUST classify the close leg (SL / TP / manual) and write `close_reason` back to the journal row alongside the corrected `exit_price`. The `OrderGroupStatus` MUST be upgraded from `Closed` to `StoppedOut` or `TookProfit` based on the identified leg. Manual closes stay `Closed`. | P0 | router/fill_reconciler + engine/order_group |
| FR-6 | When SL/TP order IDs are unknown (Bybit pre-resolution failure), `FillReconciler` MUST fall back to `fetchMyTrades` scoped by `since = entry_time − 1s`, `until = now`, matched by ID first then by side+qty. A new sidecar endpoint `POST /trades/by-group` MUST encapsulate the per-exchange paging logic. | P1 | router/fill_reconciler + sidecar/handlers |
| FR-7 | The proximity-detection logic at `fill_detector.rs:342–376` (CEX-08 SL-vs-TP inference from WS exit price) MUST be removed. Classification belongs in the reconciler, where REST data is authoritative. | P0 | router/fill_detector |
| FR-8 | The journal API MUST surface a `status: "reconciling"` field for rows where `needs_reconciliation = true`, and MUST omit `net_pnl` / `r_multiple` for those rows. Daily stats aggregations MUST exclude reconciling rows (already enforced at `fill_reconciler.rs:313`). | P0 | router/journal_service + extension/types |
| FR-9 | The `testudo-journal` UI MUST render reconciling rows with a skeleton/syncing indicator — neutral border, no $ figure, no R-multiple, no win/loss color classification. The row MUST appear in History so the user has continuity from the "position closed" event. | P0 | testudo-journal |
| FR-10 | A regression test MUST exist that feeds a synthetic Bybit `Filled` payload for a triggered TP where raw `order.price = SL_trigger`, and asserts journal `exit_price` equals the actual avg fill (not the trigger), and that the entry's avg is never returned as the exit. | P0 | router (integration test) |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Reproducer test (RED). Sidecar→router boundary integration test feeding a Bybit-shaped `Filled` payload for a triggered TP whose raw `order.price = SL_trigger`. Asserts current journal records the wrong exit_price. | Bug is captured in CI; fix has a target. |
| CP-2 | Reconciler discrimination (FR-3, FR-4). Replace `order_ids: &[String]` with typed `CloseCandidates`. Entry exclusion + qty/timestamp gating. Unit tests around tie-breaking. | Reconciler picks the close leg, not the entry. |
| CP-3 | Strip `price`/`average` from `OrderUpdateEvent` (FR-1). Compile errors guide the audit. Remove `fill_detector.rs:430,472,532` shortcut and `:342–376` proximity logic (FR-2, FR-7). | WS path is transition-only; reconciler is sole exit-price author. |
| CP-4 | `close_reason` + status maturity (FR-5). Reconciler upgrades `OrderGroupStatus` and writes back close-leg identity. Migration adds `close_reason` column to `journal_trades` if not present. | Two-stage status model lands. |
| CP-5 | `fetchMyTrades` fallback + sidecar endpoint (FR-6). `POST /trades/by-group`. Bybit-specific paging encapsulated in sidecar. | ID-less brackets are recoverable. |
| CP-6 | UX reconciling state (FR-8, FR-9). Journal API serializer + `testudo-journal` row component + extension Positions/History views. | User sees Syncing… not phantom-loss. |
| CP-7 | Regression test passes (FR-10). Reproducer goes from RED to GREEN. Full suite green. | Fix is structural and guarded. |

### Key Types

```rust
// router/src/services/fill_reconciler.rs
pub struct CloseCandidates {
    pub entry_order_id: String,             // sanity-check timestamp/qty only; never returned
    pub sl_order_id: Option<String>,
    pub tp_order_id: Option<String>,
    pub manual_close_order_id: Option<String>,
    pub group_terminalized_at: DateTime<Utc>,
    pub expected_qty: Decimal,
    pub qty_tolerance: Decimal,             // exchange precision step
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CloseReason {
    StopLoss,
    TakeProfit,
    Manual,
}

pub struct CloseFill {
    pub exit_price: Decimal,
    pub close_reason: CloseReason,
    pub matched_order_id: String,
    pub transaction_time: DateTime<Utc>,
}
```

```rust
// router/src/services/cex_client.rs — pruned OrderUpdateEvent (FR-1)
#[derive(Debug, Clone, Deserialize)]
pub struct OrderUpdateEvent {
    pub id: String,
    pub symbol: String,
    pub status: String,        // "closed" | "canceled"
    pub side: String,
    pub timestamp: Option<i64>,
    #[serde(skip)]
    pub user_id: Option<uuid::Uuid>,
    // NOTE: price, average, amount, filled, remaining REMOVED.
    // Economics live in REST. WS is transition-only.
}
```

### Reconciler discrimination algorithm (FR-3, FR-4)

```
fn pick_close_leg(candidates, fetched_orders) -> Option<CloseFill>:
    let slop = Duration::from_secs(90)
    let cutoff = candidates.group_terminalized_at - slop

    // Exit candidates: SL, TP, manual_close. Never entry.
    let exit_ids = [candidates.sl_order_id,
                    candidates.tp_order_id,
                    candidates.manual_close_order_id]
                    .into_iter().flatten()

    for id in exit_ids:
        let order = fetched_orders.get(id)?
        if order.avg_price.is_none() || order.avg_price <= 0: continue
        if (order.filled_qty - candidates.expected_qty).abs() > candidates.qty_tolerance: continue
        if order.transaction_time < cutoff: continue

        let close_reason = match id:
            id == sl_order_id => StopLoss
            id == tp_order_id => TakeProfit
            id == manual_close_order_id => Manual

        return Some(CloseFill { exit_price: order.avg_price,
                                 close_reason,
                                 matched_order_id: id,
                                 transaction_time: order.transaction_time })

    None  // → keep needs_reconciliation = true; sweep retries
```

### `fetchMyTrades` fallback (FR-6)

When `pick_close_leg` returns `None` AND `sl_order_id.is_none() && tp_order_id.is_none()` (Bybit ID-less bracket case):

```
POST /trades/by-group
{
  "exchange": "bybit",
  "credentials": {...},
  "symbol": "ETH/USDT:USDT",
  "since_ms": entry_time_ms - 1000,
  "until_ms": now_ms,
  "expected_qty": "0.09",
  "qty_tolerance": "0.001",
  "entry_side": "sell"          // close side will be the inverse
}
→
{
  "matched": {
    "order_id": "...",
    "avg_price": "2369.78",
    "filled_qty": "0.09",
    "transaction_time_ms": 1745680112000,
    "side": "buy"
  }
}
```

Sidecar implementation calls `exchange.fetchMyTrades(symbol, since, undefined, { until })` and walks results filtering by side+qty match. Returns the most recent matching trade.

### Two-stage status model (FR-5)

```
FillDetector path on terminal close:
    update_group_status(group_id, OrderGroupStatus::Closed)   // transient certainty
    emit_trade_closed(group, exit_price=0, needs_recon=true)

FillReconciler path after REST resolution:
    UPDATE journal_trades SET
        exit_price = $real_price,
        close_reason = $reason,           -- 'sl' | 'tp' | 'manual'
        needs_reconciliation = FALSE
        WHERE trade_group_id = $1 AND needs_reconciliation = TRUE

    if reason == StopLoss:
        update_group_status(group_id, OrderGroupStatus::StoppedOut)
    elif reason == TakeProfit:
        update_group_status(group_id, OrderGroupStatus::TookProfit)
    else:  // Manual stays Closed
        no-op
```

### Paved Roads

- **`FillReconciler` (`router/src/services/fill_reconciler.rs`)** — already exists; refactor `fetch_real_price` → `pick_close_leg` with typed inputs.
- **FIX-02 HL REST reconciliation pattern** — same shape, generalized.
- **`needs_reconciliation` schema column** — already in place from FIX-08 migration `20260424000000_journal_trades_needs_reconciliation.up.sql`.
- **Sidecar architecture** — existing `POST /orders/open`, `POST /order` etc; add `POST /trades/by-group` adjacent.
- **`testudo-journal` SWR cache** — pending rows participate in cache normally; UI state derives from `status: "reconciling"`.

### Files

- `testudo-exchange/crates/router/src/services/fill_reconciler.rs` — refactor `fetch_real_price` → `pick_close_leg`; add `CloseCandidates`, `CloseReason`, `CloseFill` types; add `close_reason` write-back; add status upgrade.
- `testudo-exchange/crates/router/src/services/fill_detector.rs` — remove `Some(p) if p > Decimal::ZERO` shortcut at `:430, :472, :532`; remove proximity logic at `:342–376`; always set `needs_reconciliation = true`; pass `CloseCandidates` to reconciler.
- `testudo-exchange/crates/router/src/services/cex_client.rs` — strip `price`/`average`/`amount`/`filled`/`remaining` from `OrderUpdateEvent` (FR-1).
- `testudo-exchange/crates/router/src/services/trade_closed_payload.rs` — drop exit_price parameter (always 0 at write time).
- `testudo-exchange/crates/router/src/services/journal_service.rs` — surface `status: "reconciling"` in API responses; suppress `net_pnl`/`r_multiple` when pending.
- `testudo-exchange/crates/router/src/models/journal.rs` — add `close_reason: Option<String>` field.
- `testudo-exchange/crates/sqlx_postgres/migrations/{TS}_journal_trades_close_reason.up.sql` — add `close_reason TEXT NULL` column.
- `testudo-cex/src/ws-fills.ts` — strip `price`/`average`/`amount`/`filled`/`remaining` from `OrderUpdatePayload` (FR-1). Match becomes id-only when available, side+qty as last resort.
- `testudo-cex/src/handlers.ts` — add `POST /trades/by-group` endpoint (FR-6).
- `testudo-journal/src/components/{TradeRow,TradeHistory}.tsx` (TBD precise paths) — render `status: "reconciling"` rows with skeleton/sync indicator (FR-9).
- `testudo-extension/src/components/{Positions,History}.tsx` (TBD) — same UX treatment.
- `testudo-exchange/crates/router/src/services/integration_tests.rs` (or new) — FR-10 reproducer.

### Dependencies Added

None. All within the existing crate set.

---

## Acceptance Criteria

- [ ] FR-10 reproducer test passes: synthetic Bybit `Filled` payload for triggered TP yields journal `exit_price == actual_avg_fill`, not `SL_trigger`, and entry avg is never returned.
- [ ] `OrderUpdateEvent` no longer contains `price`/`average`/`amount`/`filled`/`remaining`. Compile passes.
- [ ] `fill_detector.rs` always sets `needs_reconciliation = true` on terminal close. Old shortcut removed.
- [ ] `FillReconciler::pick_close_leg` excludes entry by construction (entry not in candidate set).
- [ ] Reconciler writes `close_reason` and upgrades `OrderGroupStatus` (`Closed → StoppedOut | TookProfit`) on resolution.
- [ ] Bybit ID-less bracket case: `POST /trades/by-group` returns the canonical close, journal exit_price corrects.
- [ ] Journal API returns `status: "reconciling"` for pending rows; daily stats exclude pending rows.
- [ ] `testudo-journal` and extension render pending rows with sync indicator, no $ figure, no win/loss class.
- [ ] Verification commands pass:
  ```bash
  cd testudo-exchange && cargo clippy --all-targets && cargo test
  cd testudo-cex && bun test
  cd testudo-journal && bun run build
  cd testudo-extension && bun run typecheck
  ```

---

## Risks

1. **Reconciler latency under exchange REST slowness** — `fetchOrder` may take 100–500ms; user sees "Reconciling…" longer than expected. Mitigation: 3-attempt retry with backoff (already implemented); UX explicitly designed for this state, not hidden.
2. **`fetchMyTrades` rate limit** — Bybit V5 allows 600 req/5s for trade history; sustained bracket trading on ID-less path could hit limits. Mitigation: tight `since` window (entry_time − 1s); cache entry timestamps; only invoke fallback when ID path fails.
3. **Clock drift slop false positives** — 90s window could match an unrelated trade in extreme cases. Mitigation: combined gating (id-priority + qty match + side match) makes accidental match probability negligible; slop is the safety net, not the discriminator.
4. **Existing tests assert `event.price`/`event.average`** — they'll break on FR-1. Mitigation: this is the regression guard doing its job; rewrite tests to assert new contract.
5. **safe-cex fork drift** — we're not patching safe-cex Bybit's `mapOrder`, leaving Bug A in vendor code. Internal store displays may show wrong prices. Mitigation: scope is journal correctness; safe-cex display correctness is deferred (CP-7+ optional follow-up).
6. **Migration ordering** — `close_reason` column must exist before reconciler writes it. Mitigation: standard sqlx migration ordering; deploy migration before binary rollout.

---

## Completion Signal

This spec is complete when:
1. All seven checkpoints are committed to master.
2. The FR-10 reproducer test is green in CI.
3. Production journal recordings on Bybit live closes match exchange Trade History exactly (smoke test on a small live trade).
4. `cargo clippy --all-targets && cargo test` and all `bun run build` / `bun test` commands pass across components.
5. The 2026-04-27 ETHUSDT trade row is corrected via the admin reconciliation sweep (or a one-off SQL backfill, with audit trail).
