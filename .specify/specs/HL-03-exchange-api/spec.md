# Specification: HyperliquidExchangeApi Core Trait Implementation

**Spec ID:** HL-03-exchange-api
**Date:** 2026-03-16
**Status:** Draft
**Class:** Feature / Exchange Integration
**Priority:** P1 — core trading functionality
**Depends on:** HL-01 (universe), HL-02 (auth)
**Series:** HL-01 through HL-06 (native Hyperliquid integration)

---

## Problem Statement

The `ExchangeApi` trait defines 6 methods for trade management. Need a `HyperliquidExchangeApi` implementation using the Rust SDK that slots in alongside `ShadowExchangeApi` and `CexExchangeApi` without modifying either.

---

## User Stories

- **As a trader**, I want to place orders on Hyperliquid through the same interface as other exchanges, so that trade management works identically.
- **As a developer**, I want a third `ExchangeApi` implementation that uses native Rust SDK calls, bypassing the Node.js sidecar.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Implement `get_balance` via `InfoProvider::user_state(addr)` — extract `margin_summary.account_value` | High | Router |
| FR-2 | Implement `place_order` via `ExchangeProvider::place_order()` with builder pattern | High | Router |
| FR-3 | Implement `amend_order` via `ExchangeProvider::modify_order()` (native modify, no cancel+replace) | High | Router |
| FR-4 | Implement `cancel_order` via `ExchangeProvider::cancel_order(asset, oid)` — parse oid from string to u64 | High | Router |
| FR-5 | Implement `cancel_all_orders` via open orders query + `bulk_cancel()` | High | Router |
| FR-6 | Implement `get_position` via `InfoProvider::user_state(addr)` — filter `asset_positions` by symbol | High | Router |
| FR-7 | Symbol normalization: `BTC_USDT` → `"BTC"` via `AssetUniverse::to_hl_coin()` | High | Router |
| FR-8 | UUID v5 CLOIDs: deterministic from `testudo:{group_id}:{role}` | Medium | Router |
| FR-9 | Construct `ExchangeProvider` per-call from cached signer (HL-02 AuthCache) | High | Router |
| FR-10 | Map `ApiOrderType::StopLoss` → trigger order with `stop_price` as trigger, market execution | High | Router |

---

## Technical Implementation

### Method Mapping

| Trait Method | SDK Call | Notes |
|---|---|---|
| `get_balance` | `InfoProvider::user_state(addr)` | Extract `margin_summary.account_value` |
| `place_order` | `ExchangeProvider::place_order(OrderRequest)` | Use `AssetUniverse::resolve()` for asset index |
| `amend_order` | `ExchangeProvider::modify_order(oid, new_order)` | Native modify, no cancel+replace |
| `cancel_order` | `ExchangeProvider::cancel_order(asset, oid)` | Parse oid from string to u64 |
| `cancel_all_orders` | `InfoProvider::open_orders()` + `bulk_cancel()` | Two-step: fetch then cancel |
| `get_position` | `InfoProvider::user_state(addr)` | Filter `asset_positions` by symbol |

### Order Type Mapping

```rust
ApiOrderType::Market → OrderRequest { order_type: Limit(Ioc), ... }  // market = aggressive IOC
ApiOrderType::Limit  → OrderRequest { order_type: Limit(Gtc), ... }
ApiOrderType::StopLoss → OrderRequest { order_type: Trigger { is_market: true, trigger_px, tpsl: "sl" }, reduce_only: true }
```

### CLOID Strategy

UUID v5 (namespace UUID + `testudo:{group_id}:{role}`) → deterministic, reversible, valid 128-bit hex.

### Key Types

```rust
pub struct HyperliquidExchangeApi {
    info: InfoProvider,
    universe: Arc<AssetUniverse>,
    auth_cache: Arc<AuthCache>,
    account_repo: ExchangeAccountRepository,
    network: Network,
}
```

### Files

- `crates/router/src/services/hyperliquid/exchange_api.rs`
- Update `crates/router/src/services/hyperliquid/mod.rs`

---

## Acceptance Criteria

- [ ] `HyperliquidExchangeApi` implements all 6 `ExchangeApi` trait methods
- [ ] `get_balance` returns account value as `Decimal`
- [ ] `place_order` creates limit, market, and stop-loss orders via SDK
- [ ] `amend_order` uses native modify (not cancel+replace)
- [ ] `cancel_order` parses string order ID to u64
- [ ] `cancel_all_orders` fetches open orders then bulk cancels
- [ ] `get_position` filters asset positions by coin name
- [ ] Symbol normalization strips `_USDT` suffix
- [ ] CLOIDs are deterministic UUID v5 from group_id + role
- [ ] Unit tests with mocked SDK responses — one per trait method + error paths
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **SDK response shape changes** — Hyperliquid may update API responses. Mitigation: pin SDK version.
2. **Order ID format** — Hyperliquid uses u64 order IDs, existing system uses string IDs. Mitigation: convert at boundary.
3. **Decimal precision** — SDK uses string decimals. Mitigation: parse to `rust_decimal::Decimal` at boundary.

---

## Completion Signal

This spec is complete when:
1. All 6 ExchangeApi methods implemented and tested
2. Symbol normalization works bidirectionally
3. CLOIDs are deterministic and reversible
4. `cargo clippy --all-targets && cargo test` passes
5. Code committed to master
