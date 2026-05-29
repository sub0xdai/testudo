//! ENG-01a — Dignitas routes.
//! ENG-01b — Handle claim / release / visibility / identity endpoints.
//!
//! All endpoints require JWT auth (wired via `JwtMiddleware` in `main.rs`).
//! GET endpoints return `{ ... }` JSON objects; PATCH/DELETE returns 204 No Content.

// @anchor exchange:router:dignitas
// @tags api

use actix_web::{web, HttpResponse, Result};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    middleware::AuthenticatedUser,
    services::dignitas::{
        handles::{HandleError, HandleService, VisibilityPatch},
        streak::{self, StreakWire},
    },
    types::app::AppState,
};

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

fn internal_error(err: impl std::fmt::Display) -> HttpResponse {
    tracing::error!("dignitas route error: {}", err);
    HttpResponse::InternalServerError().json(ErrorBody {
        code: "dignitas_internal",
        message: "Dignitas request failed".to_string(),
    })
}

// ─── GET /api/v1/dignitas/me ──────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct InputContributionsWire {
    pub drawdown_adherence: String,
    pub risk_per_trade_consistency: String,
    pub setup_adherence: String,
    pub coach_severity_penalty: String,
    pub journal_consistency: String,
}

#[derive(Debug, Serialize)]
pub struct DignitasMeResponse {
    pub score: String,
    pub delta_7d: Option<String>,
    pub cold_start: bool,
    /// Closed trades counted in the trailing 30d window for the latest
    /// snapshot. Drives "PRELIMINARY — N of M trades" UI copy.
    pub trade_count_30d: i64,
    pub pill_hidden: bool,
    pub contributions: InputContributionsWire,
    /// ENG-01c: `null` when user has no RSK-03 coach reports yet.
    pub streak: Option<StreakWire>,
}

/// Return the user's streak wire only when they have any coach_reports.
/// Before first coach report, streak is semantically meaningless — surface
/// as `null` so the UI can render the `STREAK —` fallback.
async fn load_streak_wire_if_coach_data(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
) -> Result<Option<StreakWire>, sqlx::Error> {
    let (has_coach,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM coach_reports WHERE user_id = $1)")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    if !has_coach {
        return Ok(None);
    }
    let row = streak::get_current(pool, user_id).await?;
    Ok(row.as_ref().map(StreakWire::from))
}

#[derive(sqlx::FromRow)]
struct SnapshotRow {
    date: chrono::NaiveDate,
    score: Decimal,
    cold_start: bool,
    trade_count_30d: i32,
    drawdown_adherence: Decimal,
    risk_per_trade_consistency: Decimal,
    setup_adherence: Decimal,
    coach_severity_penalty: Decimal,
    journal_consistency: Decimal,
}

