//! TS-01 — UC-3 backlog sweeper.
//!
//! The two write hooks (`update_trade_notes` and the JNL-20 draft merge) fire
//! attribution when a note arrives. They cannot cover three cases: a spawn
//! lost to a process restart, a transient failure the trader never revisits,
//! and every note written before this feature existed. This task is the
//! recovery path for all three, and `judgment_status = 'not_attempted'` is its
//! work queue.
//!
//! # Why the queue is `not_attempted` and not `failed`
//!
//! [`attribute_note`](super::attribution::attribute_note) leaves the row
//! `not_attempted` when a call fails for a reason that says nothing about
//! whether the judgement is answerable (timeout, 429, 529, dropped
//! connection). That is what makes this sweeper a retry loop rather than a
//! one-shot. Only a rejected credential or a malformed request settles a row
//! as `failed`, because those will not resolve on their own.
//!
//! # Rate limiting
//!
//! One request per row, [`SWEEP_BATCH_SIZE`] rows per sweep, one sweep per
//! interval. A finished sweep therefore cannot approach the account-wide
//! 1,200 rpm ceiling. The bound that matters more is the failure case: if the
//! service is down, every row in the batch fails the same way, so the sweep
//! aborts after [`MAX_CONSECUTIVE_RETRYABLE_FAILURES`] rather than spending
//! the batch rediscovering one outage.
//!
//! Design and scope: `docs/plans/typesafe-jev-sniper-integration.md`.

// @anchor exchange:router:typesafe-sweep
// @tags api

use std::sync::Arc;
use std::time::Duration;

use sqlx::{FromRow, PgPool};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::attribution::{
    attribute_note, load_note_context, load_tag_candidates, Attribution, AttributionTrigger,
};
use super::client::SystemOneClient;
use super::types::TypeSafeError;

/// Rows per sweep.
///
/// Small on purpose: each row is one billed request, and a bounded batch means
/// a restart cannot leave thousands of in-flight judgements behind it.
pub const SWEEP_BATCH_SIZE: i64 = 25;

/// Consecutive retryable failures that abort the sweep until the next interval.
///
/// Three identical transient failures in a row is evidence about the service,
/// not about the rows. Continuing would burn the rest of the batch proving the
/// same thing.
pub const MAX_CONSECUTIVE_RETRYABLE_FAILURES: usize = 3;

/// Default gap between sweeps.
pub const DEFAULT_INTERVAL_SECS: u64 = 900;

/// Bounds on the configurable interval. The floor keeps a misconfigured value
/// from turning the sweeper into a hot loop; the ceiling keeps it from
/// effectively never running.
pub const MIN_INTERVAL_SECS: u64 = 60;
pub const MAX_INTERVAL_SECS: u64 = 86_400;

/// Environment override for the sweep interval.
pub const INTERVAL_ENV: &str = "TYPESAFE_SWEEP_INTERVAL_SECS";

/// One row's worth of work: enough to load its context and its vocabulary.
#[derive(Debug, Clone, Copy, FromRow)]
struct PendingRow {
    id: Uuid,
    user_id: Uuid,
}

/// Counts from one sweep, logged and returned for tests and operators.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SweepSummary {
    pub picked: usize,
    pub attributed: usize,
    pub no_note: usize,
    pub deferred: usize,
    pub failed: usize,
    /// Rows never attempted because the sweep aborted early. They stay queued.
    pub abandoned: usize,
    /// The sweep stopped after consecutive transient failures.
    pub stopped_early: bool,
}

/// Clamps the configured interval into a usable range.
///
/// Pure so the bounds are testable without a clock or a task.
pub fn sweep_interval(configured_secs: Option<u64>) -> Duration {
    let secs = configured_secs
        .unwrap_or(DEFAULT_INTERVAL_SECS)
        .clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS);
    Duration::from_secs(secs)
}

/// True once consecutive transient failures justify abandoning the sweep.
///
/// Pure: the abort decision is the part worth testing, and it needs no pool.
pub fn should_abort_sweep(consecutive_retryable_failures: usize) -> bool {
    consecutive_retryable_failures >= MAX_CONSECUTIVE_RETRYABLE_FAILURES
}

