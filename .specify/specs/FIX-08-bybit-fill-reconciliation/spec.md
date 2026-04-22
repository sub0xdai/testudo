# Specification: Bybit Fill-Price Reconciliation

**Spec ID:** FIX-08-bybit-fill-reconciliation
**Date:** 2026-04-22
**Status:** Draft
**Class:** Fix / Backend
**Priority:** P0 — journal P&L is currently fabricated for every CEX-routed live trade that exits via stop or take-profit. Everything downstream (win rate, profit factor, R-multiple, Dignitas inputs) is polluted.
**Depends on:** None (code-level fix)
**Siblings:** FIX-02 (same problem class for Hyperliquid; this spec is the Bybit analogue)

---

## Problem Statement

On 2026-04-22, a user on Bybit reported that their winning TAO short showed as a -$21.60 loss in the desk journal. Investigation on `n0x` PostgreSQL (`99cc6a3b-...-38d679afd021`, TAO_USDT 2026-04-21) found:

- `journal_trades.exit_price = 281.66` for a trade whose **stop was at 247.84**.
- An earlier TAO short on 2026-04-20 also recorded `exit_price ≈ 281.61`.
- Both exit prices are within 5 cents of each other despite different days and different configured stops. That is not a real fill price — it is the same wrong field captured twice.
- Balance-snapshot deltas tell the real story: 04-20 = -$0.98, 04-21 = +$1.84.

Root cause is in `crates/router/src/services/fill_detector.rs:261`:

```rust
let exit_price = event.average.or(event.price);
```

When a Bybit stop-loss fires, the `OrderUpdateEvent` emitted by the CCXT sidecar's `watchOrders` stream has `average` either `None` or stale, and `price` populated with a non-fill field (likely the order's `triggerPrice`, a `markPrice` snapshot, or a mid-stream placeholder). The FillDetector captures this and persists it as `exit_price`; the downstream `compute_derived_fields` then back-computes `realized_pnl` from the bogus exit — producing mathematically consistent nonsense.

FIX-02 solved the same class of problem for Hyperliquid by adding a REST `fetchOrder` / `fetchMyTrades` reconciliation after WS notified a fill. This spec applies the same discipline to the Bybit (and by extension all CCXT-routed) path.

---

## User Stories

- **As a trader whose TP or SL just filled on Bybit**, I want the recorded exit price to match what actually executed on the exchange, so my journal, stats, and Dignitas inputs reflect reality.
- **As an operator backfilling bad rows**, I want an offline reconciliation command that walks historical `journal_trades` with `source = 'testudo'` and re-fetches fill prices from the exchange, so pre-fix trades can be corrected without a manual SQL job.

---

## Non-Goals

- **No change to the live placement / decision loop.** Only the fill-detector post-fill reconciliation is in scope.
- **No per-exchange adapter overhaul.** The reconciliation is a common layer that calls CCXT's `fetchOrder` via the sidecar; exchange-specific quirks live in CCXT.
- **No retroactive balance-delta estimation** (what the 2026-04-22 manual cleanup did). Reconciliation pulls the real fill from the exchange.
- **No reconciliation for `source = 'import_ccxt'` rows.** Those already use REST `fetchMyTrades` by construction — they are not affected by the WS bug.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | After the FillDetector writes a TradeClosed event, async-spawn a reconciliation task that queries `POST /orders/fetch` on the CCXT sidecar for the filled `exchange_order_id` | High | router |
| FR-2 | On successful fetch, update `journal_trades.exit_price` and recompute `realized_pnl`, `realized_pnl_pct`, `net_pnl`, `r_multiple` from the authoritative fill. Use the existing `compute_derived_fields` | High | router |
| FR-3 | CCXT sidecar exposes `POST /order/fetch` (if not already) that resolves an order to its fill trades and returns `avgFillPrice` + `fills: [{ price, amount, fee }]` | High | ccxt-sidecar |
| FR-4 | Reconciliation has a bounded retry (e.g. 3 attempts, exponential backoff 1s/4s/16s) — the fill may not be settled at the moment the WS event arrives | Medium | router |
| FR-5 | Rebuild journal_daily_stats for any `stat_date` whose rows got corrected | Medium | router |
| FR-6 | Offline backfill command `cargo run --bin reconcile-fills -- --since <date>` walks rows with `source = 'testudo'` and `exchange IN ('bybit', 'woo', 'binance')` and reconciles each. Idempotent: re-running over already-reconciled rows produces no change | Medium | router |
| FR-7 | Rows that fail reconciliation (order not found, exchange down) are flagged — add `journal_trades.needs_reconciliation BOOLEAN NOT NULL DEFAULT FALSE` — so they can be re-attempted later instead of silently poisoning stats | Medium | backend |
| FR-8 | When `needs_reconciliation = true`, stats aggregations exclude the row. Frontend surfaces "N trades pending reconciliation" in the P&L calendar tooltip | Low | router + journal |

