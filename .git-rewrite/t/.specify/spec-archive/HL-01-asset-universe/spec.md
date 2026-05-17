# Specification: Asset Universe Cache & Symbol Resolution

**Spec ID:** HL-01-asset-universe
**Date:** 2026-03-16
**Status:** Complete
**Class:** Infrastructure / Exchange Integration
**Priority:** P1 — prerequisite for HL-03 and HL-04
**Depends on:** None (first in series)
**Series:** HL-01 through HL-06 (native Hyperliquid integration)

---

## Problem Statement

Hyperliquid identifies assets by integer index (0=BTC, 1=ETH), not strings. Orders require `szDecimals` for quantity precision. Without a cached mapping of coin names to their indices and decimal metadata, no orders can be placed.

The existing codebase uses string symbol formats (`BTC_USDT`). Hyperliquid uses bare coin names (`BTC`). A translation layer is needed.

---

## User Stories

- **As a developer**, I want a cached mapping of coin names to Hyperliquid asset indices, so that order placement can translate symbols correctly.
- **As a developer**, I want `szDecimals` available per asset, so that order quantities are formatted with correct precision.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create `crates/router/src/services/hyperliquid/mod.rs` module structure | High | Router |
| FR-2 | Create `AssetUniverse` struct caching `coin_name → (asset_index, sz_decimals, max_leverage)` | High | Router |
| FR-3 | Fetch metadata via `InfoProvider::meta()` from `hyperliquid-sdk-rs` | High | Router |
| FR-4 | `resolve(coin) → Result<u32>` for asset index lookup (case-insensitive) | High | Router |
| FR-5 | `sz_decimals(coin) → Result<u32>` for decimal precision lookup | High | Router |
| FR-6 | `to_hl_coin("BTC_USDT") → "BTC"` symbol normalization | High | Router |
| FR-7 | `from_hl_coin("BTC") → "BTC_USDT"` reverse conversion | High | Router |
| FR-8 | `from_entries()` constructor for testing without live API | Medium | Router |
| FR-9 | Testnet vs mainnet URL selection via `Network` enum | Medium | Router |

---

## Technical Implementation

### Key Types

```rust
pub struct AssetMeta {
    pub index: u32,
    pub sz_decimals: u32,
    pub max_leverage: u32,
}

pub struct AssetUniverse {
    assets: HashMap<String, AssetMeta>,  // "BTC" → { index: 0, sz_decimals: 5, ... }
}
```

### Files

- `crates/router/src/services/hyperliquid/mod.rs` — module declaration
- `crates/router/src/services/hyperliquid/universe.rs` — AssetUniverse implementation
- `crates/router/src/services/mod.rs` — register `pub mod hyperliquid`

### Dependencies Added

- `hyperliquid-sdk-rs = "0.1.2"` — Hyperliquid Rust SDK
- `alloy = { version = "0.1", features = ["signers", "signer-local"] }` — Ethereum crypto

---

## Acceptance Criteria

- [x] `AssetUniverse::fetch(Network)` loads from Hyperliquid API
- [x] `resolve("BTC")` returns `Ok(0)`
- [x] `resolve("UNKNOWN")` returns `Err(AssetNotFound)`
- [x] Case-insensitive: `resolve("btc")` works
- [x] `to_hl_coin("BTC_USDT")` returns `"BTC"`
- [x] `from_hl_coin("BTC")` returns `"BTC_USDT"`
- [x] Unit tests pass with mocked data via `from_entries()`
- [x] `cargo check -p router` passes

---

## Completion Signal

This spec is complete when:
1. AssetUniverse builds and caches the coin→index mapping
2. Symbol normalization works in both directions
3. All unit tests pass
4. Code committed to master
