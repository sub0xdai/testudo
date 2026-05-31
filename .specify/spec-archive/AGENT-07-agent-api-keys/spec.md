# Specification: Scoped Agent API Keys — Decoupled Agent Authentication

**Spec ID:** AGENT-07-agent-api-keys
**Date:** 2026-05-30
**Status:** Draft
**Class:** Feature / Auth
**Priority:** P1 — decouples agent auth from user SIWE; enables multi-agent, per-agent audit, revocable credentials
**Depends on:** AGENT-06-onboarding-status
**Series:** AGENT-06 through AGENT-07 (Agent Onboarding UX)

---

## Problem Statement

Today, every AI agent trading on Testudo authenticates via the same SIWE (Sign-In With Ethereum) bearer token as the human user. This means:

1. **Agent identity is fused with user identity.** The agent has the same permissions as the user — if the agent's token is compromised, the attacker has full account access. There is no blast radius boundary.

2. **No per-agent audit trail.** All `source` fields in `SignalInput` are self-reported strings. There's no cryptographic guarantee that `"agent:hermes_v1.2"` actually came from Hermes — any agent can claim any source.

3. **No scoped permissions.** An agent that only needs to submit signals and read journal has exactly the same access as one that needs to manage exchange accounts, risk config, and withdrawals. Claude Code, Cursor, and Copilot all use scoped OAuth tokens with explicit permission sets. Testudo has no equivalent.

4. **Token lifecycle is tied to user sessions.** SIWE tokens expire in 1 hour. Agents running autonomous loops must handle token refresh, which complicates agent runtimes and creates failure modes.

5. **Multi-agent management is impossible.** A user running two agents (e.g., a momentum breakout agent and a mean-reversion agent) cannot independently revoke one without killing both. They cannot set different risk limits per agent. They cannot track P&L per agent except through manual `source` filtering.

The solution: scoped agent API keys (`tudo_sk_...`) that are separate from user SIWE tokens, carry explicit permissions, have their own expiry, are independently revocable, and have built-in audit identity. This pattern is battle-tested by Claude Code's `claude login` OAuth flow, Cursor's session tokens, and Copilot's device-code flow.

---

## User Stories

- **As a user**, I want to create an API key for my trading agent with specific permissions (e.g., "can submit signals, cannot change risk config"), so that I can safely delegate trading without giving full account access.
- **As a user**, I want to revoke an agent's API key independently of others, so that I can kill a misbehaving agent without disrupting my other agents.
- **As an AI agent**, I want to authenticate with a single `X-Agent-Key` header instead of managing SIWE token lifecycle, so that my autonomous loop is simpler and more reliable.
- **As a platform operator**, I want per-agent audit trails tied to cryptographic key IDs (not self-reported strings), so that I can trace every trade to a specific agent with certainty.
- **As a user onboarding through an agent**, I want the agent to request a key with auto-generated permissions during onboarding, so that I don't have to navigate a dashboard to set up agent access.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `POST /api/v1/agent-keys` creates a scoped agent API key with `name`, `permissions`, and optional `expires_in_days` | High | Router |
| FR-2 | Key is returned exactly once in the 201 response with format `tudo_sk_<base64url>`. Subsequent GETs return metadata only (key value is irrecoverable — hashed at rest) | High | Router |
| FR-3 | `GET /api/v1/agent-keys` lists all keys for the authenticated user with `id`, `name`, `permissions`, `created_at`, `expires_at`, `last_used_at`, `is_revoked` | High | Router |
| FR-4 | `DELETE /api/v1/agent-keys/:id` revokes a key (soft delete — sets `is_revoked = true`, records `revoked_at`). Revocation is immediate. | High | Router |
| FR-5 | `PATCH /api/v1/agent-keys/:id` updates `name` and/or `permissions` for a non-revoked key. Permissions changes take effect immediately. | Medium | Router |
| FR-6 | Auth middleware accepts `X-Agent-Key: tudo_sk_...` header and resolves to `AuthenticatedAgent { user_id, key_id, permissions }` | High | Middleware |
| FR-7 | Permissions are enforced per-endpoint: signal submission checks `trade:execute`, journal reads check `journal:read`, journal writes check `journal:write`, exchange management checks `exchange:manage`, risk config checks `risk:configure` | High | Middleware |
| FR-8 | Every trade/journalevent/action executed with an agent key records `agent_key_id` alongside the existing `source` field | Medium | Router |
| FR-9 | Key expiry: if `expires_in_days` is set, key auto-expires and returns 401. Default: no expiry (until revoked). Max: 365 days. | Medium | Router |
| FR-10 | Keys are hashed with SHA-256 at rest. Raw key is returned only at creation time. | High | sqlx_postgres |
| FR-11 | Key creation requires authentication (SIWE bearer token). Key usage does NOT require SIWE — the key itself is the credential. | High | Router |
| FR-12 | Rate limiting: agent keys share the same per-user rate limits as SIWE tokens (30 signals/min, standard limits for reads) | Low | Middleware |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Types + DB migration + `POST /agent-keys` with key generation and hashed storage | 201 with key, key irrecoverable on GET, hashed at rest |
| CP-2 | `GET /agent-keys` + `DELETE /agent-keys/:id` + `PATCH /agent-keys/:id` | Full CRUD lifecycle, revocation immediate |
| CP-3 | `X-Agent-Key` auth middleware — resolve to `AuthenticatedAgent`, enforce permissions | 401 on revoked/expired, 403 on insufficient permissions, 200 on valid |
| CP-4 | Wire key_id into journal/trade events. Add per-agent filtering to journal endpoints. | `agent_key_id` in trade_groups, filterable in summary/insights |

