//! AGENT-03: Agent journal service — composes StatsEngine, TimeSeriesService,
//! and CoachService into agent-friendly responses for the three memory endpoints.
//!
//! Pure composition layer over existing analytics machinery. No new SQL, no new
//! crates, no mutable state.

// @anchor exchange:router:agent_journal
// @tags api

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::models::agent_journal::{
    AgentInsight, AgentSummary, AgentSummaryQuery, CompareFilters, CompareRequest,
    ComparisonResult, DeltaDirection, EquityPoint, MetricDelta, OverallStats,
    PatternKind as AgentPatternKind, Severity as AgentSeverity, SetupBreakdown,
    SetupDelta, TimeframeInfo, TradeCitation,
};
use crate::services::coach::CoachService;
use crate::services::coach::types::{
    CoachDigest, FlaggedPattern, PatternKind, Severity,
};
use crate::services::journal_stats::{StatsEngine, StatsFilter};
use crate::services::journal_timeseries::{
    SetupBreakdown as TsSetupBreakdown, TimeSeriesService,
};

// ── AgentJournalService ───────────────────────────────────────────────

pub struct AgentJournalService {
    pool: PgPool,
}

impl AgentJournalService {
    pub fn new(pool: PgPool, _analytics_pool: PgPool) -> Self {
        // Use the same pool for both — analytics_pool is kept for future hot/cold
        // separation; StatsEngine + TimeSeriesService both use the passed pool.
        Self { pool }
    }

    /// Build a consolidated performance summary for an agent.
    ///
    /// Composes StatsEngine (overview + performance + risk) with TimeSeriesService
    /// (setup breakdown + equity curve) and a top-trades fetch into a single
    /// `AgentSummary` response.
    pub async fn build_summary(
        &self,
        user_id: Uuid,
        query: &AgentSummaryQuery,
    ) -> Result<AgentSummary, sqlx::Error> {
        let (date_from, date_to) = resolve_timeframe(query.timeframe.as_deref());
        let filter = to_stats_filter(query, date_from, date_to);
        let label = timeframe_label(query.timeframe.as_deref());

        let stats_engine = StatsEngine::new(self.pool.clone());
        let ts_service = TimeSeriesService::new(self.pool.clone());

        // Run independent queries in parallel.
        let (overview_result, perf_result, risk_result, setup_result, equity_result) = tokio::join!(
            stats_engine.account_overview(user_id, &filter),
            stats_engine.performance_stats(user_id, &filter),
            stats_engine.risk_stats(user_id, &filter),
            ts_service.setup_breakdown(user_id, &filter),
            ts_service.equity_curve(user_id, &filter),
        );

        let overview = overview_result?;
        let perf = perf_result?;
        let risk = risk_result?;
        let setups = setup_result?;
        let equity = equity_result?;

        let top_trades = fetch_top_trades(&self.pool, user_id, &filter).await?;

        // Compute avg_hold_hours from duration. StatsEngine returns seconds.
        let avg_hold_hours = if perf.avg_duration_secs > 0 {
            Some(Decimal::from(perf.avg_duration_secs) / Decimal::from(3600))
        } else {
            None
        };

        let overall = OverallStats {
            trade_count: overview.total_trades,
            win_rate: perf.win_rate,
            avg_r_multiple: perf.avg_r_multiple,
            total_pnl: overview.net_pnl,
            max_drawdown: risk.max_drawdown,
            profit_factor: perf.profit_factor,
            sharpe_ratio: None, // requires risk-free rate; omitted for now
            avg_hold_hours,
        };

        let by_setup: Vec<SetupBreakdown> = setups
            .into_iter()
            .map(|s| SetupBreakdown {
                setup: s.setup_tag,
                trade_count: s.trade_count,
                win_rate: s.win_rate,
                avg_r_multiple: s.avg_r.unwrap_or(Decimal::ZERO),
                total_pnl: s.net_pnl,
            })
            .collect();

        let equity_points: Vec<EquityPoint> = equity
            .into_iter()
            .map(|p| EquityPoint {
                date: p.date,
                cumulative_pnl: p.cumulative_pnl,
                equity: p.equity,
            })
            .collect();

        Ok(AgentSummary {
            timeframe: TimeframeInfo {
                label,
                from: date_from,
                to: date_to,
            },
            overall,
            by_setup,
            top_trades,
            equity: equity_points,
        })
    }

