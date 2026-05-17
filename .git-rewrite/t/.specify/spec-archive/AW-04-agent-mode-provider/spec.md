# Specification: Auth Refactor — Agent-Mode ExchangeProvider

**Spec ID:** AW-04-agent-mode-provider
**Date:** 2026-03-16
**Status:** Complete
**Class:** Refactor / Backend
**Priority:** P0 — trades must flow through agent wallets
**Depends on:** AW-01 (agent-key-generation)
**Series:** AW-01 through AW-05 (Hyperliquid agent wallet authentication)

---

## Problem Statement

`HyperliquidAuth` currently has a single mode: parse a private key, derive an address, verify they match. This works for main-key accounts where `api_key` = main address and `api_secret` = main private key.

With agent wallets, the semantics change:
- The **signer** is the agent key (stored in `api_secret_encrypted`)
- The **query address** for balances/positions/WS is the user's main wallet address (stored in `wallet_address`)
- `ExchangeProvider` must be constructed via `mainnet_agent(signer, agent_address)` instead of `mainnet(signer)`
- The address-mismatch check in `from_credentials()` must be skipped (the signer address is the agent, not the user)

This spec refactors `HyperliquidAuth` to support both modes via an `AuthMode` enum, and updates all downstream consumers to use `query_address()` instead of `auth.address` directly.

---

## User Stories