### Permission Model

```rust
// crates/router/src/models/agent_key.rs — NEW

/// Permission scopes for agent API keys.
/// Each scope maps to one or more endpoint groups.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentPermission {
    /// Submit trade signals (POST /api/v1/signals)
    TradeExecute,
    /// Read journal data (GET /journal/agent/*)
    JournalRead,
    /// Write journal entries (POST /journal/entries, /journal/tags, etc.)
    JournalWrite,
    /// Manage exchange accounts (POST /exchanges/accounts)
    ExchangeManage,
    /// Configure risk settings (PUT /risk-config)
    RiskConfigure,
    /// Read account data (GET /auth/me, GET /exchanges/accounts)
    AccountRead,
}

/// Default permission set for trading agents.
/// Sufficient for the autonomous trading loop: signal + journal read/write.
pub fn default_agent_permissions() -> Vec<AgentPermission> {
    vec![
        AgentPermission::TradeExecute,
        AgentPermission::JournalRead,
        AgentPermission::JournalWrite,
        AgentPermission::AccountRead,
    ]
}
```

### Key Types

```rust
// crates/router/src/models/agent_key.rs — NEW (continued)

/// Request to create a new agent API key.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateAgentKeyRequest {
    /// Human-readable name for this key (e.g. "Momentum Breakout Agent v1")
    #[validate(length(min = 1, max = 128))]
    pub name: String,

    /// Permission set. If omitted, defaults to `default_agent_permissions()`.
    #[serde(default = "default_agent_permissions")]
    pub permissions: Vec<AgentPermission>,

    /// Days until expiry. If None, key never expires (until revoked).
    /// Max: 365. Default: None.
    #[validate(range(min = 1, max = 365))]
    pub expires_in_days: Option<i32>,
}

/// Metadata for a created agent key (stored in DB).
/// The raw key value is NOT stored — only its SHA-256 hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub key_hash: String,        // SHA-256 of raw key
    pub key_prefix: String,      // First 8 chars for UI display ("tudo_sk_")
    pub permissions: Vec<AgentPermission>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_revoked: bool,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Returned to the user at creation time (includes raw key).
#[derive(Debug, Serialize)]
pub struct CreateAgentKeyResponse {
    pub id: Uuid,
    pub name: String,
    /// The raw API key. Only returned once. Format: "tudo_sk_<base64url>"
    pub key: String,
    pub permissions: Vec<AgentPermission>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Returned when listing keys (no raw key value — irrecoverable).
#[derive(Debug, Serialize)]
pub struct AgentKeySummary {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,    // "tudo_sk_a1b2c3d4..."
    pub permissions: Vec<AgentPermission>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_revoked: bool,
}

/// Request to update an agent key.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAgentKeyRequest {
    #[validate(length(min = 1, max = 128))]
    pub name: Option<String>,
    pub permissions: Option<Vec<AgentPermission>>,
}

/// Extracted from X-Agent-Key header by auth middleware.
#[derive(Debug, Clone)]
pub struct AuthenticatedAgent {
    pub user_id: Uuid,
    pub key_id: Uuid,
    pub permissions: Vec<AgentPermission>,
}
```

### Key Generation