    /// Build insights from the latest stored coach report.
    ///
    /// Maps `FlaggedPattern` entries from the coach digest into
    /// `AgentInsight` values with human-readable headlines, details,
    /// and recommendations. Also computes ad-hoc insights for
    /// low win-rate setups from the digest's week stats.
    ///
    /// Returns an empty list when no coach reports exist for this user.
    pub async fn build_insights(
        &self,
        user_id: Uuid,
        coach_service: &Arc<CoachService>,
    ) -> Vec<AgentInsight> {
        let report = match coach_service.latest_for(user_id).await {
            Ok(Some((report, _has_new))) => report,
            _ => return Vec::new(),
        };

        let digest = &report.digest;
        let mut insights: Vec<AgentInsight> = Vec::new();

        // Map flagged patterns → agent insights.
        for flagged in &digest.flagged_patterns {
            let (headline, detail, recommendation) =
                pattern_to_insight_text(flagged, digest);
            insights.push(AgentInsight {
                pattern: map_pattern_kind(flagged.pattern),
                severity: map_severity(flagged.severity),
                headline,
                detail,
                recommendation,
                evidence_count: flagged.evidence.len() as i64,
            });
        }

        // Ad-hoc: low win-rate setups.
        for (setup_tag, baseline) in &digest.week_stats.by_setup {
            let wr_pct = baseline.win_rate * Decimal::from(100);
            if wr_pct < Decimal::from(40) && baseline.trade_count >= 3 {
                insights.push(AgentInsight {
                    pattern: AgentPatternKind::SetupFatigue,
                    severity: AgentSeverity::Notable,
                    headline: format!(
                        "Low win rate on \"{}\" setups",
                        setup_tag
                    ),
                    detail: format!(
                        "{} has {:.1}% win rate over {} trades. \
                         Avg R: {:.2}. Consider reviewing entry criteria \
                         for this setup.",
                        setup_tag, wr_pct, baseline.trade_count, baseline.avg_r_multiple,
                    ),
                    recommendation: Some(
                        "Reduce position size or pause this setup until \
                         edge is confirmed".into(),
                    ),
                    evidence_count: baseline.trade_count,
                });
            }
        }

        insights
    }

    /// Build a side-by-side comparison of two time periods.
    ///
    /// Runs StatsEngine (overview + performance + risk) for both periods
    /// in parallel, computes per-metric deltas and directions, and merges
    /// per-setup breakdowns from TimeSeriesService.
    pub async fn build_comparison(
        &self,
        user_id: Uuid,
        request: &CompareRequest,
    ) -> Result<ComparisonResult, sqlx::Error> {
        let filter_a = compare_to_stats_filter(
            request.period_a.from,
            request.period_a.to,
            request.filters.as_ref(),
        );
        let filter_b = compare_to_stats_filter(
            request.period_b.from,
            request.period_b.to,
            request.filters.as_ref(),
        );

        let stats_a = StatsEngine::new(self.pool.clone());
        let stats_b = StatsEngine::new(self.pool.clone());
        let ts_a = TimeSeriesService::new(self.pool.clone());
        let ts_b = TimeSeriesService::new(self.pool.clone());

        // Run both periods' queries in parallel: overview + performance + risk.
        let (
            period_a_data,
            period_b_data,
            setups_a,
            setups_b,
        ) = tokio::join!(
            async {
                let o = stats_a.account_overview(user_id, &filter_a).await?;
                let p = stats_a.performance_stats(user_id, &filter_a).await?;
                let r = stats_a.risk_stats(user_id, &filter_a).await?;
                Ok::<_, sqlx::Error>((o, p, r))
            },
            async {
                let o = stats_b.account_overview(user_id, &filter_b).await?;
                let p = stats_b.performance_stats(user_id, &filter_b).await?;
                let r = stats_b.risk_stats(user_id, &filter_b).await?;
                Ok::<_, sqlx::Error>((o, p, r))
            },
            async { ts_a.setup_breakdown(user_id, &filter_a).await },
            async { ts_b.setup_breakdown(user_id, &filter_b).await },
        );

        let (overview_a, perf_a, risk_a) = period_a_data?;
        let (overview_b, perf_b, risk_b) = period_b_data?;
        let setups_a = setups_a?;
        let setups_b = setups_b?;

        use crate::models::agent_journal::PeriodInfo;

        let period_a = PeriodInfo {
            from: request.period_a.from,
            to: request.period_a.to,
            trade_count: overview_a.total_trades,
            win_rate: perf_a.win_rate,
            avg_r_multiple: perf_a.avg_r_multiple,
            total_pnl: overview_a.net_pnl,
            max_drawdown: risk_a.max_drawdown,
            profit_factor: perf_a.profit_factor,
            sharpe_ratio: None,
        };

        let period_b = PeriodInfo {
            from: request.period_b.from,
            to: request.period_b.to,
            trade_count: overview_b.total_trades,
            win_rate: perf_b.win_rate,
            avg_r_multiple: perf_b.avg_r_multiple,
            total_pnl: overview_b.net_pnl,
            max_drawdown: risk_b.max_drawdown,
            profit_factor: perf_b.profit_factor,
            sharpe_ratio: None,
        };

        let deltas = vec![
            metric_delta("trade_count",
                Decimal::from(period_a.trade_count),
                Decimal::from(period_b.trade_count),
                false),
            metric_delta("win_rate",
                period_a.win_rate, period_b.win_rate, false),
            metric_delta("avg_r_multiple",
                period_a.avg_r_multiple, period_b.avg_r_multiple, false),
            metric_delta("total_pnl",
                period_a.total_pnl, period_b.total_pnl, false),
            // Drawdown: lower is better, so invert direction.
            metric_delta("max_drawdown",
                period_a.max_drawdown, period_b.max_drawdown, true),
            metric_delta("profit_factor",
                period_a.profit_factor, period_b.profit_factor, false),
        ];

        let by_setup_deltas = merge_setup_deltas(&setups_a, &setups_b);

        Ok(ComparisonResult {
            period_a,
            period_b,
            deltas,
            by_setup_deltas,
        })
    }
}

