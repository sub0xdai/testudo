# Quality Checklist - 012 CCXT Multi-Exchange Support

**Spec:** 012-ccxt-multi-exchange
**Date:** 2026-02-22

## Sidecar Quality

- [ ] Express server binds to `127.0.0.1` only (not `0.0.0.0`)
- [ ] All numeric values serialized as strings (no floating point loss)
- [ ] Exchange pool has TTL eviction (30min) and max size cap (100)
- [ ] Credentials never logged or written to disk by sidecar
- [ ] CCXT `enableRateLimit: true` on all exchange instances
- [ ] Error mapping covers all CCXT exception types
- [ ] `GET /health` returns pool size for monitoring
- [ ] `GET /exchanges` returns full CCXT exchange list

## Rust Client Quality

- [ ] `CcxtClient` uses `Result<T,E>` for all operations
- [ ] `SidecarCredentials` has no `Debug` impl (prevents accidental logging)
- [ ] Timeout configured (default 10s) — prevents hanging on sidecar crash
- [ ] Error mapping from HTTP status codes to typed `CcxtClientError`
- [ ] All response decimal strings parsed with `rust_decimal::Decimal`

## CcxtExchangeApi Quality

- [ ] Per-request credential lookup from `ExchangeAccountRepository`
- [ ] Symbol conversion: internal `BTC_USDT` -> CCXT `BTC/USDT:USDT`
- [ ] Order ID convention preserved: `ORDER_ID:SYMBOL`
- [ ] Implements all 5 `ExchangeApi` trait methods
- [ ] `ExchangeApiError` mapping from `CcxtClientError`

## Safety

- [ ] Sandbox/testnet default — production requires `CCXT_SANDBOX=false`
- [ ] Sidecar health checked on startup (warning if unreachable, not fatal)
- [ ] Paper trading path completely untouched (ShadowExchangeApi unchanged)
- [ ] Existing exchange_accounts table reused — no schema migration needed
- [ ] `DecryptedCredentials` lifetime is per-request only (not cached in Rust)

## Testing

- [ ] Sidecar: unit tests for handlers with mock CCXT exchanges
- [ ] Sidecar: error mapping test for all CCXT exception types
- [ ] Rust: `CcxtClient` unit tests with mock HTTP server
- [ ] Rust: `CcxtExchangeApi` tests with mock `CcxtClient`
- [ ] Rust: symbol conversion tests (`BTC_USDT`, `ETH_USDT`, `SOL_USDT`)
- [ ] Existing `ShadowExchangeApi` tests pass (no regression)
- [ ] `cargo clippy` clean on all new code
- [ ] `cargo test` all pass
- [ ] `npm test` in testudo-ccxt passes

## Integration

- [ ] Trade creation route selects correct manager per auth mode
- [ ] Credential validation works for non-Binance exchanges
- [ ] `/exchanges/supported` endpoint returns sidecar exchange list
- [ ] Docker Compose starts sidecar alongside db and redis
- [ ] K8s deployment uses ClusterIP (not Ingress-exposed)

## Cleanup

- [ ] `ccxt_adapter.rs` deleted (no lingering imports)
- [ ] `ccxt_types.rs` deleted
- [ ] `ccxt_auth.rs` deleted
- [ ] `binance_futures_executor.rs` deleted
- [ ] `binance_executor.rs` deleted
- [ ] `futures_types.rs` deleted
- [ ] `adapters/mod.rs` updated — no references to deleted modules
- [ ] `BinanceFuturesExchangeApi` removed from `exchange_api.rs`
- [ ] No `BINANCE_API_KEY`/`BINANCE_API_SECRET` env var references in startup code
- [ ] `cargo build` succeeds with zero warnings from removed references