- **As a trader**, I want my balance and position queries to show my real account state (not the agent's empty account), so that I can make informed trading decisions.
- **As a developer**, I want a single `query_address()` method that returns the correct address regardless of auth mode, so that I don't need conditional logic scattered across the codebase.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `AuthMode` enum: `Direct` (existing) vs `Agent { user_address: Address }` | High | Auth |
| FR-2 | `HyperliquidAuth` gains `auth_mode: AuthMode` field | High | Auth |
| FR-3 | New constructor: `from_agent_credentials(agent_key, wallet_address)` — no address mismatch check | High | Auth |
| FR-4 | `query_address()` method: returns `user_address` for Agent, `signer.address()` for Direct | High | Auth |
| FR-5 | `build_exchange()` dispatches: Direct → `ExchangeProvider::mainnet(signer)`, Agent → `ExchangeProvider::mainnet_agent(signer, agent_addr)` | High | ExchangeApi |
| FR-6 | All info queries (`get_balance`, `get_position`, etc.) use `auth.query_address()` | High | ExchangeApi |
| FR-7 | WS fill subscription uses `query_address()` for `user_address` parameter | High | WsFills |
| FR-8 | `load_auth()` in `HyperliquidExchangeApi` reads `auth_mode` from `DecryptedCredentials` to dispatch constructor | High | ExchangeApi |
| FR-9 | `AuthCache` works transparently with both modes | Medium | Auth |

---

## Technical Implementation

### AuthMode Enum

```rust
pub enum AuthMode {
    /// Direct signing with user's main key (legacy/testing)
    Direct,
    /// Agent wallet: signer is agent key, queries use user's main address
    Agent { user_address: Address },
}
```

### Updated HyperliquidAuth

```rust
pub struct HyperliquidAuth {
    pub signer: PrivateKeySigner,
    pub address: Address,         // Signer's derived address (agent or main)
    pub auth_mode: AuthMode,      // NEW
}

impl HyperliquidAuth {
    // Existing: validates address matches derived
    pub fn from_credentials(api_key: &str, secret: &str) -> Result<Self, AuthError> {
        // ... unchanged, sets auth_mode = AuthMode::Direct
    }

    // NEW: agent mode, no address mismatch check
    pub fn from_agent_credentials(agent_key: &str, wallet_address: &str) -> Result<Self, AuthError> {
        let signer = agent_key.parse::<PrivateKeySigner>()?;
        let agent_address = signer.address();
        let user_address = wallet_address.parse::<Address>()?;
        Ok(Self {
            signer,
            address: agent_address,
            auth_mode: AuthMode::Agent { user_address },
        })
    }

    // NEW: returns the address to use for info queries
    pub fn query_address(&self) -> Address {
        match &self.auth_mode {
            AuthMode::Direct => self.address,
            AuthMode::Agent { user_address } => *user_address,
        }
    }
}
```

### ExchangeApi Method Changes

| Method | Current | Agent Mode |
|--------|---------|------------|
| `build_exchange()` | `ExchangeProvider::mainnet(signer)` | `ExchangeProvider::mainnet_agent(signer, agent_addr)` |
| `get_balance()` | `info.user_state(auth.address)` | `info.user_state(auth.query_address())` |
| `get_position()` | `info.user_state(auth.address)` | `info.user_state(auth.query_address())` |
| `place_order()` | Direct L1 signing | Agent-wrapped L1 signing (SDK handles automatically) |
| `cancel_order()` | Direct L1 signing | Agent-wrapped L1 signing (SDK handles automatically) |
| WS subscribe | `user_address: auth.address` | `user_address: auth.query_address()` |

### load_auth() Dispatch

```rust
fn load_auth(&self, user_id: Uuid, exchange_account_id: Option<Uuid>) -> Result<HyperliquidAuth> {
    let creds = self.account_repo.load_credentials(account_id, user_id).await?;

    match creds.auth_mode.as_str() {
        "agent_wallet" => {
            let wallet_addr = creds.wallet_address
                .ok_or(AuthError::MissingWalletAddress)?;
            HyperliquidAuth::from_agent_credentials(&creds.api_secret, &wallet_addr)
        }
        _ => {
            // Existing path: api_key = address, api_secret = private key
            HyperliquidAuth::from_credentials(&creds.api_key, &creds.api_secret)
        }
    }
}
```

### Files

- **Modify:** `crates/router/src/services/hyperliquid/auth.rs` — `AuthMode` enum, `from_agent_credentials()`, `query_address()`
- **Modify:** `crates/router/src/services/hyperliquid/exchange_api.rs` — `build_exchange()` dispatch, all query methods use `query_address()`
- **Modify:** `crates/router/src/services/hyperliquid/ws_fills.rs` — use `query_address()` for subscription user_address

---

## Acceptance Criteria

- [x] `AuthMode::Direct` preserves all existing behavior unchanged
- [x] `AuthMode::Agent` skips address-mismatch validation
- [x] `from_agent_credentials()` parses agent key and wallet address
- [x] `query_address()` returns user's main address in Agent mode
- [x] `query_address()` returns signer address in Direct mode
- [x] `build_exchange()` uses `mainnet_agent()` for Agent mode
- [x] `get_balance()` queries user's wallet address (not agent address)
- [x] `get_position()` queries user's wallet address (not agent address)
- [x] WS fill subscription uses `query_address()` for user_address
- [x] `load_auth()` dispatches based on `auth_mode` from `DecryptedCredentials`
- [x] All existing Direct-mode tests pass unchanged
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Agent order signing** — the SDK's `ExchangeProvider::mainnet_agent()` handles agent-wrapped signing automatically. Mitigation: verified in SDK source (`exchange/mod.rs` wraps action with agent metadata).
2. **AuthCache invalidation** — changing `auth_mode` on an account won't clear the cached auth. Mitigation: `AW-05` migration path invalidates cache for migrated accounts. New accounts have no cached entry.
3. **Address confusion** — `auth.address` is the agent address, `auth.query_address()` is the user address. Mitigation: grep all usages of `auth.address` and replace with `auth.query_address()` where appropriate. The only place `auth.address` should be used directly is in `build_exchange()`.

---

## Completion Signal

This spec is complete when:
1. `HyperliquidAuth` supports both Direct and Agent modes
2. All info queries use `query_address()` for correct address dispatch
3. `build_exchange()` dispatches to correct `ExchangeProvider` constructor
4. All existing tests pass (Direct mode unchanged)
5. New unit tests verify Agent mode behavior
6. `cargo clippy --all-targets && cargo test` passes
7. Code committed to master
