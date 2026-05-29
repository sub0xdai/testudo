//! Coach service orchestration.
//!
//! `generate_for` is the full pipeline: digest → narrate → validate → persist.
//! Narrator and validator failures fall through to a stats-only row with
//! `model_used = "unavailable"` and `narrative_sections = NULL` so the UI
//! can render "● coach unavailable this week" without special-casing a
//! missing report.
//!
//! Read helpers (`latest_for`, `archive_for`), preference mutations
//! (`get_preference`, `set_preference`), and banner state flips
//! (`mark_viewed`, `dismiss_banner`) back the HTTP endpoints wired in T8.

// @anchor exchange:router:service
// @tags api

use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use super::digest::build_digest;
use super::narrator::Narrator;
use super::types::{
    CoachConfig, CoachDigest, NarratedReport, NarrativeSection, NarratorError, StoredCoachReport,
};
use super::validator::validate;

/// Sentinel model name persisted when either the narrator or the citation
/// validator fails. The frontend uses this to render the stats-only fallback.
const FALLBACK_MODEL: &str = "unavailable";

/// Result alias for CoachService public surface.
pub type CoachResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Orchestrates digest composition, narration, validation, persistence,
/// and per-user read/preference flows for the AI trade coach.
pub struct CoachService {
    pool: PgPool,
    analytics_pool: PgPool,
    narrator: Arc<dyn Narrator>,
    config: CoachConfig,
}

impl CoachService {
    pub fn new(
        pool: PgPool,
        analytics_pool: PgPool,
        narrator: Arc<dyn Narrator>,
        config: CoachConfig,
    ) -> Self {
        Self {
            pool,
            analytics_pool,
            narrator,
            config,
        }
    }

    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub(crate) fn analytics_pool(&self) -> &PgPool {
        &self.analytics_pool
    }

    pub(crate) fn narrator(&self) -> &Arc<dyn Narrator> {
        &self.narrator
    }

    pub(crate) fn config(&self) -> &CoachConfig {
        &self.config
    }

    /// Latest report for `user_id` plus a `has_new_indicator` flag derived
    /// from `users.coach_banner_last_viewed_at`.
    pub async fn latest_for(
        &self,
        user_id: Uuid,
    ) -> CoachResult<Option<(StoredCoachReport, bool)>> {
        let row = sqlx::query_as::<_, CoachReportRow>(SELECT_REPORT_COLUMNS_LATEST)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };

        let last_viewed: Option<(Option<DateTime<Utc>>,)> =
            sqlx::query_as("SELECT coach_banner_last_viewed_at FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        let has_new = match last_viewed.and_then(|row| row.0) {
            Some(viewed_at) => row.generated_at > viewed_at,
            None => true,
        };

