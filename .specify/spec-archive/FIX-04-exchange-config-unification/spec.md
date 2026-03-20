# Specification: Exchange Configuration Unification

**Spec ID:** FIX-04-exchange-config-unification
**Date:** 2026-03-16
**Status:** Complete
**Class:** Refactor / Correctness
**Priority:** P1 — users see exchanges they cannot register
**Depends on:** None
**Series:** FIX-01 through FIX-07 (Hyperliquid audit remediation)
**Audit Refs:** Critical #6, Medium #15, Medium #16

---

## Problem Statement

Exchange configuration has three separate sources of truth that contradict each other:

1. **UI display list** (`routes/exchanges.rs:30-76`): Shows `binance, woo, bybit, okx, hyperliquid`
2. **Validation allowlist** (`validation.rs:97`): Allows `binance, coinbase, kraken, hyperliquid`
3. **Runtime string comparisons**: `"hyperliquid"` appears 15+ times as a magic string, sometimes case-sensitive (`ws_subscription_manager.rs:221`), sometimes normalized to lowercase (`validation.rs:101`)

Result: WOO, Bybit, OKX are displayed to users but rejected on registration. Coinbase and Kraken pass validation but aren't displayed. The string `"agent_wallet"` also appears 15+ times with no constant.

---

## User Stories

- **As a trader**, I want the exchanges shown in the UI to actually work when I try to register them.
- **As a developer**, I want exchange names defined once and referenced everywhere, so that adding a new exchange requires changing one place.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Define a single `ExchangeName` enum with all supported exchanges | High | Router (types) |
| FR-2 | `GET /exchanges` list and `ValidatedExchangeName` derive from the same source | High | Router (routes, validation) |
| FR-3 | All `"hyperliquid"` string comparisons replaced with the enum or constant | High | Multiple files |
| FR-4 | All `"agent_wallet"` / `"api_key"` string comparisons replaced with `AuthMode` enum constants | High | Multiple files |
| FR-5 | Exchange name comparisons are case-insensitive everywhere | Medium | Router |

---

## Technical Implementation

### Exchange Name Constants

```rust
// New file: crates/router/src/types/exchange_names.rs
// Or add to existing types module

/// Supported exchange identifiers — single source of truth.
pub mod exchanges {
    pub const HYPERLIQUID: &str = "hyperliquid";
    pub const BINANCE: &str = "binance";
    pub const WOO: &str = "woo";
    pub const BYBIT: &str = "bybit";
    pub const OKX: &str = "okx";

    /// All supported exchanges (for validation and display).
    pub const SUPPORTED: &[&str] = &[HYPERLIQUID, BINANCE, WOO, BYBIT, OKX];
}

/// Auth mode constants — single source of truth.
pub mod auth_modes {
    pub const API_KEY: &str = "api_key";
    pub const AGENT_WALLET: &str = "agent_wallet";
}
```

### Validation Update

```rust
// validation.rs — derive from the shared constant
impl ValidatedExchangeName {
    const SUPPORTED_EXCHANGES: &[&str] = exchanges::SUPPORTED;
}
```

### Display List Update

```rust
// routes/exchanges.rs — list_exchanges() returns exchanges from SUPPORTED
// Feature metadata (auth_type, required_fields) lives alongside the constant
```

### String Replacement Targets

| Current | Replace With | Files |
|---------|-------------|-------|
| `"hyperliquid"` | `exchanges::HYPERLIQUID` | exchange_api.rs, routing.rs, ws_subscription_manager.rs, exchanges.rs |
| `"agent_wallet"` | `auth_modes::AGENT_WALLET` | exchange_account.rs (7x), exchanges.rs (2x), ws_subscription_manager.rs, exchange_api.rs |
| `"api_key"` | `auth_modes::API_KEY` | exchange_account.rs (1x) |

### Files

- `crates/router/src/types/exchange_names.rs` — new constants module (or add to existing types)
- `crates/router/src/utils/validation.rs` — derive from shared constants
- `crates/router/src/routes/exchanges.rs` — use constants for display + routing
- `crates/router/src/services/hyperliquid/exchange_api.rs` — replace magic strings
- `crates/router/src/services/hyperliquid/routing.rs` — replace magic strings
- `crates/router/src/services/ws_subscription_manager.rs` — replace + normalize case
- `crates/router/src/repositories/exchange_account.rs` — replace auth_mode strings

---

## Acceptance Criteria

- [x] Single source of truth for supported exchange names
- [x] `GET /exchanges` and validation allowlist are derived from the same constant
- [x] Zero instances of bare `"hyperliquid"` string in non-test code
- [x] Zero instances of bare `"agent_wallet"` or `"api_key"` string in non-test code
- [x] Exchange name comparisons are case-insensitive
- [x] Adding a new exchange requires changing exactly one location
- [x] All tests updated and passing
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Database values are lowercase strings** — existing data stores `"hyperliquid"` in the `exchange_name` column. Constants must match the DB convention. Mitigation: constants are lowercase strings, matching existing data.

---

## Completion Signal

This spec is complete when:
1. All exchange and auth_mode strings are constants
2. UI display and validation are consistent
3. No magic strings remain in production code
4. All tests pass
5. Code committed to master
