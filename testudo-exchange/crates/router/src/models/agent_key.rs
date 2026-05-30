//! Agent API key types — scoped credentials for autonomous trading agents.
//!
//! Keys are SHA-256 hashed at rest, irrecoverable after creation, independently
//! revocable. Each key carries a permission set that limits what the agent can do.

// @anchor exchange:router:agent-key-types
// @tags api

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Permission scopes for agent API keys.
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
    /// Read account data (GET /auth/me, GET /exchanges/accounts, GET /onboarding/status)
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

/// How this request was authenticated.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Full-access SIWE/SIWS bearer token.
    Siwe,
    /// Scoped agent API key.
    AgentKey {
        key_id: Uuid,
        permissions: Vec<AgentPermission>,
    },
}

/// Claims extracted from an agent key, stored in request extensions.
#[derive(Debug, Clone)]
pub struct AgentKeyClaims {
    pub user_id: Uuid,
    pub key_id: Uuid,
    pub permissions: Vec<AgentPermission>,
}

// ── Request types ─────────────────────────────────────────────────────

/// Request to create a new agent API key.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateAgentKeyRequest {
    #[validate(length(min = 1, max = 128))]
    pub name: String,

    #[serde(default = "default_agent_permissions")]
    pub permissions: Vec<AgentPermission>,

    /// Days until expiry. None = never expires (until revoked). Max 365.
    #[validate(range(min = 1, max = 365))]
    pub expires_in_days: Option<i32>,
}

/// Request to update an agent key (partial update).
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAgentKeyRequest {
    #[validate(length(min = 1, max = 128))]
    pub name: Option<String>,
    pub permissions: Option<Vec<AgentPermission>>,
}

// ── Response types ─────────────────────────────────────────────────────

/// Returned at creation time — includes the raw key. Only shown once.
#[derive(Debug, Serialize)]
pub struct CreateAgentKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub key: String,
    pub permissions: Vec<AgentPermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Returned when listing keys — NO raw key value.
#[derive(Debug, Serialize)]
pub struct AgentKeySummary {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub permissions: Vec<AgentPermission>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_revoked: bool,
}

// ── DB row type ────────────────────────────────────────────────────────

/// Row from the `agent_keys` table.
#[derive(Debug, sqlx::FromRow)]
pub struct AgentKeyRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub permissions: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_revoked: bool,
    pub revoked_at: Option<DateTime<Utc>>,
}
