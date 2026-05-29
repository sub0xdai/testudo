//! QNT-01a — User Settings Routes
//!
//! GET  /api/v1/user/settings - Returns the user's settings blob plus an
//!                              `unlocked` flag derived from tagged-trade count.
//! PATCH /api/v1/user/settings - Updates the Dynamic Risk toggle. Server-side
//!                               unlock gate (≥30 tagged closed trades) prevents
//!                               flipping `dynamic_risk_enabled` to true before
//!                               the user has enough calibration data.

// @anchor exchange:router:user_settings
// @tags api

use actix_web::{web, HttpResponse, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{middleware::AuthenticatedUser, types::app::AppState};

/// Minimum tagged closed trades required to enable Dynamic Risk.
pub const UNLOCK_THRESHOLD: i64 = 30;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserSettings {
    #[serde(default)]
    pub dynamic_risk_enabled: bool,
    #[serde(default)]
    pub dynamic_risk_unlocked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct UserSettingsResponse {
    pub settings: UserSettings,
    pub unlocked: bool,
    pub tagged_trade_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct PatchUserSettingsRequest {
    pub dynamic_risk_enabled: bool,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tagged_trade_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<i64>,
}

fn internal_error(err: impl std::fmt::Display) -> HttpResponse {
    tracing::error!("user_settings route error: {}", err);
    HttpResponse::InternalServerError().json(ErrorBody {
        error: "user_settings_internal",
        message: "User settings request failed".to_string(),
        tagged_trade_count: None,
        required: None,
    })
}

/// Pure helper: tagged-trade count crosses the unlock threshold.
pub fn is_unlocked(tagged_trade_count: i64) -> bool {
    tagged_trade_count >= UNLOCK_THRESHOLD
}

/// Pure helper: applies an enable-change to the settings blob, stamping
/// `dynamic_risk_unlocked_at` once on the first successful enable. Subsequent
/// disable→enable cycles preserve the original unlock timestamp.
pub fn apply_enable_change(
    settings: &mut UserSettings,
    new_enabled: bool,
    now: DateTime<Utc>,
) {
    settings.dynamic_risk_enabled = new_enabled;
    if new_enabled && settings.dynamic_risk_unlocked_at.is_none() {
        settings.dynamic_risk_unlocked_at = Some(now);
    }
}

async fn fetch_or_create_settings(
    pool: &sqlx::Pool<sqlx::Postgres>,
    user_id: uuid::Uuid,
) -> Result<UserSettings, sqlx::Error> {
    sqlx::query("INSERT INTO user_settings (user_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(user_id)
        .execute(pool)
        .await?;

    let row: (serde_json::Value,) =
        sqlx::query_as("SELECT settings FROM user_settings WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

    Ok(serde_json::from_value(row.0).unwrap_or_default())
}

async fn count_tagged_trades(
    pool: &sqlx::Pool<sqlx::Postgres>,
    user_id: uuid::Uuid,
) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM journal_trades WHERE user_id = $1 AND setup_tag IS NOT NULL",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

async fn save_settings(
    pool: &sqlx::Pool<sqlx::Postgres>,
    user_id: uuid::Uuid,
    settings: &UserSettings,
) -> Result<(), sqlx::Error> {
    let blob = serde_json::to_value(settings).expect("UserSettings always serializes");
    sqlx::query(
        "INSERT INTO user_settings (user_id, settings, updated_at) \
         VALUES ($1, $2, NOW()) \
         ON CONFLICT (user_id) DO UPDATE SET settings = EXCLUDED.settings, updated_at = NOW()",
    )
    .bind(user_id)
    .bind(blob)
    .execute(pool)
    .await?;
    Ok(())
}

/// GET /api/v1/user/settings
pub async fn get_user_settings(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    let settings = match fetch_or_create_settings(&app_state.pool, user.user_id).await {
        Ok(s) => s,
        Err(e) => return Ok(internal_error(e)),
    };

    let count = match count_tagged_trades(&app_state.pool, user.user_id).await {
        Ok(c) => c,
        Err(e) => return Ok(internal_error(e)),
    };

    Ok(HttpResponse::Ok().json(UserSettingsResponse {
        settings,
        unlocked: is_unlocked(count),
        tagged_trade_count: count,
    }))
}

/// PATCH /api/v1/user/settings
pub async fn patch_user_settings(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    req: web::Json<PatchUserSettingsRequest>,
) -> Result<HttpResponse> {
    let mut settings = match fetch_or_create_settings(&app_state.pool, user.user_id).await {
        Ok(s) => s,
        Err(e) => return Ok(internal_error(e)),
    };

    let count = match count_tagged_trades(&app_state.pool, user.user_id).await {
        Ok(c) => c,
        Err(e) => return Ok(internal_error(e)),
    };

    // Unlock gate: only enforced on enable transitions. Disabling is always allowed.
    if req.dynamic_risk_enabled && !is_unlocked(count) {
        return Ok(HttpResponse::Conflict().json(ErrorBody {
            error: "unlock_gate",
            message: format!(
                "Dynamic Risk requires ≥ {UNLOCK_THRESHOLD} tagged closed trades (you have {count})."
            ),
            tagged_trade_count: Some(count),
            required: Some(UNLOCK_THRESHOLD),
        }));
    }

    apply_enable_change(&mut settings, req.dynamic_risk_enabled, Utc::now());

    if let Err(e) = save_settings(&app_state.pool, user.user_id, &settings).await {
        return Ok(internal_error(e));
    }

    Ok(HttpResponse::Ok().json(UserSettingsResponse {
        settings,
        unlocked: is_unlocked(count),
        tagged_trade_count: count,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_threshold_is_30() {
        assert_eq!(UNLOCK_THRESHOLD, 30);
    }

    #[test]
    fn is_unlocked_below_threshold_returns_false() {
        assert!(!is_unlocked(0));
        assert!(!is_unlocked(29));
    }

    #[test]
    fn is_unlocked_at_threshold_returns_true() {
        assert!(is_unlocked(30));
    }

    #[test]
    fn is_unlocked_well_above_threshold_returns_true() {
        assert!(is_unlocked(10_000));
    }

    #[test]
    fn apply_enable_change_first_enable_stamps_unlocked_at() {
        let mut s = UserSettings::default();
        let now = Utc::now();
        apply_enable_change(&mut s, true, now);
        assert!(s.dynamic_risk_enabled);
        assert_eq!(s.dynamic_risk_unlocked_at, Some(now));
    }

    #[test]
    fn apply_enable_change_disable_then_reenable_preserves_original_unlocked_at() {
        let mut s = UserSettings::default();
        let first_enable = Utc::now();
        apply_enable_change(&mut s, true, first_enable);

        let later = first_enable + chrono::Duration::days(7);
        apply_enable_change(&mut s, false, later);
        assert!(!s.dynamic_risk_enabled);
        assert_eq!(
            s.dynamic_risk_unlocked_at,
            Some(first_enable),
            "unlocked_at must persist across disable"
        );

        let later_again = later + chrono::Duration::hours(1);
        apply_enable_change(&mut s, true, later_again);
        assert!(s.dynamic_risk_enabled);
        assert_eq!(
            s.dynamic_risk_unlocked_at,
            Some(first_enable),
            "unlocked_at must NOT advance on re-enable"
        );
    }

    #[test]
    fn apply_enable_change_disable_when_never_enabled_does_not_stamp() {
        let mut s = UserSettings::default();
        apply_enable_change(&mut s, false, Utc::now());
        assert!(!s.dynamic_risk_enabled);
        assert_eq!(s.dynamic_risk_unlocked_at, None);
    }

    #[test]
    fn user_settings_default_is_disabled_with_no_unlock_timestamp() {
        let s = UserSettings::default();
        assert!(!s.dynamic_risk_enabled);
        assert_eq!(s.dynamic_risk_unlocked_at, None);
    }

    #[test]
    fn user_settings_round_trips_through_jsonb_default_shape() {
        // Matches the migration DEFAULT '{"dynamic_risk_enabled": false, "dynamic_risk_unlocked_at": null}'::jsonb
        let raw = serde_json::json!({
            "dynamic_risk_enabled": false,
            "dynamic_risk_unlocked_at": null,
        });
        let parsed: UserSettings = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed, UserSettings::default());
    }
}
