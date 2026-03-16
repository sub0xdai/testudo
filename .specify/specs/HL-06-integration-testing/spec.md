# Specification: Integration Testing & Testnet Validation

**Spec ID:** HL-06-integration-testing
**Date:** 2026-03-16
**Status:** Draft
**Class:** Testing / Validation
**Priority:** P1 — validates entire HL series
**Depends on:** HL-01 through HL-05
**Series:** HL-01 through HL-06 (native Hyperliquid integration)

---

## Problem Statement

Need end-to-end validation against Hyperliquid testnet before production use. Unit tests with mocked responses verify correctness of translation logic; testnet tests verify SDK compatibility with the live API.

---

## User Stories

- **As a developer**, I want comprehensive unit tests for all HL components with mocked SDK responses, so that CI stays fast and deterministic.
- **As a developer**, I want conditional testnet integration tests, so that I can validate against the real Hyperliquid API without breaking CI.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Unit test suite: mocked SDK responses for HL-01 through HL-05 components | High | Testing |
| FR-2 | Integration test file at `crates/router/src/services/hyperliquid/tests/` | High | Testing |
| FR-3 | Testnet tests conditional on `HL_TESTNET_KEY` env var — skip gracefully when absent | High | Testing |
| FR-4 | Testnet: construct signer, derive address | Medium | Testing |
| FR-5 | Testnet: fetch meta, resolve BTC asset index | Medium | Testing |
| FR-6 | Testnet: query account balance | Medium | Testing |
| FR-7 | Testnet: place limit → verify open → cancel → verify canceled | Medium | Testing |
| FR-8 | Testnet: fetch positions | Medium | Testing |
| FR-9 | Testnet: subscribe to WebSocket order updates, verify event format | Medium | Testing |
| FR-10 | All existing 709+ Rust tests continue passing | High | Testing |

---

## Technical Implementation

### Testnet Endpoints

- REST: `https://api.hyperliquid-testnet.xyz`
- WebSocket: `wss://api.hyperliquid-testnet.xyz/ws`

### Test Structure

```
crates/router/src/services/hyperliquid/
├── mod.rs
├── auth.rs          (unit tests inline)
├── universe.rs      (unit tests inline)
├── exchange_api.rs  (unit tests inline)
├── ws_fills.rs      (unit tests inline)
├── routing.rs       (unit tests inline)
└── tests/
    ├── mod.rs
    └── integration.rs  (testnet tests, #[ignore] by default)
```

### Testnet Test Pattern

```rust
#[tokio::test]
#[ignore]  // Run with: HL_TESTNET_KEY=<key> cargo test hyperliquid -- --ignored
async fn testnet_fetch_meta() {
    let key = std::env::var("HL_TESTNET_KEY").expect("HL_TESTNET_KEY required");
    let universe = AssetUniverse::fetch(Network::Testnet).await.unwrap();
    assert!(universe.resolve("BTC").is_ok());
}
```

### Files

- `crates/router/src/services/hyperliquid/tests/mod.rs`
- `crates/router/src/services/hyperliquid/tests/integration.rs`

---

## Acceptance Criteria

- [ ] All inline unit tests pass for HL-01 through HL-05
- [ ] Integration test file exists with `#[ignore]` testnet tests
- [ ] Testnet tests skip gracefully when `HL_TESTNET_KEY` absent
- [ ] `cargo test` passes (all existing + new unit tests)
- [ ] `cargo clippy --all-targets` passes with zero warnings
- [ ] Testnet validation passes manually: `HL_TESTNET_KEY=<key> cargo test hyperliquid -- --ignored`

---

## Verification Commands

```bash
# All tests (unit only, CI-safe)
cd testudo-exchange && cargo clippy --all-targets && cargo test

# Testnet validation (manual, requires credentials)
HL_TESTNET_KEY=<private_key_hex> cargo test hyperliquid -- --ignored
```

---

## Completion Signal

This spec is complete when:
1. All unit tests pass in CI
2. Testnet integration tests validate against live API
3. `cargo clippy --all-targets && cargo test` passes
4. Code committed to master
