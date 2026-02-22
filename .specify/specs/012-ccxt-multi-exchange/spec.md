# 012: CCXT Multi-Exchange Support

**Status:** Draft
**Date:** 2026-02-22
**Depends on:** EXT-10 (Binance Futures execution), EXT-09 (backend trade manager)
**Supersedes:** EXT-10 (BinanceFuturesExecutor replaced by CCXT sidecar)
**Phase:** 9

## Overview

Replace the direct `BinanceFuturesExecutor` (1,012 lines) and dormant CCXT Rust adapter (~3,840 lines) with a Node.js sidecar running the real CCXT library. This enables users to trade on any exchange CCXT supports -- Binance, WooX, Bybit, BitMEX, OKX, and 100+ others -- through a single unified code path.

**Current:** Live trading is hardcoded to Binance Futures via direct HTTP calls. Exchange credentials are loaded from environment variables at startup. Only one exchange is supported.

**Target:** Live trading routes through a CCXT Node.js sidecar. Exchange credentials are per-user in PostgreSQL. Any CCXT-supported exchange works. Paper trading via `ShadowExchangeApi` is unchanged.

```
Paper trading:  ShadowExchangeApi (unchanged, in-process Rust)
Live trading:   CcxtExchangeApi (Rust) -> HTTP 127.0.0.1:3100 -> CCXT Sidecar (Node.js) -> Any exchange
```

## User Stories

- **US-1:** As a trader, I want to select my preferred exchange (Binance, WooX, Bybit, etc.) so that I can trade on the platform where I hold funds.
- **US-2:** As a trader, I want to add exchange API credentials and have them validated immediately so that I know they work before placing trades.
- **US-3:** As a trader, I want the trade manager (break-even, trailing stop, partial TP) to work identically regardless of which exchange I use.
- **US-4:** As a system operator, I want a single sidecar service to handle all exchange integrations so that adding new exchanges requires zero Rust code changes.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Sidecar language | Node.js | CCXT reference implementation, most up-to-date, fastest to get new exchange support. Bun/Node already in the project stack. |
| Sidecar vs FFI | HTTP sidecar on localhost | Clean process isolation. CCXT has no Rust binding. FFI to Node/Go is fragile. HTTP on localhost adds ~1ms which is negligible vs 100-500ms exchange API calls. |
| Replace vs coexist | Full replacement | Single code path for all live trading. No Binance-specific fast path to maintain. CCXT handles exchange-specific quirks (amend fallback, rate limits, auth). |
| Credential flow | Per-request from PostgreSQL | Enables multi-user multi-exchange. Hot credential rotation without restart. ~1ms DB lookup is negligible. Existing `ExchangeAccountRepository` with AES-256-GCM encryption is reused unchanged. |
| Exchange instance caching | Sidecar-side pool with 30min TTL | CCXT instances hold rate limiter state + market metadata. Credential hash as cache key. Avoids recreating on every request. |
| Symbol format | Internal `BTC_USDT` -> CCXT `BTC/USDT:USDT` | CCXT uses unified format. Conversion is one function. CCXT handles the final exchange-native conversion (BTCUSDT for Binance, BTC-USDT for Bybit, PERP_BTC_USDT for WooX). |
| ShadowEngine | Unchanged | Paper trading has zero sidecar dependency. Only live trading routes through CCXT. |

---

## Functional Requirements

### FR-1: CCXT Sidecar Service

New Node.js HTTP service at `testudo-ccxt/`.

- **FR-1.1:** Express server binding to `127.0.0.1:3100` (localhost only, never exposed to internet).
- **FR-1.2:** Exchange instance pool keyed by `sha256(exchange_id + apiKey + sandbox)`. TTL eviction at 30 minutes of inactivity. Max pool size 100 instances.
- **FR-1.3:** All CCXT exchange instances created with `{ enableRateLimit: true }`. CCXT handles per-exchange rate limiting internally.
- **FR-1.4:** REST endpoints:

