# Specification: E2E Testing, Migration & Agent Rotation

**Spec ID:** AW-05-e2e-testing-rotation
**Date:** 2026-03-16
**Status:** Complete
**Class:** Testing / Infrastructure
**Priority:** P1 — validates full flow + lifecycle management
**Depends on:** AW-01, AW-02, AW-03, AW-04
**Series:** AW-01 through AW-05 (Hyperliquid agent wallet authentication)

---

## Problem Statement

With AW-01 through AW-04 implementing the agent wallet infrastructure, auth refactor, approval protocol, and frontend flow, this spec validates the entire chain end-to-end and adds lifecycle management: migration of existing direct-key accounts, agent revocation, TTL-based rotation prompts, and a feature flag for gradual rollout.

Agent wallets on Hyperliquid don't expire on-chain (no built-in TTL), but security best practice is periodic key rotation. Since the backend doesn't hold the user's main key, rotation requires user interaction (signing a new approval). This spec implements backend-tracked TTL with user notification for re-approval.

---

## User Stories

- **As a trader**, I want to migrate my existing Hyperliquid account from direct-key to agent-wallet mode, so that I benefit from improved security without re-creating my account.
- **As a trader**, I want to revoke an agent wallet if I suspect compromise, so that unauthorized trading stops immediately.
- **As a trader**, I want to be notified when my agent wallet is approaching rotation, so that I can re-authorize proactively.
- **As a developer**, I want a feature flag to gate agent wallet functionality, so that we can roll out incrementally.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Integration test: init → approve → place order → cancel → verify on testnet | High | Testing |
| FR-2 | Route `POST /api/v1/exchanges/agent-wallet/migrate` converts existing direct-key account to agent-wallet mode | High | Router |
| FR-3 | Route `DELETE /api/v1/exchanges/agent-wallet/:id/revoke` deactivates agent, sets `is_active = false` | High | Router |
| FR-4 | `AgentRotationService` tracks TTL per agent-wallet account | Medium | Service |
| FR-5 | Default TTL: 23 hours (configurable via env var `AGENT_WALLET_TTL_HOURS`) | Medium | Service |
| FR-6 | When TTL approaches (1 hour before expiry), send WebSocket notification to user | Medium | Service |
| FR-7 | Feature flag: `HYPERLIQUID_AGENT_WALLET_ENABLED` env var gates all agent-wallet routes | High | Router |
| FR-8 | `ExchangeAccountResponse` includes `auth_mode` and `wallet_address` fields | Medium | Types |
| FR-9 | Revoked agents have `permissions` updated to `{ "agent_approved": false, "revoked_at": "..." }` | Medium | Repository |
| FR-10 | Migration preserves existing `exchange_account_id` (no broken references) | High | Repository |

---

## Technical Implementation

### Integration Test Suite

```rust
#[tokio::test]
#[ignore] // Requires HL_TESTNET_KEY and HL_WALLET_ADDRESS env vars
async fn test_agent_wallet_full_lifecycle() {
    // 1. Generate agent keypair
    // 2. Approve agent on testnet (requires real wallet signature)
    // 3. Verify agent appears in extra_agents()
    // 4. Place a limit order via agent
    // 5. Verify order appears in open orders
    // 6. Cancel the order
    // 7. Verify order no longer in open orders
}

#[tokio::test]
#[ignore]
async fn test_agent_wallet_query_address_dispatch() {
    // Verify that balance/position queries use wallet_address, not agent_address
}

#[tokio::test]
#[ignore]
async fn test_agent_wallet_revocation() {
    // After revocation, trades should fail with appropriate error
}
```

### Migration Endpoint

```rust
// POST /api/v1/exchanges/agent-wallet/migrate
pub struct MigrateToAgentWalletRequest {
    pub account_id: Uuid,
    pub wallet_address: String, // User's main wallet address
}

pub struct MigrateToAgentWalletResponse {
    pub account_id: Uuid,
    pub agent_address: String,
    pub message: String, // "Agent keypair generated. Please approve via wallet."
}
```

The handler:
1. Loads existing account (ownership-verified)
2. Verifies `auth_mode == "api_key"` and `exchange_name == "hyperliquid"`
3. Generates new agent keypair
4. Updates account: encrypts agent key (replaces old secret), sets `auth_mode = "agent_wallet"`, stores `wallet_address`
5. Sets `is_active = false` (requires re-approval via AW-02 flow)
6. Invalidates `AuthCache` entry
7. Returns agent address for frontend to start approval flow

### Revocation Endpoint

```rust
// DELETE /api/v1/exchanges/agent-wallet/:id/revoke
pub struct RevokeAgentResponse {
    pub success: bool,
    pub message: String,
}
```

The handler:
1. Loads account (ownership-verified)
2. Verifies `auth_mode == "agent_wallet"`
3. Sets `is_active = false`
4. Updates `permissions = { "agent_approved": false, "revoked_at": "<timestamp>" }`
5. Invalidates `AuthCache` entry
6. Returns success

