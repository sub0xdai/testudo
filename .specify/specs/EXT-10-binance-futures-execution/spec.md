# EXT-10: Binance Futures Live Execution

**Status:** Completed
**Date:** 2026-02-11
**Depends on:** EXT-09 (backend trade manager), EXT-05 (auth/live mode)
**Phase:** 6

## Overview

Implement `BinanceFuturesExecutor` and `BinanceFuturesExchangeApi` to enable live order execution on Binance USDT-M Futures. The existing `ExchangeApi` trait and `TradeManagerService` remain unchanged -- this spec fills the stub `BinanceExchangeApi` with a real Futures implementation and wires mode-aware routing so paper trades use `ShadowExchangeApi` and live trades use `BinanceFuturesExchangeApi`.

## User Stories

- **US-1:** As a trader in LIVE mode, I want my extension trades to place real orders on Binance Futures so that I can execute directly from TradingView.
- **US-2:** As a trader, I want the trade manager's break-even, trailing stop, and partial TP rules to amend my Binance orders automatically so that I don't need to manage positions manually.
- **US-3:** As a trader, I want safety guardrails (testnet default, balance checks, rate limiting) so that I don't lose money due to software errors.

## Functional Requirements

### FR-1: BinanceFuturesExecutor (common_utils)

New struct in `common_utils/src/adapters/binance_futures_executor.rs`.

- **FR-1.1:** Target Binance USDT-M Futures API.
  - Production: `https://fapi.binance.com`
  - Testnet: `https://testnet.binancefuture.com`
- **FR-1.2:** Reuse `CCXTAuthenticator` for HMAC-SHA256 request signing.
- **FR-1.3:** Same `real-api` feature flag pattern as `BinanceExecutor` -- mock responses when feature is off, real HTTP when on.
- **FR-1.4:** Implement these operations:
  - `execute(order) -> FuturesOrderResult` via `POST /fapi/v1/order`
    - Support order types: `LIMIT`, `MARKET`, `STOP_MARKET`, `TAKE_PROFIT_MARKET`
    - Include `positionSide=BOTH` (one-way mode)
  - `amend_order(symbol, order_id, params) -> FuturesOrderResult` via `PUT /fapi/v1/order`
  - `cancel(order_id, symbol) -> ()` via `DELETE /fapi/v1/order`
  - `get_order(order_id, symbol) -> FuturesOrderResult` via `GET /fapi/v1/order`
  - `get_balance() -> Vec<FuturesBalance>` via `GET /fapi/v2/balance`
  - `get_position(symbol) -> Vec<FuturesPosition>` via `GET /fapi/v2/positionRisk`
  - `set_leverage(symbol, leverage) -> ()` via `POST /fapi/v1/leverage`
- **FR-1.5:** 5-second request timeout (matching spot executor).
- **FR-1.6:** Parse rate limit headers (`X-MBX-USED-WEIGHT-1M`). Track usage and reject locally when approaching 2400 weight/min.
- **FR-1.7:** Parse Binance error codes into typed `ExecutionError` variants:
  - `-2010` -> InsufficientBalance
  - `-1013` -> InvalidOrder
  - `-1121` -> InvalidSymbol
  - `-2015` -> AuthenticationFailed
  - `429` -> RateLimited with Retry-After
- **FR-1.8:** Testnet constructor `BinanceFuturesExecutor::testnet(key, secret)`.

### FR-2: BinanceFuturesExchangeApi (router/services)

Replace the stub `BinanceExchangeApi` in `exchange_api.rs` with `BinanceFuturesExchangeApi`.

- **FR-2.1:** Implement `ExchangeApi::get_balance` -- call `GET /fapi/v2/balance`, find USDT asset, return `walletBalance + crossUnPnl`.
- **FR-2.2:** Implement `ExchangeApi::place_order`:
  - Convert `PlaceOrderRequest` fields to Futures params
  - Symbol conversion via `symbol::to_binance`
  - Set leverage on first order per symbol (lazy, via `POST /fapi/v1/leverage`)
  - Return Binance orderId as string
