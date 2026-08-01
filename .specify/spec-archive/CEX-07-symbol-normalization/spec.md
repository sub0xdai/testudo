# Specification: Symbol Normalization and Rust Backend Updates

**Spec ID:** CEX-07-symbol-normalization
**Date:** 2026-03-15
**Status:** Draft
**Class:** Migration / Backend
**Priority:** P1 — both sidecar and backend must agree on symbol format
**Depends on:** CEX-04 (handlers), CEX-05 (fill streaming)
**Series:** CEX-01 through CEX-08 (safe-cex migration)

---

## Problem Statement

Symbol format differs between systems:
- Rust backend uses `BTC_USDT` internally
- CCXT used `BTC/USDT:USDT` (futures format)
- safe-cex uses `BTCUSDT` internally

The sidecar must translate between Rust's `BTC_USDT` and safe-cex's `BTCUSDT`. The Rust backend's `to_ccxt_symbol()` and `from_ccxt_symbol()` functions must be updated. Additionally, this is the opportunity to simplify the Rust backend now that safe-cex handles bracket orders and fill streaming natively.

---

## User Stories

- **As the backend**, I want symbol conversion that works with safe-cex's format, so that order placement and fill detection use correct symbols.
- **As the backend**, I want simplified trade placement now that safe-cex handles bracket sequencing, so that the 3-step sequential placement code is removed.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Sidecar: `fromInternal("BTC_USDT")` returns `"BTCUSDT"` | High | Sidecar/symbols |
| FR-2 | Sidecar: `toInternal("BTCUSDT")` returns `"BTC_USDT"` using market data | High | Sidecar/symbols |
| FR-3 | Rust: rename `ccxt_client.rs` to `cex_client.rs` | Medium | Backend |
| FR-4 | Rust: update `to_ccxt_symbol()` from `BTC/USDT:USDT` to `BTCUSDT` | High | Backend |
| FR-5 | Rust: update `from_ccxt_symbol()` to parse `BTCUSDT` back to `BTC_USDT` | High | Backend |
| FR-6 | Rust: `SidecarOrderResponse` handle `string[]` return from `placeOrder` | High | Backend |
| FR-7 | Rust: simplify `trade_management.rs` — single bracket call replaces 3-step sequential | Medium | Backend |
| FR-8 | Rust: remove deferred SL/TP placement logic from `fill_detector.rs` | Medium | Backend |
| FR-9 | Rust: remove instant-fill detection code (safe-cex handles sequencing) | Low | Backend |
| FR-10 | All existing Rust tests pass after changes | High | Backend |
| FR-11 | `cargo clippy --all-targets` clean | High | Backend |

---

## Technical Implementation

### 1) Symbol Normalization (Sidecar)

**File:** `testudo-cex/src/symbols.ts`

```typescript
// Rust backend format -> safe-cex format
export function fromInternal(symbol: string): string {
  // "BTC_USDT" -> "BTCUSDT"
  return symbol.replace("_", "");
}

// safe-cex format -> Rust backend format
export function toInternal(symbol: string, markets: Market[]): string {
  // "BTCUSDT" -> "BTC_USDT"
  // Use market data to find correct split point
  const market = markets.find((m) => m.symbol === symbol);
  if (market) {
    return `${market.base}_${market.quote}`;
  }
  // Fallback: assume USDT quote
  if (symbol.endsWith("USDT")) {
    return `${symbol.slice(0, -4)}_USDT`;
  }
  return symbol;
}
```

### 2) Rust Client Rename (FR-3)

Rename `ccxt_client.rs` to `cex_client.rs` and update all imports. The `CcxtClient` struct becomes `CexClient`. All references in `mod.rs`, `exchange_api.rs`, `fill_detector.rs`, and `trade_management.rs` must be updated.

### 3) Symbol Conversion Update (FR-4, FR-5)

**File:** `testudo-exchange/crates/router/src/services/cex_client.rs`

```rust
// Before (CCXT format):
fn to_ccxt_symbol(symbol: &str) -> String {
    // "BTC_USDT" -> "BTC/USDT:USDT"
}

// After (safe-cex format):
fn to_cex_symbol(symbol: &str) -> String {
    // "BTC_USDT" -> "BTCUSDT"
    symbol.replace('_', "")
}

fn from_cex_symbol(symbol: &str) -> String {
    // "BTCUSDT" -> "BTC_USDT"
    // Strip known quote currencies from the end
    for quote in &["USDT", "USDC", "BUSD"] {
        if symbol.ends_with(quote) {
            let base = &symbol[..symbol.len() - quote.len()];
            return format!("{}_{}", base, quote);
        }
    }
    symbol.to_string()
}
```

### 4) Trade Management Simplification (FR-7)

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs`

The current 3-step sequential placement (entry -> SL -> TP with instant-fill detection) is replaced by a single `place_order` call that passes `stop_loss_trigger` and `take_profit_trigger` through to safe-cex's `placeOrder({stopLoss, takeProfit})`.

### 5) Fill Detector Simplification (FR-8)

**File:** `testudo-exchange/crates/router/src/services/fill_detector.rs`

Remove the deferred SL/TP placement code from `FillKind::Entry` handler. With safe-cex handling fill events reliably, the existing OCO logic (`cancel_all_related_orders`) works as-is. The fill_detector finally receives data on its `order_rx` channel.

---

## Acceptance Criteria

- [ ] Symbol conversion works for all traded pairs (BTC, ETH, SOL, etc.)
- [ ] `cargo clippy --all-targets` passes
- [ ] `cargo test` passes (all 800+ tests)
- [ ] Rust backend communicates with new sidecar using updated symbol format
- [ ] Client renamed from `CcxtClient` to `CexClient`
- [ ] Trade placement simplified to single bracket call
- [ ] Deferred SL/TP code removed from fill_detector
- [ ] Extension build unaffected (`bun run build`)

---

## Risks

1. **Symbol edge cases** — pairs like `1000PEPE_USDT` need correct split. Mitigation: use market data for `toInternal`, fallback to USDT suffix stripping.
2. **Breaking tests** — renaming and restructuring touches many files. Mitigation: run full test suite after each change.
3. **Incomplete simplification** — removing deferred placement may leave dead code. Mitigation: `cargo clippy` catches unused code.

---

## Completion Signal

This spec is complete when:
1. Symbol normalization implemented in sidecar
2. Rust client renamed and symbol functions updated
3. Trade management simplified
4. All tests pass
5. Changes committed to master
