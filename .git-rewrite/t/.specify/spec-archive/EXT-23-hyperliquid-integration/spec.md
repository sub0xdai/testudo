# EXT-23: Hyperliquid Integration

| Field    | Value                                                    |
|----------|----------------------------------------------------------|
| Status   | Draft                                                    |
| Date     | 2026-03-01                                               |
| Depends  | EXT-21, EXT-22, 012-ccxt-multi-exchange                  |
| Phase    | Exchange Integration — Decentralized Perpetual Futures    |

## 1. Overview

### Current State

Testudo supports centralized exchanges (WOO, Binance, Bybit, OKX) via the CCXT sidecar. All exchanges share the same integration pattern: API key + secret stored server-side, CCXT handles symbol mapping and order placement, the backend is the sole executor.

### Target State

Add Hyperliquid as a first-class exchange option alongside CEXs. Hyperliquid is a decentralized perps exchange that authenticates via Ethereum wallet signatures instead of API keys. This requires a fundamentally different auth and execution model:

- **No API keys** — authentication is via EIP-712 typed data signing with an Ethereum private key.
- **No CCXT support** — Hyperliquid has its own REST/WebSocket API and TypeScript SDK (`@nktkas/hyperliquid`).
- **Client-side signing** — the extension holds an agent wallet key and signs order payloads locally. The private key never reaches the server.
- **Backend computes, extension signs** — the existing risk pipeline (PositionSizer, DecisionLoop, shadow engine) constructs the order. The extension signs it. The backend submits the signed payload.

### Design Principle

Hyperliquid is a **parallel integration path**, not a replacement. The CCXT sidecar continues to handle all CEXs. A new `testudo-hyperliquid` sidecar (or module within the existing CCXT sidecar) handles Hyperliquid-specific API calls. The backend abstracts over both via the existing `ExchangeApi` trait. From the user's perspective, Hyperliquid is just another exchange in the dropdown — with a one-time wallet connection step instead of pasting API keys.

## 2. User Stories

- **US-1**: As a trader, I can connect my Hyperliquid account by clicking "Connect Wallet" in the extension, signing one MetaMask approval, and trading without further popups.
- **US-2**: As a trader, I can use Alt+X on TradingView to place trades on Hyperliquid with the same entry + SL + TP workflow I use on CEXs.
- **US-3**: As a trader, I see my Hyperliquid balance and open positions in the extension popup, just like CEX accounts.
- **US-4**: As a trader, when my SL fills on Hyperliquid, the sibling TP is cancelled automatically (same OCO behavior as CEXs).

## 3. Architecture

### 3.1 CEX Flow (existing — unchanged)

```
Alt+X → Extension → POST /api/v1/trades (JWT)
  → Backend (risk, sizing, shadow)
  → CCXT Sidecar (createOrder)
  → Exchange API
```

### 3.2 Hyperliquid Flow (new)

```
Alt+X → Extension → POST /api/v1/trades (JWT + walletAddress)
  → Backend (risk, sizing, shadow)
  → Backend constructs unsigned Hyperliquid order payload
  ← Returns unsigned payload to extension
  → Extension signs with agent wallet key (viem, local)
  → Extension sends signed payload back to backend
  → Backend POSTs to api.hyperliquid.xyz/exchange
  ← Order confirmation (oid) stored in OrderGroup
```

### 3.3 Agent Wallet Setup (one-time)

```
Extension generates fresh keypair (agent wallet)
  → Content script calls window.ethereum.request({
      method: 'eth_signTypedData_v4',
      params: [userAddress, approveAgentTypedData]
    })
  → MetaMask popup: user approves agent wallet
  → Extension stores agent private key in encrypted extension storage
  → Extension sends agent wallet address + user wallet address to backend
  → Backend stores wallet addresses (no private keys) as exchange account
```

The agent wallet can only trade — it cannot withdraw funds or transfer assets. The user can revoke it from Hyperliquid's UI at any time.

## 4. Functional Requirements

### FR-1: Wallet Connection UI

**Files:** `testudo-extension/src/popup/components/ExchangeManager.tsx`, new `WalletConnect.tsx`

When the user selects "Hyperliquid" from the exchange dropdown, show a "Connect Wallet" flow instead of API key fields:

1. Detect `window.ethereum` provider via content script bridge
2. Request `eth_requestAccounts` to get the user's wallet address
3. Generate a fresh agent keypair using `viem/accounts` (`generatePrivateKey()`)
4. Construct the `approveAgent` EIP-712 typed data:
   ```json
   {
     "type": "approveAgent",
     "hyperliquidChain": "Mainnet",
     "signatureChainId": "0xa4b1",
     "agentAddress": "<generated-agent-address>",
     "agentName": "Testudo Sniper",
     "nonce": <timestamp_ms>
   }
   ```