// ── Pure helpers ──────────────────────────────────────────────────────

/// Resolve a timeframe shorthand to (date_from, date_to) bounds.
/// Defaults to 90 days when None. "all" returns no bounds.
fn resolve_timeframe(timeframe: Option<&str>) -> (Option<NaiveDate>, Option<NaiveDate>) {
    let now = Utc::now().date_naive();
    let days = match timeframe {
        Some("7d") => 7,
        Some("30d") => 30,
        Some("90d") | None => 90,
        Some("all") => return (None, None),
        Some(other) => {
            // Try to parse as integer days; fall back to 90
            other.parse::<i64>().unwrap_or(90)
        }
    };
    let from = now - chrono::Duration::days(days);
    (Some(from), Some(now))
}

/// Human-readable label for the timeframe.
fn timeframe_label(timeframe: Option<&str>) -> String {
    match timeframe {
        Some("7d") => "Last 7 Days".into(),
        Some("30d") => "Last 30 Days".into(),
        Some("90d") => "Last 90 Days".into(),
        Some("all") => "All Time".into(),
        Some(other) => format!("Last {} Days", other.parse::<i64>().unwrap_or(90)),
        None => "Last 90 Days".into(),
    }
}

/// Convert an AgentSummaryQuery + resolved dates into a StatsFilter.
fn to_stats_filter(
    query: &AgentSummaryQuery,
    date_from: Option<NaiveDate>,
    date_to: Option<NaiveDate>,
) -> StatsFilter {
    StatsFilter {
        exchange: query.exchange.clone(),
        symbol: query.symbol.clone(),
        date_from,
        date_to,
        tags: None,
        source: query.source.clone(),
        setup_tag: query.setup_tag.clone(),
        side: query.side.clone(),
    }
}

