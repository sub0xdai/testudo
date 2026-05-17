//! ENG-01c — Dignitas streak maintenance.
//!
//! Tracks consecutive days without a `Concerning` coach-report flag.
//! Reset is silent (no notifications) and the previous `days_clean` is
//! preserved as `longest_ever` on every reset (trophy of best run).
//!
//! Hooked into the ENG-01a daily scheduler (`schedule::run_batch`).

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

/// Streak row. One per user. Matches `dignitas_streak` table columns 1:1
/// except for `updated_at` (managed by the DB).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct StreakRow {
    pub user_id: Uuid,
    pub days_clean: i64,
    pub longest_ever: i64,
    pub last_concerning_flag_at: Option<DateTime<Utc>>,
    pub streak_started_at: Option<DateTime<Utc>>,
}

impl StreakRow {
    fn empty(user_id: Uuid) -> Self {
        Self {
            user_id,
            days_clean: 0,
            longest_ever: 0,
            last_concerning_flag_at: None,
            streak_started_at: None,
        }
    }
}

/// Pure decision function — computes the next streak state given the
/// current state and whether a new Concerning flag was found since
/// `current.last_concerning_flag_at`.
///
/// `new_concerning_at`: generated_at of the most recent Concerning-flagged
/// coach report with `generated_at > current.last_concerning_flag_at`.
/// `None` when no new Concerning flag exists.
///
/// Reset semantics (FR-2, FR-3):
/// - `days_clean` -> 0
/// - `longest_ever` -> MAX(old longest_ever, old days_clean)
/// - `last_concerning_flag_at` -> the flag's generated_at
/// - `streak_started_at` -> `now` (the reset moment; next day's tick makes it day 1)
///
/// No-flag semantics:
/// - `days_clean` += 1
/// - `streak_started_at` stays; only set if previously None
pub fn next_state(
    current: &StreakRow,
    new_concerning_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> StreakRow {
    match new_concerning_at {
        Some(flagged_at) => StreakRow {
            user_id: current.user_id,
            days_clean: 0,
            longest_ever: current.longest_ever.max(current.days_clean),
            last_concerning_flag_at: Some(flagged_at),
            streak_started_at: Some(now),
        },
        None => StreakRow {
            user_id: current.user_id,
            days_clean: current.days_clean + 1,
            longest_ever: current.longest_ever,
            last_concerning_flag_at: current.last_concerning_flag_at,
            streak_started_at: current.streak_started_at.or(Some(now)),
        },
    }
}

/// Public API for the `/api/v1/dignitas/me` extension and public profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct StreakWire {
    pub days_clean: i64,
    pub longest_ever: i64,
}

impl From<&StreakRow> for StreakWire {
    fn from(row: &StreakRow) -> Self {
        Self {
            days_clean: row.days_clean,
            longest_ever: row.longest_ever,
        }
    }
}

// ─── DB wrappers ─────────────────────────────────────────────────────────────

/// Fetch the current streak row, inserting a zeroed row when missing.
pub async fn load_or_init(pool: &PgPool, user_id: Uuid) -> Result<StreakRow, sqlx::Error> {
    let existing: Option<StreakRow> = sqlx::query_as(
        "SELECT user_id, days_clean, longest_ever, last_concerning_flag_at, \
         streak_started_at FROM dignitas_streak WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = existing {
        return Ok(row);
    }

    sqlx::query(
        "INSERT INTO dignitas_streak (user_id) VALUES ($1) \
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(StreakRow::empty(user_id))
}

/// Fetch `coach_reports.generated_at` of the most recent Concerning-flagged
/// report for `user_id` with `generated_at > since` (idempotency gate).
///
/// Concerning is stored inside the JSONB `digest_json.flagged_patterns[*]`
/// payload; we use a JSONB path query to avoid pulling the full digest.
pub async fn latest_concerning_flag_after(
    pool: &PgPool,
    user_id: Uuid,
    since: Option<DateTime<Utc>>,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    let cutoff = since.unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).expect("epoch"));

    let row: Option<(DateTime<Utc>,)> = sqlx::query_as(
        "SELECT generated_at FROM coach_reports \
         WHERE user_id = $1 \
           AND generated_at > $2 \
           AND jsonb_path_exists( \
                 digest_json, \
                 '$.flagged_patterns[*] ? (@.severity == \"Concerning\")' \
               ) \
         ORDER BY generated_at DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(cutoff)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(ts,)| ts))
}

