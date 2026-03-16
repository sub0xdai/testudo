# Specification: EIP-712 Approval Protocol

**Spec ID:** AW-02-eip712-approval
**Date:** 2026-03-16
**Status:** Complete
**Class:** Core / Protocol
**Priority:** P0 — enables frontend signing flow
**Depends on:** AW-01 (agent-key-generation)
**Series:** AW-01 through AW-05 (Hyperliquid agent wallet authentication)

---

## Problem Statement

After AW-01 generates an agent keypair on the backend, the user must authorize it on-chain via Hyperliquid's `approveAgent` action. This requires the user to sign an EIP-712 typed data message with their main wallet (via MetaMask/WalletConnect), then the backend submits the signed approval to Hyperliquid's API.

The Hyperliquid SDK's `approve_agent()` function assumes the backend holds the user's private key to sign directly. Since we explicitly don't hold the main key (that's the entire point), we need to:
1. Construct the EIP-712 typed data matching the SDK's exact encoding
2. Return it to the frontend for `eth_signTypedData_v4`
3. Accept the signature back, assemble the full payload, and POST to Hyperliquid

This spec implements the approval protocol as two backend endpoints plus a verification step.

---

## User Stories

- **As a trader**, I want to approve the agent wallet by signing a message in MetaMask, so that I never expose my private key to any backend.
- **As a developer**, I want the EIP-712 typed data construction verified against the SDK source, so that signature verification never fails due to encoding mismatches.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `build_eip712_typed_data(agent_address, chain, nonce)` constructs EIP-712 JSON matching SDK's `ApproveAgent` action encoding exactly | High | Service |
| FR-2 | Route `POST /api/v1/exchanges/agent-wallet/approve-data` returns typed data JSON for frontend signing | High | Router |
| FR-3 | Route `POST /api/v1/exchanges/agent-wallet/approve` accepts `{ account_id, signature }`, assembles payload, POSTs to Hyperliquid API | High | Router |
| FR-4 | After successful approval, verify registration via `info.extra_agents(wallet_address)` | High | Service |
| FR-5 | On verification success, update account `is_active = true` and `permissions = { "agent_approved": true }` | Medium | Repository |
| FR-6 | EIP-712 domain uses chainId `421614` (Arbitrum Sepolia) per SDK convention | High | Service |
| FR-7 | Nonce is millisecond timestamp at request time | Medium | Service |

---

## Technical Implementation

### EIP-712 Parameters (from SDK source)

| Parameter | Value |
|-----------|-------|
| Domain name | `"HyperliquidSignTransaction"` |
| Domain version | `"1"` |
| Domain chainId | `421614` (Arbitrum Sepolia — always, even for mainnet) |
| Primary type | `"HyperliquidTransaction:ApproveAgent"` |
| Type string | `HyperliquidTransaction:ApproveAgent(string hyperliquidChain,address agentAddress,string agentName,uint64 nonce)` |
| `hyperliquidChain` | `"Mainnet"` or `"Testnet"` (from `Network` config) |
| `agentName` | `""` (empty string — not used for basic approval) |

### Approve-Data Endpoint

```rust
// POST /api/v1/exchanges/agent-wallet/approve-data
pub struct ApproveDataRequest {
    pub account_id: Uuid,
}

pub struct ApproveDataResponse {
    pub typed_data: serde_json::Value,  // Full EIP-712 JSON for eth_signTypedData_v4
    pub nonce: u64,                     // Millisecond timestamp used
    pub agent_address: String,          // For frontend display
}
```

The handler:
1. Loads account by `account_id` (ownership-verified via JWT user_id)
2. Verifies `auth_mode == "agent_wallet"` and `is_active == false`
3. Decrypts agent address from `api_key_encrypted`
4. Generates nonce (current time ms)
5. Builds EIP-712 typed data JSON
6. Returns typed data for frontend to pass to `eth_signTypedData_v4`

### Approve Endpoint

```rust
// POST /api/v1/exchanges/agent-wallet/approve
pub struct ApproveAgentRequest {
    pub account_id: Uuid,
    pub signature: String,  // 0x-prefixed hex, 65 bytes (r + s + v)
    pub nonce: u64,         // Must match the nonce from approve-data
}

pub struct ApproveAgentResponse {
    pub success: bool,
    pub agent_address: String,
    pub message: String,
}
```

The handler:
1. Loads account, decrypts agent address
2. Reconstructs EIP-712 hash using same nonce
3. Assembles Hyperliquid API payload: `{ "action": { "type": "approveAgent", ... }, "nonce": ..., "signature": { "r": ..., "s": ..., "v": ... } }`
4. POSTs to `https://api.hyperliquid.xyz/exchange` (or testnet equivalent)
5. On success: calls `info.extra_agents(wallet_address)` to verify agent appears
6. Updates account: `is_active = true`, `permissions = { "agent_approved": true }`
7. Returns success response

### Hyperliquid API Payload Format

```json
{
  "action": {
    "type": "approveAgent",
    "hyperliquidChain": "Mainnet",
    "agentAddress": "0x...",
    "agentName": "",
    "nonce": 1710600000000
  },
  "nonce": 1710600000000,
  "signature": {
    "r": "0x...",
    "s": "0x...",
    "v": 27
  }
}
```

### Files

- **Create:** `crates/router/src/services/hyperliquid/agent_approval.rs` — `build_eip712_typed_data()`, `submit_approval()`, `verify_registration()`
- **Modify:** `crates/router/src/routes/exchanges.rs` — `approve_data`, `approve_agent` handlers + route registration
- **Modify:** `crates/router/src/types/exchanges.rs` — request/response types
- **Modify:** `crates/router/src/services/hyperliquid/mod.rs` — register `agent_approval` module

### Dependencies Added

- None new — `reqwest` already available for HTTP POST to Hyperliquid API, `serde_json` for EIP-712 construction

---

## Acceptance Criteria

- [x] `build_eip712_typed_data()` output matches SDK's ApproveAgent encoding (verified by unit test comparing known-good output)
- [x] `POST /approve-data` returns valid EIP-712 JSON for a pending agent-wallet account
- [x] `POST /approve-data` returns 400 for non-agent-wallet accounts
- [x] `POST /approve-data` returns 400 for already-approved accounts
- [x] `POST /approve` submits correctly formatted payload to Hyperliquid API
- [x] `POST /approve` verifies agent registration after successful submission
- [x] Account updated to `is_active = true` after successful approval
- [x] EIP-712 domain chainId is `421614` regardless of mainnet/testnet
- [x] Signature parsing handles both `0x`-prefixed and bare hex
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **EIP-712 encoding mismatch** — if our typed data doesn't exactly match the SDK's encoding, signatures will be rejected. Mitigation: unit test comparing byte-for-byte output against SDK's known encoding. Reference: `hyperliquid-sdk-rs/src/providers/exchange/mod.rs` approve_agent implementation.
2. **Nonce replay** — Hyperliquid may reject reused nonces. Mitigation: nonce is millisecond timestamp, generated fresh per request. Frontend must use `approve-data` → sign → `approve` within reasonable window.
3. **Network mismatch** — `hyperliquidChain` field must match the network the backend is configured for. Mitigation: read from `Network` enum (same config used by `HyperliquidExchangeApi`).

---

## Completion Signal

This spec is complete when:
1. EIP-712 typed data matches SDK encoding (unit test verified)
2. Full approve-data → sign → approve flow works with testnet wallet
3. Agent registration verified via `extra_agents()` query
4. All acceptance criteria met
5. `cargo clippy --all-targets && cargo test` passes
6. Code committed to master
