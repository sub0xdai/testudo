//! JNL-04: Time-Series Aggregation Queries
//!
//! Chart-ready data structures for equity curves, daily P&L, symbol distributions,
//! duration/profitability correlations, and return distribution histograms.
//! All financial math uses `rust_decimal::Decimal` — no f64 for money.

use chrono::NaiveDate;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use uuid::Uuid;

use super::journal_stats::StatsFilter;

// ── Response structs ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EquityCurvePoint {
    pub date: NaiveDate,
    pub cumulative_pnl: Decimal,
    /// True account equity when balance snapshots are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equity: Option<Decimal>,
    pub peak: Decimal,
    pub drawdown: Decimal,
    pub drawdown_pct: Decimal,
    /// True when equity values come from real balance snapshots.
    #[serde(default)]
    pub is_true_equity: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyPnlPoint {
    pub date: NaiveDate,
    pub net_pnl: Decimal,
    pub trade_count: i32,
    pub win_count: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolBreakdown {
    pub symbol: String,
    pub trade_count: i64,
    pub net_pnl: Decimal,
    pub win_rate: Decimal,
    pub avg_r: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupBreakdown {
    pub setup_tag: String,
    pub trade_count: i64,
    pub net_pnl: Decimal,
    pub win_rate: Decimal,
    pub avg_r: Option<Decimal>,
    pub expectancy: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct DurationProfitPoint {
    pub duration_minutes: f64,
    pub net_pnl: Decimal,
    pub symbol: String,
    pub side: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReturnBucket {
    pub bucket_label: String,
    pub day_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeDistribution {
    pub hour: i32,
    pub day_of_week: i32,
    pub trade_count: i64,
    pub net_pnl: Decimal,
}

// ── SQL row types ────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct DailyRow {
    stat_date: NaiveDate,
    net_pnl: Decimal,
    trade_count: i64,
    win_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct SymbolRow {
    symbol: String,
    trade_count: i64,
    net_pnl: Decimal,
    win_rate: Decimal,
    avg_r: Option<Decimal>,
}

#[derive(Debug, sqlx::FromRow)]
struct SetupRow {
    setup_tag: String,
    trade_count: i64,
    net_pnl: Decimal,
    win_rate: Decimal,
    avg_r: Option<Decimal>,
    expectancy: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct DurationRow {
    duration_secs: i32,
    net_pnl: Decimal,
    symbol: String,
    side: String,
}

#[derive(Debug, sqlx::FromRow)]
struct TimeRow {
    hour: i32,
    day_of_week: i32,
    trade_count: i64,
    net_pnl: Decimal,
}

// ── Pure computation helpers ─────────────────────────────────────────

/// Build equity curve from ordered daily P&L values.
/// Computes running cumulative P&L, peak, and drawdown at each point.
pub fn compute_equity_curve(daily_pnls: &[(NaiveDate, Decimal)]) -> Vec<EquityCurvePoint> {
    compute_equity_curve_with_base(daily_pnls, None)
}

/// Build equity curve with an optional starting balance.
/// When `starting_balance` is Some, equity = starting_balance + cumulative_pnl,
/// and drawdown uses peak equity as denominator.
pub fn compute_equity_curve_with_base(
    daily_pnls: &[(NaiveDate, Decimal)],
    starting_balance: Option<Decimal>,
) -> Vec<EquityCurvePoint> {
    let mut result = Vec::with_capacity(daily_pnls.len());
    let mut cumulative = Decimal::ZERO;
    let mut peak = Decimal::ZERO;
    let base = starting_balance.unwrap_or(Decimal::ZERO);
    let has_base = starting_balance.is_some();
    let hundred = Decimal::from(100);

    for &(date, pnl) in daily_pnls {
        cumulative += pnl;
        let equity_value = base + cumulative;

        if equity_value > peak {
            peak = equity_value;
        }
        let drawdown = equity_value - peak;
        let drawdown_pct = if peak > Decimal::ZERO {
            (drawdown / peak) * hundred
        } else {
            Decimal::ZERO
        };

        result.push(EquityCurvePoint {
            date,
            cumulative_pnl: cumulative,
            equity: if has_base { Some(equity_value) } else { None },
            peak,
            drawdown,
            drawdown_pct,
            is_true_equity: false, // fallback, not real snapshots
        });
    }

    result
}

/// Build equity curve from real balance snapshots.
/// Each point represents actual account equity at that date.
pub fn compute_equity_curve_from_snapshots(
    snapshots: &[(NaiveDate, Decimal)],
) -> Vec<EquityCurvePoint> {
    let mut result = Vec::with_capacity(snapshots.len());
    let mut peak = Decimal::ZERO;
    let hundred = Decimal::from(100);

    for &(date, equity) in snapshots {
        if equity > peak {
            peak = equity;
        }
        let drawdown = equity - peak;
        let drawdown_pct = if peak > Decimal::ZERO {
            (drawdown / peak) * hundred
        } else {
            Decimal::ZERO
        };

        result.push(EquityCurvePoint {
            date,
            cumulative_pnl: equity, // for backward compat with frontend
            equity: Some(equity),
            peak,
            drawdown,
            drawdown_pct,
            is_true_equity: true,
        });
    }

    result
}

/// Bucket daily returns into 1% bands for histogram display.
/// Returns are computed as day_pnl / abs(prior_cumulative) * 100.
/// Days with zero starting equity are excluded (no meaningful return).
pub fn compute_return_buckets(daily_pnls: &[(NaiveDate, Decimal)]) -> Vec<ReturnBucket> {
    let mut buckets: BTreeMap<i32, i64> = BTreeMap::new();
    let mut cumulative = Decimal::ZERO;
    let hundred = Decimal::from(100);

    for &(_date, pnl) in daily_pnls {
        let equity_start = cumulative;
        cumulative += pnl;

        if equity_start.abs() > Decimal::ZERO {
            let return_pct = (pnl / equity_start.abs()) * hundred;
            let bucket = return_pct.floor().to_i32().unwrap_or(0);
            *buckets.entry(bucket).or_insert(0) += 1;
        }
    }

    buckets
        .into_iter()
        .map(|(b, count)| ReturnBucket {
            bucket_label: format!("{}%", b),
            day_count: count,
        })
        .collect()
}

// ── TimeSeriesService ────────────────────────────────────────────────

pub struct TimeSeriesService {
    pool: PgPool,
    snapshot_svc: super::balance_snapshot::BalanceSnapshotService,
}

impl TimeSeriesService {
    pub fn new(pool: PgPool) -> Self {
        let snapshot_svc = super::balance_snapshot::BalanceSnapshotService::new(pool.clone());
        Self { pool, snapshot_svc }
    }

    /// JNL-13: Equity curve with three-tier data source:
    /// 1. Real balance snapshots (is_true_equity = true)
    /// 2. starting_balance + cumulative P&L (equity field populated)
    /// 3. Raw cumulative P&L (original behavior)
    pub async fn equity_curve(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<Vec<EquityCurvePoint>, sqlx::Error> {
        // Tier 1: Try real balance snapshots
        if self.snapshot_svc.has_snapshots(user_id, filter.date_from, filter.date_to).await? {
            let snapshots = self.snapshot_svc
                .daily_equity(user_id, filter.date_from, filter.date_to)
                .await?;
            if !snapshots.is_empty() {
                let data: Vec<(NaiveDate, Decimal)> = snapshots
                    .iter()
                    .map(|r| (r.snapshot_date, r.equity))
                    .collect();
                return Ok(compute_equity_curve_from_snapshots(&data));
            }
        }

        // Tier 2 & 3: Fall back to daily P&L aggregates
        let rows = self.fetch_daily_aggregates(user_id, filter).await?;
        let daily_pnls: Vec<(NaiveDate, Decimal)> =
            rows.iter().map(|r| (r.stat_date, r.net_pnl)).collect();

        // Tier 2: Check for starting_balance on exchange account
        let starting_balance = if let Some(ref exchange) = filter.exchange {
            if let Ok(Some(account_id)) = self.snapshot_svc
                .resolve_account_id(user_id, exchange)
                .await
            {
                self.snapshot_svc.get_starting_balance(account_id).await.unwrap_or(None)
            } else {
                None
            }
        } else {
            None
        };

        Ok(compute_equity_curve_with_base(&daily_pnls, starting_balance))
    }

    /// FR-2: Daily P&L bar chart data.
    pub async fn daily_pnl(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<Vec<DailyPnlPoint>, sqlx::Error> {
        let rows = self.fetch_daily_aggregates(user_id, filter).await?;
        Ok(rows
            .into_iter()
            .map(|r| DailyPnlPoint {
                date: r.stat_date,
                net_pnl: r.net_pnl,
                trade_count: r.trade_count as i32,
                win_count: r.win_count as i32,
            })
            .collect())
    }

    /// FR-4: Symbol distribution — trade count and P&L per symbol.
    pub async fn symbol_breakdown(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<Vec<SymbolBreakdown>, sqlx::Error> {
        let rows = sqlx::query_as::<_, SymbolRow>(
            "SELECT symbol, \
                COUNT(*) as trade_count, \
                COALESCE(SUM(net_pnl), 0) as net_pnl, \
                COALESCE( \
                    (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC \
                    / GREATEST(COUNT(*), 1) * 100, 0 \
                ) as win_rate, \
                AVG(r_multiple) FILTER (WHERE r_multiple IS NOT NULL) as avg_r \
            FROM journal_trades \
            WHERE user_id = $1 \
                AND ($2::TEXT IS NULL OR exchange = $2) \
                AND ($3::TEXT IS NULL OR symbol = $3) \
                AND ($4::DATE IS NULL OR closed_at >= $4) \
                AND ($5::DATE IS NULL OR closed_at <= $5) \
            GROUP BY symbol \
            ORDER BY trade_count DESC",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(&filter.symbol)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SymbolBreakdown {
                symbol: r.symbol,
                trade_count: r.trade_count,
                net_pnl: r.net_pnl,
                win_rate: r.win_rate,
                avg_r: r.avg_r,
            })
            .collect())
    }

    /// RSK-02: Setup distribution — per-setup aggregates grouped case-insensitively.
    /// NULL or empty `setup_tag` is bucketed under `(untagged)`.
    pub async fn setup_breakdown(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<Vec<SetupBreakdown>, sqlx::Error> {
        let rows = sqlx::query_as::<_, SetupRow>(
            "SELECT \
                COALESCE(NULLIF(LOWER(setup_tag), ''), '(untagged)') AS setup_tag, \
                COUNT(*) AS trade_count, \
                COALESCE(SUM(net_pnl), 0) AS net_pnl, \
                COALESCE( \
                    (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC \
                    / GREATEST(COUNT(*), 1) * 100, 0 \
                ) AS win_rate, \
                AVG(r_multiple) FILTER (WHERE r_multiple IS NOT NULL) AS avg_r, \
                COALESCE(SUM(net_pnl) / GREATEST(COUNT(*), 1), 0) AS expectancy \
            FROM journal_trades \
            WHERE user_id = $1 \
                AND ($2::TEXT IS NULL OR exchange = $2) \
                AND ($3::TEXT IS NULL OR symbol = $3) \
                AND ($4::DATE IS NULL OR closed_at >= $4) \
                AND ($5::DATE IS NULL OR closed_at <= $5) \
            GROUP BY COALESCE(NULLIF(LOWER(setup_tag), ''), '(untagged)') \
            ORDER BY expectancy DESC",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(&filter.symbol)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SetupBreakdown {
                setup_tag: r.setup_tag,
                trade_count: r.trade_count,
                net_pnl: r.net_pnl,
                win_rate: r.win_rate,
                avg_r: r.avg_r,
                expectancy: r.expectancy,
            })
            .collect())
    }

    /// FR-5: Duration vs profitability scatter data.
    pub async fn duration_profit(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<Vec<DurationProfitPoint>, sqlx::Error> {
        let rows = sqlx::query_as::<_, DurationRow>(
            "SELECT duration_secs, net_pnl, symbol, side \
            FROM journal_trades \
            WHERE user_id = $1 \
                AND ($2::TEXT IS NULL OR exchange = $2) \
                AND ($3::TEXT IS NULL OR symbol = $3) \
                AND ($4::DATE IS NULL OR closed_at >= $4) \
                AND ($5::DATE IS NULL OR closed_at <= $5) \
            ORDER BY closed_at",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(&filter.symbol)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DurationProfitPoint {
                duration_minutes: r.duration_secs as f64 / 60.0,
                net_pnl: r.net_pnl,
                symbol: r.symbol,
                side: r.side,
            })
            .collect())
    }

    /// FR-6: Return distribution histogram (bucketed daily % returns).
    pub async fn return_distribution(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<Vec<ReturnBucket>, sqlx::Error> {
        let rows = self.fetch_daily_aggregates(user_id, filter).await?;
        let daily_pnls: Vec<(NaiveDate, Decimal)> =
            rows.iter().map(|r| (r.stat_date, r.net_pnl)).collect();
        Ok(compute_return_buckets(&daily_pnls))
    }

    /// FR-8: Trade time distribution (hour-of-day, day-of-week).
    pub async fn time_distribution(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<Vec<TimeDistribution>, sqlx::Error> {
        let rows = sqlx::query_as::<_, TimeRow>(
            "SELECT \
                EXTRACT(HOUR FROM opened_at)::INTEGER as hour, \
                (EXTRACT(ISODOW FROM opened_at)::INTEGER - 1) as day_of_week, \
                COUNT(*)::BIGINT as trade_count, \
                COALESCE(SUM(net_pnl), 0) as net_pnl \
            FROM journal_trades \
            WHERE user_id = $1 \
                AND ($2::TEXT IS NULL OR exchange = $2) \
                AND ($3::TEXT IS NULL OR symbol = $3) \
                AND ($4::DATE IS NULL OR closed_at >= $4) \
                AND ($5::DATE IS NULL OR closed_at <= $5) \
            GROUP BY hour, day_of_week \
            ORDER BY day_of_week, hour",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(&filter.symbol)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| TimeDistribution {
                hour: r.hour,
                day_of_week: r.day_of_week,
                trade_count: r.trade_count,
                net_pnl: r.net_pnl,
            })
            .collect())
    }

    // ── Internal query helpers ────────────────────────────────────────

    /// Fetch daily stats aggregated across exchanges.
    async fn fetch_daily_aggregates(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<Vec<DailyRow>, sqlx::Error> {
        sqlx::query_as::<_, DailyRow>(
            "SELECT stat_date, \
                SUM(net_pnl) as net_pnl, \
                SUM(trade_count)::BIGINT as trade_count, \
                SUM(win_count)::BIGINT as win_count \
            FROM journal_daily_stats \
            WHERE user_id = $1 \
                AND ($2::TEXT IS NULL OR exchange = $2) \
                AND ($3::DATE IS NULL OR stat_date >= $3) \
                AND ($4::DATE IS NULL OR stat_date <= $4) \
            GROUP BY stat_date \
            ORDER BY stat_date",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_all(&self.pool)
        .await
    }
}

// ── Unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    // ── compute_equity_curve ─────────────────────────────────────────

    #[test]
    fn test_empty_equity_curve() {
        let result = compute_equity_curve(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_day_equity_curve() {
        let data = vec![(date(2026, 3, 1), dec!(100))];
        let result = compute_equity_curve(&data);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].cumulative_pnl, dec!(100));
        assert_eq!(result[0].equity, None);
        assert!(!result[0].is_true_equity);
        assert_eq!(result[0].peak, dec!(100));
        assert_eq!(result[0].drawdown, Decimal::ZERO);
        assert_eq!(result[0].drawdown_pct, Decimal::ZERO);
    }

    #[test]
    fn test_equity_curve_with_starting_balance() {
        let data = vec![
            (date(2026, 3, 1), dec!(10)),   // equity=1010, peak=1010
            (date(2026, 3, 2), dec!(-9)),   // equity=1001, peak=1010, dd=-9
        ];
        let result = compute_equity_curve_with_base(&data, Some(dec!(1000)));

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].equity, Some(dec!(1010)));
        assert_eq!(result[0].peak, dec!(1010));
        assert_eq!(result[1].equity, Some(dec!(1001)));
        // Drawdown: (1001 - 1010) / 1010 * 100 ≈ -0.89%
        assert!(result[1].drawdown_pct > dec!(-1));
        assert!(result[1].drawdown_pct < Decimal::ZERO);
        assert!(!result[0].is_true_equity); // fallback, not snapshots
    }

    #[test]
    fn test_equity_curve_from_snapshots() {
        let data = vec![
            (date(2026, 3, 1), dec!(1000)),
            (date(2026, 3, 2), dec!(1050)),
            (date(2026, 3, 3), dec!(980)),
        ];
        let result = compute_equity_curve_from_snapshots(&data);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].equity, Some(dec!(1000)));
        assert!(result[0].is_true_equity);
        assert_eq!(result[1].peak, dec!(1050));
        assert_eq!(result[2].equity, Some(dec!(980)));
        // Drawdown: (980 - 1050) / 1050 * 100
        assert!(result[2].drawdown_pct < Decimal::ZERO);
        // cumulative_pnl set to equity for backward compat
        assert_eq!(result[2].cumulative_pnl, dec!(980));
    }

    #[test]
    fn test_equity_curve_with_drawdown() {
        let data = vec![
            (date(2026, 3, 1), dec!(100)),  // cum=100, peak=100
            (date(2026, 3, 2), dec!(50)),   // cum=150, peak=150
            (date(2026, 3, 3), dec!(-80)),  // cum=70,  peak=150, dd=-80
            (date(2026, 3, 4), dec!(30)),   // cum=100, peak=150, dd=-50
        ];
        let result = compute_equity_curve(&data);

        assert_eq!(result.len(), 4);

        // Day 1-2: no drawdown
        assert_eq!(result[0].drawdown, Decimal::ZERO);
        assert_eq!(result[1].drawdown, Decimal::ZERO);

        // Day 3: drawdown of -80 from peak 150
        assert_eq!(result[2].cumulative_pnl, dec!(70));
        assert_eq!(result[2].peak, dec!(150));
        assert_eq!(result[2].drawdown, dec!(-80));
        // dd_pct = -80/150 * 100
        assert!(result[2].drawdown_pct < Decimal::ZERO);

        // Day 4: partial recovery, drawdown still -50
        assert_eq!(result[3].cumulative_pnl, dec!(100));
        assert_eq!(result[3].peak, dec!(150));
        assert_eq!(result[3].drawdown, dec!(-50));
    }

    #[test]
    fn test_equity_curve_all_gains() {
        let data = vec![
            (date(2026, 3, 1), dec!(10)),
            (date(2026, 3, 2), dec!(20)),
            (date(2026, 3, 3), dec!(30)),
        ];
        let result = compute_equity_curve(&data);

        for point in &result {
            assert_eq!(point.drawdown, Decimal::ZERO);
            assert_eq!(point.drawdown_pct, Decimal::ZERO);
        }
        assert_eq!(result[2].cumulative_pnl, dec!(60));
        assert_eq!(result[2].peak, dec!(60));
    }

    #[test]
    fn test_equity_curve_all_losses() {
        let data = vec![
            (date(2026, 3, 1), dec!(-10)),
            (date(2026, 3, 2), dec!(-20)),
        ];
        let result = compute_equity_curve(&data);

        // Peak stays at 0, drawdown = cumulative
        assert_eq!(result[0].cumulative_pnl, dec!(-10));
        assert_eq!(result[0].peak, Decimal::ZERO);
        assert_eq!(result[0].drawdown, dec!(-10));
        assert_eq!(result[0].drawdown_pct, Decimal::ZERO); // peak is 0, can't compute %

        assert_eq!(result[1].cumulative_pnl, dec!(-30));
        assert_eq!(result[1].drawdown, dec!(-30));
    }

    #[test]
    fn test_equity_curve_new_peak_after_drawdown() {
        let data = vec![
            (date(2026, 3, 1), dec!(100)),   // cum=100, peak=100
            (date(2026, 3, 2), dec!(-30)),   // cum=70,  peak=100
            (date(2026, 3, 3), dec!(60)),    // cum=130, peak=130 (new peak)
        ];
        let result = compute_equity_curve(&data);

        assert_eq!(result[2].cumulative_pnl, dec!(130));
        assert_eq!(result[2].peak, dec!(130));
        assert_eq!(result[2].drawdown, Decimal::ZERO);
    }

    // ── compute_return_buckets ───────────────────────────────────────

    #[test]
    fn test_empty_return_buckets() {
        let result = compute_return_buckets(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_first_day_excluded_from_buckets() {
        let data = vec![(date(2026, 3, 1), dec!(100))];
        let result = compute_return_buckets(&data);
        assert!(result.is_empty());
    }

    #[test]
    fn test_return_buckets_positive() {
        let data = vec![
            (date(2026, 3, 1), dec!(100)), // cum=100, excluded
            (date(2026, 3, 2), dec!(5)),   // return = 5/100*100 = 5% → bucket 5
        ];
        let result = compute_return_buckets(&data);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bucket_label, "5%");
        assert_eq!(result[0].day_count, 1);
    }

    #[test]
    fn test_return_buckets_negative() {
        let data = vec![
            (date(2026, 3, 1), dec!(100)),
            (date(2026, 3, 2), dec!(-3)), // return = -3/100*100 = -3% → bucket -3
        ];
        let result = compute_return_buckets(&data);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bucket_label, "-3%");
        assert_eq!(result[0].day_count, 1);
    }

    #[test]
    fn test_return_buckets_fractional_floors() {
        let data = vec![
            (date(2026, 3, 1), dec!(100)),
            (date(2026, 3, 2), dec!(1.5)),  // 1.5% → floor → bucket "1%"
            (date(2026, 3, 3), dec!(-1.5)), // equity=101.5, return=-1.477% → floor → "-2%"
        ];
        let result = compute_return_buckets(&data);

        assert_eq!(result.len(), 2);
        // BTreeMap orders by key: -2 first, then 1
        assert_eq!(result[0].bucket_label, "-2%");
        assert_eq!(result[1].bucket_label, "1%");
    }

    #[test]
    fn test_return_buckets_multiple_same_bucket() {
        let data = vec![
            (date(2026, 3, 1), dec!(100)),
            (date(2026, 3, 2), dec!(2)),   // 2/100*100 = 2% → bucket 2
            (date(2026, 3, 3), dec!(2.5)), // 2.5/102*100 ≈ 2.45% → bucket 2
        ];
        let result = compute_return_buckets(&data);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bucket_label, "2%");
        assert_eq!(result[0].day_count, 2);
    }

    #[test]
    fn test_return_buckets_zero_pnl_day_skipped() {
        let data = vec![
            (date(2026, 3, 1), dec!(100)),
            (date(2026, 3, 2), dec!(0)), // 0% → floor → bucket "0%"
        ];
        let result = compute_return_buckets(&data);

        // 0/100*100 = 0%, floor(0) = 0
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bucket_label, "0%");
        assert_eq!(result[0].day_count, 1);
    }

    #[test]
    fn test_return_buckets_ordered_by_bucket() {
        let data = vec![
            (date(2026, 3, 1), dec!(100)),
            (date(2026, 3, 2), dec!(5)),   // 5%
            (date(2026, 3, 3), dec!(-10)), // ~-9.52% → bucket -10
            (date(2026, 3, 4), dec!(1)),   // ~1.04% → bucket 1
        ];
        let result = compute_return_buckets(&data);

        // BTreeMap returns sorted keys: -10, 1, 5
        assert_eq!(result.len(), 3);
        assert!(result[0].bucket_label.starts_with('-'));
        assert_eq!(result[1].bucket_label, "1%");
        assert_eq!(result[2].bucket_label, "5%");
    }
}
