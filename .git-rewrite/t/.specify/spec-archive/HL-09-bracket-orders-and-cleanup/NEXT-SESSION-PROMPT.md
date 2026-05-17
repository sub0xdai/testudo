# Next Session Prompt — HL-09 Completion + Live Trading Unblock

Paste this to resume:

---

## Context

HL-09 (bracket orders, close position fix, ghost cleanup) is code-complete and committed. All 972 Rust tests pass. Extension builds clean.

**BLOCKER: Trades fail because the agent wallet is not registered on Hyperliquid.**

Error: `"User or API Wallet 0x2d20b0a05182016be5bc7ff5745540d0fc09ee1c does not exist."`

The DB account `ca65e1ba-75fb-4f10-99bf-15be30ddf9f2` has been reset to `is_active = false` so the approval flow can be re-triggered.

## Immediate TODO

1. **Restart the backend server** — it needs the latest build with:
   - `is_definitive_rejection()` fix (prevents ghost order creation)
   - `format_exchange_error()` showing real errors
   - `POST /trades/cleanup` endpoint
   - HL-09 bracket order placement

2. **Re-approve agent wallet** via testudo-web:
   - Open testudo-web Account page
   - Connect MetaMask (wallet `0xC285F922116959Db9eAF9f07729faBB7370A5b36`)
   - Click AUTHORIZE AGENT WALLET
   - Sign EIP-712 in MetaMask
   - This submits approval to Hyperliquid API, registering agent `0x2d20...`

3. **Test live trade** — Alt+X on TradingView with SL/TP, verify:
   - Entry + SL + TP all visible on HL web UI (3 orders)
   - Close position from extension works
   - CLEAR ALL button in extension purges ghost orders

4. **If approval fails** — check:
   - `testudo-exchange/crates/router/src/services/hyperliquid/agent_approval.rs` — `submit_approval()` and `verify_registration()`
   - The `agentName: null` field in EIP-712 typed data (was fixed from `""` to `null`)
   - Server logs for the actual HL API response

## What Was Done This Session

### HL-09 Implementation (committed to master)
- `place_trigger_order()` helper on `HyperliquidExchangeApi` — places SL/TP as separate trigger orders after entry
- `TakeProfit` added to `ApiOrderType` enum, all consumers updated (Shadow, CEX, HL place + amend)
- `ExchangeDataStatus::Success` handled — synthetic "success" ID for atomic market fills
- `POST /trades/cleanup` endpoint + `CLEANUP_TRADES` message handler in extension
- `CLEAR ALL` button in ActiveOrders component
- 6 new tests (TakeProfit build, Success status, CLOID SL/TP suffixes)

### Bug Fixes
- `is_definitive_rejection()`: added "does not exist" + "not found" — prevents ghost orders from definitive failures
- `format_exchange_error()`: passes through actual `ExchangeApiError` message instead of generic "rejected by exchange"
- `agentName` test assertion fixed (null vs empty string)

### Extension UX
- Toast CSS deduplicated into single `TOAST_CSS` constant
- Error toast: 85% opacity bg + white text (was 10% translucent + red text — unreadable)
- Error duration: 5s (was 2s), success/info: 3s

### DB Cleanup
- Deleted 6 stale inactive HL agent wallet accounts
- Reset active account `ca65e1ba` to `is_active = false` for re-approval

## Key Files
- `crates/router/src/services/hyperliquid/exchange_api.rs` — place_trigger_order, build_order_request TakeProfit, Success handling
- `crates/router/src/routes/trade_management.rs` — cleanup_stale_trades, is_definitive_rejection, format_exchange_error
- `crates/router/src/routes/exchanges.rs` — approve_agent flow (line 1019)
- `testudo-web/src/components/WalletConnect.tsx` — frontend approval flow
- `testudo-extension/src/modal.tsx` — TOAST_CSS, showToast duration
- `testudo-extension/src/popup/components/ActiveOrders.tsx` — CLEAR ALL button
- `testudo-extension/src/background.ts` — cleanupTrades handler

## RAM Warning
MCP server processes orphan on session end (even with /exit). Run `pkill -f claude` between sessions to reclaim memory. 88 orphaned processes were consuming ~8GB this session.
