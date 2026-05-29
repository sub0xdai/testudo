//! Weekly coach scheduler.
//!
//! `spawn_weekly_task` wires a long-running `tokio::spawn` that wakes every
//! hour, checks `is_trigger_moment` (Sunday 18:00 UTC) + `already_fired_this_week`
//! (DB idempotency guard), then runs the batch when both gates pass.
//!
//! Week window: the job analyses the seven days ending at the most recent
//! Sunday 00:00 UTC (so a Sun 18:00 UTC firing looks at [prev Sun 00:00,
//! this Sun 00:00)). `compute_week_bounds` is a pure helper so the boundary
//! logic is unit-testable without a clock.

// @anchor exchange:router:schedule
// @tags api

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Duration as ChronoDuration, TimeZone, Timelike, Utc, Weekday};
use futures_util::stream::{self, StreamExt};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::service::CoachService;

/// Hour of day (UTC) the job fires on Sundays.
const TRIGGER_HOUR_UTC: u32 = 18;

/// Poll interval. The wall-clock test runs hourly; misses ≤ 1h are fine
/// for a weekly report, and the week idempotency guard prevents drift.
const POLL_INTERVAL: Duration = Duration::from_secs(3600);

/// How many users' reports to generate concurrently.
const BATCH_CONCURRENCY: usize = 10;

/// True when `now` is Sunday between `TRIGGER_HOUR_UTC:00` and
/// `TRIGGER_HOUR_UTC:59` UTC. Paired with `already_fired_this_week` for
/// exactly-once semantics across hourly wake-ups.
pub(super) fn is_trigger_moment(now: DateTime<Utc>) -> bool {
    now.weekday() == Weekday::Sun && now.hour() == TRIGGER_HOUR_UTC
}

/// Window to analyse, anchored at the most recent Sunday 00:00 UTC.
/// Returns `(week_start, week_end)` as a half-open `[start, end)` interval
/// of length exactly 7 days.
pub(super) fn compute_week_bounds(now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let midnight_today = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .expect("midnight UTC for a valid date is unambiguous");
    // Days since most recent Sunday — Sun=0, Mon=1, ..., Sat=6.
    let days_since_sunday = now.weekday().num_days_from_sunday() as i64;
    let current_sun_midnight = midnight_today - ChronoDuration::days(days_since_sunday);
    let week_end = current_sun_midnight;
    let week_start = week_end - ChronoDuration::days(7);
    (week_start, week_end)
}

/// `true` iff the coach_reports table already has a row for this week's
/// `week_start` (for any user — the cron fires for everyone at once, so a
/// single row is sufficient evidence that the batch ran).
pub(super) async fn already_fired_this_week(
    pool: &PgPool,
    week_start: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let (exists,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM coach_reports WHERE week_start = $1)")
            .bind(week_start)
            .fetch_one(pool)
            .await?;
    Ok(exists)
}

/// Fetch ids of all users who still have the coach enabled.
async fn list_enabled_users(pool: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE coach_enabled = TRUE")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Generate reports for every opted-in user with bounded concurrency.
/// Individual failures are logged but do not abort the batch.
pub(super) async fn run_batch(
    coach: &CoachService,
    pool: &PgPool,
    week_start: DateTime<Utc>,
    week_end: DateTime<Utc>,
) {
    let users = match list_enabled_users(pool).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!(error = %e, "coach: failed to list opted-in users");
            return;
        }
    };

    let total = users.len();
    tracing::info!(
        week_start = %week_start,
        week_end = %week_end,
        users = total,
        "coach: starting weekly batch",
    );

    let mut generated = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    let mut stream = stream::iter(users.into_iter().map(|user_id| async move {
        let result = coach.generate_for(user_id, week_start, week_end).await;
        (user_id, result)
    }))
    .buffer_unordered(BATCH_CONCURRENCY);

    while let Some((user_id, result)) = stream.next().await {
        match result {
            Ok(Some(_)) => generated += 1,
            Ok(None) => skipped += 1,
            Err(e) => {
                failed += 1;
                tracing::warn!(user_id = %user_id, error = %e, "coach: generate_for failed");
            }
        }
    }

    tracing::info!(
        total = total,
        generated = generated,
        skipped = skipped,
        failed = failed,
        "coach: weekly batch complete",
    );
}

