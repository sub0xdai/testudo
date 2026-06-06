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

// AUTH-03: Permission, AuthMethod, and AgentKeyClaims now live in the policy module.
pub use crate::policy::{AgentKeyClaims, AuthMethod, Permission};

/// Default permission set for trading agents.
/// Sufficient for the autonomous trading loop: signal + journal read/write.
pub fn default_agent_permissions() -> Vec<Permission> {
    crate::policy::default_permissions()
}

// ── Request types ─────────────────────────────────────────────────────

/// Request to create a new agent API key.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateAgentKeyRequest {
    #[validate(length(min = 1, max = 128))]
    pub name: String,

    #[serde(default = "default_agent_permissions")]
    pub permissions: Vec<Permission>,

    /// Days until expiry. None = never expires (until revoked). Max 365.
    #[validate(range(min = 1, max = 365))]
    pub expires_in_days: Option<i32>,
}

/// Request to update an agent key (partial update).
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAgentKeyRequest {
    #[validate(length(min = 1, max = 128))]
    pub name: Option<String>,
    pub permissions: Option<Vec<Permission>>,
}

// ── Response types ─────────────────────────────────────────────────────

/// Returned at creation time — includes the raw key. Only shown once.
#[derive(Debug, Serialize)]
pub struct CreateAgentKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub key: String,
    pub permissions: Vec<Permission>,
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
    pub permissions: Vec<Permission>,
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
