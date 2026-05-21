# Specification: Circle/Arc Exchange Adapter — USDC Settlement via Developer-Controlled Wallets

**Spec ID:** AGENT-04-circle-adapter
**Date:** 2026-05-21
**Status:** Draft
**Class:** Infrastructure / Integration
**Priority:** P0 — required for Agora Agents Hackathon submission; zero Circle tool usage without this
**Depends on:** AGENT-01-signal-endpoint (signals need a target exchange), AW-01-through-AW-05 (agent wallet pattern to replicate)
**Series:** AGENT-04 through AGENT-05 (Hackathon Delivery)

---

## Problem Statement

AGENT-01 through AGENT-03 provide a complete agent pipeline: signal ingestion → risk validation → exchange execution → WebSocket alerts → journal feedback. But the exchange dispatch only covers CEX APIs (Binance, Bybit) and Hyperliquid. The Agora Agents Hackathon requires **Circle tool usage** (Criterion 3, 20% weighting): on-chain USDC settlement via Circle's developer-controlled wallets on the Arc Network.

Arc is Circle's EVM-compatible Layer-1 blockchain (Chain ID `5042002`, testnet live since Oct 2025) where **USDC serves as the native gas token**. Every trade settlement is an on-chain USDC transfer — not an exchange order book fill. The Circle Developer-Controlled Wallets SDK provides the same delegate-key security model already implemented in Testudo for Hyperliquid agent wallets (AW-01 through AW-05): a server-held keypair can execute transfers but not withdraw to external addresses, scoped by the entity secret.

Testudo's `ExchangeApi` trait is designed for extension — new exchange adapters implement `place_order()`, `cancel_order()`, `get_balance()`, and a constructor. The `AuthMode` enum already supports `Direct` and `Agent { user_address }` variants. Adding `AuthMode::CircleAgent` and a `CircleExchangeApi` implementation is a direct extension of existing patterns.

However, Circle's SDK is Node.js/TypeScript only (no Rust SDK). The existing `testudo-cex` sidecar pattern (Bun + Express, port 3100) is the natural integration point — the Circle adapter runs as a REST sidecar, and the Rust router calls it via HTTP, identical to how `CexExchangeApi` calls the CCXT sidecar today.

---

## User Stories

- **As a hackathon judge**, I want to see an AI agent submit a trade that settles on-chain as a USDC transfer on Arc testnet, so that I can verify real Circle tool usage.
- **As an agent developer**, I want to route trades to Circle wallets with the same API contract as CEX and Hyperliquid, so that I can backtest strategies across multiple settlement venues.
- **As a user**, I want my agent's Arc wallet balance visible in the Testudo dashboard alongside my CEX and Hyperliquid accounts.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Circle sidecar server (Bun + Express, port 3101) exposes REST endpoints matching testudo-cex contract: `POST /balance`, `POST /order`, `POST /order/cancel`, `GET /health` | High | sidecar |
| FR-2 | Sidecar auto-funds wallets on startup via Circle Faucet API if balance < 100 USDC | High | sidecar |
| FR-3 | `POST /order` on Circle sidecar submits a USDC transfer from agent wallet to recipient wallet on Arc testnet | High | sidecar |
| FR-4 | Transfer amount calculated from TradeIntent quantity × current market price (fetched via price feed or mock oracle) | High | sidecar |
| FR-5 | `AuthMode::CircleAgent { wallet_id, entity_secret }` added to Rust router's auth system | High | Router |
| FR-6 | `CircleExchangeApi` implements `ExchangeApi` trait, calling the Circle sidecar via HTTP | High | Router |
| FR-7 | `POST /balance` returns USDC balance from Arc testnet wallet via Circle SDK `getWallet()` | Medium | sidecar |
| FR-8 | Credential storage follows AW-01 pattern: `auth_mode = 'circle_agent'`, `api_key_encrypted` = wallet ID, `api_secret_encrypted` = entity secret (encrypted via AesGcmVault) | High | Router + DB |
| FR-9 | DB migration adds `circle_agent` to `auth_mode` CHECK constraint | High | Database |
| FR-10 | Arcscan block explorer link appears in trade details for Circle orders | Low | Router |

---

## Technical Implementation

### Arc Network Details

