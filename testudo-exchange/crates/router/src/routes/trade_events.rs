//! Trade Events Route (019f)
//!
//! Provides `GET /api/v1/trades/{id}/events` — returns the append-only
//! event history for a trade group, ordered by sequence number.
//! User-scoped: only the owning user can see events.
//! Uses the same dual-auth pattern as trade_management (JWT Bearer or X-User-Id).

// @anchor exchange:router:trade_events
// @tags api

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use crate::routes::trade_management::TradeManagementState;

#[derive(Serialize)]
struct EventRow {
    seq: i64,
    event_type: String,
    payload: serde_json::Value,
    created_at: String,
}

#[derive(Serialize)]
struct EventsResponse {
    events: Vec<EventRow>,
}

/// GET /api/v1/trades/{id}/events
///
/// Returns the event history for a trade group, ordered by seq.
/// User-scoped: uses dual auth (JWT or X-User-Id).
pub async fn get_trade_events(
    req: HttpRequest,
    path: web::Path<Uuid>,
    trade_state: web::Data<TradeManagementState>,
) -> HttpResponse {
    let group_id = path.into_inner();

    // Extract user_id using the same dual-auth pattern as trade management routes.
    let user_id = match extract_user_id_from_request(&req, &trade_state).await {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // Verify the trade group belongs to this user via the engine
    let group = trade_state.engine_handle.get_trade_group(group_id).await;
    match group {
        Some(g) if g.user_id == user_id => {}
        Some(_) => {
            return HttpResponse::Forbidden().json(serde_json::json!({
                "error": "Access denied"
            }));
        }
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "Trade group not found"
            }));
        }
    }

    // Get pool from app state
    let pool = match req.app_data::<web::Data<crate::types::app::AppState>>() {
        Some(state) => &state.pool,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal configuration error"
            }));
        }
    };

    // Query events from the append-only log
    let rows: Vec<sqlx::postgres::PgRow> = match sqlx::query(
        "SELECT seq, event_type, payload, created_at \
         FROM trade_events \
         WHERE group_id = $1 AND user_id = $2 \
         ORDER BY seq ASC",
    )
    .bind(group_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error = %e, "Failed to query trade events");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to query events"
            }));
        }
    };

    let events: Vec<EventRow> = rows
        .iter()
        .map(|row: &sqlx::postgres::PgRow| EventRow {
            seq: row.get("seq"),
            event_type: row.get("event_type"),
            payload: row.get("payload"),
            created_at: row
                .get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                .to_rfc3339(),
        })
        .collect();

    HttpResponse::Ok().json(EventsResponse { events })
}

/// Extract user ID from request using the dual-auth pattern.
/// Tries JWT Bearer token first, falls back to X-User-Id header.
async fn extract_user_id_from_request(
    req: &HttpRequest,
    state: &TradeManagementState,
) -> Result<Uuid, HttpResponse> {
    // Try JWT Bearer token first
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if let Some(ref token_service) = state.token_service {
                    match token_service.verify_access_token(token) {
                        Ok(claims) => {
                            if let Ok(user_id) = Uuid::parse_str(&claims.sub) {
                                return Ok(user_id);
                            }
                        }
                        Err(_) => {
                            return Err(HttpResponse::Unauthorized().json(serde_json::json!({
                                "error": "Invalid or expired token"
                            })));
                        }
                    }
                }
            }
        }
    }

    // Fall back to X-User-Id header
    match req.headers().get("X-User-Id") {
        Some(value) => match value.to_str().ok().and_then(|s| Uuid::parse_str(s).ok()) {
            Some(id) => Ok(id),
            None => Err(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid user ID format"
            }))),
        },
        None => Err(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "X-User-Id header or Authorization Bearer token required"
        }))),
    }
}