/// Fetch top 5 trades by R-multiple (descending, nulls last), falling back to
/// net_pnl for trades without an R value. Returns TradeCitation entries with
/// short_id (first 8 hex chars of UUID) for citation tokens.
async fn fetch_top_trades(
    pool: &PgPool,
    user_id: Uuid,
    filter: &StatsFilter,
) -> Result<Vec<TradeCitation>, sqlx::Error> {
    #[derive(Debug, sqlx::FromRow)]
    struct TopTradeRow {
        id: Uuid,
        symbol: String,
        side: String,
        opened_at: chrono::DateTime<Utc>,
        net_pnl: Decimal,
        r_multiple: Option<Decimal>,
        setup_tag: Option<String>,
    }

    let rows = sqlx::query_as::<_, TopTradeRow>(
        "SELECT id, symbol, side, opened_at, net_pnl, r_multiple, setup_tag \
         FROM journal_trades \
         WHERE user_id = $1 \
            AND ($2::TEXT IS NULL OR exchange = $2) \
            AND ($3::TEXT IS NULL OR symbol = $3) \
            AND ($4::DATE IS NULL OR closed_at >= $4) \
            AND ($5::DATE IS NULL OR closed_at <= $5) \
            AND ($6::TEXT IS NULL OR source = $6) \
            AND ($7::TEXT IS NULL OR LOWER(setup_tag) = LOWER($7)) \
            AND ($8::TEXT IS NULL OR side = $8) \
         ORDER BY r_multiple DESC NULLS LAST, net_pnl DESC \
         LIMIT 5",
    )
    .bind(user_id)
    .bind(&filter.exchange)
    .bind(&filter.symbol)
    .bind(filter.date_from)
    .bind(filter.date_to)
    .bind(&filter.source)
    .bind(&filter.setup_tag)
    .bind(&filter.side)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            // First 8 hex chars of UUID for [T-xxxxxxxx] citation token.
            let short_id = r.id.to_string()[..8].to_string();
            TradeCitation {
                id: r.id,
                short_id,
                symbol: r.symbol,
                side: r.side,
                opened_at: r.opened_at,
                pnl: r.net_pnl,
                r_multiple: r.r_multiple,
                setup_tag: r.setup_tag,
            }
        })
        .collect())
}

// ── Coach pattern → agent insight mapping ─────────────────────────────

/// Map coach PatternKind → agent PatternKind (1:1).
fn map_pattern_kind(k: PatternKind) -> AgentPatternKind {
    match k {
        PatternKind::SizingDrift => AgentPatternKind::SizingDrift,
        PatternKind::FrequencySpike => AgentPatternKind::FrequencySpike,
        PatternKind::SessionAnomaly => AgentPatternKind::SessionAnomaly,
        PatternKind::SetupFatigue => AgentPatternKind::SetupFatigue,
        PatternKind::CorrelationStack => AgentPatternKind::CorrelationStack,
        PatternKind::StreakRisk => AgentPatternKind::StreakRisk,
    }
}

/// Map coach Severity → agent Severity (1:1).
fn map_severity(s: Severity) -> AgentSeverity {
    match s {
        Severity::Info => AgentSeverity::Info,
        Severity::Notable => AgentSeverity::Notable,
        Severity::Concerning => AgentSeverity::Concerning,
    }
}

