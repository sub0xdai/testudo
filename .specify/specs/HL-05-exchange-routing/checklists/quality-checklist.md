# Quality Checklist — HL-05 Exchange Routing

**Spec ID:** HL-05-exchange-routing
**Date:** 2026-03-16

## Implementation

- [ ] RoutingExchangeApi wrapper struct
- [ ] exchange_name-based routing logic (CCXT vs Hyperliquid)
- [ ] WsSubscriptionManager Hyperliquid detection
- [ ] main.rs conditional dependency injection
- [ ] HYPERLIQUID_ENABLED env var toggle

## Verification

- [ ] `cargo clippy --all-targets` passes with zero warnings
- [ ] `cargo test` passes with zero failures
- [ ] Existing CCXT tests remain unchanged and passing