| Endpoint | CCXT Method | Request Params | Response |
|----------|------------|----------------|----------|
| `POST /balance` | `fetchBalance({ type })` | `{ type: "future"\|"spot" }` | `{ asset, total, free, used }` (string decimals) |
| `POST /order` | `setLeverage()` + `createOrder()` | `{ symbol, side, type, amount, price?, stopPrice?, leverage? }` | `{ id, status, symbol, side, type, amount, filled, remaining, average, price }` |
| `POST /order/edit` | `editOrder()` | `{ orderId, symbol, type, side, amount?, price? }` | Same as order response |
| `POST /order/cancel` | `cancelOrder()` | `{ orderId, symbol }` | `{ success: true }` |
| `POST /position` | `fetchPositions()` | `{ symbol? }` | `[{ symbol, side, contracts, entryPrice, unrealizedPnl }]` |
| `POST /leverage` | `setLeverage()` | `{ leverage, symbol }` | `{ success: true }` |
| `GET /health` | — | — | `{ ok: true, poolSize, uptime }` |
| `GET /exchanges` | `ccxt.exchanges` | — | `string[]` of all supported exchange IDs |

- **FR-1.5:** Every `POST` request body includes envelope: `{ exchange_id, credentials: { apiKey, secret, password? }, sandbox, params }`.
- **FR-1.6:** Error mapping from CCXT exceptions to HTTP status codes:
  - `AuthenticationError` -> 401
  - `InsufficientFunds` -> 402
  - `OrderNotFound` -> 404
  - `RateLimitExceeded` -> 429
  - `ExchangeNotAvailable` -> 503
  - `NetworkError` -> 502
  - All others -> 500
- **FR-1.7:** Response body on error: `{ error: string, code: string }`.
- **FR-1.8:** All numeric values in responses serialized as strings to preserve decimal precision (CCXT uses floats internally; string serialization avoids JS floating point loss).
- **FR-1.9:** `POST /order` calls `setLeverage(leverage, symbol)` before `createOrder()` when `leverage` param is present and > 0.
- **FR-1.10:** Sandbox/testnet mode: when `sandbox: true` in the request envelope, the CCXT exchange instance is created with `sandbox: true`. CCXT handles per-exchange testnet URL routing.

### FR-2: Rust CCXT Client

New module `crates/router/src/services/ccxt_client.rs`.

- **FR-2.1:** `CcxtSidecarConfig` struct with `base_url` (from `CCXT_SIDECAR_URL` env var, default `http://127.0.0.1:3100`) and `timeout_secs` (default 10).
- **FR-2.2:** `CcxtClient` struct with reqwest HTTP client. Methods:
  - `fetch_balance(exchange_id, creds, sandbox, params) -> SidecarBalanceResponse`
  - `create_order(exchange_id, creds, sandbox, params) -> SidecarOrderResponse`
  - `edit_order(exchange_id, creds, sandbox, params) -> SidecarOrderResponse`
  - `cancel_order(exchange_id, creds, sandbox, params) -> ()`
  - `fetch_positions(exchange_id, creds, sandbox, params) -> Vec<SidecarPositionResponse>`
  - `health_check() -> ()`
- **FR-2.3:** `SidecarCredentials { api_key, secret, password? }` — ephemeral struct, never logged or serialized to storage.
- **FR-2.4:** `CcxtClientError` enum with typed variants: `Unavailable`, `AuthenticationFailed`, `InsufficientFunds`, `OrderNotFound`, `RateLimited`, `ExchangeError(String)`.
- **FR-2.5:** Map sidecar HTTP status codes to `CcxtClientError` variants matching FR-1.6.

### FR-3: CcxtExchangeApi

New `ExchangeApi` implementation in `crates/router/src/services/exchange_api.rs`.

- **FR-3.1:** `CcxtExchangeApi` struct holding `CcxtClient`, `ExchangeAccountRepository`, and `sandbox: bool`.
- **FR-3.2:** Credential lookup per call:
  1. `account_repo.list_by_user(user_id)` -> get user's first active exchange account
  2. `account_repo.load_credentials(account_id, user_id)` -> `DecryptedCredentials { exchange_name, api_key, api_secret }`
  3. Construct `SidecarCredentials` from decrypted values
  4. Forward to `CcxtClient` with `exchange_name` as `exchange_id`
- **FR-3.3:** Symbol conversion: `to_ccxt_symbol("BTC_USDT")` -> `"BTC/USDT:USDT"` (CCXT unified futures format).
- **FR-3.4:** Implement all 5 `ExchangeApi` trait methods:
  - `get_balance` -> `ccxt_client.fetch_balance()`, parse string decimal to `rust_decimal::Decimal`
  - `place_order` -> `ccxt_client.create_order()`, return order ID string
  - `amend_order` -> `ccxt_client.edit_order()`, preserve `ORDER_ID:SYMBOL` convention
  - `cancel_order` -> `ccxt_client.cancel_order()`
  - `get_position` -> `ccxt_client.fetch_positions()`, map to `PositionInfo`