5. Call `eth_signTypedData_v4` via content script → MetaMask popup
6. Submit the signed `approveAgent` action to `https://api.hyperliquid.xyz/exchange`
7. On success, store agent private key in `browser.storage.local` (encrypted)
8. Register the account on the backend: `POST /api/v1/exchange-accounts` with `exchange_name: "hyperliquid"`, `wallet_address`, `agent_address` (no API key/secret)

### FR-2: Content Script Wallet Bridge

**Files:** `testudo-extension/src/content-script.ts` or new `wallet-bridge.ts`

The content script runs on TradingView pages where MetaMask injects `window.ethereum`. The extension popup and background worker cannot access `window.ethereum` directly.

Add a message-based bridge:

- **Background → Content Script**: `{ type: "WALLET_REQUEST", method: "eth_requestAccounts" | "eth_signTypedData_v4", params: [...] }`
- **Content Script → Background**: `{ type: "WALLET_RESPONSE", result: ... }` or `{ type: "WALLET_ERROR", error: ... }`

The content script executes `window.ethereum.request(...)` and relays the result. This bridge is only used during setup (agent approval) — all subsequent signing uses the stored agent key.

### FR-3: Agent Wallet Signing Service

**Files:** new `testudo-extension/src/hyperliquid-signer.ts`

A local signing module that:

1. Loads the agent private key from encrypted extension storage
2. Constructs EIP-712 typed data for Hyperliquid actions (order, cancel, updateLeverage)
3. Signs using `viem`'s `signTypedData` with the agent account
4. Returns the signature components `{ r, s, v }`

This runs entirely in the extension — no network calls, no MetaMask popups.

```typescript
interface HyperliquidSigner {
  signOrder(payload: HyperliquidOrderAction, nonce: number): Promise<Signature>;
  signCancel(payload: HyperliquidCancelAction, nonce: number): Promise<Signature>;
  getAgentAddress(): string;
  getUserAddress(): string;
  isConfigured(): boolean;
}
```

### FR-4: Hyperliquid Sidecar Module

**Files:** new `testudo-ccxt/src/hyperliquid.js` (or separate `testudo-hyperliquid/` service)

A module within the CCXT sidecar (same Node.js process, different route prefix) that handles Hyperliquid-specific API calls using `@nktkas/hyperliquid` SDK:

