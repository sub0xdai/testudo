# Specification: Financial Precision Migration — f64 to Decimal

**Spec ID:** FIX-01-financial-precision
**Date:** 2026-03-16
**Status:** Complete
**Class:** Refactor / Financial Correctness
**Priority:** P0 — IEEE 754 floating-point used for real-money calculations
**Depends on:** None (first in series)
**Series:** FIX-01 through FIX-07 (Hyperliquid audit remediation)
**Audit Refs:** Critical #1, Critical #3, High #10

---

## Problem Statement

The Hyperliquid WebSocket fill subscriber (`ws_fills.rs`) and the shared `OrderUpdateEvent` type (`cex_client.rs`) use `f64` for all financial fields: `price`, `amount`, `filled`, `remaining`, `average`. The project's own rules (`rust-backend.md`) mandate `rust_decimal::Decimal` for all financial math — never `f64`.

IEEE 754 floating-point arithmetic is imprecise: `0.1 + 0.2 != 0.3`. In a trading system that manages real positions, this causes:
- Micro-discrepancies in fill tracking (0.09999999 vs 0.1)
- Position quantity mismatches between what the exchange reports and what we track
- Potential for invisible positions (zero-check on a near-zero float)

Additionally, `parse_decimal` in `exchange_api.rs` and `unwrap_or(0.0)` in `ws_fills.rs` silently coerce parse failures to zero. A malformed API response becomes a zero balance or invisible position — silent data corruption in production.

Finally, `amend_order` defaults to `is_buy = true` when `side` is `None` and accepts `quantity = Decimal::ZERO` — both can cause catastrophic order execution errors.

---

## User Stories

- **As a trader**, I want accurate fill quantities and prices, so that my position tracking matches the exchange exactly.
- **As a developer**, I want parse failures to surface as errors, so that data corruption is caught immediately rather than silently propagated.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Migrate `OrderUpdateEvent` financial fields from `Option<f64>` to `Option<Decimal>` | High | Router (cex_client.rs) |
| FR-2 | Update `HyperliquidFillSubscriber::translate()` to parse to `Decimal`, propagating errors | High | Router (ws_fills.rs) |
| FR-3 | Update all consumers of `OrderUpdateEvent` to use `Decimal` | High | Router |
| FR-4 | Replace `parse_decimal` with a function returning `Result<Decimal, ExchangeApiError>` | High | Router (exchange_api.rs) |
| FR-5 | Make `amend_order` return an error when `side` is `None` | High | Router (exchange_api.rs) |
| FR-6 | Make `amend_order` return an error when `quantity` resolves to zero | High | Router (exchange_api.rs) |
| FR-7 | Update CCXT sidecar `OrderUpdateEvent` translation to also use `Decimal` | Medium | Router (ws_subscription_manager.rs) |

---

## Technical Implementation

### OrderUpdateEvent Migration

```rust
// BEFORE (cex_client.rs)
pub struct OrderUpdateEvent {
    pub price: Option<f64>,
    pub amount: Option<f64>,
    pub filled: Option<f64>,
    pub remaining: Option<f64>,
    pub average: Option<f64>,
}

// AFTER
pub struct OrderUpdateEvent {
    pub price: Option<Decimal>,
    pub amount: Option<Decimal>,
    pub filled: Option<Decimal>,
    pub remaining: Option<Decimal>,
    pub average: Option<Decimal>,
}
```

### Parse Failure Handling

```rust
// BEFORE (exchange_api.rs:255)
fn parse_decimal(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or(Decimal::ZERO)
}

// AFTER
fn parse_decimal(s: &str) -> Result<Decimal, ExchangeApiError> {
    Decimal::from_str(s).map_err(|e| {
        ExchangeApiError::Exchange(format!("Failed to parse decimal '{}': {}", s, e))
    })
}
```

### WS Fill Translation

```rust
// BEFORE (ws_fills.rs:146)
let orig_sz: f64 = order.orig_sz.parse().unwrap_or(0.0);

// AFTER
let orig_sz: Decimal = Decimal::from_str(&order.orig_sz).map_err(|e| {
    tracing::error!("Failed to parse orig_sz '{}': {}", order.orig_sz, e);
    e
})?;
// translate() returns Option<OrderUpdateEvent> instead of OrderUpdateEvent
```

### Amend Safety

```rust
// BEFORE (exchange_api.rs:377)
None => true, // default; should not happen in practice

// AFTER
None => return Err(ExchangeApiError::Internal("Amend requires side".into())),
```

### Files

- `crates/router/src/services/cex_client.rs` — `OrderUpdateEvent` type change
- `crates/router/src/services/hyperliquid/ws_fills.rs` — translate() migration
- `crates/router/src/services/hyperliquid/exchange_api.rs` — parse_decimal, amend_order fixes
- `crates/router/src/services/ws_subscription_manager.rs` — CCXT OrderUpdateEvent construction
- `crates/router/src/services/fill_detector.rs` — consumer of OrderUpdateEvent (update comparisons)

### Impact Radius

This is a cross-cutting change. Every producer and consumer of `OrderUpdateEvent` must be updated. Grep for `OrderUpdateEvent` and `f64` in the financial paths to find all sites.

---

## Acceptance Criteria

- [x] `OrderUpdateEvent` uses `Decimal` for all financial fields
- [x] `HyperliquidFillSubscriber::translate()` parses to `Decimal` and returns `Option<OrderUpdateEvent>` (None on parse failure, with error log)
- [x] `parse_decimal` returns `Result<Decimal, ExchangeApiError>`
- [x] All callers of `parse_decimal` propagate the error with `?`
- [x] `amend_order` returns error on `None` side
- [x] `amend_order` returns error on zero quantity
- [x] No `f64` remains in any financial calculation path
- [x] All existing tests updated and passing
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Breaking change for CCXT sidecar consumers** — The sidecar WebSocket manager also constructs `OrderUpdateEvent`. Must be updated simultaneously. Mitigation: grep all construction sites before starting.
2. **Decimal serialization** — If `OrderUpdateEvent` is serialized to JSON anywhere, `Decimal` serializes differently from `f64`. Mitigation: check all serde paths.

---

## Completion Signal

This spec is complete when:
1. Zero `f64` in any financial data path
2. Parse failures propagate as errors, never silently become zero
3. Amend safety guards prevent side/quantity defaults
4. All tests pass
5. Code committed to master