        let stored = row.try_into_stored()?;
        Ok(Some((stored, has_new)))
    }

    /// Paginated archive ordered newest-first.
    pub async fn archive_for(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> CoachResult<Vec<StoredCoachReport>> {
        let rows = sqlx::query_as::<_, CoachReportRow>(SELECT_REPORT_COLUMNS_ARCHIVE)
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|r| r.try_into_stored())
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn get_preference(&self, user_id: Uuid) -> CoachResult<bool> {
        let pref: (bool,) = sqlx::query_as("SELECT coach_enabled FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(pref.0)
    }

    pub async fn set_preference(&self, user_id: Uuid, enabled: bool) -> CoachResult<()> {
        sqlx::query("UPDATE users SET coach_enabled = $2 WHERE id = $1")
            .bind(user_id)
            .bind(enabled)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_viewed(&self, user_id: Uuid) -> CoachResult<()> {
        sqlx::query("UPDATE users SET coach_banner_last_viewed_at = NOW() WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn dismiss_banner(&self, user_id: Uuid, report_id: Uuid) -> CoachResult<()> {
        sqlx::query(
            "UPDATE coach_reports SET banner_dismissed_at = NOW() \
             WHERE id = $1 AND user_id = $2",
        )
        .bind(report_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Build digest → narrate → validate → persist. Returns `Ok(None)` when
    /// a skip rule fires (opt-out, lifetime-trades, week-trades, no flags).
    pub async fn generate_for(
        &self,
        user_id: Uuid,
        week_start: DateTime<Utc>,
        week_end: DateTime<Utc>,
    ) -> CoachResult<Option<StoredCoachReport>> {
        let Some(digest) = build_digest(
            &self.pool,
            &self.analytics_pool,
            user_id,
            week_start,
            week_end,
            &self.config,
        )
        .await?
        else {
            return Ok(None);
        };

        let narrator_result = self.narrator.narrate(&digest).await;
        let prepared = prepare_report(&digest, narrator_result);
        let stored = self.persist(&digest, prepared).await?;
        Ok(Some(stored))
    }

    async fn persist(
        &self,
        digest: &CoachDigest,
        prepared: PreparedReport,
    ) -> CoachResult<StoredCoachReport> {
        let digest_json = serde_json::to_value(digest)?;
        let sections_json = prepared
            .narrative_sections
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;

        let row = sqlx::query_as::<_, CoachReportRow>(
            "INSERT INTO coach_reports ( \
                user_id, week_start, week_end, model_used, headline, \
                narrative_sections_json, digest_json, cache_hit_ratio \
             ) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (user_id, week_start) DO NOTHING \
             RETURNING id, user_id, week_start, week_end, generated_at, model_used, \
                       headline, narrative_sections_json, digest_json, \
                       cache_hit_ratio, banner_dismissed_at",
        )
        .bind(digest.user_id)
        .bind(digest.week_start)
        .bind(digest.week_end)
        .bind(&prepared.model_used)
        .bind(prepared.headline.as_deref())
        .bind(sections_json.as_ref())
        .bind(&digest_json)
        .bind(prepared.cache_hit_ratio)
        .fetch_optional(&self.pool)
        .await?;

        let row = match row {
            Some(r) => r,
            None => {
                // Idempotent re-run — fetch the existing row.
                sqlx::query_as::<_, CoachReportRow>(
                    "SELECT id, user_id, week_start, week_end, generated_at, model_used, \
                            headline, narrative_sections_json, digest_json, \
                            cache_hit_ratio, banner_dismissed_at \
                     FROM coach_reports \
                     WHERE user_id = $1 AND week_start = $2",
                )
                .bind(digest.user_id)
                .bind(digest.week_start)
                .fetch_one(&self.pool)
                .await?
            }
        };

        row.try_into_stored().map_err(Into::into)
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Pure prep helper — testable without a database.
// ─────────────────────────────────────────────────────────────────────────

/// Output of `prepare_report` — the fields that vary between a valid
/// narrated report and a stats-only fallback. Everything else on
/// `StoredCoachReport` is either from the digest or assigned by the DB.
#[derive(Debug)]
pub(super) struct PreparedReport {
    pub model_used: String,
    pub headline: Option<String>,
    pub narrative_sections: Option<Vec<NarrativeSection>>,
    pub cache_hit_ratio: Option<Decimal>,
}

impl PreparedReport {
    fn stats_only() -> Self {
        Self {
            model_used: FALLBACK_MODEL.to_string(),
            headline: None,
            narrative_sections: None,
            cache_hit_ratio: None,
        }
    }

    fn from_report(report: NarratedReport) -> Self {
        Self {
            model_used: report.model_used,
            headline: Some(report.headline),
            narrative_sections: Some(report.sections),
            cache_hit_ratio: report.cache_hit_ratio,
        }
    }
}

/// Combine narrator output with citation validation and produce the
/// row-level fields ready for persistence. Both narrator and validator
/// failures collapse to the stats-only fallback shape.
pub(super) fn prepare_report(
    digest: &CoachDigest,
    narrator_result: Result<NarratedReport, NarratorError>,
) -> PreparedReport {
    match narrator_result {
        Ok(report) => match validate(&report, digest) {
            Ok(()) => PreparedReport::from_report(report),
            Err(e) => {
                tracing::warn!(
                    user_id = %digest.user_id,
                    error = %e,
                    "coach: citation validation failed, falling back to stats-only",
                );
                PreparedReport::stats_only()
            }
        },
        Err(e) => {
            tracing::warn!(
                user_id = %digest.user_id,
                error = %e,
                "coach: narrator failed, falling back to stats-only",
            );
            PreparedReport::stats_only()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// DB row ↔ StoredCoachReport conversion
// ─────────────────────────────────────────────────────────────────────────

const SELECT_REPORT_COLUMNS_LATEST: &str =
    "SELECT id, user_id, week_start, week_end, generated_at, model_used, headline, \
            narrative_sections_json, digest_json, cache_hit_ratio, banner_dismissed_at \
     FROM coach_reports \
     WHERE user_id = $1 \
     ORDER BY generated_at DESC \
     LIMIT 1";

const SELECT_REPORT_COLUMNS_ARCHIVE: &str =
    "SELECT id, user_id, week_start, week_end, generated_at, model_used, headline, \
            narrative_sections_json, digest_json, cache_hit_ratio, banner_dismissed_at \
     FROM coach_reports \
     WHERE user_id = $1 \
     ORDER BY generated_at DESC \
     LIMIT $2 OFFSET $3";

#[derive(Debug, sqlx::FromRow)]
struct CoachReportRow {
    id: Uuid,
    user_id: Uuid,
    week_start: DateTime<Utc>,
    week_end: DateTime<Utc>,
    generated_at: DateTime<Utc>,
    model_used: String,
    headline: Option<String>,
    narrative_sections_json: Option<serde_json::Value>,
    digest_json: serde_json::Value,
    cache_hit_ratio: Option<Decimal>,
    banner_dismissed_at: Option<DateTime<Utc>>,
}

impl CoachReportRow {
    fn try_into_stored(self) -> Result<StoredCoachReport, serde_json::Error> {
        let digest: CoachDigest = serde_json::from_value(self.digest_json)?;
        let narrative_sections: Option<Vec<NarrativeSection>> = match self.narrative_sections_json
        {
            Some(v) => Some(serde_json::from_value(v)?),
            None => None,
        };
        Ok(StoredCoachReport {
            id: self.id,
            user_id: self.user_id,
            week_start: self.week_start,
            week_end: self.week_end,
            generated_at: self.generated_at,
            model_used: self.model_used,
            headline: self.headline,
            narrative_sections,
            digest,
            cache_hit_ratio: self.cache_hit_ratio,
            banner_dismissed_at: self.banner_dismissed_at,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Tests — pure prepare_report helper (DB flow verified by regression)
// ─────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::TimeZone;
    use rust_decimal_macros::dec;

    use super::super::types::{
        CoachDigest, FlaggedPattern, NarrativeSection, PatternKind, Severity, TradeEvidence,
        UserBaseline, WeekStats,
    };
    use super::*;

    fn fixture_trade() -> TradeEvidence {
        let id = Uuid::new_v4();
        let opened = Utc.with_ymd_and_hms(2026, 4, 13, 14, 0, 0).unwrap();
        TradeEvidence {
            id,
            short_id: id.simple().to_string().chars().take(8).collect(),
            symbol: "BTC_USDT".to_string(),
            side: "long".to_string(),
            opened_at: opened,
            closed_at: opened + chrono::Duration::hours(1),
            pnl: dec!(-10),
            r_multiple: Some(dec!(-1)),
            setup_tag: None,
            position_size_usd: dec!(2000),
        }
    }

    fn fixture_digest(trade: TradeEvidence) -> CoachDigest {
        let user_id = Uuid::new_v4();
        let week_start = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap();
        CoachDigest {
            user_id,
            week_start,
            week_end: week_start + chrono::Duration::days(7),
            baseline: UserBaseline {
                avg_trades_per_day: dec!(1),
                avg_position_size_usd: dec!(1000),
                typical_session_hours_utc: vec![13, 14, 15, 16],
                win_rate: dec!(0.5),
                avg_r_multiple: dec!(1),
                p90_trades_per_6h: dec!(2),
                setup_baselines: HashMap::new(),
            },
            week_stats: WeekStats {
                trade_count: 1,
                win_rate: Decimal::ZERO,
                total_pnl: dec!(-10),
                total_r: dec!(-1),
                trades_by_hour_utc: [0; 24],
                by_setup: HashMap::new(),
            },
            flagged_patterns: vec![FlaggedPattern {
                pattern: PatternKind::SizingDrift,
                severity: Severity::Notable,
                evidence: vec![trade.id],
                metrics: serde_json::json!({}),
            }],
            flagged_trades: vec![trade],
        }
    }

    fn valid_report(digest: &CoachDigest) -> NarratedReport {
        let trade = &digest.flagged_trades[0];
        NarratedReport {
            headline: format!("Size drift on [T-{}]", trade.short_id),
            sections: vec![NarrativeSection {
                pattern: PatternKind::SizingDrift,
                body: format!("Post-loss size doubled on [T-{}].", trade.short_id),
                citations: vec![trade.id],
            }],
            model_used: "deepseek-chat".into(),
            cache_hit_ratio: Some(dec!(0.8)),
            generated_at: Utc.with_ymd_and_hms(2026, 4, 20, 18, 0, 0).unwrap(),
        }
    }

    #[test]
    fn prepare_report_happy_path_preserves_narrator_fields() {
        let digest = fixture_digest(fixture_trade());
        let report = valid_report(&digest);

        let prepared = prepare_report(&digest, Ok(report));
        assert_eq!(prepared.model_used, "deepseek-chat");
        assert!(prepared.headline.is_some());
        assert!(prepared.narrative_sections.is_some());
        assert_eq!(prepared.cache_hit_ratio, Some(dec!(0.8)));
    }

    #[test]
    fn prepare_report_narrator_timeout_collapses_to_stats_only() {
        let digest = fixture_digest(fixture_trade());
        let prepared = prepare_report(&digest, Err(NarratorError::Timeout));
        assert_eq!(prepared.model_used, FALLBACK_MODEL);
        assert!(prepared.headline.is_none());
        assert!(prepared.narrative_sections.is_none());
        assert!(prepared.cache_hit_ratio.is_none());
    }

    #[test]
    fn prepare_report_narrator_rate_limit_collapses_to_stats_only() {
        let digest = fixture_digest(fixture_trade());
        let prepared = prepare_report(&digest, Err(NarratorError::RateLimit));
        assert_eq!(prepared.model_used, FALLBACK_MODEL);
        assert!(prepared.narrative_sections.is_none());
    }

    #[test]
    fn prepare_report_narrator_parse_error_collapses_to_stats_only() {
        let digest = fixture_digest(fixture_trade());
        let prepared = prepare_report(&digest, Err(NarratorError::Parse("bad json".into())));
        assert_eq!(prepared.model_used, FALLBACK_MODEL);
        assert!(prepared.headline.is_none());
    }

    #[test]
    fn prepare_report_unknown_citation_token_collapses_to_stats_only() {
        // Report body references a token that is not in the digest's
        // flagged_trades short_ids → validator rejects → stats-only.
        let digest = fixture_digest(fixture_trade());
        let mut report = valid_report(&digest);
        report.sections[0].body = "Ghost citation [T-deadbeef] here.".into();

        let prepared = prepare_report(&digest, Ok(report));
        assert_eq!(prepared.model_used, FALLBACK_MODEL);
        assert!(prepared.narrative_sections.is_none());
    }

    #[test]
    fn prepare_report_unknown_citation_uuid_collapses_to_stats_only() {
        let digest = fixture_digest(fixture_trade());
        let mut report = valid_report(&digest);
        report.sections[0].citations = vec![Uuid::new_v4()]; // not in digest

        let prepared = prepare_report(&digest, Ok(report));
        assert_eq!(prepared.model_used, FALLBACK_MODEL);
    }
}