1. **`POST /hyperliquid/meta`** — Fetch perpetual metadata (asset IDs, symbols). Cache on startup and refresh periodically.
2. **`POST /hyperliquid/balance`** — Fetch clearinghouse state for a wallet address. Returns `accountValue`, `withdrawable`, `marginUsed`, positions.
3. **`POST /hyperliquid/order`** — Submit a pre-signed order payload to Hyperliquid's exchange endpoint. The sidecar is a relay — it does not sign.
4. **`POST /hyperliquid/cancel`** — Submit a pre-signed cancel payload.
5. **`WebSocket /ws/hyperliquid/orders`** — Subscribe to `orderUpdates` and `userFills` for a wallet address. Push events to the Rust backend (same pattern as EXT-22's `/ws/orders`).

**Asset ID resolution:** The sidecar maintains a `symbol → assetId` mapping from the `/info` `meta` endpoint. The backend sends symbols like `BTC` or `ETH`; the sidecar resolves to the integer ID.

### FR-5: Backend Exchange Abstraction

**Files:** `crates/router/src/services/exchange_api.rs`, `crates/router/src/routes/trade_management.rs`, `crates/router/src/services/ccxt_client.rs`

Extend the `ExchangeApi` trait and `CcxtClient` to handle the Hyperliquid two-phase flow:

1. **Detection**: When the active exchange account is `exchange_name: "hyperliquid"`, the backend enters the Hyperliquid code path.
2. **Order construction**: `create_trade` builds the order payload (asset ID, side, price, size, reduce-only, trigger) but does NOT submit it. Returns the unsigned payload to the extension.
3. **New endpoint**: `POST /api/v1/trades/sign` — Extension sends back the signed payload. Backend validates the signature matches the expected order, then relays to the Hyperliquid sidecar.
4. **Cancel**: Backend constructs cancel payload, extension signs, backend submits.

The two-phase flow requires a new response type from `create_trade`:

```rust
enum TradeCreationResult {
    // CEX: order placed immediately
    Executed { group_id: Uuid, exchange_order_ids: Vec<String> },
    // Hyperliquid: needs client signature
    PendingSignature {
        group_id: Uuid,
        unsigned_payloads: Vec<UnsignedHyperliquidAction>,
    },
}
```

### FR-6: Hyperliquid TP/SL Grouping

Hyperliquid supports native order grouping:
- `"na"` — standalone order
- `"normalTpsl"` — entry + TP + SL placed atomically
- `"positionTpsl"` — TP/SL auto-adjusts with position size changes

Use `"normalTpsl"` grouping to place entry + SL + TP as a **single atomic action** requiring only **one signature**. This means:

1. Backend constructs all three orders (entry, SL trigger, TP trigger) in one payload
2. Extension signs once
3. Hyperliquid handles OCO cancellation natively — when SL fills, TP is cancelled by the exchange

This eliminates the need for the FillDetectorService OCO logic for Hyperliquid trades. The shadow engine still tracks state for UI purposes, but Hyperliquid is the source of truth for order lifecycle.

### FR-7: Hyperliquid Balance Display

**Files:** `testudo-extension/src/background.ts`, popup components

The existing balance display fetches from the CCXT sidecar. For Hyperliquid:

1. Background worker calls the Hyperliquid sidecar's `/hyperliquid/balance` endpoint with the user's wallet address
2. Maps the clearinghouse response to the existing balance display format:
   - `accountValue` → total balance
   - `withdrawable` → available balance
   - `totalMarginUsed` → used margin
3. Position list from `assetPositions` maps to the existing position card format

### FR-8: Hyperliquid Fill Detection

**Files:** `crates/router/src/services/fill_detector.rs`

Subscribe to Hyperliquid WebSocket events for the connected wallet:

1. `orderUpdates` — real-time order status changes (same purpose as CCXT `watchOrders`)
2. `userFills` — fill events with `coin`, `px`, `sz`, `side`, `closedPnl`, `oid`

Since Hyperliquid handles OCO natively (FR-6), the fill detector's role for Hyperliquid is primarily **UI notification** — pushing status updates to the extension so positions reflect real-time state.

The fill detector should detect the exchange type and branch:
- **CEX**: Full OCO cancellation logic (existing EXT-22 behavior)
- **Hyperliquid**: State update + UI notification only (exchange handles OCO)

## 5. Hyperliquid API Reference

### Order Payload

```json
{
  "action": {
    "type": "order",
    "orders": [
      {
        "a": 0,          // asset ID (0 = BTC, from /info meta)
        "b": true,       // true = buy/long, false = sell/short
        "p": "95000",    // price (string)
        "s": "0.01",     // size in base currency (string)
        "r": false,      // reduce-only
        "t": {
          "limit": { "tif": "Gtc" }           // limit order
          // OR
          "trigger": {
            "isMarket": true,                   // market trigger (for SL)
            "triggerPx": "91000",               // trigger price
            "tpsl": "sl"                        // "sl" or "tp"
          }
        },
        "c": "0x..."     // optional client order ID
      }
    ],
    "grouping": "normalTpsl"  // atomic entry + SL + TP
  },
  "nonce": 1709280000000,
  "signature": { "r": "...", "s": "...", "v": "..." },
  "vaultAddress": null
}
```

### Cancel Payload

```json
{
  "action": {
    "type": "cancel",
    "cancels": [
      { "a": 0, "o": 12345 }   // asset ID + order ID
    ]
  },
  "nonce": 1709280000000,
  "signature": { "r": "...", "s": "...", "v": "..." },
  "vaultAddress": null
}
```

### Balance Query (no signature needed)

```json
POST https://api.hyperliquid.xyz/info
{ "type": "clearinghouseState", "user": "0x..." }
```

### Asset ID Resolution (no signature needed)

```json
POST https://api.hyperliquid.xyz/info
{ "type": "meta" }
// Response: { "universe": [{ "name": "BTC", ... }, { "name": "ETH", ... }] }
// BTC = index 0, ETH = index 1, etc.
```

### WebSocket Subscriptions

```json
// Order status updates
{ "method": "subscribe", "subscription": { "type": "orderUpdates", "user": "0x..." } }

// Trade fills
{ "method": "subscribe", "subscription": { "type": "userFills", "user": "0x..." } }
```

## 6. Files to Modify / Create

| File | Change | Component |
|------|--------|-----------|
| `testudo-extension/src/popup/components/ExchangeManager.tsx` | FR-1: Conditional wallet connect vs API key form | Extension |
| `testudo-extension/src/popup/components/WalletConnect.tsx` | FR-1: New wallet connection component | Extension |
| `testudo-extension/src/wallet-bridge.ts` | FR-2: Content script ↔ window.ethereum bridge | Extension |
| `testudo-extension/src/hyperliquid-signer.ts` | FR-3: Local agent wallet signing | Extension |
| `testudo-extension/src/background.ts` | FR-1,3,7: Wallet message handling, balance fetch | Extension |
| `testudo-extension/src/content-script.ts` | FR-2: Register wallet bridge listener | Extension |
| `testudo-extension/src/types.ts` | FR-1: Add wallet-based account types | Extension |
| `testudo-ccxt/src/hyperliquid.js` | FR-4: Hyperliquid API module | Sidecar |
| `testudo-ccxt/src/server.js` | FR-4: Mount Hyperliquid routes | Sidecar |
| `testudo-ccxt/package.json` | FR-4: Add `@nktkas/hyperliquid`, `viem` deps | Sidecar |
| `crates/router/src/services/exchange_api.rs` | FR-5: Extend trait for two-phase signing | Backend |
| `crates/router/src/routes/trade_management.rs` | FR-5,6: Hyperliquid order construction, `/trades/sign` endpoint | Backend |
| `crates/router/src/services/ccxt_client.rs` | FR-4,5: Hyperliquid sidecar client methods | Backend |
| `crates/router/src/services/fill_detector.rs` | FR-8: Branch CEX vs Hyperliquid fill handling | Backend |
| `crates/router/src/main.rs` | FR-4,8: Register Hyperliquid routes and WS | Backend |

## 7. Acceptance Criteria

- [ ] User can connect Hyperliquid via wallet (MetaMask/Rabby) with one-click agent approval
- [ ] Agent wallet key is stored encrypted in extension storage, never sent to backend
- [ ] Hyperliquid appears as a selectable exchange in the extension dropdown
- [ ] Alt+X on TradingView places entry + SL + TP on Hyperliquid as a single `normalTpsl` grouped order
- [ ] Only one signature is required per trade (agent wallet signs locally, no MetaMask popup)
- [ ] User sees Hyperliquid balance (account value, available, margin used) in the popup
- [ ] Open positions on Hyperliquid display in the positions panel
- [ ] Order fill events arrive via WebSocket and update the UI in real-time
- [ ] Backend risk pipeline (PositionSizer, fixed fractional) applies to Hyperliquid trades
- [ ] Cancel trade propagates to Hyperliquid (cancel by order ID)
- [ ] CEX flow is completely unaffected — no regressions on WOO/Binance/Bybit
- [ ] All existing tests pass (`cargo test`, `vitest run`, `npm test`)
- [ ] New tests: wallet bridge, agent signer, Hyperliquid sidecar module, two-phase trade flow

## 8. Security Considerations

- **Agent key scope**: The agent wallet can only trade. It cannot withdraw, transfer, or approve other agents. Blast radius is limited to trading actions.
- **Key storage**: Agent private key stored in `browser.storage.local` with encryption. Cleared on account removal.
- **No server-side keys**: The backend never sees the agent private key. It only stores the public wallet addresses.
- **Revocation**: User can revoke the agent wallet from Hyperliquid's web UI at any time, immediately invalidating the stored key.
- **Content script isolation**: The wallet bridge only responds to messages from the extension's own background worker (verified via `runtime.id`).

## 9. Out of Scope

- **Spot trading** — Perps only, matching the existing CEX integration.
- **Vault trading** — `vaultAddress` is set to `null`. Vault support is a future extension.
- **Sub-accounts** — Single main account per wallet connection.
- **Deposits/withdrawals** — Users manage funds on Hyperliquid's own UI.
- **Agent wallet rotation** — V1 uses a single agent wallet. Key rotation is a future improvement.
- **Multiple wallets** — One Hyperliquid wallet per Testudo account. Multi-wallet is out of scope.

## 10. Implementation Order

1. **FR-4** (Hyperliquid sidecar module) — Can build and test independently against Hyperliquid testnet
2. **FR-2** (Wallet bridge) — Content script ↔ `window.ethereum` plumbing
3. **FR-1** (Wallet connection UI) — Connect wallet + agent approval flow
4. **FR-3** (Agent signer) — Local signing service with viem
5. **FR-5** (Backend abstraction) — Two-phase trade flow, `/trades/sign` endpoint
6. **FR-6** (TP/SL grouping) — `normalTpsl` atomic order construction
7. **FR-7** (Balance display) — Clearinghouse state → popup
8. **FR-8** (Fill detection) — WebSocket subscriptions for order updates

## 11. Verification

1. `cd testudo-exchange && cargo test` — all existing + new tests pass
2. `cd testudo-ccxt && npm test` — sidecar tests pass including Hyperliquid module
3. `cd testudo-extension && npx vitest run` — extension tests pass
4. Manual: Connect wallet via MetaMask on TradingView → agent approved → Hyperliquid appears as active exchange
5. Manual: Alt+X → place BTC long with SL + TP → single signature → all three orders on Hyperliquid
6. Manual: SL triggers → TP auto-cancelled by Hyperliquid → extension UI updates in real-time
7. Manual: Switch back to WOO → CEX flow works identically to before
