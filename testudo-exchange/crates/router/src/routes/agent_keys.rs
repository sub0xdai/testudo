//! Agent API key CRUD endpoints.
//!
//! POST /api/v1/agent-keys    — create a new key (returns raw key once)
//! GET  /api/v1/agent-keys    — list all keys (no raw key values)
//! DELETE /api/v1/agent-keys/{id} — revoke a key (soft delete)
//! PATCH /api/v1/agent-keys/{id}  — update name/permissions

// @anchor exchange:router:agent-keys-routes
// @tags api

use actix_web::{web, HttpResponse, Result};
use chrono::Utc;
use validator::Validate;

use crate::middleware::AuthenticatedUser;
use crate::models::agent_key::{
    AgentKeySummary, CreateAgentKeyRequest, CreateAgentKeyResponse, UpdateAgentKeyRequest,
};
use crate::services::agent_key::{generate_agent_key, key_prefix_from_raw};
use crate::types::app::AppState;
use crate::types::auth::ErrorResponse;

/// POST /api/v1/agent-keys
///
/// Creates a scoped agent API key. The raw key is returned exactly once in the
/// 201 response. The key is SHA-256 hashed before storage — it is irrecoverable
/// after this response.
pub async fn create_key(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    req: web::Json<CreateAgentKeyRequest>,
) -> Result<HttpResponse> {
    if let Err(errors) = req.validate() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "validation_error",
            &format!("{:?}", errors),
        )));
    }

    let (raw_key, hash) = generate_agent_key();
    let prefix = key_prefix_from_raw(&raw_key);
    let permissions_json =
        serde_json::to_value(&req.permissions).unwrap_or_default();

    let expires_at = req.expires_in_days.map(|days| {
        Utc::now() + chrono::Duration::days(days as i64)
    });

    let row: crate::models::agent_key::AgentKeyRow = sqlx::query_as(
        "INSERT INTO agent_keys (user_id, name, key_hash, key_prefix, permissions, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING id, user_id, name, key_hash, key_prefix, permissions, created_at, \
                   expires_at, last_used_at, is_revoked, revoked_at"
    )
    .bind(user.user_id)
    .bind(&req.name)
    .bind(&hash)
    .bind(&prefix)
    .bind(&permissions_json)
    .bind(expires_at)
    .fetch_one(&app_state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create agent key: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to create agent key")
    })?;

    tracing::info!(
        "Agent key created: id={} name='{}' user={}",
        row.id, req.name, user.user_id
    );
    // NOTE: never log the raw key.

    Ok(HttpResponse::Created().json(CreateAgentKeyResponse {
        id: row.id,
        name: row.name,
        key: raw_key,
        permissions: req.permissions.clone(),
        expires_at: row.expires_at,
        created_at: row.created_at,
    }))
}

/// GET /api/v1/agent-keys
///
/// Lists all agent keys for the authenticated user.
/// Raw key values are NEVER included — only `key_prefix` for display.
pub async fn list_keys(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    let rows: Vec<crate::models::agent_key::AgentKeyRow> = sqlx::query_as(
        "SELECT id, user_id, name, key_hash, key_prefix, permissions, \
         created_at, expires_at, last_used_at, is_revoked, revoked_at \
         FROM agent_keys WHERE user_id = $1 ORDER BY created_at DESC"
    )
    .bind(user.user_id)
    .fetch_all(&app_state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list agent keys: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to list agent keys")
    })?;

    let keys: Vec<AgentKeySummary> = rows
        .into_iter()
        .map(|r| {
            let permissions: Vec<crate::policy::Permission> =
                serde_json::from_value(r.permissions).unwrap_or_default();
            AgentKeySummary {
                id: r.id,
                name: r.name,
                key_prefix: r.key_prefix,
                permissions,
                created_at: r.created_at,
                expires_at: r.expires_at,
                last_used_at: r.last_used_at,
                is_revoked: r.is_revoked,
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(keys))
}

/// DELETE /api/v1/agent-keys/{key_id}
///
/// Revokes an agent key (soft delete). Revocation takes effect immediately —
/// the key will fail authentication on the next request.
pub async fn revoke_key(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<uuid::Uuid>,
) -> Result<HttpResponse> {
    let key_id = path.into_inner();

    let result = sqlx::query(
        "UPDATE agent_keys SET is_revoked = true, revoked_at = now() \
         WHERE id = $1 AND user_id = $2 AND is_revoked = false"
    )
    .bind(key_id)
    .bind(user.user_id)
    .execute(&app_state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to revoke agent key: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to revoke agent key")
    })?;

    if result.rows_affected() == 0 {
        return Ok(HttpResponse::NotFound().json(ErrorResponse::new(
            "not_found",
            "Agent key not found or already revoked",
        )));
    }

    tracing::info!("Agent key revoked: id={} user={}", key_id, user.user_id);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Agent key revoked"
    })))
}

/// PATCH /api/v1/agent-keys/{key_id}
///
/// Partially updates an agent key. Only `name` and `permissions` can be
/// changed. Revoked or expired keys cannot be updated.
pub async fn update_key(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<uuid::Uuid>,
    req: web::Json<UpdateAgentKeyRequest>,
) -> Result<HttpResponse> {
    let key_id = path.into_inner();

    if let Some(ref name) = req.name {
        if name.is_empty() || name.len() > 128 {
            return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
                "validation_error",
                "Name must be between 1 and 128 characters",
            )));
        }
    }

    // Fetch current key to verify ownership and status
    let existing: Option<crate::models::agent_key::AgentKeyRow> = sqlx::query_as(
        "SELECT id, user_id, name, key_hash, key_prefix, permissions, \
         created_at, expires_at, last_used_at, is_revoked, revoked_at \
         FROM agent_keys WHERE id = $1 AND user_id = $2"
    )
    .bind(key_id)
    .bind(user.user_id)
    .fetch_optional(&app_state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch agent key for update: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to update agent key")
    })?;

    let existing = match existing {
        None => {
            return Ok(HttpResponse::NotFound().json(ErrorResponse::new(
                "not_found",
                "Agent key not found",
            )));
        }
        Some(r) => r,
    };

    if existing.is_revoked {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "already_revoked",
            "Cannot update a revoked key",
        )));
    }

    // Apply updates
    let new_name = req.name.as_deref().unwrap_or(&existing.name);
    let new_permissions = match &req.permissions {
        Some(perms) => serde_json::to_value(perms).unwrap_or(existing.permissions),
        None => existing.permissions,
    };

    sqlx::query(
        "UPDATE agent_keys SET name = $1, permissions = $2 WHERE id = $3"
    )
    .bind(new_name)
    .bind(&new_permissions)
    .bind(key_id)
    .execute(&app_state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update agent key: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to update agent key")
    })?;

    tracing::info!("Agent key updated: id={} name='{}'", key_id, new_name);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Agent key updated"
    })))
}