/// Spawn the weekly coach scheduler. Ticks hourly; fires the batch when
/// the trigger moment hits and the week hasn't already been processed.
/// Cancellation via the shared shutdown token is checked between ticks.
pub fn spawn_weekly_task(
    coach: Arc<CoachService>,
    pool: PgPool,
    shutdown: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        // Skip the immediate first tick — `tokio::time::interval` fires once
        // at t=0 by default and we don't want the batch to run on router boot.
        interval.tick().await;
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("coach: scheduler shutting down");
                    break;
                }
                _ = interval.tick() => {
                    let now = Utc::now();
                    if !is_trigger_moment(now) {
                        continue;
                    }
                    let (week_start, week_end) = compute_week_bounds(now);
                    match already_fired_this_week(&pool, week_start).await {
                        Ok(true) => {
                            tracing::debug!(week_start = %week_start, "coach: already fired this week");
                        }
                        Ok(false) => {
                            run_batch(&coach, &pool, week_start, week_end).await;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "coach: idempotency check failed");
                        }
                    }
                }
            }
        }
    })
}

// ─────────────────────────────────────────────────────────────────────
// Tests — pure helpers (DB flow verified by regression / manual QA)
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn is_trigger_moment_fires_only_on_sunday_18_utc() {
        // Sunday 2026-04-19 18:30 UTC → fire.
        let sun_18 = Utc.with_ymd_and_hms(2026, 4, 19, 18, 30, 0).unwrap();
        assert!(is_trigger_moment(sun_18));

        // Sunday 17:59 UTC → not yet.
        let sun_17 = Utc.with_ymd_and_hms(2026, 4, 19, 17, 59, 0).unwrap();
        assert!(!is_trigger_moment(sun_17));

        // Sunday 19:00 UTC → past the hour window.
        let sun_19 = Utc.with_ymd_and_hms(2026, 4, 19, 19, 0, 0).unwrap();
        assert!(!is_trigger_moment(sun_19));

        // Saturday 18:00 UTC → wrong day.
        let sat_18 = Utc.with_ymd_and_hms(2026, 4, 18, 18, 0, 0).unwrap();
        assert!(!is_trigger_moment(sat_18));
    }

    #[test]
    fn compute_week_bounds_on_sunday_returns_prior_seven_days() {
        // Fire moment: Sunday 2026-04-19 18:00 UTC.
        let now = Utc.with_ymd_and_hms(2026, 4, 19, 18, 0, 0).unwrap();
        let (start, end) = compute_week_bounds(now);
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 4, 12, 0, 0, 0).unwrap());
        assert_eq!(end, Utc.with_ymd_and_hms(2026, 4, 19, 0, 0, 0).unwrap());
        assert_eq!(end - start, ChronoDuration::days(7));
    }

    #[test]
    fn compute_week_bounds_on_wednesday_rolls_back_to_prior_sunday() {
        // Mid-week manual run: Wednesday 2026-04-15 10:00 UTC should still
        // point at the most recent Sunday boundary.
        let now = Utc.with_ymd_and_hms(2026, 4, 15, 10, 0, 0).unwrap();
        let (start, end) = compute_week_bounds(now);
        assert_eq!(end, Utc.with_ymd_and_hms(2026, 4, 12, 0, 0, 0).unwrap());
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 4, 5, 0, 0, 0).unwrap());
    }

    #[test]
    fn compute_week_bounds_is_always_half_open_seven_day_window() {
        // Spot-check several weekdays to confirm the invariant.
        for day in 13..=19 {
            let now = Utc.with_ymd_and_hms(2026, 4, day, 12, 0, 0).unwrap();
            let (start, end) = compute_week_bounds(now);
            assert_eq!(end - start, ChronoDuration::days(7), "day {day}");
            assert_eq!(start.hour(), 0);
            assert_eq!(end.hour(), 0);
        }
    }
}
