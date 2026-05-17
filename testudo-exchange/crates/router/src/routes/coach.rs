//! RSK-03 — AI Trade Coach Routes
//!
//! All endpoints require JWT auth (wired via `JwtMiddleware` in `main.rs`).
//! Responses use the envelope `{ data: ... }` for read endpoints; mutating
//! endpoints return 204 No Content on success.

use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    middleware::AuthenticatedUser,
    services::coach::StoredCoachReport,
    types::app::AppState,
};

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

fn internal_error(err: impl std::fmt::Display) -> HttpResponse {
    tracing::error!("coach route error: {}", err);
    HttpResponse::InternalServerError().json(ErrorBody {
        code: "coach_internal",
        message: "Coach request failed".to_string(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// GET /api/v1/coach/latest
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct LatestResponse {
    data: Option<StoredCoachReport>,
    has_new_indicator: bool,
}

pub async fn get_latest(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    match app_state.coach_service.latest_for(user.user_id).await {
        Ok(Some((report, has_new))) => Ok(HttpResponse::Ok().json(LatestResponse {
            data: Some(report),
            has_new_indicator: has_new,
        })),
        Ok(None) => Ok(HttpResponse::Ok().json(LatestResponse {
            data: None,
            has_new_indicator: false,
        })),
        Err(e) => Ok(internal_error(e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// GET /api/v1/coach/archive?limit=&offset=
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ArchiveQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ArchiveResponse {
    data: Vec<StoredCoachReport>,
}

pub async fn get_archive(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<ArchiveQuery>,
) -> Result<HttpResponse> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    match app_state
        .coach_service
        .archive_for(user.user_id, limit, offset)
        .await
    {
        Ok(reports) => Ok(HttpResponse::Ok().json(ArchiveResponse { data: reports })),
        Err(e) => Ok(internal_error(e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// GET /api/v1/coach/preference
// PATCH /api/v1/coach/preference
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct PreferenceResponse {
    coach_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePreferenceRequest {
    pub enabled: bool,
}

pub async fn get_preference(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    match app_state.coach_service.get_preference(user.user_id).await {
        Ok(enabled) => Ok(HttpResponse::Ok().json(PreferenceResponse {
            coach_enabled: enabled,
        })),
        Err(e) => Ok(internal_error(e)),
    }
}

pub async fn update_preference(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<UpdatePreferenceRequest>,
) -> Result<HttpResponse> {
    match app_state
        .coach_service
        .set_preference(user.user_id, body.enabled)
        .await
    {
        Ok(()) => Ok(HttpResponse::NoContent().finish()),
        Err(e) => Ok(internal_error(e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// POST /api/v1/coach/mark-viewed
// ─────────────────────────────────────────────────────────────────────────

pub async fn mark_viewed(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    match app_state.coach_service.mark_viewed(user.user_id).await {
        Ok(()) => Ok(HttpResponse::NoContent().finish()),
        Err(e) => Ok(internal_error(e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// PATCH /api/v1/coach/{report_id}/dismiss-banner
// ─────────────────────────────────────────────────────────────────────────

pub async fn dismiss_banner(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let report_id = path.into_inner();
    match app_state
        .coach_service
        .dismiss_banner(user.user_id, report_id)
        .await
    {
        Ok(()) => Ok(HttpResponse::NoContent().finish()),
        Err(e) => Ok(internal_error(e)),
    }
}