/// Rows waiting for attribution, newest first.
///
/// Newest first because the backlog is drained over many sweeps, and the
/// recent trades are the ones the coach and Dignitas read next. Oldest first
/// would leave new notes unattributed until an arbitrarily large backlog
/// cleared.
///
/// `notes <> ''` does not exclude whitespace-only notes; those reach
/// [`attribute_note`], resolve to `no_note`, and leave the queue from there.
async fn load_pending(pool: &PgPool, limit: i64) -> Result<Vec<PendingRow>, sqlx::Error> {
    let limit = limit.clamp(1, SWEEP_BATCH_SIZE);
    sqlx::query_as::<_, PendingRow>(
        "SELECT id, user_id FROM journal_trades \
         WHERE judgment_status = 'not_attempted' AND notes IS NOT NULL AND notes <> '' \
         ORDER BY closed_at DESC \
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Attributes up to [`SWEEP_BATCH_SIZE`] queued rows.
///
/// Reuses [`attribute_note`], so the sweeper writes through exactly the same
/// transaction paths as the real-time hooks. A second copy of that logic could
/// drift from the invariants the CHECK constraints enforce.
pub async fn run_sweep(
    pool: &PgPool,
    client: &dyn SystemOneClient,
    limit: i64,
) -> Result<SweepSummary, sqlx::Error> {
    let pending = load_pending(pool, limit).await?;
    let mut summary = SweepSummary {
        picked: pending.len(),
        ..SweepSummary::default()
    };
    let mut consecutive_retryable = 0usize;

    for (index, row) in pending.iter().enumerate() {
        let context = match load_note_context(pool, row.id, row.user_id).await? {
            Some(context) => context,
            // Deleted between the pick and the load. Nothing to do.
            None => continue,
        };

        // Per row rather than cached: one indexed aggregate over the trader's
        // own trades, and a sweep is 25 rows at most every few minutes.
        let candidates = load_tag_candidates(pool, row.user_id).await?;

        let outcome = attribute_note(
            pool,
            client,
            &context,
            &candidates,
            AttributionTrigger::Sweep,
        )
        .await?;

        match outcome {
            Attribution::Attributed { .. } => {
                summary.attributed += 1;
                consecutive_retryable = 0;
            }
            Attribution::NoNote => {
                summary.no_note += 1;
                consecutive_retryable = 0;
            }
            Attribution::Failed(ref err) if err_is_retryable(err) => {
                summary.deferred += 1;
                consecutive_retryable += 1;
                if should_abort_sweep(consecutive_retryable) {
                    summary.stopped_early = true;
                    summary.abandoned = pending.len() - index - 1;
                    tracing::warn!(
                        deferred = summary.deferred,
                        abandoned = summary.abandoned,
                        error = %err,
                        "typesafe: sweep aborted after consecutive transient failures",
                    );
                    break;
                }
            }
            Attribution::Failed(_) => {
                summary.failed += 1;
                consecutive_retryable = 0;
            }
        }
    }

    if summary.picked > 0 {
        tracing::info!(
            picked = summary.picked,
            attributed = summary.attributed,
            no_note = summary.no_note,
            deferred = summary.deferred,
            failed = summary.failed,
            abandoned = summary.abandoned,
            stopped_early = summary.stopped_early,
            "typesafe: sweep complete",
        );
    }

    Ok(summary)
}

/// Narrow projection of the error classification the sweeper branches on.
///
/// `attribute_note` already decided this and encoded it in the row's status;
/// the sweeper needs the same answer to decide whether to keep going. Sharing
/// `is_retryable` keeps the two from disagreeing.
fn err_is_retryable(err: &TypeSafeError) -> bool {
    err.is_retryable()
}

/// Spawn the backlog sweeper.
///
/// The first tick is consumed before the loop, so a restart does not start a
/// sweep while the rest of the router is still coming up. A restart therefore
/// costs at most one interval of delay, and nothing is lost because the queue
/// is durable.
///
/// Callers must not spawn this when the integration is disabled. `None` on the
/// client is how `TYPESAFE_ENABLED=false` and a missing credential are both
/// represented, so the strongest form of respecting the flag is not creating
/// the task at all.
pub fn spawn_sweep_task(
    client: Arc<dyn SystemOneClient>,
    pool: PgPool,
    interval: Duration,
    shutdown: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // Consume the immediate first tick: `interval` fires at t=0.
        ticker.tick().await;

        tracing::info!(
            interval_secs = interval.as_secs(),
            batch_size = SWEEP_BATCH_SIZE,
            "typesafe: backlog sweeper started",
        );

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("typesafe: backlog sweeper shutting down");
                    break;
                }
                _ = ticker.tick() => {
                    match run_sweep(&pool, client.as_ref(), SWEEP_BATCH_SIZE).await {
                        Ok(summary) if summary.picked == 0 => {
                            tracing::debug!("typesafe: sweep found no pending rows");
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!(error = %e, "typesafe: sweep failed");
                        }
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_falls_back_to_the_default_when_unset() {
        assert_eq!(
            sweep_interval(None),
            Duration::from_secs(DEFAULT_INTERVAL_SECS)
        );
        assert_eq!(sweep_interval(None), Duration::from_secs(900));
    }

    #[test]
    fn interval_is_clamped_at_both_ends() {
        // A too-small value would make the sweeper a hot loop against a
        // metered API, so the floor is a correctness bound, not a preference.
        assert_eq!(sweep_interval(Some(0)), Duration::from_secs(MIN_INTERVAL_SECS));
        assert_eq!(sweep_interval(Some(1)), Duration::from_secs(MIN_INTERVAL_SECS));
        assert_eq!(
            sweep_interval(Some(59)),
            Duration::from_secs(MIN_INTERVAL_SECS)
        );
        assert_eq!(sweep_interval(Some(3_600)), Duration::from_secs(3_600));
        assert_eq!(
            sweep_interval(Some(u64::MAX)),
            Duration::from_secs(MAX_INTERVAL_SECS)
        );
    }

    #[test]
    fn sweep_aborts_only_after_consecutive_transient_failures() {
        assert!(!should_abort_sweep(0));
        assert!(!should_abort_sweep(1));
        assert!(!should_abort_sweep(MAX_CONSECUTIVE_RETRYABLE_FAILURES - 1));
        assert!(should_abort_sweep(MAX_CONSECUTIVE_RETRYABLE_FAILURES));
        assert!(should_abort_sweep(MAX_CONSECUTIVE_RETRYABLE_FAILURES + 10));
    }

    #[test]
    fn the_abort_threshold_is_small_enough_to_protect_the_budget() {
        // The point of the abort is to spend a few requests learning about an
        // outage instead of a whole batch. If the batch ever grew below the
        // threshold the guard would be dead code.
        assert!(
            (MAX_CONSECUTIVE_RETRYABLE_FAILURES as i64) < SWEEP_BATCH_SIZE,
            "an abort threshold at or above the batch size can never fire"
        );
    }

    #[test]
    fn the_transient_classification_matches_what_attribute_note_deferred_on() {
        // The sweeper must branch on the same predicate that decided the row's
        // status. These are the four transient classes and the three settled
        // ones, asserted so a change to either side is visible here.
        for err in [
            TypeSafeError::Timeout,
            TypeSafeError::RateLimit { retry_after: None },
            TypeSafeError::Overloaded { retry_after: None },
            TypeSafeError::Network("connection reset".into()),
        ] {
            assert!(err_is_retryable(&err), "{err} must keep the row queued");
        }
        for err in [
            TypeSafeError::Unauthorized,
            TypeSafeError::BadRequest("bad field".into()),
            TypeSafeError::Parse("bad shape".into()),
        ] {
            assert!(!err_is_retryable(&err), "{err} must settle the row");
        }
    }

    #[test]
    fn one_sweep_cannot_approach_the_account_request_ceiling() {
        // 1,200 requests per minute is the vendor's account-wide ceiling, and
        // it is shared by every user. Even a full batch of failures stays far
        // under it, and the interval spaces sweeps out.
        const VENDOR_REQUESTS_PER_MINUTE: u64 = 1_200;
        assert!(
            (SWEEP_BATCH_SIZE as u64) < VENDOR_REQUESTS_PER_MINUTE,
            "a single sweep must not be able to exhaust the shared limit"
        );
        let sweeps_per_minute = 60 / sweep_interval(None).as_secs().max(1);
        assert!(
            (SWEEP_BATCH_SIZE as u64) * sweeps_per_minute < VENDOR_REQUESTS_PER_MINUTE,
            "sustained sweeps must stay under the shared limit"
        );
    }

    #[test]
    fn default_interval_leaves_a_full_batch_well_inside_the_ceiling() {
        // 25 rows every 15 minutes sustained: 100 requests per hour.
        let per_hour = (SWEEP_BATCH_SIZE as u64) * (3_600 / DEFAULT_INTERVAL_SECS);
        assert!(
            per_hour < 200,
            "sustained sweep load of {per_hour}/hour is higher than intended"
        );
    }

    #[test]
    fn summary_defaults_to_an_empty_sweep() {
        let summary = SweepSummary::default();
        assert_eq!(summary.picked, 0);
        assert!(!summary.stopped_early);
        // `abandoned` is computed from the batch, never accumulated.
        assert_eq!(summary.abandoned, 0);
    }
}