/// Persist the new streak state. UPSERT on user_id.
pub async fn save(pool: &PgPool, row: &StreakRow) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO dignitas_streak \
           (user_id, days_clean, longest_ever, last_concerning_flag_at, streak_started_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, NOW()) \
         ON CONFLICT (user_id) DO UPDATE SET \
           days_clean = EXCLUDED.days_clean, \
           longest_ever = EXCLUDED.longest_ever, \
           last_concerning_flag_at = EXCLUDED.last_concerning_flag_at, \
           streak_started_at = EXCLUDED.streak_started_at, \
           updated_at = NOW()",
    )
    .bind(row.user_id)
    .bind(row.days_clean)
    .bind(row.longest_ever)
    .bind(row.last_concerning_flag_at)
    .bind(row.streak_started_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Apply one daily tick: load current state, look for a new Concerning flag
/// since last one seen, compute next state, persist. Idempotent per calendar
/// day via the scheduler's `already_fired_today` gate + the per-flag
/// `generated_at > last_concerning_flag_at` gate.
pub async fn apply_daily_tick(pool: &PgPool, user_id: Uuid) -> Result<StreakRow, sqlx::Error> {
    let current = load_or_init(pool, user_id).await?;
    let new_flag =
        latest_concerning_flag_after(pool, user_id, current.last_concerning_flag_at).await?;
    let next = next_state(&current, new_flag, Utc::now());
    save(pool, &next).await?;
    Ok(next)
}

/// Fetch the current streak for `/api/v1/dignitas/me`. Returns `None` when
/// the user has never had the streak row created (no snapshot has run yet).
pub async fn get_current(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<StreakRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT user_id, days_clean, longest_ever, last_concerning_flag_at, \
         streak_started_at FROM dignitas_streak WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn tick(user_id: Uuid, days_clean: i64, longest: i64) -> StreakRow {
        StreakRow {
            user_id,
            days_clean,
            longest_ever: longest,
            last_concerning_flag_at: None,
            streak_started_at: None,
        }
    }

    fn ts(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
    }

    #[test]
    fn fresh_user_with_no_flag_increments_to_one_and_sets_started_at() {
        let uid = Uuid::new_v4();
        let now = ts(2026, 4, 22);
        let next = next_state(&StreakRow::empty(uid), None, now);

        assert_eq!(next.days_clean, 1);
        assert_eq!(next.longest_ever, 0);
        assert_eq!(next.streak_started_at, Some(now));
        assert_eq!(next.last_concerning_flag_at, None);
    }

    #[test]
    fn no_flag_tick_increments_days_clean_without_touching_longest() {
        let uid = Uuid::new_v4();
        let current = tick(uid, 7, 92);
        let next = next_state(&current, None, ts(2026, 4, 22));

        assert_eq!(next.days_clean, 8);
        assert_eq!(next.longest_ever, 92);
    }

    #[test]
    fn concerning_flag_on_day_eight_resets_and_records_longest() {
        let uid = Uuid::new_v4();
        let current = tick(uid, 8, 3);
        let flag = ts(2026, 4, 22);
        let now = ts(2026, 4, 22);

        let next = next_state(&current, Some(flag), now);

        assert_eq!(next.days_clean, 0);
        assert_eq!(next.longest_ever, 8, "old days_clean beats old longest_ever");
        assert_eq!(next.last_concerning_flag_at, Some(flag));
        assert_eq!(next.streak_started_at, Some(now));
    }

    #[test]
    fn concerning_flag_preserves_longest_when_already_larger() {
        let uid = Uuid::new_v4();
        let current = tick(uid, 5, 200);
        let next = next_state(&current, Some(ts(2026, 4, 22)), ts(2026, 4, 22));

        assert_eq!(next.days_clean, 0);
        assert_eq!(next.longest_ever, 200, "longest trophy survives a short-streak break");
    }

    #[test]
    fn non_concerning_tick_does_not_overwrite_existing_started_at() {
        let uid = Uuid::new_v4();
        let anchor = ts(2026, 4, 1);
        let current = StreakRow {
            user_id: uid,
            days_clean: 20,
            longest_ever: 20,
            last_concerning_flag_at: None,
            streak_started_at: Some(anchor),
        };
        let next = next_state(&current, None, ts(2026, 4, 22));

        assert_eq!(next.streak_started_at, Some(anchor), "started_at anchors to first tick");
        assert_eq!(next.days_clean, 21);
    }

    #[test]
    fn wire_conversion_exposes_only_two_fields() {
        let uid = Uuid::new_v4();
        let row = StreakRow {
            user_id: uid,
            days_clean: 47,
            longest_ever: 92,
            last_concerning_flag_at: Some(ts(2026, 3, 6)),
            streak_started_at: Some(ts(2026, 3, 7)),
        };
        let wire: StreakWire = (&row).into();
        assert_eq!(wire.days_clean, 47);
        assert_eq!(wire.longest_ever, 92);
    }
}

// ─── Integration tests (require live Postgres) ───────────────────────────────
//
// Run with:
// ```bash
// DATABASE_URL=postgres://user:pass@localhost/testudo \
//     cargo test -p router streak_integration -- --ignored
// ```
#[cfg(test)]
mod streak_integration {
    use super::*;
    use chrono::Duration;
    use sqlx::postgres::PgPoolOptions;

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

    /// Insert a coach_report for `user_id` containing a single flagged_pattern
    /// with the given severity. `generated_at` controls the timestamp so tests
    /// can simulate flags older or newer than the streak's last-seen cutoff.
    async fn insert_coach_report(
        pool: &PgPool,
        user_id: Uuid,
        severity: &str,
        generated_at: DateTime<Utc>,
    ) {
        let digest = serde_json::json!({
            "flagged_patterns": [{
                "pattern": "SizingDrift",
                "severity": severity,
                "evidence": [],
                "metrics": {}
            }],
            "user_id": user_id,
            "flagged_trades": []
        });
        // week_start/end are arbitrary but must satisfy the UNIQUE(user_id, week_start).
        let week_start = generated_at - Duration::days(7);
        sqlx::query(
            "INSERT INTO coach_reports \
               (user_id, week_start, week_end, generated_at, model_used, digest_json) \
             VALUES ($1, $2, $3, $4, 'test', $5)",
        )
        .bind(user_id)
        .bind(week_start)
        .bind(generated_at)
        .bind(generated_at)
        .bind(digest)
        .execute(pool)
        .await
        .expect("insert coach_report");
    }

    async fn cleanup(pool: &PgPool, user_id: Uuid) {
        let _ = sqlx::query("DELETE FROM dignitas_streak WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM coach_reports WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    /// AC: "Concerning flag on day 8 of a streak resets days_clean = 0 and
    /// sets longest_ever = 8." Then next day increments to 1.
    #[tokio::test]
    #[ignore]
    async fn concerning_flag_resets_streak_and_trophies_longest() {
        let pool = pool().await;
        let user_id = make_user(&pool).await;

        // Prime the row as if 8 days had already accrued.
        sqlx::query(
            "INSERT INTO dignitas_streak \
               (user_id, days_clean, longest_ever, streak_started_at) \
             VALUES ($1, 8, 3, NOW() - INTERVAL '8 days')",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("seed streak row");

        // Insert a Concerning flag dated now.
        insert_coach_report(&pool, user_id, "Concerning", Utc::now()).await;

        let result = apply_daily_tick(&pool, user_id).await.expect("tick");

        assert_eq!(result.days_clean, 0, "Concerning flag resets days_clean");
        assert_eq!(result.longest_ever, 8, "longest_ever trophies the broken streak");
        assert!(result.last_concerning_flag_at.is_some(), "last flag recorded");

        cleanup(&pool, user_id).await;
    }

    /// AC: "Two Concerning flags in the same day produce one reset, not two."
    /// The idempotency gate is `generated_at > last_concerning_flag_at`.
    #[tokio::test]
    #[ignore]
    async fn idempotent_on_repeated_same_day_concerning() {
        let pool = pool().await;
        let user_id = make_user(&pool).await;

        // Seed: 5 days clean, 0 longest.
        sqlx::query(
            "INSERT INTO dignitas_streak \
               (user_id, days_clean, longest_ever, streak_started_at) \
             VALUES ($1, 5, 0, NOW() - INTERVAL '5 days')",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("seed streak row");

        let flag_at = Utc::now();
        insert_coach_report(&pool, user_id, "Concerning", flag_at).await;

        // First tick: resets to 0, longest_ever = 5.
        let first = apply_daily_tick(&pool, user_id).await.expect("first tick");
        assert_eq!(first.days_clean, 0);
        assert_eq!(first.longest_ever, 5);
        let first_flag_at = first.last_concerning_flag_at.expect("flag set");

        // Same-day second tick with NO new coach_report → should increment,
        // NOT double-reset. (This is the idempotency contract: scheduler may
        // tick at most once per day anyway, but the gate must hold in isolation.)
        let second = apply_daily_tick(&pool, user_id).await.expect("second tick");
        assert_eq!(second.days_clean, 1, "subsequent tick increments");
        assert_eq!(second.longest_ever, 5, "longest trophy unchanged");
        assert_eq!(second.last_concerning_flag_at, Some(first_flag_at));

        cleanup(&pool, user_id).await;
    }

    /// AC: "Info/Notable flags do not reset the streak."
    /// Only Concerning resets — Info and Notable are lower severities and
    /// count as normal coach output, not behaviour worthy of breaking a streak.
    #[tokio::test]
    #[ignore]
    async fn non_concerning_flags_do_not_reset() {
        let pool = pool().await;
        let user_id = make_user(&pool).await;

        sqlx::query(
            "INSERT INTO dignitas_streak \
               (user_id, days_clean, longest_ever, streak_started_at) \
             VALUES ($1, 10, 20, NOW() - INTERVAL '10 days')",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("seed streak row");

        insert_coach_report(&pool, user_id, "Notable", Utc::now()).await;
        insert_coach_report(
            &pool,
            user_id,
            "Info",
            Utc::now() - Duration::minutes(1),
        )
        .await;

        let result = apply_daily_tick(&pool, user_id).await.expect("tick");
        assert_eq!(result.days_clean, 11, "Notable + Info increment, do not reset");
        assert_eq!(result.longest_ever, 20);

        cleanup(&pool, user_id).await;
    }
}
