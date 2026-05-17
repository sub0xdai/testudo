# Quality Checklist — 016-ghost-position-cleanup

**Spec ID:** 016-ghost-position-cleanup
**Date:** 2026-03-06

---

## Pre-Implementation

- [ ] Read `rehydration.rs` and confirm `to_ccxt_symbol` import path
- [ ] Read `price_feed.rs` and confirm `PriceFeedService` constructor signature
- [ ] Read `trade_manager/service.rs` and confirm `positions` field type
- [ ] Read `main.rs` wiring for PriceFeedService and TradeManagerService
- [ ] Verify `to_ccxt_symbol()` is publicly accessible from `exchange_api.rs`

## FR-1: Symbol Normalization Fix

- [ ] Import `to_ccxt_symbol` in `rehydration.rs`
- [ ] Apply conversion before `fetch_open_orders` call
- [ ] Apply conversion before `fetch_positions` call
- [ ] Unit test: rehydration uses correct symbol format

## FR-2: Price Feed Live Symbol Polling

- [ ] Add `get_active_symbols()` to `TradeManagerService`
- [ ] Update `PriceFeedService` to accept optional `Arc<TradeManagerService>`
- [ ] Merge live symbols into `tick()` symbol collection
- [ ] Update `PriceFeedService::new()` in `main.rs`
- [ ] Unit test: live-only symbol appears in price feed

## FR-3: Stale Position Error Handling

- [ ] Distinguish transient vs definitive verification errors
- [ ] Mark stale positions as Cancelled on definitive failure
- [ ] Log clearly when marking positions Cancelled
- [ ] Unit test: stale position marked Cancelled after failed verification

## Post-Implementation

- [ ] `cargo clippy --all-targets` passes with no warnings
- [ ] `cargo test` — all existing + new tests pass
- [ ] No new `unwrap()` calls in production code
- [ ] All financial math uses `rust_decimal::Decimal`
