# HL-07: Hyperliquid Live Trading Fixes

**Status:** Complete
**Priority:** P0
**Created:** 2026-03-19

## Context

First live test of Hyperliquid integration via agent wallet on mainnet. Several issues discovered and fixed, one critical issue remaining.

## Completed Fixes

### 1. wagmi v3 → v2 Downgrade
- **Problem:** RainbowKit 2.2.10 requires wagmi ^2.9.0, but wagmi 3.5.0 was installed. `@metamask/sdk` couldn't resolve.
- **Fix:** Downgraded wagmi to ^2.19.3 in `testudo-web/package.json`. Used npm instead of bun (bun hangs on @metamask/sdk resolution). Removed stubs.
- **Files:** `testudo-web/package.json`, `testudo-web/vite.config.ts`

### 2. EIP-712 chainId Fix (Mainnet vs Testnet)
- **Problem:** `agent_approval.rs` hardcoded `SIGNATURE_CHAIN_ID = 421614` (Arbitrum Sepolia testnet) for all networks. MetaMask rejected with "chainId 421614 must match active chainId 42161".
- **Fix:** Replaced constant with `signature_chain_id(network)` function: mainnet=42161, testnet=421614. Updated `submit_approval` to use same function for `signatureChainId`.
- **File:** `testudo-exchange/crates/router/src/services/hyperliquid/agent_approval.rs`
- **Tests:** All 10 tests pass.

### 3. Balance Endpoint — Hyperliquid Native Path
- **Problem:** `get_exchange_balance` handler always routed through CCXT sidecar, which doesn't support Hyperliquid. Balance showed `$--`.
- **Fix:** Added `get_hyperliquid_balance()` function that calls Hyperliquid info API (`clearinghouseState`) directly using `hl_http_client` from AppState. Dispatches based on `creds.exchange_name == "hyperliquid"`.
- **File:** `testudo-exchange/crates/router/src/routes/exchanges.rs`

### 4. Test Connection — Hyperliquid Native Path
- **Problem:** Same as #3 — test button routed through CCXT sidecar.
- **Fix:** Added Hyperliquid branch in `test_exchange_connection` handler.
- **File:** `testudo-exchange/crates/router/src/routes/exchanges.rs`

### 5. Extension Balance Asset Matching
- **Problem:** `MainView.tsx` searched for asset `"USDT"` but Hyperliquid returns `"USDC"`.
- **Fix:** Changed to `b.asset === "USDT" || b.asset === "USDC"`.
- **File:** `testudo-extension/src/popup/components/MainView.tsx`

### 6. RUST_LOG Noise Reduction
- **Problem:** Default `debug` log level flooded terminal with hyper/sqlx chatter, making errors impossible to find.
- **Fix:** Added `RUST_LOG=router=info,warn` to `.env`.
- **File:** `testudo-exchange/.env`

### 7. Agent Wallet Route Feature Flag
- **Problem:** Agent wallet routes gated behind `HYPERLIQUID_AGENT_WALLET_ENABLED=true` env var — returned 404 when not set.
- **Fix:** User added the env var to `.env`. (No code change needed.)

### 8. Order Placement — Agent Address Mismatch (FIXED)
- **Error:** `Place order failed: HTTP error: status 422, body: Failed to deserialize the JSON body into the target type`
- **Root cause:** `build_exchange()` in `exchange_api.rs` passed `auth.address` (the agent's derived address) to `ExchangeProvider::mainnet_agent(signer, agent_address)`, but the SDK expects the **user's wallet address** as `agent_address`. The SDK wraps orders in an `agent` action with `agentAddress` — passing the wrong address caused Hyperliquid to reject the request because no agent approval existed for that address.
- **Fix:** Destructure `AuthMode::Agent { user_address }` and pass `*user_address` instead of `auth.address`.
- **File:** `testudo-exchange/crates/router/src/services/hyperliquid/exchange_api.rs` (lines 139-144)
- **Tests:** All 959 tests pass.

## Other Notes
- Ghost positions appear in shadow engine when exchange rejects — the rollback doesn't happen. This is a known design tradeoff (fire-and-forget placement), but the toast should show the actual exchange error, not a generic message.
- WOO sidecar rate limiting: CCXT sidecar hammering WOO WebSocket reconnects. Unrelated to Hyperliquid but cluttering logs.
- WebSocket on port 4000 (ws-stream) not running — not needed for HTTP order placement but needed for fill detection.
