//! HIST-01: Trade history import endpoints

// @anchor exchange:router:imports
// @tags api

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::middleware::AuthenticatedUser;
use crate::services::import_worker::enqueue_import;
use crate::types::app::AppState;

type Result<T> = std::result::Result<T, actix_web::Error>;

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub exchange_name: String,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub job_id: i64,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ImportStatusEntry {
    pub id: i64,
    pub exchange_name: String,
    pub status: String,
    pub created_at: String,
    pub processed_at: Option<String>,
}

/// POST /api/v1/trades/import
/// Enqueue a trade history import job for a connected exchange.
pub async fn start_import(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<ImportRequest>,
) -> Result<HttpResponse> {
    // Find the user's exchange account
    let account = app_state
        .exchange_account_repo
        .find_by_exchange(user.user_id, &body.exchange_name)
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!(
                "Failed to look up exchange account: {e}"
            ))
        })?
        .ok_or_else(|| {
            actix_web::error::ErrorNotFound(format!(
                "No active account found for exchange: {}",
                body.exchange_name
            ))
        })?;

    let queue = &app_state.pg_queue.queue;
    let job_id = enqueue_import(queue, user.user_id, account.id, &body.exchange_name)
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Failed to enqueue import: {e}"))
        })?;

    tracing::info!(
        job_id,
        exchange = %body.exchange_name,
        user_id = %user.user_id,
        "Import job enqueued"
    );

    Ok(HttpResponse::Accepted().json(ImportResponse {
        job_id,
        status: "queued".to_string(),
    }))
}

/// GET /api/v1/trades/import/status
/// List import jobs for the current user.
pub async fn import_status(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    #[derive(sqlx::FromRow)]
    struct ImportRow {
        id: i64,
        payload: serde_json::Value,
        status: String,
        created_at: chrono::DateTime<chrono::Utc>,
        processed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let rows: Vec<ImportRow> = sqlx::query_as(
        "SELECT id, payload, status, created_at, processed_at \
         FROM queue_imports \
         WHERE payload->>'user_id' = $1 \
         ORDER BY created_at DESC \
         LIMIT 20",
    )
    .bind(user.user_id.to_string())
    .fetch_all(&app_state.pool)
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!(
            "Failed to query import status: {e}"
        ))
    })?;

    let entries: Vec<ImportStatusEntry> = rows
        .into_iter()
        .map(|row| {
            let exchange_name = row
                .payload
                .get("exchange_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            ImportStatusEntry {
                id: row.id,
                exchange_name,
                status: row.status,
                created_at: row.created_at.to_rfc3339(),
                processed_at: row.processed_at.map(|t| t.to_rfc3339()),
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(entries))
}