/// Generate human-readable headline, detail, and recommendation for a
/// flagged coach pattern. Extracts metrics from the pattern's `metrics`
/// JSON blob and cross-references with the digest's baseline data.
fn pattern_to_insight_text(
    flagged: &FlaggedPattern,
    digest: &CoachDigest,
) -> (String, String, Option<String>) {
    match flagged.pattern {
        PatternKind::SizingDrift => {
            let multiplier = flagged
                .metrics
                .get("size_multiplier")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            (
                format!(
                    "Position sizes are {}× your 30-day average",
                    multiplier
                ),
                format!(
                    "Your recent trades show position sizes significantly \
                     above your {:.2} USD baseline. This increases \
                     risk of ruin and drawdown depth.",
                    digest.baseline.avg_position_size_usd
                ),
                Some("Reduce position size to baseline levels or lower until \
                      confidence in edge is restored.".into()),
            )
        }
        PatternKind::FrequencySpike => {
            let current = flagged
                .metrics
                .get("trades_this_week")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let baseline = digest.baseline.avg_trades_per_day;
            (
                format!(
                    "Trade frequency spike: {}/week vs {:.1}/day baseline",
                    current, baseline
                ),
                format!(
                    "You are trading significantly more frequently than your \
                     30-day average. High-frequency periods often correlate \
                     with impulsive decisions and degraded win rates."
                ),
                Some("Consider enforcing a maximum daily trade limit. \
                      Quality over quantity.".into()),
            )
        }
        PatternKind::SessionAnomaly => {
            let typical = digest
                .baseline
                .typical_session_hours_utc
                .iter()
                .map(|h| h.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            (
                "Trading outside your typical session hours".into(),
                format!(
                    "Your best performance historically falls in UTC hours \
                     [{}]. Recent trades deviate from this pattern, which \
                     may indicate emotional or fatigue-driven decisions.",
                    typical
                ),
                Some("Restrict trading to your historically optimal hours \
                      where possible.".into()),
            )
        }
        PatternKind::SetupFatigue => {
            let tag = flagged
                .metrics
                .get("setup_tag")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            (
                format!("Setup fatigue detected on \"{}\" setups", tag),
                format!(
                    "Declining performance on \"{}\" setups suggests the \
                     edge may be fading or the market regime has shifted. \
                     Continuing to trade a fatigued setup erodes capital.",
                    tag
                ),
                Some("Pause this setup and wait for confirming signals \
                      before re-entry.".into()),
            )
        }
        PatternKind::CorrelationStack => {
            let count = flagged
                .metrics
                .get("correlated_positions")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            (
                format!("{} correlated positions open simultaneously", count),
                format!(
                    "Multiple positions are highly correlated, effectively \
                     multiplying your exposure to a single market move. \
                     This violates the independence assumption in Kelly \
                     sizing and position management."
                ),
                Some("Close overlapping positions. Maintain a maximum of \
                      one correlated position per direction.".into()),
            )
        }
        PatternKind::StreakRisk => {
            let streak_len = flagged
                .metrics
                .get("streak_length")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let direction = flagged
                .metrics
                .get("direction")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            (
                format!("Active {} streak of {} consecutive trades", direction, streak_len),
                format!(
                    "Extended streaks, whether winning or losing, can lead \
                     to overconfidence (on wins) or revenge trading (on \
                     losses). Both states degrade decision quality."
                ),
                Some("Take a break. Clear your head. Return with a fresh \
                      session and strict risk limits.".into()),
            )
        }
    }
}

// ── Comparison helpers ────────────────────────────────────────────────

/// Build a StatsFilter from comparison date range and optional filters.
fn compare_to_stats_filter(
    from: NaiveDate,
    to: NaiveDate,
    filters: Option<&CompareFilters>,
) -> StatsFilter {
    match filters {
        Some(f) => StatsFilter {
            exchange: f.exchange.clone(),
            symbol: f.symbol.clone(),
            date_from: Some(from),
            date_to: Some(to),
            tags: None,
            source: f.source.clone(),
            setup_tag: f.setup_tag.clone(),
            side: f.side.clone(),
        },
        None => StatsFilter {
            exchange: None,
            symbol: None,
            date_from: Some(from),
            date_to: Some(to),
            tags: None,
            source: None,
            setup_tag: None,
            side: None,
        },
    }
}

/// Compute a single metric delta with percentage change and direction.
///
/// `invert_direction` flips the interpretation: used for metrics where
/// lower is better (e.g., drawdown).
fn metric_delta(
    metric: &str,
    value_a: Decimal,
    value_b: Decimal,
    invert_direction: bool,
) -> MetricDelta {
    let delta_pct = if value_a != Decimal::ZERO {
        ((value_b - value_a) / value_a) * Decimal::from(100)
    } else if value_b != Decimal::ZERO {
        Decimal::from(100)
    } else {
        Decimal::ZERO
    };

    let raw_direction = if delta_pct > Decimal::from(5) {
        DeltaDirection::Improved
    } else if delta_pct < Decimal::new(-5, 0) {
        DeltaDirection::Declined
    } else {
        DeltaDirection::Neutral
    };

    let direction = if invert_direction {
        match raw_direction {
            DeltaDirection::Improved => DeltaDirection::Declined,
            DeltaDirection::Declined => DeltaDirection::Improved,
            DeltaDirection::Neutral => DeltaDirection::Neutral,
        }
    } else {
        raw_direction
    };

    MetricDelta {
        metric: metric.to_string(),
        value_a,
        value_b,
        delta_pct,
        direction,
    }
}

/// Merge per-setup breakdowns from two periods into per-setup deltas.
/// Only setups present in either period are included.
fn merge_setup_deltas(
    setups_a: &[TsSetupBreakdown],
    setups_b: &[TsSetupBreakdown],
) -> Vec<SetupDelta> {
    use std::collections::BTreeMap;

    let mut by_setup: BTreeMap<&str, (Option<&TsSetupBreakdown>, Option<&TsSetupBreakdown>)> =
        BTreeMap::new();

    for s in setups_a {
        by_setup.entry(&s.setup_tag).or_default().0 = Some(s);
    }
    for s in setups_b {
        by_setup.entry(&s.setup_tag).or_default().1 = Some(s);
    }

    by_setup
        .into_iter()
        .map(|(tag, (a, b))| {
            let zero = TsSetupBreakdown {
                setup_tag: tag.to_string(),
                trade_count: 0,
                net_pnl: Decimal::ZERO,
                win_rate: Decimal::ZERO,
                avg_r: None,
                expectancy: Decimal::ZERO,
            };
            let sa = a.unwrap_or(&zero);
            let sb = b.unwrap_or(&zero);
            SetupDelta {
                setup: tag.to_string(),
                trade_count_a: sa.trade_count,
                trade_count_b: sb.trade_count,
                win_rate_a: sa.win_rate,
                win_rate_b: sb.win_rate,
                total_pnl_a: sa.net_pnl,
                total_pnl_b: sb.net_pnl,
                avg_r_a: sa.avg_r,
                avg_r_b: sb.avg_r,
            }
        })
        .collect()
}

// ── Unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ── resolve_timeframe ────────────────────────────────────────────

    #[test]
    fn test_timeframe_none_defaults_to_90d() {
        let (from, to) = resolve_timeframe(None);
        assert!(from.is_some());
        assert!(to.is_some());
        let days = (to.unwrap() - from.unwrap()).num_days();
        assert_eq!(days, 90);
    }

    #[test]
    fn test_timeframe_7d() {
        let (from, to) = resolve_timeframe(Some("7d"));
        let days = (to.unwrap() - from.unwrap()).num_days();
        assert_eq!(days, 7);
    }

    #[test]
    fn test_timeframe_30d() {
        let (from, to) = resolve_timeframe(Some("30d"));
        let days = (to.unwrap() - from.unwrap()).num_days();
        assert_eq!(days, 30);
    }

    #[test]
    fn test_timeframe_90d() {
        let (from, to) = resolve_timeframe(Some("90d"));
        let days = (to.unwrap() - from.unwrap()).num_days();
        assert_eq!(days, 90);
    }

    #[test]
    fn test_timeframe_all_returns_no_bounds() {
        let (from, to) = resolve_timeframe(Some("all"));
        assert!(from.is_none());
        assert!(to.is_none());
    }

    #[test]
    fn test_timeframe_custom_days() {
        let (from, to) = resolve_timeframe(Some("14"));
        let days = (to.unwrap() - from.unwrap()).num_days();
        assert_eq!(days, 14);
    }

    #[test]
    fn test_timeframe_invalid_falls_back_to_90() {
        let (from, to) = resolve_timeframe(Some("foobar"));
        let days = (to.unwrap() - from.unwrap()).num_days();
        assert_eq!(days, 90);
    }

    // ── timeframe_label ──────────────────────────────────────────────

    #[test]
    fn test_label_none() {
        assert_eq!(timeframe_label(None), "Last 90 Days");
    }

    #[test]
    fn test_label_7d() {
        assert_eq!(timeframe_label(Some("7d")), "Last 7 Days");
    }

    #[test]
    fn test_label_30d() {
        assert_eq!(timeframe_label(Some("30d")), "Last 30 Days");
    }

    #[test]
    fn test_label_all() {
        assert_eq!(timeframe_label(Some("all")), "All Time");
    }

    // ── to_stats_filter ──────────────────────────────────────────────

    #[test]
    fn test_to_stats_filter_maps_all_fields() {
        let query = AgentSummaryQuery {
            timeframe: Some("30d".into()),
            symbol: Some("ETH_USDT".into()),
            side: Some("LONG".into()),
            setup_tag: Some("breakout".into()),
            exchange: Some("hyperliquid".into()),
            source: Some("agent:hermes_v1.2".into()),
            format: crate::models::agent_journal::SummaryFormat::Json,
        };
        let from = NaiveDate::from_ymd_opt(2026, 1, 1);
        let to = NaiveDate::from_ymd_opt(2026, 3, 31);
        let filter = to_stats_filter(&query, from, to);

        assert_eq!(filter.exchange.unwrap(), "hyperliquid");
        assert_eq!(filter.symbol.unwrap(), "ETH_USDT");
        assert_eq!(filter.side.unwrap(), "LONG");
        assert_eq!(filter.setup_tag.unwrap(), "breakout");
        assert_eq!(filter.source.unwrap(), "agent:hermes_v1.2");
        assert_eq!(filter.date_from, from);
        assert_eq!(filter.date_to, to);
    }

    // ── map_pattern_kind ─────────────────────────────────────────

    #[test]
    fn test_map_all_pattern_kinds() {
        use crate::services::coach::types::PatternKind as CoachPattern;
        let variants = [
            CoachPattern::SizingDrift,
            CoachPattern::FrequencySpike,
            CoachPattern::SessionAnomaly,
            CoachPattern::SetupFatigue,
            CoachPattern::CorrelationStack,
            CoachPattern::StreakRisk,
        ];
        for v in &variants {
            let mapped = map_pattern_kind(*v);
            // Round-trip: the name should match
            let coach_name = format!("{:?}", v);
            let agent_name = format!("{:?}", mapped);
            assert_eq!(coach_name, agent_name);
        }
    }

    #[test]
    fn test_map_all_severities() {
        use crate::services::coach::types::Severity as CoachSeverity;
        let variants = [
            CoachSeverity::Info,
            CoachSeverity::Notable,
            CoachSeverity::Concerning,
        ];
        for v in &variants {
            let mapped = map_severity(*v);
            let coach_name = format!("{:?}", v);
            let agent_name = format!("{:?}", mapped);
            assert_eq!(coach_name, agent_name);
        }
    }

    #[test]
    fn test_pattern_insight_sizing_drift_has_headline() {
        use crate::services::coach::types::{
            CoachDigest, FlaggedPattern, PatternKind, Severity, TradeEvidence,
            UserBaseline, WeekStats,
        };
        let digest = CoachDigest {
            user_id: Uuid::nil(),
            week_start: Utc::now(),
            week_end: Utc::now(),
            baseline: UserBaseline {
                avg_trades_per_day: Decimal::ONE,
                avg_position_size_usd: Decimal::new(1000, 0),
                typical_session_hours_utc: vec![14, 15, 16],
                win_rate: Decimal::new(55, 2),
                avg_r_multiple: Decimal::new(15, 1),
                p90_trades_per_6h: Decimal::new(3, 0),
                setup_baselines: std::collections::HashMap::new(),
            },
            week_stats: WeekStats {
                trade_count: 1,
                win_rate: Decimal::ONE,
                total_pnl: Decimal::ZERO,
                total_r: Decimal::ZERO,
                trades_by_hour_utc: [0; 24],
                by_setup: std::collections::HashMap::new(),
            },
            flagged_patterns: vec![],
            flagged_trades: vec![],
        };
        let flagged = FlaggedPattern {
            pattern: PatternKind::SizingDrift,
            severity: Severity::Concerning,
            evidence: vec![],
            metrics: serde_json::json!({"size_multiplier": "2.1"}),
        };
        let (headline, detail, recommendation) =
            pattern_to_insight_text(&flagged, &digest);
        assert!(headline.contains("2.1"));
        assert!(headline.contains("average"));
        assert!(detail.contains("1000"));
        assert!(recommendation.is_some());
    }

    #[test]
    fn test_pattern_insight_streak_risk_has_direction() {
        use crate::services::coach::types::{
            CoachDigest, FlaggedPattern, PatternKind, Severity, UserBaseline,
            WeekStats,
        };
        let digest = CoachDigest {
            user_id: Uuid::nil(),
            week_start: Utc::now(),
            week_end: Utc::now(),
            baseline: UserBaseline {
                avg_trades_per_day: Decimal::ONE,
                avg_position_size_usd: Decimal::new(500, 0),
                typical_session_hours_utc: vec![],
                win_rate: Decimal::new(5, 1),
                avg_r_multiple: Decimal::new(10, 1),
                p90_trades_per_6h: Decimal::new(2, 0),
                setup_baselines: std::collections::HashMap::new(),
            },
            week_stats: WeekStats {
                trade_count: 1,
                win_rate: Decimal::ONE,
                total_pnl: Decimal::ZERO,
                total_r: Decimal::ZERO,
                trades_by_hour_utc: [0; 24],
                by_setup: std::collections::HashMap::new(),
            },
            flagged_patterns: vec![],
            flagged_trades: vec![],
        };
        let flagged = FlaggedPattern {
            pattern: PatternKind::StreakRisk,
            severity: Severity::Notable,
            evidence: vec![Uuid::nil(), Uuid::nil()],
            metrics: serde_json::json!({
                "streak_length": 7,
                "direction": "losing"
            }),
        };
        let (headline, _detail, recommendation) =
            pattern_to_insight_text(&flagged, &digest);
        assert!(headline.contains("7"));
        assert!(headline.contains("losing"));
        assert!(recommendation.is_some());
        assert!(recommendation.unwrap().contains("break"));
    }

    // ── metric_delta ─────────────────────────────────────────────

    #[test]
    fn test_metric_delta_improved() {
        let d = metric_delta("win_rate", dec!(50), dec!(60), false);
        assert_eq!(d.direction, DeltaDirection::Improved);
        assert!(d.delta_pct > Decimal::ZERO);
    }

    #[test]
    fn test_metric_delta_declined() {
        let d = metric_delta("win_rate", dec!(60), dec!(50), false);
        assert_eq!(d.direction, DeltaDirection::Declined);
        assert!(d.delta_pct < Decimal::ZERO);
    }

    #[test]
    fn test_metric_delta_neutral_small_change() {
        let d = metric_delta("win_rate", dec!(55), dec!(56), false);
        assert_eq!(d.direction, DeltaDirection::Neutral);
    }

    #[test]
    fn test_metric_delta_drawdown_inverts_direction() {
        // Drawdown goes from -1000 to -500 (improvement)
        let d = metric_delta("max_drawdown", dec!(-1000), dec!(-500), true);
        // Value improved (less drawdown) but direction should be Improved
        assert_eq!(d.direction, DeltaDirection::Improved);
    }

    #[test]
    fn test_metric_delta_drawdown_worsen_inverts_direction() {
        // Drawdown goes from -500 to -1000 (worse)
        let d = metric_delta("max_drawdown", dec!(-500), dec!(-1000), true);
        assert_eq!(d.direction, DeltaDirection::Declined);
    }

    #[test]
    fn test_metric_delta_zero_denominator() {
        let d = metric_delta("profit_factor", Decimal::ZERO, dec!(2), false);
        assert_eq!(d.direction, DeltaDirection::Improved);
        assert_eq!(d.delta_pct, Decimal::from(100));
    }

    #[test]
    fn test_metric_delta_both_zero() {
        let d = metric_delta("profit_factor", Decimal::ZERO, Decimal::ZERO, false);
        assert_eq!(d.direction, DeltaDirection::Neutral);
        assert_eq!(d.delta_pct, Decimal::ZERO);
    }

    // ── merge_setup_deltas ───────────────────────────────────────

    #[test]
    fn test_merge_setup_deltas_both_have_setups() {
        let a = vec![TsSetupBreakdown {
            setup_tag: "breakout".into(),
            trade_count: 10,
            net_pnl: dec!(1000),
            win_rate: dec!(60),
            avg_r: Some(dec!(2)),
            expectancy: dec!(100),
        }];
        let b = vec![TsSetupBreakdown {
            setup_tag: "breakout".into(),
            trade_count: 15,
            net_pnl: dec!(1500),
            win_rate: dec!(65),
            avg_r: Some(dec!(2.1)),
            expectancy: dec!(100),
        }];
        let deltas = merge_setup_deltas(&a, &b);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].setup, "breakout");
        assert_eq!(deltas[0].trade_count_a, 10);
        assert_eq!(deltas[0].trade_count_b, 15);
    }

    #[test]
    fn test_merge_setup_deltas_only_in_one_period() {
        let a = vec![TsSetupBreakdown {
            setup_tag: "breakout".into(),
            trade_count: 10,
            net_pnl: dec!(1000),
            win_rate: dec!(60),
            avg_r: Some(dec!(2)),
            expectancy: dec!(100),
        }];
        let b = vec![TsSetupBreakdown {
            setup_tag: "reversal".into(),
            trade_count: 5,
            net_pnl: dec!(500),
            win_rate: dec!(50),
            avg_r: Some(dec!(1.5)),
            expectancy: dec!(100),
        }];
        let deltas = merge_setup_deltas(&a, &b);
        assert_eq!(deltas.len(), 2);
        // breakout: only in A, B side is zero-value
        let breakout = deltas.iter().find(|d| d.setup == "breakout").unwrap();
        assert_eq!(breakout.trade_count_a, 10);
        assert_eq!(breakout.trade_count_b, 0);
    }
}