- **FR-2.3:** Implement `ExchangeApi::amend_order`:
  - Use native `PUT /fapi/v1/order` for in-place amendment
  - On failure, fall back to cancel + replace
  - If cancel succeeds but replace fails, log critical and emit alert
- **FR-2.4:** Implement `ExchangeApi::cancel_order` -- `DELETE /fapi/v1/order` with symbol from managed position.
- **FR-2.5:** Implement `ExchangeApi::get_position` -- `GET /fapi/v2/positionRisk`, filter by symbol, map to `PositionInfo`.
- **FR-2.6:** Balance check before `place_order` -- verify available margin locally before sending to Binance.

### FR-3: Mode-Aware Trade Manager Wiring

- **FR-3.1:** `TradeManagementState` holds two optional `TradeManagerService` instances:
  - `trade_manager_shadow: Option<Arc<TradeManagerService>>` (existing, renamed)
  - `trade_manager_live: Option<Arc<TradeManagerService>>` (new, backed by `BinanceFuturesExchangeApi`)
- **FR-3.2:** Trade creation route selects the appropriate trade manager based on auth mode:
  - Paper mode (X-User-Id header) -> shadow trade manager
  - Live mode (JWT auth) -> live trade manager
- **FR-3.3:** Both trade managers subscribe to the same `PriceFeedService` broadcast channel.
- **FR-3.4:** `GET /trades/{id}/management` checks both managers.

### FR-4: Safety Guardrails

- **FR-4.1:** Testnet by default. Production requires `BINANCE_FUTURES_LIVE=true` environment variable.
- **FR-4.2:** Order confirmation -- after `place_order`, poll `get_order` up to 3 times at 500ms intervals to confirm acceptance. No retry on missing order (avoid double-fills).
- **FR-4.3:** Amend failure protection -- if amend fallback (cancel+replace) leaves a gap where the position has no stop loss, the evaluator retries replacement immediately. Critical log + WS alert on total failure.
- **FR-4.4:** Rate limit tracking from response headers. Local rejection at 90% of 2400 weight/min threshold.

### FR-5: Configuration

- **FR-5.1:** API credentials loaded from user's exchange account config (existing `exchange_accounts` table).
- **FR-5.2:** Default leverage configurable via environment variable `BINANCE_FUTURES_LEVERAGE` (default: 1).
- **FR-5.3:** Position mode: one-way (`positionSide=BOTH`). Hedge mode not supported in v1.

## File Context

### New files:
- `testudo-exchange/crates/common_utils/src/adapters/binance_futures_executor.rs`
- `testudo-exchange/crates/common_utils/src/adapters/futures_types.rs`

### Modified files:
- `testudo-exchange/crates/common_utils/src/adapters/mod.rs` (export new executor)
- `testudo-exchange/crates/router/src/services/exchange_api.rs` (replace stub with real impl)
- `testudo-exchange/crates/router/src/services/mod.rs` (export BinanceFuturesExchangeApi)
- `testudo-exchange/crates/router/src/routes/trade_management.rs` (mode-aware manager selection)
- `testudo-exchange/crates/router/src/main.rs` (wire live trade manager)

## Acceptance Criteria

1. `BinanceFuturesExecutor` passes all unit tests with mock mode (feature flag off)
2. `BinanceFuturesExchangeApi` implements all 5 `ExchangeApi` methods
3. Paper mode trades continue to use `ShadowExchangeApi` (no regression)
4. Live mode trades route through `BinanceFuturesExchangeApi`
5. Rate limiting tracks Binance weight headers and rejects locally at threshold
6. Amend fallback (cancel+replace) handles partial failure with alert
7. Testnet is default; production requires explicit env var
8. `cargo clippy && cargo test` pass with zero warnings in new code

## Completion Signal

All acceptance criteria met. Tests pass. Commit message: `feat: implement EXT-10 Binance Futures live execution`.