| Parameter | Value |
|-----------|-------|
| Chain ID | `5042002` |
| RPC URL (standard) | `https://rpc.testnet.arc.network` |
| RPC URL (Canteen) | `https://rpc.testnet.arc-node.thecanteenapp.com/v1/{server_token}` |
| Native currency | USDC (18 decimals on-chain, 6 decimals ERC-20) |
| Block explorer | `https://testnet.arcscan.app` |
| Circle Faucet | `https://faucet.circle.com/api/request` |
| Circle API base | `https://api.circle.com/v1/w3s` |

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  External Agent                                              │
│  POST /api/v1/signals { symbol: "ETH_USDC", side: "LONG" }  │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  testudo-exchange (Rust)                                     │
│  DecisionLoop → risk validation → position sizing            │
│  ExchangeApi::place_order() → CircleExchangeApi              │
│  AuthMode::CircleAgent { wallet_id, entity_secret }          │
└──────────────────────┬──────────────────────────────────────┘
                       │ HTTP POST /order
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  testudo-circle (Bun + Express, port 3101)                   │
│  Circle SDK: createTransaction(USDC, from_wallet, to_wallet) │
│  Arc testnet RPC via Canteen endpoint                        │
└──────────────────────┬──────────────────────────────────────┘
                       │ on-chain USDC transfer
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  Arc Network (Chain ID 5042002)                              │
│  USDC transfer settled on-chain                              │
│  Viewable on testnet.arcscan.app                             │
└─────────────────────────────────────────────────────────────┘
```

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Scaffold Circle sidecar: Bun + Express, health endpoint, Circle SDK init, wallet creation on startup | Sidecar starts, POST /health returns 200, wallets created on Circle |
| CP-2 | Implement balance + order endpoints in sidecar | POST /balance returns USDC amount, POST /order submits USDC transfer |
| CP-3 | Add `AuthMode::CircleAgent` + DB migration + Rust `CircleExchangeApi` | Circle agent credentials encrypt/decrypt, ExchangeApi trait implemented |
| CP-4 | Wire router → sidecar HTTP calls, end-to-end signal → USDC transfer | Agent POST /signals → Circle sidecar → USDC on Arc testnet |
| CP-5 | Faucet auto-funding, Arcscan explorer links, error handling polish | Wallet auto-funded on startup, trade details link to Arcscan |

### Sidecar Endpoint Contract

The Circle sidecar follows the same contract as `testudo-cex/src/server.ts` for compatibility:

```typescript
// POST /balance
// Request:  { account_id: string }
// Response: { balance: string, currency: "USDC", wallet_address: string }

// POST /order
// Request:  { account_id: string, symbol: string, side: "BUY" | "SELL",
//             quantity: number, price?: number }
// Response: { order_id: string, status: "filled", tx_hash: string,
//             amount_usdc: string, explorer_url: string }

// POST /order/cancel
// (No-op for on-chain settlement — USDC transfers are final)

