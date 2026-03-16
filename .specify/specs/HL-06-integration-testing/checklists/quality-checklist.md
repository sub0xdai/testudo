# Quality Checklist — HL-06 Integration Testing

**Spec ID:** HL-06-integration-testing
**Date:** 2026-03-16

## Implementation

- [ ] Testnet integration test file created
- [ ] Conditional compilation on HL_TESTNET_KEY env var
- [ ] Auth validation test (key derivation + address)
- [ ] Universe fetch test (asset metadata retrieval)
- [ ] Balance query test (user_state endpoint)
- [ ] Order lifecycle test (place, amend, cancel)
- [ ] Position query test (asset_positions)
- [ ] WebSocket subscription test (order updates stream)

## Verification

- [ ] `cargo test` passes for unit tests (no testnet key required)
- [ ] `HL_TESTNET_KEY=... cargo test --ignored` passes on testnet