- **FR-3.5:** Leverage: pass `req.leverage` to the sidecar's `create_order` params. The sidecar handles `setLeverage()` before order placement (FR-1.9).
- **FR-3.6:** Error mapping: `CcxtClientError` -> `ExchangeApiError` (existing trait error type).

### FR-4: Rewire Application Startup

Modify `crates/router/src/main.rs`.

- **FR-4.1:** Replace `BinanceFuturesExecutor` construction block (env vars `BINANCE_API_KEY`, `BINANCE_API_SECRET`, `BINANCE_FUTURES_LIVE`, `BINANCE_FUTURES_LEVERAGE`) with `CcxtExchangeApi` construction.
- **FR-4.2:** Enable CCXT via `CCXT_SIDECAR_URL` or `CCXT_ENABLED=true` env var.
- **FR-4.3:** Sandbox mode default: `CCXT_SANDBOX != "false"` (testnet by default, production requires explicit opt-in).
- **FR-4.4:** `TradeManagerService` for live mode wraps `CcxtExchangeApi`. Shadow mode wraps `ShadowExchangeApi` (unchanged).
- **FR-4.5:** Health check: on startup, verify sidecar is reachable via `GET /health`. Log warning if unreachable but don't fail startup (allows paper-only mode).

### FR-5: Credential Validation via Sidecar

Modify `crates/router/src/routes/exchanges.rs`.

- **FR-5.1:** Replace Binance-only `credential_validator.validate_binance()` with sidecar `POST /balance` call using provided credentials. If balance fetch succeeds -> credentials are valid for any exchange.
- **FR-5.2:** `test_exchange_connection` endpoint delegates to sidecar for all exchanges.
- **FR-5.3:** New `GET /exchanges/supported` endpoint proxying sidecar's `GET /exchanges` — provides the UI with the complete list of available exchanges.
- **FR-5.4:** `add_exchange_account` accepts any `exchange_name` value that the sidecar supports (not just `"binance"`).

### FR-6: Code Cleanup

- **FR-6.1:** Delete from `crates/common_utils/src/adapters/`:
  - `ccxt_adapter.rs` (~1,923 lines)
  - `ccxt_types.rs` (~665 lines)
  - `ccxt_auth.rs` (~1,252 lines)
  - `binance_futures_executor.rs` (~1,012 lines)
  - `binance_executor.rs`
  - `futures_types.rs`
- **FR-6.2:** Update `crates/common_utils/src/adapters/mod.rs` — remove all `pub mod`/`pub use` for deleted files. Keep: `credential_validator`, `market_data`, `position_sync`, `position_types`, `account_state`, `execution_types`.
- **FR-6.3:** Remove `BinanceFuturesExchangeApi` from `exchange_api.rs` (lines 249-511).
- **FR-6.4:** Update `ExecutionService` — replace `BinanceExecutorAdapter` with sidecar-backed adapter, or deprecate the older `/order` route in favor of the `/trades` path.
- **FR-6.5:** Remove integration tests specific to deleted Binance executor. Add tests for `CcxtExchangeApi`.

### FR-7: Infrastructure

- **FR-7.1:** `testudo-ccxt/Dockerfile`: Node 20 Alpine, npm ci production, port 3100, non-root user.
- **FR-7.2:** Add `ccxt-sidecar` service to `testudo-exchange/docker/docker-compose.yml` on `gateway` network, `expose: ["3100"]` (internal only).
- **FR-7.3:** K8s `testudo-ops/ccxt-sidecar/deployment.yml` — Deployment + ClusterIP Service on port 3100 (not Ingress-exposed). Liveness/readiness probes on `GET /health`.

---

## File Context

### New files:

| File | Purpose |
|------|---------|
| `testudo-ccxt/package.json` | Node.js sidecar dependencies (ccxt, express) |
| `testudo-ccxt/src/server.js` | Express HTTP server |
| `testudo-ccxt/src/pool.js` | Exchange instance cache with TTL |
| `testudo-ccxt/src/handlers.js` | Route handlers (balance, order, position, leverage) |
| `testudo-ccxt/src/errors.js` | CCXT exception -> HTTP status mapping |
| `testudo-ccxt/Dockerfile` | Container image |
| `testudo-exchange/crates/router/src/services/ccxt_client.rs` | Rust HTTP client for sidecar |
| `testudo-ops/ccxt-sidecar/deployment.yml` | K8s deployment |