// GET /health
// Response: { status: "ok", chain_id: 5042002, wallet_count: number }
```

### Paved Roads

- `crates/router/src/services/exchange_api.rs` — `ExchangeApi` trait to implement
- `crates/router/src/services/hyperliquid/exchange_api.rs` — reference implementation of trait for another chain
- `crates/router/src/services/hyperliquid/mod.rs` — `AuthMode` enum pattern to extend
- `crates/router/src/repositories/exchange_account.rs` — credential storage with `AesGcmVault`
- `crates/sqlx_postgres/migrations/20260316000000_add_agent_wallet_columns.up.sql` — migration template
- `testudo-cex/src/server.ts` — sidecar pattern to replicate
- `testudo-cex/src/gateway.ts` — exchange lifecycle management pattern
- `@circle-fin/developer-controlled-wallets` (npm) — Circle SDK
- `viem` (npm) — Ethereum JSON-RPC for direct Arc RPC calls

### Files

**New files:**
- `testudo-circle/` — **NEW directory** — Circle sidecar
  - `package.json` — dependencies: `@circle-fin/developer-controlled-wallets`, `express`, `viem`, `dotenv`
  - `tsconfig.json`
  - `src/server.ts` — Express server, PSK middleware, route wiring (port 3101)
  - `src/circle-client.ts` — Circle SDK wrapper: wallet creation, balance query, USDC transfer
  - `src/handlers.ts` — balance, order, health handlers
  - `src/types.ts` — request/response Zod schemas
  - `.env.example` — `CIRCLE_API_KEY`, `CIRCLE_ENTITY_SECRET`, `ARC_RPC_URL`, `SIDECAR_PSK`

**Modified files:**
- `testudo-exchange/crates/router/src/services/circle_exchange_api.rs` — **NEW** — `CircleExchangeApi` struct implementing `ExchangeApi` trait, HTTP calls to sidecar
- `testudo-exchange/crates/router/src/services/circle/mod.rs` — **NEW** — `AuthMode::CircleAgent`, `CircleAuth`
- `testudo-exchange/crates/router/src/services/exchange_api.rs` — add `Circle` variant to exchange dispatch
- `testudo-exchange/crates/router/src/repositories/exchange_account.rs` — add `circle_agent` to auth_mode handling
- `testudo-exchange/crates/router/src/routes/signal.rs` — add Circle routing in live-mode dispatch
- `testudo-exchange/crates/sqlx_postgres/migrations/20260521000000_add_circle_agent_auth_mode.up.sql` — **NEW**
- `testudo-exchange/crates/sqlx_postgres/migrations/20260521000000_add_circle_agent_auth_mode.down.sql` — **NEW**

### Dependencies Added

**Sidecar (npm):**
- `@circle-fin/developer-controlled-wallets` — wallet management, transfers
- `viem` — Arc RPC calls (balance checks, transaction receipts)
- (express, dotenv, zod already in the project)

**Rust (no new crates):**
- All HTTP calls use existing `reqwest` (same as CEX sidecar)
- No new Rust dependencies

---

## Acceptance Criteria

- [ ] `POST /api/v1/exchanges/accounts` accepts `exchange_name = "circle_arc"` and `auth_mode = "circle_agent"`
- [ ] Circle agent wallet is created on Circle platform (visible in Circle dashboard) on account creation
- [ ] `AuthMode::CircleAgent` decrypts entity secret, derives wallet details
- [ ] `CircleExchangeApi::place_order()` calls sidecar `POST /order` and returns `OrderResult`
- [ ] `POST /api/v1/signals` with a Circle-linked account routes to `CircleExchangeApi`
- [ ] USDC transfer appears on Arc testnet block explorer (testnet.arcscan.app) after signal execution
- [ ] `POST /balance` returns real USDC balance from Arc testnet wallet
- [ ] Circle sidecar starts on port 3101 without errors
- [ ] Sidecar auto-funds wallet on startup when balance < 100 USDC testnet
- [ ] `GET /health` on sidecar returns chain_id `5042002`
- [ ] Error path: insufficient balance → 422 with clear error message
- [ ] Error path: Circle API down → 503 with retry-after header
- [ ] `cargo clippy --all-targets && cargo test` passes in testudo-exchange
- [ ] Sidecar builds clean: `cd testudo-circle && bun install && bun run build`
- [ ] PSK protection on sidecar (matches testudo-cex security pattern)

---

## Risks

1. **Circle SDK is Node.js only** — No Rust SDK. Sidecar pattern adds an HTTP hop (est. 5-10ms latency, acceptable for agent trades). Mitigation: keep sidecar localhost, use persistent HTTP connections.
2. **On-chain finality vs exchange fills** — USDC transfers on Arc have block confirmation time (~2s). This is slower than CEX order matching (~ms) but faster than Hyperliquid finality (~3-5s). Agent strategies must account for this latency.
3. **USDC as "trade size"** — Unlike CEX futures where 1 ETH long = contract notional, on-chain USDC settlement means the trade IS the transfer. Positioning must be expressed in USDC amounts directly. The risk engine's position sizing already works in account-percentage terms, which maps naturally to USDC.
4. **Testnet only** — Arc mainnet is not yet launched. All trades execute on testnet USDC (no real value). Hackathon judges evaluate on technical correctness, not real P&L.
5. **Canteen RPC vs standard RPC** — The hackathon uses Canteen's hosted RPC (`rpc.testnet.arc-node.thecanteenapp.com`). The sidecar must accept RPC URL via env var to support both.

---

## Completion Signal

This spec is complete when:
1. Circle sidecar serves balance/order/health endpoints
2. Rust router dispatches to CircleExchangeApi for circle_agent accounts
3. Agent signal → USDC transfer on Arc testnet works end-to-end
4. All 15 acceptance criteria met
5. `cargo clippy --all-targets && cargo test` passes
6. Code committed to master
