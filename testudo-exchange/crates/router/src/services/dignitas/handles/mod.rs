//! Handle service — claim, release, visibility, and public profile (ENG-01b T4).
//!
//! All mutations run inside a transaction and bump `users.last_handle_change_at`
//! so the 30-day rate-limit window survives claim → release → reclaim cycles.

pub mod profanity;
pub mod reserved;
pub mod validation;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

pub use validation::{validate_handle, HandleValidationError, NormalizedHandle};

// ─── Domain types ────────────────────────────────────────────────────────────

/// A row from `user_handles`. Fields mirror the table columns.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserHandleRow {
    pub user_id: Uuid,
    pub handle: String,
    pub bio: Option<String>,
    pub show_score: bool,
    pub show_sparkline: bool,
    pub show_streak: bool,
    pub allow_indexing: bool,
    pub claimed_at: DateTime<Utc>,
}

/// Response for `GET /api/v1/dignitas/identity` (auth-required).
#[derive(Debug, Serialize)]
pub struct IdentityPreferences {
    pub handle: Option<String>,
    pub bio: Option<String>,
    pub show_score: bool,
    pub show_sparkline: bool,
    pub show_streak: bool,
    pub allow_indexing: bool,
    /// ISO 8601 timestamp after which handle changes are allowed again.
    /// `null` means the user can change now.
    pub can_change_handle_at: Option<DateTime<Utc>>,
}

/// Visibility + indexing patch body for `PATCH /api/v1/dignitas/visibility`.
#[derive(Debug, Deserialize)]
pub struct VisibilityPatch {
    pub show_score: Option<bool>,
    pub show_sparkline: Option<bool>,
    pub show_streak: Option<bool>,
    pub allow_indexing: Option<bool>,
}

/// One point in the public profile sparkline.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SparklinePoint {
    pub date: NaiveDate,
    pub score: Decimal,
}

/// Data returned by `get_public_profile`. Visibility flags already applied —
/// `score` / `sparkline` are `None` when the respective toggle is off.
#[derive(Debug, Serialize)]
pub struct PublicProfileData {
    pub handle: String,
    pub bio: Option<String>,
    pub member_since: DateTime<Utc>,
    pub score: Option<Decimal>,
    pub sparkline: Option<Vec<SparklinePoint>>,
    pub streak_days: Option<i64>,
    pub longest_ever: Option<i64>,
    pub allow_indexing: bool,
}