### Modified files:

| File | Changes |
|------|---------|
| `crates/router/src/services/exchange_api.rs` | Add `CcxtExchangeApi`, remove `BinanceFuturesExchangeApi` |
| `crates/router/src/services/mod.rs` | Export `ccxt_client`, `CcxtExchangeApi` |
| `crates/router/src/main.rs` | Replace Binance executor wiring with CCXT sidecar wiring |
| `crates/router/src/routes/exchanges.rs` | Generalize credential validation, add `/exchanges/supported` |
| `crates/router/src/routes/trade_management.rs` | Simplify manager selection (no Binance-specific logic) |
| `crates/common_utils/src/adapters/mod.rs` | Remove deleted module exports |
| `testudo-exchange/docker/docker-compose.yml` | Add ccxt-sidecar service |

### Deleted files:

| File | Lines | Reason |
|------|-------|--------|
| `crates/common_utils/src/adapters/ccxt_adapter.rs` | ~1,923 | Mock CCXT replaced by real CCXT sidecar |
| `crates/common_utils/src/adapters/ccxt_types.rs` | ~665 | Types now in sidecar JSON contract |
| `crates/common_utils/src/adapters/ccxt_auth.rs` | ~1,252 | Auth handled by CCXT library |
| `crates/common_utils/src/adapters/binance_futures_executor.rs` | ~1,012 | Direct HTTP replaced by sidecar |
| `crates/common_utils/src/adapters/binance_executor.rs` | ~500 | Spot executor replaced by sidecar |
| `crates/common_utils/src/adapters/futures_types.rs` | ~150 | Futures types now in sidecar |

### Unchanged files (verification):

| File | Why unchanged |
|------|---------------|
| `crates/router/src/services/exchange_api.rs` (ShadowExchangeApi) | Paper trading path untouched |
| `crates/router/src/services/trade_manager/service.rs` | Takes `Arc<dyn ExchangeApi>` — already exchange-agnostic |
| `crates/router/src/repositories/exchange_account.rs` | Credential storage/retrieval reused as-is |

---

## Acceptance Criteria

1. `npm test` in `testudo-ccxt/` passes all sidecar unit tests
2. `cargo build && cargo test` passes with zero Binance executor references remaining
3. `curl http://localhost:3100/exchanges | jq length` returns 100+ supported exchanges
4. Paper mode trades use `ShadowExchangeApi` (no regression — existing extension E2E tests pass)
5. Live mode trades route through `CcxtExchangeApi` -> sidecar -> CCXT
6. Credential validation works for any exchange (`POST /exchanges/accounts` with `exchange_name: "woox"`)
7. Exchange instance pool evicts stale instances (verified by sidecar health endpoint showing pool size)
8. Sidecar binds to localhost only — not reachable from external network
9. `grep -rn "BinanceFuturesExecutor\|BinanceFuturesExchangeApi\|ccxt_adapter\|ccxt_auth\|ccxt_types" crates/` returns zero matches outside of git history
10. String decimal serialization: order amounts/prices pass through without floating point loss

---

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Sidecar crash takes down live trading | High | Health check in router; fallback to shadow mode if sidecar unreachable. K8s restart policy. |
| CCXT library update breaks API contract | Medium | Pin CCXT version in package.json. Test before bumping. Sidecar has its own release cycle. |
| Credential exposure in transit | Medium | Localhost-only binding. K8s NetworkPolicy restricting pod access. Credentials never written to disk or logged. |
| Rate limiting differences across exchanges | Low | CCXT `enableRateLimit: true` handles per-exchange limits. Sidecar doesn't add its own. |
| Node.js memory leak from exchange pool | Low | TTL eviction + max pool size. Health endpoint reports pool size for monitoring. |
| CCXT doesn't support amend on some exchanges | Low | `editOrder()` internally falls back to cancel+replace. CCXT documents which exchanges support native amend. |
| Latency increase (localhost HTTP hop) | Low | ~1ms added to 100-500ms exchange calls. Negligible. |

---

## Completion Signal

All acceptance criteria met. Sidecar starts, router connects, paper and live trades work through unified path. Commit message: `feat: 012 CCXT multi-exchange sidecar — replace direct Binance with universal exchange support`.
