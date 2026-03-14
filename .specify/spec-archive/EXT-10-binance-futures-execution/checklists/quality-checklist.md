# Quality Checklist - EXT-10 Binance Futures Live Execution

**Spec:** EXT-10-binance-futures-execution
**Date:** 2026-02-11

## Implementation Quality

- [ ] All new code uses `Result<T,E>` not `unwrap()`
- [ ] All monetary values use `rust_decimal::Decimal` not `f64`
- [ ] Feature flag `real-api` gates all HTTP calls
- [ ] Mock implementations return realistic test data
- [ ] Rate limit tracking uses atomic counters (thread-safe)
- [ ] Symbol conversion reuses existing `symbol::to_binance`

## Safety

- [ ] Testnet default verified -- no `fapi.binance.com` calls without `BINANCE_FUTURES_LIVE=true`
- [ ] Order confirmation polling has bounded retries (max 3)
- [ ] Amend fallback logs critical on total failure
- [ ] Balance check prevents insufficient margin orders
- [ ] No retry on missing order after place (double-fill prevention)

## Testing

- [ ] Unit tests for all `BinanceFuturesExecutor` methods (mock mode)
- [ ] Unit tests for `BinanceFuturesExchangeApi` trait implementation
- [ ] Unit tests for mode-aware routing (paper -> shadow, live -> binance)
- [ ] Unit tests for rate limit threshold rejection
- [ ] Unit tests for amend fallback (cancel+replace path)
- [ ] Existing shadow engine tests pass (no regression)
- [ ] `cargo clippy` clean on new code
- [ ] `cargo test` all pass

## Integration

- [ ] Trade creation route selects correct manager per mode
- [ ] Both managers receive price feed broadcasts
- [ ] Management status endpoint checks both managers
- [ ] API credentials loaded from exchange_accounts table
