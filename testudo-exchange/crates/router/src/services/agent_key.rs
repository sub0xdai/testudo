//! Agent API key generation and resolution.
//!
//! Keys are generated with OsRng CSPRNG, formatted as `tudo_sk_<base64url>`,
//! and SHA-256 hashed before storage. Raw keys are NEVER logged or stored.

// @anchor exchange:router:agent-key-service
// @tags api

use base64::Engine;
use chrono::Utc;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::agent_key::{AgentKeyClaims, AgentKeyRow};

/// Generate a new agent API key.
///
/// Returns `(raw_key_for_user, sha256_hash_for_db)`.
/// Format: `tudo_sk_<32 random bytes as base64url unpadded>`.
/// The hash is hex-encoded SHA-256 of the raw key.
pub fn generate_agent_key() -> (String, String) {
    let mut key_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut key_bytes);

    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key_bytes);
    let raw_key = format!("tudo_sk_{}", encoded);
    let hash = sha256_hash(&raw_key);

    (raw_key, hash)
}

/// Compute the SHA-256 hex digest of a string.
pub fn sha256_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Extract the key prefix for display. "tudo_sk_" + first 8 chars of base64.
pub fn key_prefix_from_raw(raw_key: &str) -> String {
    if raw_key.len() <= 16 {
        raw_key.to_string()
    } else {
        raw_key[..16].to_string()
    }
}

/// Resolve an `X-Agent-Key` header value to an `AgentKeyClaims`.
///
/// Returns `None` if the key is invalid, not found, revoked, or expired.
pub async fn resolve_agent_key(
    pool: &PgPool,
    key_header: &str,
) -> Result<Option<AgentKeyClaims>, sqlx::Error> {
    let key = key_header.trim();

    // Fast reject: must start with "tudo_sk_"
    if !key.starts_with("tudo_sk_") {
        return Ok(None);
    }

    let hash = sha256_hash(key);

    let row: Option<AgentKeyRow> = sqlx::query_as(
        "SELECT id, user_id, name, key_hash, key_prefix, permissions, \
         created_at, expires_at, last_used_at, is_revoked, revoked_at \
         FROM agent_keys WHERE key_hash = $1"
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;

    let row = match row {
        None => return Ok(None),
        Some(r) => r,
    };

    // Check revocation
    if row.is_revoked {
        return Ok(None);
    }

    // Check expiry
    if let Some(expires_at) = row.expires_at {
        if expires_at < Utc::now() {
            return Ok(None);
        }
    }

    // Deserialize permissions from JSONB
    let permissions: Vec<crate::models::agent_key::AgentPermission> =
        serde_json::from_value(row.permissions).unwrap_or_default();

    // Update last_used_at asynchronously — fire and forget.
    let pool_clone = pool.clone();
    let key_id = row.id;
    tokio::spawn(async move {
        let _ = sqlx::query(
            "UPDATE agent_keys SET last_used_at = now() WHERE id = $1"
        )
        .bind(key_id)
        .execute(&pool_clone)
        .await;
    });

    Ok(Some(AgentKeyClaims {
        user_id: row.user_id,
        key_id: row.id,
        permissions,
    }))
}
