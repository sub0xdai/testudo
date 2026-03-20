# Specification: Reuse Existing Agent Wallet Instead of Regenerating

**Spec ID:** AW-06-agent-wallet-reuse
**Date:** 2026-03-20
**Status:** Draft
**Class:** Feature / Auth
**Priority:** P0 — Every wallet authorize click orphans the previous approved agent, breaking trading
**Depends on:** AW-01 through AW-05 (agent wallet lifecycle)
**Series:** AW-06 (standalone fix)

---

## Problem Statement

Every time the user clicks "AUTHORIZE AGENT WALLET" in testudo-web, `init_agent_wallet` generates a brand new random keypair (`PrivateKeySigner::random()`) and inserts a new `exchange_accounts` row. The previous approved agent wallet becomes orphaned — Hyperliquid still recognizes the old key, but the backend is now using the new (unapproved) one.

The user must then complete the full EIP-712 approval flow again: MetaMask connect → sign typed data → submit to Hyperliquid. This needs to happen after every accidental re-click, page refresh that re-triggers the flow, or any UI interaction that calls `initAgentWallet`.

Root cause: `init_agent_wallet()` in `routes/exchanges.rs:920` has no deduplication check. It always generates a new keypair and inserts a new record. The `exchange_accounts` table has no unique constraint on `(user_id, wallet_address, auth_mode)` either — allowing unlimited duplicate rows.

Evidence: Agent address changed from `0x2d20b0a...ee1c` to `0x3014bde...1b76` between sessions — each is a separate DB row.

---

## User Stories

- **As a trader**, I want the authorize flow to reuse my existing approved agent wallet, so that I don't have to re-sign with MetaMask every time.
- **As a trader**, I want to be able to resume an incomplete approval flow without generating a new agent key.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `init_agent_wallet` checks for existing agent wallet by `(user_id, wallet_address)` before generating a new one | High | Router |
| FR-2 | If an active (approved) agent wallet exists, return its `account_id` and `agent_address` without generating a new key | High | Router |
| FR-3 | If a pending (unapproved) agent wallet exists, return it so the user can complete the approval flow | High | Router |
| FR-4 | Only generate a new keypair when no agent wallet exists for the user+wallet combo | High | Router |
| FR-5 | Add partial unique index on `(user_id, wallet_address)` where `auth_mode = 'agent_wallet' AND is_active = true` | Medium | Database |

---

## Technical Implementation

### 1. New Repository Method

**File:** `crates/router/src/repositories/exchange_account.rs`

```rust
/// Find existing agent wallet for a user+wallet_address combo.
/// Returns active wallets first, then pending, most recent first.
pub async fn find_agent_wallet(
    &self,
    user_id: Uuid,
    wallet_address: &str,
) -> Result<Option<ExchangeAccountRow>, RepoError>
```

Query:
```sql
SELECT id, user_id, exchange_name, permissions, is_active, created_at, last_used_at, auth_mode, wallet_address
FROM exchange_accounts
WHERE user_id = $1 AND wallet_address = $2 AND auth_mode = 'agent_wallet'
ORDER BY is_active DESC, created_at DESC
LIMIT 1
```

### 2. Updated `init_agent_wallet` Flow

**File:** `crates/router/src/routes/exchanges.rs` (~line 905)

```
1. Validate wallet address format (existing)
2. NEW: Check repo.find_agent_wallet(user_id, addr)
   a. If Some(existing) AND is_active → decrypt api_key_encrypted to get agent_address, return existing
   b. If Some(existing) AND !is_active → decrypt api_key_encrypted, return existing (resume approval)
   c. If None → generate new keypair and insert (existing behavior)
3. Return InitAgentWalletResponse { account_id, agent_address }
```

The agent address is stored in `api_key_encrypted` (encrypted via Vault). To return it, call `vault.decrypt()` on the credential row. Can reuse `load_credentials_for_approval()` which already decrypts for pending accounts — the `api_key` field contains the agent address.

### 3. Migration: Partial Unique Index

**File:** `crates/sqlx_postgres/migrations/{next_timestamp}_agent_wallet_unique.sql`

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_unique_active_agent_wallet
ON exchange_accounts(user_id, wallet_address)
WHERE auth_mode = 'agent_wallet' AND is_active = true;
```

### Files

- `crates/router/src/repositories/exchange_account.rs` — add `find_agent_wallet()`
- `crates/router/src/routes/exchanges.rs` — update `init_agent_wallet()`
- `crates/sqlx_postgres/migrations/` — new migration for unique index

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Clicking "AUTHORIZE AGENT WALLET" with an existing active agent wallet returns the same account_id and agent_address without generating a new key
- [ ] Clicking "AUTHORIZE AGENT WALLET" with a pending (unapproved) wallet returns it for the user to complete approval
- [ ] Only when no agent wallet exists is a new keypair generated
- [ ] Partial unique index prevents multiple active agent wallets per user+wallet at the DB level
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Existing duplicate rows** — The DB may have multiple agent wallet rows for the same user+wallet. Mitigation: The `find_agent_wallet` query uses `ORDER BY is_active DESC, created_at DESC LIMIT 1` to always pick the best one. Clean up old rows before adding the unique index.
2. **Decryption failure** — If the Vault key changed, existing encrypted agent addresses can't be decrypted. Mitigation: Fall through to generating a new keypair on decryption error.

---

## Completion Signal

This spec is complete when:
1. `init_agent_wallet` reuses existing agent wallets instead of generating new ones
2. All acceptance criteria met
3. `cargo clippy --all-targets && cargo test` passes
4. Code committed to master
