# Specification: Import Trade History via CCXT Sidecar (Phase 2)

**Spec ID:** HIST-02-ccxt-history-import
**Date:** 2026-03-27
**Status:** Draft
**Class:** Feature / Backend
**Priority:** P1 — HIST-01 imports Hyperliquid history only. Users with WOO, Binance, or Bybit accounts see a non-functional "IMPORT HISTORY" button. This completes the import pipeline for CEX exchanges.
**Depends on:** HIST-01-exchange-history-import
**Series:** HIST-01 (Phase 1: Hyperliquid), HIST-02 (Phase 2: CCXT)

---

## Problem Statement

HIST-01 implemented trade history import for Hyperliquid via the native SDK. CEX exchanges (WOO, Binance, Bybit) connected via the CCXT sidecar have no import path. The import worker currently rejects non-Hyperliquid exchanges with `ImportError::UnsupportedExchange`. The CCXT sidecar at `testudo-ccxt-archived/src/` has no `fetchMyTrades` endpoint.

CCXT's `fetchMyTrades(symbol, since, limit, params)` returns individual fills with `id`, `symbol`, `side`, `price`, `amount`, `cost`, `fee`, `timestamp`. Unlike Hyperliquid, CCXT fills do not include `closedPnl` — position reconstruction from raw fills is required to compute entry/exit/P&L for `journal_trades`.

---

## User Stories

- **As a trader with WOO/Binance accounts**, I want my CEX trade history imported to the dashboard, so that all my trades are visible regardless of exchange.
- **As a user**, I want the import to work the same way as Hyperliquid — click "IMPORT HISTORY" on the exchange card and see trades appear.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `POST /trades` endpoint to CCXT sidecar. Accepts same envelope format (`exchange_id`, `credentials`, `sandbox`, `params`). Calls `exchange.fetchMyTrades(symbol, since, limit, params)`. Returns array of fills with all numeric fields as strings. | High | testudo-ccxt |
| FR-2 | Add `fetch_trades()` method to `CexClient` in the Rust backend. Calls sidecar `POST /trades`. Returns `Vec<SidecarFill>` with fields: `id`, `symbol`, `side`, `price`, `amount`, `fee`, `fee_currency`, `timestamp`. | High | Router |
| FR-3 | Add `import_ccxt` branch to `ImportWorker::process_job()`. Loads credentials, calls `cex_client.fetch_trades()`, paginates via `since` parameter (CCXT standard pagination). | High | Import Worker |
| FR-4 | Implement position reconstruction from raw fills. Group fills by symbol. Track running net position. When net crosses zero, emit a completed trade with weighted-average entry, weighted-average exit, computed P&L, total fees. | High | Import Worker |
| FR-5 | Map each reconstructed trade to `TradeCloseEvent` with `source: "import_ccxt"`. Deduplicate via `exchange_fill_id` using the last fill's ID in each completed trade. | High | Import Worker |
| FR-6 | Only show "IMPORT HISTORY" in the exchange card kebab menu for exchanges that support import (Hyperliquid + any exchange where CCXT `has.fetchMyTrades` is true). Hide it for unsupported exchanges. | Medium | Frontend |
| FR-7 | Perp trades only — filter to futures/swap markets. Skip spot fills. CCXT `fetchMyTrades` params should include `{ type: 'swap' }` or equivalent per-exchange filter. | High | Sidecar |

---

## Technical Implementation

### CCXT Sidecar: `POST /trades` Handler

```javascript
// handlers.js
async function handleTrades(req, res) {
  try {
    const { exchange, params } = getExchangeAndParams(req.body);
    const { symbol, since, limit } = params;
    // fetchMyTrades returns: [{ id, symbol, side, price, amount, cost, fee: { cost, currency }, timestamp, ... }]
    const trades = await exchange.fetchMyTrades(
      symbol || undefined,
      since || undefined,
      limit || 500,
      params.extra || {}
    );
    // Stringify numerics for precision
    const safe = trades.map(t => ({
      id: String(t.id),
      symbol: t.symbol,
      side: t.side,
      price: String(t.price),
      amount: String(t.amount),
      cost: String(t.cost || '0'),
      fee_cost: String(t.fee?.cost || '0'),
      fee_currency: t.fee?.currency || 'USDT',
      timestamp: t.timestamp,
    }));
    res.json(safe);
  } catch (err) {
    handleError(err, res, 'trades');
  }
}
```