### Agent Rotation Service

```rust
pub struct AgentRotationService {
    account_repo: ExchangeAccountRepository,
    ws_sender: mpsc::Sender<WsNotification>,
    ttl_hours: u64, // Default 23, from AGENT_WALLET_TTL_HOURS
}

impl AgentRotationService {
    /// Check all active agent-wallet accounts, notify users approaching TTL
    pub async fn check_rotation_needed(&self) -> Result<Vec<Uuid>> {
        // Query accounts where:
        //   auth_mode = 'agent_wallet'
        //   is_active = true
        //   created_at OR last rotation timestamp > (now - ttl_hours + 1 hour buffer)
        // For each: send WS notification to user
    }
}
```

Rotation notification via WebSocket:
```json
{
  "type": "agent_rotation_needed",
  "account_id": "uuid",
  "agent_address": "0x...",
  "expires_at": "2026-03-17T12:00:00Z",
  "message": "Your agent wallet authorization expires soon. Please re-approve."
}
```

### Feature Flag

```rust
// In route registration (exchanges.rs scope)
if std::env::var("HYPERLIQUID_AGENT_WALLET_ENABLED").unwrap_or_default() == "true" {
    cfg.service(
        web::scope("/agent-wallet")
            .route("/init", web::post().to(init_agent_wallet))
            .route("/approve-data", web::post().to(approve_data))
            .route("/approve", web::post().to(approve_agent))
            .route("/migrate", web::post().to(migrate_to_agent_wallet))
            .route("/{id}/revoke", web::delete().to(revoke_agent))
    );
}
```

### Updated Response Type

```rust
pub struct ExchangeAccountResponse {
    pub id: Uuid,
    pub exchange_name: String,
    pub account_name: String,
    pub is_active: bool,
    pub permissions: Value,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub auth_mode: String,              // NEW: "api_key" or "agent_wallet"
    pub wallet_address: Option<String>, // NEW: truncated for display
}
```

### Files

- **Create:** `crates/router/src/services/hyperliquid/tests/agent_wallet_integration.rs` — E2E integration tests
- **Create:** `crates/router/src/services/hyperliquid/agent_rotation.rs` — TTL tracking, rotation notifications
- **Modify:** `crates/router/src/routes/exchanges.rs` — `migrate_to_agent_wallet`, `revoke_agent` handlers, feature flag scope
- **Modify:** `crates/router/src/types/exchanges.rs` — `auth_mode` + `wallet_address` in `ExchangeAccountResponse`, migrate/revoke types
- **Modify:** `crates/router/src/services/hyperliquid/mod.rs` — register `agent_rotation` module
- **Modify:** `testudo-web/src/pages/AccountPage.tsx` — revoke button, migration prompt for existing HL accounts

---

## Acceptance Criteria

- [x] Integration test covers full init → approve → trade → cancel lifecycle (ignored, requires testnet)
- [x] Migration endpoint converts direct-key account to agent-wallet mode
- [x] Migration preserves `exchange_account_id` (no broken foreign keys)
- [x] Migrated account requires re-approval (is_active = false after migration)
- [x] Revocation endpoint deactivates agent and records revocation timestamp
- [x] Revoked account cannot place trades (error returned)
- [x] `AgentRotationService` detects accounts approaching TTL
- [x] WebSocket notification sent 1 hour before TTL expiry
- [x] Feature flag `HYPERLIQUID_AGENT_WALLET_ENABLED` gates all agent-wallet routes
- [x] `ExchangeAccountResponse` includes `auth_mode` and `wallet_address`
- [x] Frontend shows revoke button for agent-wallet accounts
- [x] `cargo clippy --all-targets && cargo test` passes
- [x] `cd testudo-web && bun run build` passes

---

## Risks

1. **Migration data loss** — replacing `api_secret_encrypted` destroys the old main key. Mitigation: the migration flow should warn the user that the old key will be removed. Since the user has the key in their wallet, this is acceptable. Consider storing old key hash for audit.
2. **Rotation requires user interaction** — since we don't hold the main key, automatic re-approval is impossible. Mitigation: rotation is defense-in-depth, not mandatory. Notification-based approach is the best we can do. Agent keys don't expire on-chain.
3. **Feature flag granularity** — flag is all-or-nothing. Mitigation: acceptable for initial rollout. Per-user flags can be added later if needed.
4. **Integration test flakiness** — testnet may be slow or unavailable. Mitigation: tests are `#[ignore]` by default, only run manually or in CI with testnet credentials.

---

## Completion Signal

This spec is complete when:
1. Integration tests cover full agent wallet lifecycle
2. Migration and revocation endpoints functional
3. Agent rotation service tracks TTL and sends notifications
4. Feature flag gates all agent-wallet routes
5. `ExchangeAccountResponse` includes auth_mode and wallet_address
6. All acceptance criteria met
7. `cargo clippy --all-targets && cargo test` passes
8. `cd testudo-web && bun run build` passes
9. Code committed to master