```rust
// crates/router/src/services/agent_key.rs — NEW

use ring::rand::{SecureRandom, SystemRandom};
use base64ct::{Base64UrlUnpadded, Encoding};

/// Generate a new agent API key.
/// Format: "tudo_sk_<32 random bytes as base64url unpadded>"
/// Returns (raw_key_for_user, sha256_hash_for_db).
pub fn generate_agent_key() -> (String, String) {
    let rng = SystemRandom::new();
    let mut key_bytes = [0u8; 32];
    rng.fill(&mut key_bytes).expect("CSPRNG failure");

    let raw_key = format!("tudo_sk_{}", Base64UrlUnpadded::encode_string(&key_bytes));
    let hash = sha256_hash(&raw_key);

    (raw_key, hash)
}

fn sha256_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Resolve an X-Agent-Key header to an (optional) AuthenticatedAgent.
/// Returns None if the key is invalid, revoked, or expired.
pub async fn resolve_agent_key(
    db: &PgPool,
    key_header: &str,
) -> Result<Option<AuthenticatedAgent>, AppError> {
    let key = key_header.trim();

    // Fast reject: must start with "tudo_sk_"
    if !key.starts_with("tudo_sk_") {
        return Ok(None);
    }

    let hash = sha256_hash(key);

    // Look up by hash
    let row = sqlx::query_as!(
        AgentKeyRow,
        r#"SELECT id, user_id, permissions, is_revoked, expires_at
           FROM agent_keys
           WHERE key_hash = $1"#,
        hash
    )
    .fetch_optional(db)
    .await?;

    match row {
        None => Ok(None),
        Some(r) if r.is_revoked => Ok(None),
        Some(r) if r.expires_at.map_or(false, |exp| exp < Utc::now()) => Ok(None),
        Some(r) => {
            // Update last_used_at asynchronously
            tokio::spawn(update_last_used(db.clone(), r.id));

            Ok(Some(AuthenticatedAgent {
                user_id: r.user_id,
                key_id: r.id,
                permissions: serde_json::from_value(r.permissions)?,
            }))
        }
    }
}
```

### Auth Middleware Extension

```rust
// crates/router/src/middleware/auth.rs — modify

/// Unified auth extractor. Accepts either:
/// 1. Authorization: Bearer <siwe_token>  (traditional SIWE)
/// 2. X-Agent-Key: tudo_sk_...            (scoped agent key)
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub auth_method: AuthMethod,
}

pub enum AuthMethod {
    Siwe { session_id: Uuid },
    AgentKey { key_id: Uuid, permissions: Vec<AgentPermission> },
}

impl AuthenticatedUser {
    /// Check if this authenticated principal has a specific permission.
    /// SIWE-authenticated users have all permissions (full access).
    pub fn has_permission(&self, perm: &AgentPermission) -> bool {
        match &self.auth_method {
            AuthMethod::Siwe { .. } => true,
            AuthMethod::AgentKey { permissions, .. } => permissions.contains(perm),
        }
    }
}
```

### Permission Enforcement

```rust
// Macro or helper for route handlers

macro_rules! require_permission {
    ($user:expr, $perm:expr) => {
        if !$user.has_permission(&$perm) {
            return Err(AppError::Forbidden(format!(
                "Agent key lacks permission: {:?}",
                $perm
            )));
        }
    };
}

// Usage in signal handler:
// require_permission!(user, AgentPermission::TradeExecute);
```

### Database Changes

```sql
-- Migration: 20260530_agent_keys.sql

CREATE TABLE agent_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(128) NOT NULL,
    key_hash VARCHAR(64) NOT NULL UNIQUE,       -- SHA-256 hex
    key_prefix VARCHAR(12) NOT NULL,             -- "tudo_sk_"
    permissions JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    is_revoked BOOLEAN NOT NULL DEFAULT false,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX idx_agent_keys_user ON agent_keys(user_id);
CREATE INDEX idx_agent_keys_hash ON agent_keys(key_hash);

-- Add agent_key_id to trade groups for audit trail
ALTER TABLE trade_groups ADD COLUMN agent_key_id UUID REFERENCES agent_keys(id);
CREATE INDEX idx_trade_groups_agent_key ON trade_groups(agent_key_id);

-- Add agent_key_id to journal entries
ALTER TABLE journal_entries ADD COLUMN agent_key_id UUID REFERENCES agent_keys(id);
CREATE INDEX idx_journal_entries_agent_key ON journal_entries(agent_key_id);
```

### Route Wiring

```rust
// crates/router/src/routes/mod.rs — add:
cfg.service(
    web::scope("/api/v1/agent-keys")
        .route("", web::post().to(agent_keys::create_key))
        .route("", web::get().to(agent_keys::list_keys))
        .route("/{key_id}", web::delete().to(agent_keys::revoke_key))
        .route("/{key_id}", web::patch().to(agent_keys::update_key))
);
```

### Paved Roads

- `ExchangeAccountRepository` in `sqlx_postgres/src/repositories/api_keys.rs` — existing pattern for credential storage with encryption at rest. Agent keys follow the same pattern (hash at rest, irrecoverable after creation).
- `AuthenticatedUser` extractor in `middleware/auth.rs` — extend to accept both SIWE tokens and agent keys. Same middleware, dual auth methods.
- `signal.rs` route handler — already accepts `source: Option<String>`. Add `agent_key_id` from auth context automatically, keeping `source` as the human-readable label.
- `agent_journal.rs` summary/insights/compare routes — add optional `agent_key_id` filter parameter.
- `ring` crate — already in dependency tree for cryptographic operations. Use for CSPRNG key generation.
- `sha2` crate — likely already transitively available. If not, add explicitly.