---

## Technical Implementation

### Vertical Checkpoints

| CP | Scope | Validates |
|----|-------|-----------|
| CP-1 | Migration: `journal_trades.needs_reconciliation`. Stats queries exclude these rows | Poisoned row stops leaking into stats before the fix is live |
| CP-2 | CCXT sidecar `POST /order/fetch`. Returns normalized `{ avg_price, total_filled, fee, fills[] }` | Sidecar endpoint validated against Bybit + WOO + Binance test orders |
| CP-3 | `fill_detector.rs` post-write reconciliation task + retry wrapper. Updates exit_price + recomputes derived fields + clears needs_reconciliation on success | New TP/SL fills land with correct exit_price end-to-end |
| CP-4 | Offline backfill binary `reconcile-fills`. Reruns across all flagged rows | Historical pollution purged |

### Key Types

```rust
// crates/router/src/services/fill_reconciliation.rs (new)

#[derive(Debug, Deserialize)]
pub struct SidecarOrderFetchResponse {
    pub avg_price: Option<String>,         // None if order has no fills
    pub total_filled: String,
    pub fee: String,
    pub fills: Vec<SidecarFill>,
}

#[derive(Debug, Deserialize)]
pub struct SidecarFill {
    pub price: String,
    pub amount: String,
    pub fee: String,
    pub timestamp: i64,
}

pub async fn reconcile_close(
    pool: &PgPool,
    ccxt: &CexClient,
    journal_trade_id: Uuid,
) -> Result<ReconcileOutcome, ReconcileError> {
    // 1. Load the row + parent exchange_account credentials
    // 2. Call ccxt /order/fetch with the stored exchange_order_id
    // 3. If avg_price present -> update exit_price + recompute derived_fields
    // 4. If not fillable (order not found) -> leave needs_reconciliation=true, return NotYetSettled
}
```

### CCXT sidecar endpoint

```js
// testudo-ccxt/routes/order.js
router.post('/order/fetch', async (req, res) => {
  const { exchange, credentials, order_id, symbol } = req.body;
  const ex = getClient(exchange, credentials);
  const order = await ex.fetchOrder(order_id, symbol);
  // Bybit's fetchOrder returns .average once fills settle. If null,
  // fall back to fetchOrderTrades which returns the raw trades with .price per-trade.
  const fills = order.average == null
    ? await ex.fetchOrderTrades(order_id, symbol)
    : [];
  res.json({
    avg_price: order.average ?? null,
    total_filled: order.filled,
    fee: order.fee?.cost ?? '0',
    fills: fills.map(f => ({ price: f.price, amount: f.amount, fee: f.fee?.cost ?? '0', timestamp: f.timestamp })),
  });
});
```

### Paved Roads

- **FIX-02** — same reconciliation pattern for Hyperliquid. Extract the retry + update-derived logic into a shared `reconcile_close` fn that both paths call.
- **HIST-03** — REST `fetchMyTrades`-based import is already reliable. This spec makes the live path use REST-equivalent truthiness too.
- **Existing `CexClient` in router** — wraps sidecar HTTP calls; extend it with `fetch_order(...)`.
- **`compute_derived_fields`** — reused; the only new call is invoking it with the corrected `exit_price`.

### Files

