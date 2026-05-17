# Specification: Agent Key Generation & Backend Schema

**Spec ID:** AW-01-agent-key-generation
**Date:** 2026-03-16
**Status:** Complete
**Class:** Infrastructure / Backend
**Priority:** P0 — foundation for all subsequent AW specs
**Depends on:** None (first in series)
**Series:** AW-01 through AW-05 (Hyperliquid agent wallet authentication)

---

## Problem Statement

The current Hyperliquid credential flow (HL-02) stores the user's main ETH private key in `api_secret_encrypted`. If the database encryption key (`CREDENTIAL_ENCRYPTION_KEY`) is compromised, an attacker gains full wallet access including withdrawals — catastrophic risk for a trading platform.

Hyperliquid supports **agent wallets** — delegated keypairs that can only trade, never withdraw. The SDK provides `ExchangeProvider::mainnet_agent()` for this exact pattern. But the current schema has no way to distinguish between a main-key account and an agent-wallet account, and no field to store the user's public wallet address separately from the signer address.

This spec adds the schema foundation: new DB columns, an agent keypair generation endpoint, and updated credential structs. All changes are backwards-compatible — existing CEX accounts are unaffected.

---

## User Stories

- **As a trader**, I want to connect my Hyperliquid account without surrendering my main private key, so that my funds remain safe even if the backend is compromised.
- **As a developer**, I want a clear `auth_mode` field distinguishing CEX API-key accounts from agent-wallet accounts, so that downstream code can dispatch correctly.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | DB migration adds `auth_mode VARCHAR(20) DEFAULT 'api_key'` column to `exchange_accounts` | High | Database |
| FR-2 | DB migration adds `wallet_address VARCHAR(42)` column to `exchange_accounts` | High | Database |
| FR-3 | `DecryptedCredentials` gains `auth_mode: String` and `wallet_address: Option<String>` fields | High | Repository |
| FR-4 | `ExchangeAccountRow` gains `auth_mode: String` and `wallet_address: Option<String>` fields | High | Repository |
| FR-5 | New `insert_agent_wallet(user_id, wallet_address, agent_key, agent_address)` repository method | High | Repository |
| FR-6 | Route `POST /api/v1/exchanges/agent-wallet/init` generates agent keypair, encrypts agent key, stores wallet address, returns `{ agent_address, account_id }` | High | Router |
| FR-7 | Existing CEX account insertion (`insert()`) unaffected — `auth_mode` defaults to `'api_key'`, `wallet_address` remains NULL | High | Repository |
| FR-8 | Agent keypair generation uses `alloy::signers::local::PrivateKeySigner::random()` | Medium | Router |

---

## Technical Implementation

### DB Migration

```sql
-- Up migration
ALTER TABLE exchange_accounts
  ADD COLUMN auth_mode VARCHAR(20) NOT NULL DEFAULT 'api_key',
  ADD COLUMN wallet_address VARCHAR(42);

-- Constraint: auth_mode must be 'api_key' or 'agent_wallet'
ALTER TABLE exchange_accounts
  ADD CONSTRAINT check_auth_mode
  CHECK (auth_mode IN ('api_key', 'agent_wallet'));

-- Constraint: agent_wallet mode requires wallet_address
ALTER TABLE exchange_accounts
  ADD CONSTRAINT check_agent_wallet_has_address
  CHECK (auth_mode != 'agent_wallet' OR wallet_address IS NOT NULL);
```

### Credential Mapping (Agent Wallet Mode)

| DB Column | Stores | Notes |
|-----------|--------|-------|
| `api_key_encrypted` | Agent ETH address | Derived from agent key (for reference/display) |
| `api_secret_encrypted` | Agent private key | Generated server-side, trade-only permissions |
| `wallet_address` | User's main wallet address | Public, used for info queries + WS subscriptions |
| `auth_mode` | `'agent_wallet'` | Distinguishes from CEX `'api_key'` mode |
| `exchange_name` | `'hyperliquid'` | Unchanged |

### Updated Structs

```rust
pub struct DecryptedCredentials {
    pub exchange_name: String,
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: Option<String>,
    pub auth_mode: String,              // NEW: "api_key" or "agent_wallet"
    pub wallet_address: Option<String>, // NEW: user's main ETH address
}

pub struct ExchangeAccountRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub exchange_name: String,
    pub permissions: Option<serde_json::Value>,
    pub is_active: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub auth_mode: String,              // NEW
    pub wallet_address: Option<String>, // NEW
}
```

### Init Endpoint

```rust
// POST /api/v1/exchanges/agent-wallet/init
pub struct InitAgentWalletRequest {
    pub wallet_address: String, // User's main wallet address (0x-prefixed, 42 chars)
}

pub struct InitAgentWalletResponse {
    pub account_id: Uuid,
    pub agent_address: String, // Agent's derived ETH address
}
```

The handler:
1. Validates `wallet_address` format (0x + 40 hex chars)
2. Generates agent keypair: `PrivateKeySigner::random()`
3. Derives agent address from keypair
4. Encrypts agent private key via `AesGcmVault`
5. Calls `insert_agent_wallet()` with `auth_mode = "agent_wallet"`
6. Returns `{ account_id, agent_address }`

### Files

- **Create:** `crates/sqlx_postgres/migrations/YYYYMMDD_add_agent_wallet_columns.up.sql`
- **Create:** `crates/sqlx_postgres/migrations/YYYYMMDD_add_agent_wallet_columns.down.sql`
- **Modify:** `crates/router/src/repositories/exchange_account.rs` — `insert_agent_wallet()`, update `DecryptedCredentials`, `ExchangeAccountRow`, `load_credentials()` query
- **Modify:** `crates/router/src/routes/exchanges.rs` — `init_agent_wallet` handler + route registration
- **Modify:** `crates/router/src/types/exchanges.rs` — `InitAgentWalletRequest`, `InitAgentWalletResponse`
- **Modify:** `crates/common_utils/src/models/exchange_account.rs` — add `auth_mode`, `wallet_address` fields

### Dependencies Added

- None new — `alloy` already in workspace for `PrivateKeySigner`

---

## Acceptance Criteria

- [x] Migration adds `auth_mode` and `wallet_address` columns with correct constraints
- [x] Existing accounts default to `auth_mode = 'api_key'` with NULL `wallet_address`
- [x] `DecryptedCredentials` includes `auth_mode` and `wallet_address` fields
- [x] `ExchangeAccountRow` includes `auth_mode` and `wallet_address` fields
- [x] `insert_agent_wallet()` stores encrypted agent key with `auth_mode = 'agent_wallet'`
- [x] `POST /api/v1/exchanges/agent-wallet/init` returns valid `{ account_id, agent_address }`
- [x] Invalid wallet address format returns 400 error
- [x] Existing `insert()` path unchanged — all CEX tests still pass
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Migration on production DB** — ALTER TABLE on `exchange_accounts` while active. Mitigation: columns are nullable/defaulted, no table lock needed for ADD COLUMN with defaults in PostgreSQL 11+.
2. **Agent key entropy** — generated server-side, must use cryptographic RNG. Mitigation: `PrivateKeySigner::random()` uses `OsRng` internally.
3. **Wallet address validation** — must reject malformed addresses before DB insert. Mitigation: regex check `^0x[0-9a-fA-F]{40}$` + alloy `Address::parse_checksummed()` for EIP-55 optional validation.

---

## Completion Signal

This spec is complete when:
1. Migration applied and both up/down verified
2. Init endpoint generates and stores agent keypairs
3. All existing account flows unchanged
4. All acceptance criteria met
5. `cargo clippy --all-targets && cargo test` passes
6. Code committed to master