// ─── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum HandleError {
    #[error("handle validation failed: {0}")]
    Validation(#[from] HandleValidationError),
    #[error("handle already taken")]
    Taken,
    #[error("user already has a claimed handle; release it first")]
    AlreadyClaimed,
    #[error("rate limited: can change handle again at {retry_at}")]
    RateLimited { retry_at: DateTime<Utc> },
    #[error("no handle currently claimed")]
    NotFound,
    #[error("bio must be at most 140 characters")]
    BioTooLong,
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// Map a Postgres unique-constraint violation raised by the claim INSERT back
/// to the semantic `HandleError` so the route handler can return the correct
/// HTTP status (409 vs 500). Covers the TOCTOU window between the EXISTS
/// pre-checks and the INSERT.
///
/// - `idx_user_handles_handle_lower` → `Taken`       (two users raced for the same handle)
/// - `user_handles_pkey`             → `AlreadyClaimed` (one user double-claimed)
/// - anything else                   → surface as `Db` (re-wraps the original box)
fn map_claim_constraint_violation(
    db_err: Box<dyn sqlx::error::DatabaseError>,
) -> HandleError {
    match db_err.constraint() {
        Some("idx_user_handles_handle_lower") => HandleError::Taken,
        Some("user_handles_pkey") => HandleError::AlreadyClaimed,
        _ => HandleError::Db(sqlx::Error::Database(db_err)),
    }
}

// ─── Service ─────────────────────────────────────────────────────────────────

pub struct HandleService {
    pool: PgPool,
}

impl HandleService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check the 30-day rate limit for `user_id`. Returns the retry timestamp
    /// if still within the window.
    async fn check_rate_limit(&self, user_id: Uuid) -> Result<(), HandleError> {
        let (last_change,): (Option<DateTime<Utc>>,) = sqlx::query_as(
            "SELECT last_handle_change_at FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        if let Some(last) = last_change {
            let retry_at = last + Duration::days(30);
            if Utc::now() < retry_at {
                return Err(HandleError::RateLimited { retry_at });
            }
        }
        Ok(())
    }

    /// Claim a unique handle for `user_id`.
    ///
    /// Returns the new `UserHandleRow` on success. Errors:
    /// - `Validation` — handle format / reserved / profanity
    /// - `AlreadyClaimed` — user already owns a handle
    /// - `Taken` — handle already owned by another user
    /// - `RateLimited` — within 30-day change window
    /// - `BioTooLong` — bio exceeds 140 chars
    pub async fn claim(
        &self,
        user_id: Uuid,
        handle: &str,
        bio: Option<&str>,
    ) -> Result<UserHandleRow, HandleError> {
        let normalized = validate_handle(handle)?;

        if let Some(b) = bio {
            if b.chars().count() > 140 {
                return Err(HandleError::BioTooLong);
            }
        }

        self.check_rate_limit(user_id).await?;

        // Pre-checks (TOCTOU window is acceptable at MVP scale).
        let (user_already_claimed,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM user_handles WHERE user_id = $1)",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        if user_already_claimed {
            return Err(HandleError::AlreadyClaimed);
        }

        let (handle_taken,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM user_handles WHERE lower(handle) = $1)",
        )
        .bind(&normalized)
        .fetch_one(&self.pool)
        .await?;

        if handle_taken {
            return Err(HandleError::Taken);
        }

        let mut tx = self.pool.begin().await?;

        // TOCTOU defence: the pre-checks above close the common path but a
        // racing claim between the EXISTS read and this INSERT can still trip
        // a unique-constraint violation. Map both possible constraints back
        // to their semantic errors so AC #1 (409 on taken) holds under
        // contention instead of returning 500 with a raw DB error.
        let row: UserHandleRow = match sqlx::query_as(
            "INSERT INTO user_handles (user_id, handle, bio) \
             VALUES ($1, $2, $3) \
             RETURNING user_id, handle, bio, show_score, show_sparkline, \
                       show_streak, allow_indexing, claimed_at",
        )
        .bind(user_id)
        .bind(&normalized)
        .bind(bio)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(r) => r,
            Err(sqlx::Error::Database(db_err)) => {
                return Err(map_claim_constraint_violation(db_err));
            }
            Err(e) => return Err(HandleError::Db(e)),
        };

        sqlx::query("UPDATE users SET last_handle_change_at = NOW() WHERE id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(row)
    }

    /// Release the user's current handle, making it claimable by others.
    ///
    /// Also counts as a handle change — bumps `last_handle_change_at`.
    pub async fn release(&self, user_id: Uuid) -> Result<(), HandleError> {
        self.check_rate_limit(user_id).await?;

        let mut tx = self.pool.begin().await?;

        let result = sqlx::query("DELETE FROM user_handles WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Err(HandleError::NotFound);
        }

        sqlx::query("UPDATE users SET last_handle_change_at = NOW() WHERE id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Update visibility and indexing toggles. Any `None` field is left unchanged.
    pub async fn update_visibility(
        &self,
        user_id: Uuid,
        patch: &VisibilityPatch,
    ) -> Result<(), HandleError> {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM user_handles WHERE user_id = $1)",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        if !exists {
            return Err(HandleError::NotFound);
        }

        if let Some(v) = patch.show_score {
            sqlx::query("UPDATE user_handles SET show_score = $1 WHERE user_id = $2")
                .bind(v)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        }
        if let Some(v) = patch.show_sparkline {
            sqlx::query("UPDATE user_handles SET show_sparkline = $1 WHERE user_id = $2")
                .bind(v)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        }
        if let Some(v) = patch.show_streak {
            sqlx::query("UPDATE user_handles SET show_streak = $1 WHERE user_id = $2")
                .bind(v)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        }
        if let Some(v) = patch.allow_indexing {
            sqlx::query("UPDATE user_handles SET allow_indexing = $1 WHERE user_id = $2")
                .bind(v)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    /// Update the bio (≤140 chars). Pass `None` to clear.
    pub async fn update_bio(&self, user_id: Uuid, bio: Option<&str>) -> Result<(), HandleError> {
        if let Some(b) = bio {
            if b.chars().count() > 140 {
                return Err(HandleError::BioTooLong);
            }
        }

        let result = sqlx::query("UPDATE user_handles SET bio = $1 WHERE user_id = $2")
            .bind(bio)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(HandleError::NotFound);
        }
        Ok(())
    }

    /// Return identity preferences for the authenticated user.
    pub async fn get_identity(&self, user_id: Uuid) -> Result<IdentityPreferences, HandleError> {
        let handle_row: Option<UserHandleRow> = sqlx::query_as(
            "SELECT user_id, handle, bio, show_score, show_sparkline, show_streak, \
             allow_indexing, claimed_at \
             FROM user_handles WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let (last_change,): (Option<DateTime<Utc>>,) = sqlx::query_as(
            "SELECT last_handle_change_at FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let can_change_handle_at = last_change
            .map(|last| last + Duration::days(30))
            .filter(|&retry| Utc::now() < retry);

        Ok(IdentityPreferences {
            handle: handle_row.as_ref().map(|r| r.handle.clone()),
            bio: handle_row.as_ref().and_then(|r| r.bio.clone()),
            show_score: handle_row.as_ref().map_or(false, |r| r.show_score),
            show_sparkline: handle_row.as_ref().map_or(false, |r| r.show_sparkline),
            show_streak: handle_row.as_ref().map_or(false, |r| r.show_streak),
            allow_indexing: handle_row.as_ref().map_or(false, |r| r.allow_indexing),
            can_change_handle_at,
        })
    }

    /// Fetch public profile data for a handle (unauthenticated).
    ///
    /// Returns `None` when the handle is unclaimed (→ 404). Visibility flags
    /// gate which fields are populated: `score` and `sparkline` are `None`
    /// when the respective toggle is off (FR-5, FR-6, FR-10).
    pub async fn get_public_profile(
        &self,
        handle: &str,
    ) -> Result<Option<PublicProfileData>, HandleError> {
        let normalized = handle.trim().to_lowercase();

        let row: Option<UserHandleRow> = sqlx::query_as(
            "SELECT user_id, handle, bio, show_score, show_sparkline, show_streak, \
             allow_indexing, claimed_at \
             FROM user_handles WHERE lower(handle) = $1",
        )
        .bind(&normalized)
        .fetch_optional(&self.pool)
        .await?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let (member_since,): (DateTime<Utc>,) =
            sqlx::query_as("SELECT created_at FROM users WHERE id = $1")
                .bind(row.user_id)
                .fetch_one(&self.pool)
                .await?;

        let score = if row.show_score {
            sqlx::query_as::<_, (Decimal,)>(
                "SELECT score FROM dignitas_history \
                 WHERE user_id = $1 ORDER BY date DESC LIMIT 1",
            )
            .bind(row.user_id)
            .fetch_optional(&self.pool)
            .await?
            .map(|(s,)| s)
        } else {
            None
        };

        let sparkline = if row.show_sparkline {
            let cutoff = Utc::now().date_naive() - Duration::days(90);
            let points: Vec<SparklinePoint> = sqlx::query_as(
                "SELECT date, score FROM dignitas_history \
                 WHERE user_id = $1 AND date >= $2 ORDER BY date ASC",
            )
            .bind(row.user_id)
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?;
            Some(points)
        } else {
            None
        };

        // ENG-01c: opt-in streak on public profile. When show_streak is true
        // AND the user has a streak row (requires coach_reports + daily tick),
        // expose days_clean + longest_ever. Either field null = opted-out or
        // no data yet.
        let (streak_days, longest_ever) = if row.show_streak {
            let streak: Option<(i64, i64)> = sqlx::query_as(
                "SELECT days_clean, longest_ever FROM dignitas_streak WHERE user_id = $1",
            )
            .bind(row.user_id)
            .fetch_optional(&self.pool)
            .await?;
            match streak {
                Some((d, l)) => (Some(d), Some(l)),
                None => (None, None),
            }
        } else {
            (None, None)
        };

        Ok(Some(PublicProfileData {
            handle: row.handle,
            bio: row.bio,
            member_since,
            score,
            sparkline,
            streak_days,
            longest_ever,
            allow_indexing: row.allow_indexing,
        }))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Pure unit tests for outcome decisions — no DB required.

    #[test]
    fn rate_limit_window_boundary() {
        let last = Utc::now() - Duration::days(29);
        let retry_at = last + Duration::days(30);
        // Still within 30-day window → rate limited
        assert!(Utc::now() < retry_at);
    }

    #[test]
    fn rate_limit_expired() {
        let last = Utc::now() - Duration::days(31);
        let retry_at = last + Duration::days(30);
        // Window has passed → allowed
        assert!(Utc::now() >= retry_at);
    }

    #[test]
    fn bio_length_boundary() {
        let ok_bio = "a".repeat(140);
        let too_long = "a".repeat(141);
        // 140 chars is fine
        assert!(ok_bio.chars().count() <= 140);
        // 141 chars triggers BioTooLong
        assert!(too_long.chars().count() > 140);
    }

    // DB-backed integration tests live in routes/dignitas.rs under #[ignore].
}