**New (backend):**
- `crates/router/src/services/fill_reconciliation.rs`
- `crates/router/src/bin/reconcile_fills.rs` (offline backfill binary)
- `crates/sqlx_postgres/migrations/NNNN_needs_reconciliation.up.sql` + `.down.sql`

**New (sidecar):**
- `testudo-ccxt/routes/order-fetch.js` (or extend existing `order.js`)

**Modified:**
- `crates/router/src/services/fill_detector.rs` — spawn reconciliation after TradeClosed emit
- `crates/router/src/services/cex_client.rs` — add `fetch_order` method
- `crates/router/src/services/journal_stats.rs` — exclude `needs_reconciliation = true` rows from aggregations
- `crates/router/src/services/journal_timeseries.rs` — same exclusion
- `testudo-journal/src/api/client.ts` + P&L calendar — surface pending-reconciliation count

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Migration adds `needs_reconciliation` with sensible default. Existing poisoned rows identifiable via an update script
- [ ] CCXT sidecar `/order/fetch` returns correct avg_price on Bybit, WOO, Binance (integration test against testnet or recorded fixtures)
- [ ] FillDetector reconciliation kicks off within 1s of TP/SL fill; updates exit_price + pnl on first successful response
- [ ] Retry backoff honours 3 attempts over ~20s; after exhaustion the row stays flagged
- [ ] Offline `reconcile-fills --since 2026-04-01` walks and corrects historical rows idempotently
- [ ] Flagged rows excluded from all journal_stats aggregations (win rate, profit factor, R-multiple, expectancy, streak, daily/weekly calendar)
- [ ] Dignitas inputs (`setup_adherence`, `risk_per_trade_consistency`) also exclude flagged rows — otherwise discipline scoring is polluted by un-reconciled data
- [ ] End-to-end manual QA: place a Bybit live trade, hit SL, verify `exit_price` in DB matches the Bybit UI fill price within 1 tick
- [ ] Verification: `cd testudo-exchange && cargo clippy --all-targets && cargo test`

---

## Risks

1. **CCXT sidecar down during fill.** Reconciliation fails, row stays flagged. *Mitigation:* retry queue; offline backfill command clears backlog once sidecar is back.
2. **Exchange rate-limits `fetchOrder`.** Live burst + offline backfill could exceed limits. *Mitigation:* token-bucket limiter keyed by exchange_account_id; offline backfill paces to 1 req/s per account.
3. **`fetchOrder` returns stale data right after WS fill.** Bybit's order state needs ~100-500ms to settle; fetching too eagerly yields the pre-fill snapshot. *Mitigation:* FR-4 exponential backoff — first retry at 1s is past settlement for Bybit in practice.
4. **Flagged rows invisible to user until reconciliation completes.** A user checking their stats within seconds of a trade close sees fewer trades than reality. *Mitigation:* FR-8 pending-reconciliation counter in calendar tooltip; most reconciliations complete in <2s so user-visible window is short.
5. **Offline backfill on thousands of rows.** Running naively against live exchange APIs could take hours. *Mitigation:* `--since` filter + account-level parallelism + token-bucket throttling.

---

## Completion Signal

1. FR-1 through FR-8 implemented and tested
2. Poisoned rows on production DB re-reconciled; `journal_trades_cleanup_backup_2026_04_22` referenced as the manual-cleanup baseline
3. End-to-end manual QA confirms a live Bybit SL/TP fill lands with exchange-truth exit_price
4. Verification passes
5. Committed: `feat(fix-08): Bybit fill reconciliation — exchange-truth exit_price for all CEX live trades`

---

## Session Context (2026-04-22)

Drafted during a live-debugging session on `n0x` after user reported a winning TAO short showing as a -$21.60 loss. Investigation traced the bug to `fill_detector.rs:261` capturing a non-fill field from Bybit's WS `OrderUpdateEvent`. Manual SQL cleanup on n0x corrected the 2 affected live rows (`b3497547-...`, `19444a71-...`) using balance-snapshot deltas as truth; backup held in `journal_trades_cleanup_backup_2026_04_22`. This spec is the durable fix.