pub async fn get_me(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    let user_id = user.user_id;

    let pill_hidden = match sqlx::query_as::<_, (Option<bool>,)>(
        "SELECT dignitas_pill_hidden FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&app_state.pool)
    .await
    {
        Ok((v,)) => v.unwrap_or(false),
        Err(e) => return Ok(internal_error(e)),
    };

    let latest = match sqlx::query_as::<_, SnapshotRow>(
        "SELECT date, score, cold_start, trade_count_30d, \
         drawdown_adherence, risk_per_trade_consistency, \
         setup_adherence, coach_severity_penalty, journal_consistency \
         FROM dignitas_history WHERE user_id = $1 ORDER BY date DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&app_state.pool)
    .await
    {
        Ok(row) => row,
        Err(e) => return Ok(internal_error(e)),
    };

    let streak_wire = match load_streak_wire_if_coach_data(&app_state.pool, user_id).await {
        Ok(s) => s,
        Err(e) => return Ok(internal_error(e)),
    };

    let Some(snapshot) = latest else {
        // No snapshot row yet — first scheduler run hasn't fired for this
        // user. Surface a neutral score; the cold_start flag tells the UI
        // to render "PRELIMINARY — 0 trades" instead of a real number.
        return Ok(HttpResponse::Ok().json(DignitasMeResponse {
            score: "50".to_string(),
            delta_7d: None,
            cold_start: true,
            trade_count_30d: 0,
            pill_hidden,
            contributions: InputContributionsWire {
                drawdown_adherence: "0".to_string(),
                risk_per_trade_consistency: "0".to_string(),
                setup_adherence: "0".to_string(),
                coach_severity_penalty: "0".to_string(),
                journal_consistency: "0".to_string(),
            },
            streak: streak_wire,
        }));
    };

    let baseline_date = snapshot.date - Duration::days(7);
    let delta_7d = sqlx::query_as::<_, (Decimal,)>(
        "SELECT score FROM dignitas_history WHERE user_id = $1 AND date = $2",
    )
    .bind(user_id)
    .bind(baseline_date)
    .fetch_optional(&app_state.pool)
    .await
    .ok()
    .flatten()
    .map(|(base,)| (snapshot.score - base).to_string());

    Ok(HttpResponse::Ok().json(DignitasMeResponse {
        score: snapshot.score.to_string(),
        delta_7d,
        cold_start: snapshot.cold_start,
        trade_count_30d: snapshot.trade_count_30d as i64,
        pill_hidden,
        contributions: InputContributionsWire {
            drawdown_adherence: snapshot.drawdown_adherence.to_string(),
            risk_per_trade_consistency: snapshot.risk_per_trade_consistency.to_string(),
            setup_adherence: snapshot.setup_adherence.to_string(),
            coach_severity_penalty: snapshot.coach_severity_penalty.to_string(),
            journal_consistency: snapshot.journal_consistency.to_string(),
        },
        streak: streak_wire,
    }))
}

// ─── GET /api/v1/dignitas/history ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub days: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct HistoryPoint {
    pub date: String,
    pub score: String,
    pub cold_start: bool,
}

#[derive(Debug, Serialize)]
pub struct DignitasHistoryResponse {
    pub snapshots: Vec<HistoryPoint>,
}

#[derive(sqlx::FromRow)]
struct HistoryRow {
    date: chrono::NaiveDate,
    score: Decimal,
    cold_start: bool,
}

pub async fn get_history(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<HistoryQuery>,
) -> Result<HttpResponse> {
    let days = query.days.unwrap_or(90).clamp(1, 365);
    let cutoff = Utc::now().date_naive() - Duration::days(days);

    match sqlx::query_as::<_, HistoryRow>(
        "SELECT date, score, cold_start FROM dignitas_history \
         WHERE user_id = $1 AND date >= $2 ORDER BY date ASC",
    )
    .bind(user.user_id)
    .bind(cutoff)
    .fetch_all(&app_state.pool)
    .await
    {
        Ok(rows) => {
            let snapshots = rows
                .into_iter()
                .map(|r| HistoryPoint {
                    date: r.date.to_string(),
                    score: r.score.to_string(),
                    cold_start: r.cold_start,
                })
                .collect();
            Ok(HttpResponse::Ok().json(DignitasHistoryResponse { snapshots }))
        }
        Err(e) => Ok(internal_error(e)),
    }
}

// ─── PATCH /api/v1/dignitas/preferences ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PatchPreferencesRequest {
    pub pill_hidden: Option<bool>,
}

pub async fn patch_preferences(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<PatchPreferencesRequest>,
) -> Result<HttpResponse> {
    if let Some(hidden) = body.pill_hidden {
        if let Err(e) = sqlx::query("UPDATE users SET dignitas_pill_hidden = $1 WHERE id = $2")
            .bind(hidden)
            .bind(user.user_id)
            .execute(&app_state.pool)
            .await
        {
            return Ok(internal_error(e));
        }
    }
    Ok(HttpResponse::NoContent().finish())
}

// ─── POST /api/v1/dignitas/handle ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ClaimHandleRequest {
    pub handle: String,
    pub bio: Option<String>,
}

