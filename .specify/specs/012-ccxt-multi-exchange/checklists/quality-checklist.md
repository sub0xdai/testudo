# Quality Checklist - 012 CCXT Multi-Exchange Support

**Spec:** 012-ccxt-multi-exchange
**Date:** 2026-02-22
**Completed:** 2026-02-22 (FR-1 through FR-6; FR-7 deferred)

## Sidecar Quality

- [x] Express server binds to `127.0.0.1` only (not `0.0.0.0`)
- [x] All numeric values serialized as strings (no floating point loss)
- [x] Exchange pool has TTL eviction (30min) and max size cap (100)
- [x] Credentials never logged or written to disk by sidecar
- [x] CCXT `enableRateLimit: true` on all exchange instances
- [x] Error mapping covers all CCXT exception types
- [x] `GET /health` returns pool size for monitoring
- [x] `GET /exchanges` returns full CCXT exchange list

## Rust Client Quality

- [x] `CcxtClient` uses `Result<T,E>` for all operations
- [x] `SidecarCredentials` has no `Debug` impl (prevents accidental logging)
- [x] Timeout configured (default 10s) — prevents hanging on sidecar crash
- [x] Error mapping from HTTP status codes to typed `CcxtClientError`
- [x] All response decimal strings parsed with `rust_decimal::Decimal`

## CcxtExchangeApi Quality

- [x] Per-request credential lookup from `ExchangeAccountRepository`
- [x] Symbol conversion: internal `BTC_USDT` -> CCXT `BTC/USDT:USDT`
- [x] Order ID convention preserved: `ORDER_ID:SYMBOL`
- [x] Implements all 5 `ExchangeApi` trait methods
- [x] `ExchangeApiError` mapping from `CcxtClientError`

## Safety

- [x] Sandbox/testnet default — production requires `CCXT_SANDBOX=false`
- [x] Sidecar health checked on startup (warning if unreachable, not fatal)
- [x] Paper trading path completely untouched (ShadowExchangeApi unchanged)
- [x] Existing exchange_accounts table reused — no schema migration needed
- [x] `DecryptedCredentials` lifetime is per-request only (not cached in Rust)

## Testing

- [x] Sidecar: unit tests for handlers with mock CCXT exchanges
- [x] Sidecar: error mapping test for all CCXT exception types
- [x] Rust: `CcxtClient` unit tests (parse_decimal, error mapping, config)
- [ ] Rust: `CcxtExchangeApi` tests with mock `CcxtClient` (requires DB mock — deferred)
- [x] Rust: symbol conversion tests (`BTC_USDT`, `ETH_USDT`, `SOL_USDT`)
- [x] Existing `ShadowExchangeApi` tests pass (no regression)
- [x] `cargo clippy` clean on all new code
- [x] `cargo test` all pass (678 passed, 0 failed)
- [x] `npm test` in testudo-ccxt passes (36 passed, 0 failed)

## Integration

- [x] Trade creation route selects correct manager per auth mode
- [x] Credential validation works for non-Binance exchanges (via sidecar)
- [x] `/exchanges/supported` endpoint returns sidecar exchange list
- [ ] Docker Compose starts sidecar alongside db and redis (FR-7 deferred)
- [ ] K8s deployment uses ClusterIP (not Ingress-exposed) (FR-7 deferred)

## Cleanup

- [x] `ccxt_adapter.rs` deleted (no lingering imports)
- [ ] `ccxt_types.rs` retained (dependency: binance_data service)
- [ ] `ccxt_auth.rs` retained (dependency: binance_executor HMAC signing)
- [x] `binance_futures_executor.rs` deleted
- [ ] `binance_executor.rs` retained (dependency: position_sync, sync_service)
- [x] `futures_types.rs` deleted
- [x] `adapters/mod.rs` updated — no references to deleted modules
- [x] `BinanceFuturesExchangeApi` removed from `exchange_api.rs`
- [x] No `BINANCE_API_KEY`/`BINANCE_API_SECRET` env var references in startup code
- [x] `cargo build` succeeds with zero warnings from removed references

### Deviation Notes

Three files (`ccxt_types.rs`, `ccxt_auth.rs`, `binance_executor.rs`) were retained
because other kept modules depend on them:
- `binance_executor.rs` -> used by `position_sync.rs` and `sync_service.rs`
- `ccxt_auth.rs` -> used by `binance_executor.rs` for HMAC request signing
- `ccxt_types.rs` -> used by `binance_data.rs` service for ticker/orderbook types

These can be cleaned up in a future spec when position sync is migrated to the sidecar.
