# Quality Checklist — AW-04 Agent-Mode ExchangeProvider

**Spec ID:** AW-04-agent-mode-provider
**Date:** 2026-03-16

## Implementation

- [ ] `AuthMode` enum: `Direct` vs `Agent { user_address }`
- [ ] `from_agent_credentials()` constructor (no address mismatch check)
- [ ] `query_address()` method with correct dispatch
- [ ] `build_exchange()` dispatches to `mainnet_agent()` for Agent mode
- [ ] `get_balance()` uses `query_address()`
- [ ] `get_position()` uses `query_address()`
- [ ] `cancel_order()` / `cancel_all_orders()` — verify signing works via agent
- [ ] `load_auth()` dispatch based on `DecryptedCredentials.auth_mode`
- [ ] WS fill subscription uses `query_address()` for user_address
- [ ] Unit tests for Agent mode address dispatch

## Verification

- [ ] `cargo clippy --all-targets` passes with zero errors
- [ ] `cargo test` passes with zero failures
- [ ] All existing Direct-mode tests unchanged and passing
- [ ] No remaining direct `auth.address` usage in query paths (grep verified)