pub async fn post_handle(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<ClaimHandleRequest>,
) -> Result<HttpResponse> {
    let svc = HandleService::new(app_state.pool.clone());
    match svc.claim(user.user_id, &body.handle, body.bio.as_deref()).await {
        Ok(_) => match svc.get_identity(user.user_id).await {
            Ok(prefs) => Ok(HttpResponse::Created().json(prefs)),
            Err(e) => Ok(internal_error(e)),
        },
        Err(HandleError::Validation(e)) => Ok(HttpResponse::BadRequest().json(ErrorBody {
            code: "handle_invalid",
            message: e.to_string(),
        })),
        Err(HandleError::Taken) => Ok(HttpResponse::Conflict().json(ErrorBody {
            code: "handle_taken",
            message: "Handle already taken".to_string(),
        })),
        Err(HandleError::AlreadyClaimed) => Ok(HttpResponse::Conflict().json(ErrorBody {
            code: "handle_already_claimed",
            message: "Release your current handle before claiming a new one".to_string(),
        })),
        Err(HandleError::RateLimited { retry_at }) => {
            Ok(HttpResponse::TooManyRequests().json(serde_json::json!({
                "code": "rate_limited",
                "message": "Handle changes are rate-limited to once per 30 days",
                "can_change_handle_at": retry_at,
            })))
        }
        Err(HandleError::BioTooLong) => Ok(HttpResponse::BadRequest().json(ErrorBody {
            code: "bio_too_long",
            message: "Bio must be at most 140 characters".to_string(),
        })),
        Err(e) => Ok(internal_error(e)),
    }
}

// ─── PATCH /api/v1/dignitas/handle ───────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct PatchBioRequest {
    pub bio: Option<String>,
}

pub async fn patch_handle(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<PatchBioRequest>,
) -> Result<HttpResponse> {
    let svc = HandleService::new(app_state.pool.clone());
    match svc.update_bio(user.user_id, body.bio.as_deref()).await {
        Ok(()) => Ok(HttpResponse::NoContent().finish()),
        Err(HandleError::NotFound) => Ok(HttpResponse::NotFound().json(ErrorBody {
            code: "handle_not_found",
            message: "Claim a handle before updating bio".to_string(),
        })),
        Err(HandleError::BioTooLong) => Ok(HttpResponse::BadRequest().json(ErrorBody {
            code: "bio_too_long",
            message: "Bio must be at most 140 characters".to_string(),
        })),
        Err(e) => Ok(internal_error(e)),
    }
}

// ─── DELETE /api/v1/dignitas/handle ──────────────────────────────────────────

pub async fn delete_handle(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    let svc = HandleService::new(app_state.pool.clone());
    match svc.release(user.user_id).await {
        Ok(()) => Ok(HttpResponse::NoContent().finish()),
        Err(HandleError::NotFound) => Ok(HttpResponse::NotFound().json(ErrorBody {
            code: "handle_not_found",
            message: "No handle currently claimed".to_string(),
        })),
        Err(HandleError::RateLimited { retry_at }) => {
            Ok(HttpResponse::TooManyRequests().json(serde_json::json!({
                "code": "rate_limited",
                "message": "Handle changes are rate-limited to once per 30 days",
                "can_change_handle_at": retry_at,
            })))
        }
        Err(e) => Ok(internal_error(e)),
    }
}

// ─── PATCH /api/v1/dignitas/visibility ───────────────────────────────────────

pub async fn patch_visibility(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<VisibilityPatch>,
) -> Result<HttpResponse> {
    let svc = HandleService::new(app_state.pool.clone());
    match svc.update_visibility(user.user_id, &body).await {
        Ok(()) => Ok(HttpResponse::NoContent().finish()),
        Err(HandleError::NotFound) => Ok(HttpResponse::NotFound().json(ErrorBody {
            code: "handle_not_found",
            message: "Claim a handle before updating visibility".to_string(),
        })),
        Err(e) => Ok(internal_error(e)),
    }
}

// ─── GET /api/v1/dignitas/identity ───────────────────────────────────────────

