# Quality Checklist — CEX-07 Symbol Normalization

**Spec ID:** CEX-07-symbol-normalization
**Date:** 2026-03-15

## Sidecar Implementation

- [ ] `fromInternal("BTC_USDT")` returns `"BTCUSDT"`
- [ ] `toInternal("BTCUSDT")` returns `"BTC_USDT"` using market data
- [ ] Edge cases handled (1000PEPE, etc.)

## Rust Backend

- [ ] `ccxt_client.rs` renamed to `cex_client.rs`
- [ ] `CcxtClient` renamed to `CexClient`
- [ ] All imports updated (mod.rs, exchange_api.rs, fill_detector.rs, trade_management.rs)
- [ ] `to_cex_symbol()` converts `BTC_USDT` -> `BTCUSDT`
- [ ] `from_cex_symbol()` converts `BTCUSDT` -> `BTC_USDT`
- [ ] `SidecarOrderResponse` handles `string[]` return
- [ ] Trade management simplified to single bracket call
- [ ] Deferred SL/TP code removed from fill_detector
- [ ] Instant-fill detection code removed

## Testing

- [ ] Symbol conversion tests for common pairs (BTC, ETH, SOL, etc.)
- [ ] Symbol conversion tests for edge cases
- [ ] `cargo clippy --all-targets` clean
- [ ] `cargo test` all pass
- [ ] `bun test` all pass

## Verification

- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] `cd testudo-extension && bun run build` unaffected
- [ ] `bun run build` succeeds for sidecar
