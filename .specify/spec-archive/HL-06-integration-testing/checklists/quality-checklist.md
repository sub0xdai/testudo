# Quality Checklist — HL-06 Integration Testing

**Spec ID:** HL-06-integration-testing
**Date:** 2026-03-16

## Implementation

- [x] Testnet integration test file created
- [x] Conditional compilation on HL_TESTNET_KEY env var
- [x] Auth validation test (key derivation + address)
- [x] Universe fetch test (asset metadata retrieval)
- [x] Balance query test (user_state endpoint)
- [x] Order lifecycle test (place, verify, cancel, verify)
- [x] Position query test (user_state.asset_positions)
- [x] WebSocket subscription test (order updates stream)

## Verification

- [x] `cargo test` passes for unit tests (no testnet key required)
- [ ] `HL_TESTNET_KEY=... cargo test --ignored` passes on testnet
