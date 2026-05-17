# Quality Checklist — AW-02 EIP-712 Approval Protocol

**Spec ID:** AW-02-eip712-approval
**Date:** 2026-03-16

## Implementation

- [ ] `build_eip712_typed_data()` function with correct domain/types
- [ ] EIP-712 domain: name, version, chainId `421614`
- [ ] ApproveAgent type string matches SDK encoding
- [ ] `approve-data` route handler (ownership-verified, auth_mode check)
- [ ] `approve` route handler (signature parsing, payload assembly)
- [ ] HTTP POST to Hyperliquid exchange API endpoint
- [ ] `verify_registration()` via `info.extra_agents()`
- [ ] Account activation on successful approval
- [ ] Request/response types for both endpoints
- [ ] Module registration in `hyperliquid/mod.rs`

## Verification

- [ ] `cargo clippy --all-targets` passes with zero errors
- [ ] `cargo test` passes with zero failures
- [ ] Unit test verifies EIP-712 typed data encoding against SDK reference
- [ ] Error paths tested (invalid account, wrong auth_mode, already approved)