### Files

- `crates/router/src/routes/agent_keys.rs` — **NEW** — CRUD route handlers
- `crates/router/src/models/agent_key.rs` — **NEW** — all key types, permission enum, request/response structs
- `crates/router/src/services/agent_key.rs` — **NEW** — key generation, hashing, resolution
- `crates/router/src/middleware/auth.rs` — **MODIFY** — extend `AuthenticatedUser` to support `AuthMethod::AgentKey`, add `X-Agent-Key` header extraction
- `crates/router/src/routes/mod.rs` — add route registration
- `crates/router/src/routes/signal.rs` — **MODIFY** — record `agent_key_id` in trade_groups, add permission check
- `crates/router/src/services/agent_signal.rs` — **MODIFY** — pass `agent_key_id` through to journal service
- `crates/common_utils/src/types/order.rs` — **MODIFY** — add `agent_key_id: Option<Uuid>` to trade group struct
- `crates/db-processor/src/migrations/` — **NEW** — `20260530_agent_keys.sql` migration
- `AGENT_TRADING.md` — add Section 0 subsection: "Creating an Agent API Key" alongside existing SIWE path

### Dependencies Added

```toml
# crates/router/Cargo.toml — verify, likely already present:
sha2 = "0.10"  # SHA-256 for key hashing
ring = "0.17"  # CSPRNG for key generation (likely already in dep tree)
```

---

## Acceptance Criteria

- [ ] `POST /api/v1/agent-keys {"name":"test","permissions":["trade_execute","journal_read"]}` returns 201 with `{id, key: "tudo_sk_...", permissions, ...}`
- [ ] Same key value never returned again — subsequent `GET /agent-keys` shows `key_prefix` only
- [ ] `DELETE /api/v1/agent-keys/:id` returns 200, key immediately returns 401 on next use
- [ ] `GET /api/v1/agent-keys` returns all keys for user (without raw key values)
- [ ] `PATCH /api/v1/agent-keys/:id {"permissions": ["trade_execute"]}` updates permissions immediately
- [ ] Signal submission with `X-Agent-Key: tudo_sk_...` (valid, has `trade_execute`) succeeds
- [ ] Signal submission with valid key lacking `trade_execute` returns 403 Forbidden
- [ ] Signal submission with revoked key returns 401 Unauthorized
- [ ] Signal submission with expired key returns 401 Unauthorized
- [ ] `agent_key_id` populated in `trade_groups` when signal submitted via agent key
- [ ] `agent_key_id` is `NULL` in `trade_groups` when signal submitted via SIWE token
- [ ] Journal summary endpoint filterable by `agent_key_id`
- [ ] SIWE-authenticated requests continue to work unchanged (backward compatible)
- [ ] `cargo clippy --all-targets && cargo test` passes in testudo-exchange
- [ ] Unit tests cover: key creation + immediate revocation + permission enforcement + expiry

---

## Risks

1. **Key leakage via logs** — The raw key only appears in the 201 response and is hashed thereafter. Mitigation: mark `key` field with `#[serde(skip_serializing_if = "...")]` on GET paths, and add a `sensitive` attribute to tracing spans for the create handler. Log the key_id (UUID), never the raw key.
2. **Broad `JournalWrite` scope** — `JournalWrite` covers journal entries, tags, and trade notes. A malicious agent could spam the journal with garbage. Mitigation: rate limiting (standard per-user limits apply to agent keys too). Future: per-key write quotas.
3. **No granular signal permissions** — `TradeExecute` gives full signal submission ability (any symbol, any size). Mitigation: the risk engine still enforces per-user limits. Future: per-key symbol allowlists and max position notional.
4. **Migration rollback** — If the migration fails in production, agent key auth is unavailable but SIWE continues to work (auth middleware falls back gracefully). Mitigation: deploy migration separately from code change. Code checks for table existence before attempting agent key resolution.
5. **Dependency on `ring`** — `ring` requires a C toolchain on some platforms. Mitigation: verify `ring` is already in the dependency tree (likely via `rustls`). If not, use `rand::rngs::OsRng` + `sha2` directly.

---

## Completion Signal

This spec is complete when:
1. `POST /api/v1/agent-keys` creates irreversible, hashed-at-rest keys with scoped permissions
2. Full CRUD lifecycle (create, list, update, revoke) implemented
3. `X-Agent-Key` auth middleware resolves to `AuthenticatedAgent` with enforced permissions
4. `agent_key_id` recorded in trade_groups and journal_entries for audit trail
5. SIWE auth path continues to work (backward compatible)
6. All 15 acceptance criteria met
7. `cargo clippy --all-targets && cargo test` passes
8. `AGENT_TRADING.md` updated with agent key setup instructions
9. Code committed to master