pub async fn get_identity(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    let svc = HandleService::new(app_state.pool.clone());
    match svc.get_identity(user.user_id).await {
        Ok(prefs) => Ok(HttpResponse::Ok().json(prefs)),
        Err(e) => Ok(internal_error(e)),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_request_deserializes_without_bio() {
        let raw = r#"{"handle":"0xwhale"}"#;
        let req: ClaimHandleRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.handle, "0xwhale");
        assert!(req.bio.is_none());
    }

    #[test]
    fn claim_request_deserializes_with_bio() {
        let raw = r#"{"handle":"0xwhale","bio":"I trade the open"}"#;
        let req: ClaimHandleRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.bio.as_deref(), Some("I trade the open"));
    }

    #[test]
    fn visibility_patch_all_none() {
        let raw = r#"{}"#;
        let patch: VisibilityPatch = serde_json::from_str(raw).unwrap();
        assert!(patch.show_score.is_none());
        assert!(patch.show_sparkline.is_none());
        assert!(patch.allow_indexing.is_none());
    }

    #[test]
    fn visibility_patch_partial() {
        let raw = r#"{"show_score":true}"#;
        let patch: VisibilityPatch = serde_json::from_str(raw).unwrap();
        assert_eq!(patch.show_score, Some(true));
        assert!(patch.show_sparkline.is_none());
    }
}

/// DB-backed integration tests for the 30-day handle-change rate-limit window.
///
/// Run with:
/// ```bash
/// DATABASE_URL=postgres://user:pass@localhost/testudo \
///     cargo test -p router handle_rate_limit_integration -- --ignored
/// ```
#[cfg(test)]
mod handle_rate_limit_integration {
    use sqlx::postgres::PgPoolOptions;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::services::dignitas::handles::{HandleError, HandleService};

    async fn pool() -> PgPool {
        let url = std::env::var("DATABASE_URL").expect(
            "DATABASE_URL required — set to an initialized Postgres connection string.",
        );
        PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .expect("connect to DATABASE_URL")
    }

    /// Insert a throwaway user row. Returns the new user_id.
    async fn make_user(pool: &PgPool) -> Uuid {
        let suffix = Uuid::new_v4().simple().to_string();
        let hex40: String = format!("{suffix}{suffix}").chars().take(40).collect();
        let wallet = format!("0x{hex40}");
        sqlx::query_scalar("INSERT INTO users (wallet_address) VALUES ($1) RETURNING id")
            .bind(&wallet)
            .fetch_one(pool)
            .await
            .expect("insert test user")
    }

    /// Remove test data in FK order (children before parent).
    async fn cleanup(pool: &PgPool, user_id: Uuid) {
        let _ = sqlx::query("DELETE FROM user_handles WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM dignitas_history WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    /// Verifies the full 30-day handle-change rate-limit lifecycle:
    /// 1. First claim succeeds (no prior `last_handle_change_at`)
    /// 2. Immediate release is rate-limited (claim bumped the window)
    /// 3. Backdating `last_handle_change_at` to >30 days allows release
    /// 4. Immediate re-claim is rate-limited (release bumped the window)
    #[tokio::test]
    #[ignore]
    async fn rate_limit_window_survives_claim_release_reclaim() {
        let pool = pool().await;
        let user_id = make_user(&pool).await;
        let svc = HandleService::new(pool.clone());

        // (1) First claim — no prior last_handle_change_at, should succeed.
        let suffix = Uuid::new_v4().simple().to_string()[..8].to_string();
        let handle = format!("test{suffix}");
        svc.claim(user_id, &handle, None)
            .await
            .expect("first claim should succeed");

        // (2) Immediate release — window just set by claim → rate limited.
        match svc.release(user_id).await {
            Err(HandleError::RateLimited { retry_at }) => {
                // Confirmed: retry_at is ~30 days in the future.
                let days_until = (retry_at - chrono::Utc::now()).num_days();
                assert!(
                    days_until >= 29,
                    "expected retry_at ~30d from now, got {days_until}d"
                );
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }

        // (3) Backdate last_handle_change_at to 31 days ago so the window expires.
        sqlx::query(
            "UPDATE users SET last_handle_change_at = NOW() - INTERVAL '31 days' WHERE id = $1",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("backdate last_handle_change_at");

        // Release now succeeds (window expired).
        svc.release(user_id)
            .await
            .expect("release after window expiry should succeed");

        // (4) Immediate re-claim — window just set by release → rate limited.
        let handle2 = format!("test{}b", &suffix);
        match svc.claim(user_id, &handle2, None).await {
            Err(HandleError::RateLimited { .. }) => {} // expected
            other => panic!("expected RateLimited after release, got {other:?}"),
        }

        cleanup(&pool, user_id).await;
    }
}
