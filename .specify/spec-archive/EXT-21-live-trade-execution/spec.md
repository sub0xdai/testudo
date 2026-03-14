# EXT-21: End-to-End Live Trade Execution

| Field    | Value                                         |
|----------|-----------------------------------------------|
| Status   | Draft                                         |
| Date     | 2026-02-28                                    |
| Depends  | EXT-19, EXT-17, EXT-18, 012-ccxt-multi-exchange |
| Phase    | Extension + Backend — Trade Execution Pipeline |

## 1. Overview

### Current State
- User scrapes a trade from TradingView (e.g., BTCUSDT on Binance chart), extension sends it to the backend with `exchange_account_id` for WOO
- **Trade stays PENDING** — never placed on the exchange
- The extension shows "BTC_USD" instead of "BTC_USDT" when TradingView gives "BTCUSD" (no T)
- Backend live trade manager (`trade_manager_live`) only initializes when `CCXT_ENABLED=true` or `CCXT_SIDECAR_URL` is set; otherwise falls back to shadow engine silently
- Symbol normalization is TradingView-centric — no exchange-aware mapping
- No feedback to user when trade routing fails (shadow fallback is silent)

### Target State
- Trades placed from the extension execute on the connected exchange (WOO, Binance, Bybit, OKX)
- Symbol extracted from ANY TradingView chart maps correctly to the connected exchange's market
- Backend surfaces clear errors when CCXT sidecar is unavailable or trade placement fails
- Extension displays exchange-specific order status, not just internal "Pending"

### Root Causes Identified
1. **Symbol mismatch**: `normalizeSymbol("BTCUSD")` produces `BTC_USD` because "USD" matches before iteration ends. No crypto exchange trades "BTC/USD:USD" perps — they all use USDT.
2. **Silent shadow fallback**: When `CCXT_ENABLED` is not set, `trade_manager_live = None`, and `select_trade_manager(is_authenticated=true)` falls back to `trade_manager_shadow` (line 143 of trade_management.rs). Trade goes to paper engine, never reaches exchange.
3. **No execution feedback**: Backend returns `201 Created` with status "Pending" regardless of whether the trade was routed to shadow or live manager. Extension has no way to know.

## 2. User Stories

- **US-1**: As a trader, I chart on TradingView (any pair: BTCUSDT, BTCUSD, BTC.P) and my trade executes on whichever exchange I have connected (WOO, Binance, etc.)
- **US-2**: As a trader, I see immediate feedback if my trade cannot be placed on the exchange (sidecar down, wrong symbol, insufficient balance)
- **US-3**: As a trader, I see the actual exchange order status (open, filled, rejected) instead of a generic "Pending"

## 3. Functional Requirements

### FR-1: Exchange-Agnostic Symbol Normalization (Extension)

**File:** `testudo-extension/src/utils.ts`

Modify `normalizeSymbol()` to always produce exchange-compatible symbols:
- When matched quote is "USD", upgrade to "USDT" (no crypto exchange trades raw USD perps)
- Result: `BTCUSD` -> `BTC_USDT`, `BTCUSDT` -> `BTC_USDT`, `ETHUSD` -> `ETH_USDT`
- Preserve non-USD quotes: `ETHBTC` -> `ETH_BTC`, `BTCEUR` -> `BTC_EUR`

**Tests:** Update `utils.test.ts` — add cases for BTCUSD->BTC_USDT, ETHUSD->ETH_USDT

### FR-2: Backend Must Surface Execution Mode in Trade Response

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs`

Add `execution_mode: "live" | "shadow"` field to the trade creation response so the extension knows which path was taken. If the trade was routed to shadow because `trade_manager_live` is `None`, the response should indicate this.

### FR-3: Backend Must Reject Live Trades When Sidecar Unavailable

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs`

When a JWT-authenticated user submits a trade and `trade_manager_live` is `None`:
- Do NOT silently fall back to shadow
- Return HTTP 503 with error: `"Live trading unavailable — CCXT sidecar not configured"`
- This forces explicit configuration rather than silent paper trading

### FR-4: CCXT Sidecar Must Be Running for Live Trading

**File:** `testudo-exchange/crates/router/src/main.rs` (lines 234-258)

Ensure the sidecar is configured when the backend is intended for live trading:
- Document required env vars: `CCXT_SIDECAR_URL`, `CCXT_SANDBOX=false`
- Log a WARNING at startup if `CCXT_ENABLED` is not set and JWT auth is configured
- The `CCXT_SANDBOX` env var must be `false` for real order placement

### FR-5: Extension Surfaces Trade Execution Errors

**File:** `testudo-extension/src/background.ts` (executeTrade function)

When the backend returns an error (503, 400, etc.) for trade placement:
- Parse the error message
- Return it to the content script
- Content script shows it via toast notification

### FR-6: CCXT Symbol Mapping (Backend — Already Implemented)

**File:** `testudo-exchange/crates/router/src/services/exchange_api.rs`

Already implemented: `to_ccxt_symbol("BTC_USDT")` -> `"BTC/USDT:USDT"`. CCXT library handles mapping unified symbols to exchange-specific formats (e.g., WOO's "BTC-PERP"). No changes needed here — FR-1 ensures the correct input arrives.

## 4. Files to Modify

| File | Change | Component |
|------|--------|-----------|
| `testudo-extension/src/utils.ts` | FR-1: USD->USDT in normalizeSymbol | Extension |
| `testudo-extension/src/utils.test.ts` | FR-1: Add BTCUSD test cases | Extension |
| `testudo-exchange/crates/router/src/routes/trade_management.rs` | FR-2, FR-3: execution_mode in response, reject when no sidecar | Backend |
| `testudo-exchange/crates/router/src/main.rs` | FR-4: Startup warning when CCXT not configured | Backend |

## 5. Acceptance Criteria

- [ ] `normalizeSymbol("BTCUSD")` returns `"BTC_USDT"` (not `"BTC_USD"`)
- [ ] `normalizeSymbol("BTCUSDT")` still returns `"BTC_USDT"` (no regression)
- [ ] `normalizeSymbol("ETHUSD")` returns `"ETH_USDT"`
- [ ] Backend trade creation response includes `execution_mode` field
- [ ] JWT-authenticated trade with no CCXT sidecar returns 503, not 201
- [ ] Extension shows error toast when trade placement fails
- [ ] With CCXT sidecar running + `CCXT_SANDBOX=false` + valid WOO credentials: trade appears as open order on WOO exchange
- [ ] All existing tests pass (`vitest run` + `cargo test`)

## 6. Verification

1. `cd testudo-extension && npx vitest run` — all tests pass including new symbol cases
2. `cd testudo-exchange && cargo test` — all backend tests pass
3. Manual test: Chart BTCUSD on TradingView -> Alt+X -> extension sends BTC_USDT -> order appears on WOO
4. Manual test: Stop CCXT sidecar -> place trade -> extension shows error (not silent PENDING)

## 7. Completion Signal

All acceptance criteria checked. Backend env configured with `CCXT_SIDECAR_URL` and `CCXT_SANDBOX=false`. Trade placed from TradingView (any BTC chart) appears as open order on connected exchange.
