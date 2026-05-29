//! Dignitas daily scheduler (ENG-01a, T6).
//!
//! `spawn_daily_task` wires a long-running `tokio::spawn` that wakes every
//! hour, checks `is_trigger_moment` (UTC 00:xx) + `already_fired_today`
//! (DB idempotency guard), then runs the batch when both gates pass.
//!
//! Users are processed in sequential chunks of BATCH_SIZE with a
//! `tokio::yield_now()` between chunks so other tasks remain responsive.
//! Individual failures are logged and do not abort the batch.

// @anchor exchange:router:schedule
// @tags api

use std::time::Duration;

use chrono::{DateTime, NaiveDate, Timelike, Utc};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::snapshot::take_daily_snapshot;
use super::streak;

/// Hour of day (UTC) in which the daily batch fires.
/// D3: target is 00:30; any tick in hour 0 satisfies the condition and the
/// idempotency guard prevents double-firing within the same calendar day.
const TRIGGER_HOUR_UTC: u32 = 0;

/// Poll interval. Waking every 5 minutes to ensure we hit the trigger hour despite drift.
const POLL_INTERVAL: Duration = Duration::from_secs(300);

/// Users per sequential processing chunk. `tokio::yield_now()` is called
/// after each chunk so other async tasks remain responsive.
const BATCH_SIZE: usize = 500;

/// Returns true when `now` falls within the trigger hour.
pub(super) fn is_trigger_moment(now: DateTime<Utc>) -> bool {
    now.hour() == TRIGGER_HOUR_UTC
}

/// Returns true when the scheduler should consult the idempotency guard this
/// tick. The first iteration after boot always attempts so a missed midnight
/// batch can be recovered regardless of the current hour. Subsequent ticks
/// only attempt inside the trigger window.
pub(super) fn should_attempt_batch(first_iteration: bool, in_trigger_window: bool) -> bool {
    first_iteration || in_trigger_window
}

/// Returns true iff `dignitas_history` already contains a row for `today`
/// (any user) — sufficient evidence the daily batch has already run.
pub(super) async fn already_fired_today(
    pool: &PgPool,
    today: NaiveDate,
) -> Result<bool, sqlx::Error> {
    let (exists,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM dignitas_history WHERE date = $1)")
            .bind(today)
            .fetch_one(pool)
            .await?;
    Ok(exists)
}

async fn list_all_users(pool: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM users")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Generate snapshots for all users in sequential BATCH_SIZE chunks.
/// Individual failures are logged and do not abort the batch.
pub(super) async fn run_batch(pool: &PgPool, date: NaiveDate) {
    let users = match list_all_users(pool).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!(error = %e, "dignitas: failed to list users for daily batch");
            return;
        }
    };

    let total = users.len();
    tracing::info!(date = %date, users = total, "dignitas: starting daily batch");

    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for chunk in users.chunks(BATCH_SIZE) {
        for &user_id in chunk {
            match take_daily_snapshot(pool, user_id, date).await {
                Ok(_) => succeeded += 1,
                Err(e) => {
                    failed += 1;
                    tracing::warn!(user_id = %user_id, error = %e, "dignitas: snapshot failed");
                }
            }
            // ENG-01c: streak tick follows snapshot; independent failure mode.
            if let Err(e) = streak::apply_daily_tick(pool, user_id).await {
                tracing::warn!(user_id = %user_id, error = %e, "dignitas: streak tick failed");
            }
        }
        tokio::task::yield_now().await;
    }

    tracing::info!(
        date = %date,
        total = total,
        succeeded = succeeded,
        failed = failed,
        "dignitas: daily batch complete",
    );
}

/// Spawn the daily dignitas scheduler.
/// Wakes every 5 minutes. On boot, attempts the batch immediately so a
/// midnight that was missed (e.g. router restart at 06:00) is recovered the
/// same day. Subsequent ticks only run inside the trigger hour. The
/// `already_fired_today` idempotency guard prevents double-firing.
pub fn spawn_daily_task(pool: PgPool, shutdown: CancellationToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        let mut first_iteration = true;
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("dignitas: daily scheduler shutting down");
                    break;
                }
                _ = interval.tick() => {
                    let now = Utc::now();
                    let in_trigger_window = is_trigger_moment(now);
                    if !should_attempt_batch(first_iteration, in_trigger_window) {
                        continue;
                    }
                    first_iteration = false;

                    let today = now.date_naive();
                    match already_fired_today(&pool, today).await {
                        Ok(true) => {
                            tracing::debug!(date = %today, "dignitas: already fired today");
                        }
                        Ok(false) => {
                            run_batch(&pool, today).await;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "dignitas: idempotency check failed");
                        }
                    }
                }
            }
        }
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn is_trigger_moment_fires_only_in_midnight_hour() {
        let midnight = Utc.with_ymd_and_hms(2026, 4, 21, 0, 0, 0).unwrap();
        assert!(is_trigger_moment(midnight));

        let half_past = Utc.with_ymd_and_hms(2026, 4, 21, 0, 30, 0).unwrap();
        assert!(is_trigger_moment(half_past));

        let near_one = Utc.with_ymd_and_hms(2026, 4, 21, 0, 59, 0).unwrap();
        assert!(is_trigger_moment(near_one));

        let one_am = Utc.with_ymd_and_hms(2026, 4, 21, 1, 0, 0).unwrap();
        assert!(!is_trigger_moment(one_am));

        let end_of_prev = Utc.with_ymd_and_hms(2026, 4, 20, 23, 59, 0).unwrap();
        assert!(!is_trigger_moment(end_of_prev));
    }

    /// Boot-recovery contract: on the first tick after process start the
    /// scheduler must consult the idempotency guard regardless of the hour,
    /// so a missed midnight batch can be caught up the same day.
    #[test]
    fn first_iteration_attempts_batch_outside_trigger_hour() {
        // 06:00 UTC, well after midnight. Without boot recovery the scheduler
        // would skip until tomorrow 00:00.
        assert!(
            should_attempt_batch(true, false),
            "first iteration must attempt the batch even outside the trigger hour"
        );
    }

    /// After boot recovery has run once, only the trigger hour should cause
    /// further attempts — otherwise we would re-query the guard every tick.
    #[test]
    fn subsequent_iterations_only_attempt_in_trigger_hour() {
        assert!(
            !should_attempt_batch(false, false),
            "non-first iteration outside trigger hour must skip"
        );
        assert!(
            should_attempt_batch(false, true),
            "non-first iteration inside trigger hour must attempt"
        );
    }
}
