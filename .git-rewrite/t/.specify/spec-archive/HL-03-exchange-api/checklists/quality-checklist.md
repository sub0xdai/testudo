# Quality Checklist — HL-03 Exchange API

**Spec ID:** HL-03-exchange-api
**Date:** 2026-03-16

## Implementation

- [ ] HyperliquidExchangeApi struct implementing ExchangeApi trait
- [ ] get_balance via user_state endpoint
- [ ] place_order with OrderRequest builder
- [ ] amend_order via modify_order endpoint
- [ ] cancel_order with u64 order ID parsing
- [ ] cancel_all_orders via open_orders query + bulk_cancel
- [ ] get_position from asset_positions response
- [ ] UUID v5 CLOIDs for deterministic client order IDs
- [ ] Symbol normalization (BTC_USDT to BTC coin format)

## Verification

- [ ] `cargo clippy --all-targets` passes with zero warnings
- [ ] `cargo test` passes with zero failures