Register: `app.post('/trades', handleTrades);`

### Rust CexClient: `fetch_trades()`

```rust
#[derive(Debug, Deserialize)]
pub struct SidecarFill {
    pub id: String,
    pub symbol: String,
    pub side: String,       // "buy" | "sell"
    pub price: String,
    pub amount: String,
    pub cost: String,
    pub fee_cost: String,
    pub fee_currency: String,
    pub timestamp: i64,
}

pub async fn fetch_trades(
    &self,
    exchange_id: &str,
    creds: &SidecarCredentials,
    sandbox: bool,
    since: Option<i64>,
    limit: Option<u32>,
) -> Result<Vec<SidecarFill>, CexClientError>
```

### Position Reconstruction Algorithm

```rust
struct OpenPosition {
    symbol: String,
    side: String,           // "long" | "short"
    fills: Vec<SidecarFill>,
    net_qty: Decimal,       // running quantity
    cost_basis: Decimal,    // weighted entry cost
}

// For each fill:
// 1. If no open position for symbol, open one
// 2. If fill is same direction as position, add to it (scale in)
// 3. If fill is opposite direction:
//    a. If fill qty < position qty: partial close
//    b. If fill qty == position qty: full close → emit TradeCloseEvent
//    c. If fill qty > position qty: close + reverse → emit close, open new
```

Weighted average entry = `total_cost_basis / total_entry_qty`
P&L for longs = `(exit_price - entry_price) * close_qty`
P&L for shorts = `(entry_price - exit_price) * close_qty`

### Pagination

CCXT `fetchMyTrades` returns max ~500-1000 fills per call (exchange-dependent). Paginate by advancing `since` to `last_fill.timestamp + 1`:

```rust
let mut since = start_time_ms;
loop {
    let fills = cex_client.fetch_trades(exchange_id, &creds, false, Some(since), Some(500)).await?;
    if fills.is_empty() { break; }
    // process fills...
    since = fills.last().unwrap().timestamp + 1;
    if fills.len() < 500 { break; }
    tokio::time::sleep(Duration::from_millis(200)).await; // rate limit
}
```

### Files

**New:**
- `testudo-ccxt-archived/src/handlers.js` — Add `handleTrades` function + export

**Modified:**
- `testudo-ccxt-archived/src/server.js` — Register `app.post('/trades', handleTrades)`
- `crates/router/src/services/cex_client.rs` — Add `SidecarFill`, `fetch_trades()`
- `crates/router/src/services/import_worker.rs` — Add `import_ccxt()` method with position reconstruction
- `testudo-journal/src/components/account/ExchangeCard.tsx` — Conditionally show import button

---

## Acceptance Criteria

- [ ] CCXT sidecar responds to `POST /trades` with fill data from WOO
- [ ] Import worker processes CEX fills and creates `journal_trades` rows
- [ ] Position reconstruction correctly groups fills into round-trip trades
- [ ] P&L matches exchange-reported P&L (within fee rounding)
- [ ] Deduplication prevents duplicate imports on re-run
- [ ] Import button hidden for exchanges without `fetchMyTrades` support
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Exchange-specific fill format differences** — WOO, Binance, Bybit may return different field shapes. Mitigation: CCXT normalizes this, but test with each exchange.
2. **Position reconstruction edge cases** — Partial fills, multiple simultaneous positions in same symbol, hedging (long+short simultaneously). Mitigation: Start simple (one position per symbol), log warnings for edge cases.
3. **Rate limiting** — Exchanges limit `fetchMyTrades` calls. Mitigation: 200ms delay between pages, respect exchange-specific limits.
4. **Missing fills** — Some exchanges don't retain 90 days of history. Mitigation: Import whatever is available, log the actual range imported.

---

## Completion Signal

This spec is complete when:
1. WOO trade history imports successfully with correct P&L
2. At least one other CEX (Binance or Bybit) tested
3. All acceptance criteria met
4. Verification commands pass
5. Code committed to master
