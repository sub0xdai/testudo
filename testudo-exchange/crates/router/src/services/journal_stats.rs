//! JNL-03: Statistics Computation Engine
//!
//! Transforms raw journal_trades data into 20+ statistics for dashboard panels:
//! account overview, performance, risk, and streaks.
//! All financial math uses `rust_decimal::Decimal` — no f64.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ── Stat structs ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AccountOverview {
    pub total_trades: i64,
    pub total_pnl: Decimal,
    pub total_fees: Decimal,
    pub net_pnl: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceStats {
    pub win_rate: Decimal,
    pub profit_factor: Decimal,
    pub avg_win: Decimal,
    pub avg_loss: Decimal,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
    pub expectancy: Decimal,
    pub avg_r_multiple: Decimal,
    pub trades_per_day: Decimal,
    pub avg_duration_secs: i64,
    pub total_duration_days: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskStats {
    pub max_drawdown: Decimal,
    pub max_drawdown_pct: Decimal,
    pub worst_day: Decimal,
    pub worst_week: Decimal,
    pub worst_month: Decimal,
    pub best_day: Decimal,
    pub best_week: Decimal,
    pub best_month: Decimal,
    pub avg_r_multiple: Decimal,
    pub risk_of_ruin: Decimal,
    pub current_streak: i32,
    pub best_streak: i32,
    pub worst_streak: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct StatsFilter {
    pub exchange: Option<String>,
    pub symbol: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub tags: Option<Vec<String>>,
}

// ── Internal row types for SQL queries ────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct TradeAggRow {
    total: i64,
    wins: i64,
    losses: i64,
    gross_profit: Decimal,
    gross_loss: Decimal,
    avg_win: Decimal,
    avg_loss: Decimal,
    largest_win: Decimal,
    largest_loss: Decimal,
    avg_r: Option<Decimal>,
    avg_duration: Option<Decimal>,
    total_pnl: Decimal,
    total_fees: Decimal,
    net_pnl: Decimal,
    first_trade: Option<chrono::DateTime<chrono::Utc>>,
    last_trade: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct PnlRow {
    net_pnl: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct DayPnlRow {
    net_pnl: Decimal,
}

/// JNL-12 FR-4: SQL-side streak computation result.
#[derive(Debug, sqlx::FromRow)]
struct StreakRow {
    current_streak: i32,
    best_streak: i32,
    worst_streak: i32,
}

/// JNL-12 FR-5: SQL-side max drawdown computation result.
#[derive(Debug, sqlx::FromRow)]
struct DrawdownRow {
    max_drawdown: Decimal,
    max_drawdown_pct: Decimal,
}

/// JNL-12 FR-6: SQL-side day extremes (scalar result).
#[derive(Debug, sqlx::FromRow)]
struct DayExtremesRow {
    worst_day: Decimal,
    best_day: Decimal,
}

// ── Pure computation helpers ──────────────────────────────────────────

/// Compute streak stats from an ordered sequence of trade outcomes.
/// Positive net_pnl = win, non-positive = loss.
/// Returns (current_streak, best_streak, worst_streak).
/// current_streak: positive = consecutive wins, negative = consecutive losses.
pub fn compute_streaks(pnls: &[Decimal]) -> (i32, i32, i32) {
    if pnls.is_empty() {
        return (0, 0, 0);
    }

    let mut current: i32 = 0;
    let mut best: i32 = 0;
    let mut worst: i32 = 0;

    for pnl in pnls {
        if *pnl > Decimal::ZERO {
            // Win
            if current > 0 {
                current += 1;
            } else {
                current = 1;
            }
            if current > best {
                best = current;
            }
        } else {
            // Loss (including breakeven)
            if current < 0 {
                current -= 1;
            } else {
                current = -1;
            }
            if current < worst {
                worst = current;
            }
        }
    }

    (current, best, worst)
}

/// Compute max drawdown from a sequence of cumulative P&L values.
/// Returns (max_drawdown_abs, max_drawdown_pct).
pub fn compute_max_drawdown(cumulative_pnls: &[Decimal]) -> (Decimal, Decimal) {
    if cumulative_pnls.is_empty() {
        return (Decimal::ZERO, Decimal::ZERO);
    }

    let mut peak = cumulative_pnls[0];
    let mut max_dd = Decimal::ZERO;
    let mut max_dd_pct = Decimal::ZERO;

    for &cum in cumulative_pnls {
        if cum > peak {
            peak = cum;
        }
        let dd = peak - cum;
        if dd > max_dd {
            max_dd = dd;
            if peak > Decimal::ZERO {
                max_dd_pct = (dd / peak) * Decimal::from(100);
            }
        }
    }

    (max_dd, max_dd_pct)
}

/// Simplified risk of ruin: (loss_rate) ^ consecutive_losses_to_ruin.
/// Uses 20 as default consecutive losses to ruin (wipes a 5% risk-per-trade account).
pub fn compute_risk_of_ruin(win_rate: Decimal) -> Decimal {
    let hundred = Decimal::from(100);
    if win_rate >= hundred {
        return Decimal::ZERO;
    }
    if win_rate <= Decimal::ZERO {
        return hundred;
    }

    let loss_rate_pct = hundred - win_rate;
    let loss_rate = loss_rate_pct / hundred;
    let consecutive = 20;

    // loss_rate ^ 20 via repeated multiplication (Decimal doesn't have pow for Decimal exponent)
    let mut result = Decimal::ONE;
    for _ in 0..consecutive {
        result *= loss_rate;
    }
    result * hundred // return as percentage
}

// ── StatsEngine ───────────────────────────────────────────────────────

pub struct StatsEngine {
    pool: PgPool,
    snapshot_svc: super::balance_snapshot::BalanceSnapshotService,
}

impl StatsEngine {
    pub fn new(pool: PgPool) -> Self {
        let snapshot_svc = super::balance_snapshot::BalanceSnapshotService::new(pool.clone());
        Self { pool, snapshot_svc }
    }

    /// Compute account overview stats.
    pub async fn account_overview(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<AccountOverview, sqlx::Error> {
        let row = self.aggregate_trades(user_id, filter).await?;

        Ok(AccountOverview {
            total_trades: row.total,
            total_pnl: row.total_pnl,
            total_fees: row.total_fees,
            net_pnl: row.net_pnl,
        })
    }

    /// Compute performance stats: win rate, profit factor, expectancy, etc.
    pub async fn performance_stats(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<PerformanceStats, sqlx::Error> {
        let row = self.aggregate_trades(user_id, filter).await?;

        let total_dec = Decimal::from(row.total);
        let hundred = Decimal::from(100);

        let win_rate = if row.total > 0 {
            (Decimal::from(row.wins) / total_dec) * hundred
        } else {
            Decimal::ZERO
        };

        let loss_rate = if row.total > 0 {
            (Decimal::from(row.losses) / total_dec) * hundred
        } else {
            Decimal::ZERO
        };

        // Profit factor: gross_profit / gross_loss. Capped at 999.99 when no losses.
        let pf_cap = Decimal::new(99999, 2); // 999.99
        let profit_factor = if row.gross_loss > Decimal::ZERO {
            (row.gross_profit / row.gross_loss).min(pf_cap)
        } else if row.gross_profit > Decimal::ZERO {
            pf_cap
        } else {
            Decimal::ZERO
        };

        // Expectancy = (win_rate/100 * avg_win) - (loss_rate/100 * abs(avg_loss))
        let expectancy = (win_rate / hundred * row.avg_win)
            - (loss_rate / hundred * row.avg_loss.abs());

        // Trades per day
        let total_days = match (row.first_trade, row.last_trade) {
            (Some(first), Some(last)) => {
                let days = (last - first).num_days();
                if days > 0 { days } else { 1 }
            }
            _ => 1,
        };

        let trades_per_day = if total_days > 0 {
            total_dec / Decimal::from(total_days)
        } else {
            Decimal::ZERO
        };

        let avg_duration_secs = row
            .avg_duration
            .map(|d| {
                use rust_decimal::prelude::ToPrimitive;
                d.to_i64().unwrap_or(0)
            })
            .unwrap_or(0);

        Ok(PerformanceStats {
            win_rate,
            profit_factor,
            avg_win: row.avg_win,
            avg_loss: row.avg_loss,
            largest_win: row.largest_win,
            largest_loss: row.largest_loss,
            expectancy,
            avg_r_multiple: row.avg_r.unwrap_or(Decimal::ZERO),
            trades_per_day,
            avg_duration_secs,
            total_duration_days: total_days,
        })
    }

    /// Compute risk stats: drawdown, best/worst periods, streaks, risk of ruin.
    /// JNL-12 FR-4/FR-5: Streaks and drawdown computed server-side in SQL — zero
    /// unbounded Vec allocations in Rust.
    pub async fn risk_stats(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<RiskStats, sqlx::Error> {
        let row = self.aggregate_trades(user_id, filter).await?;

        // Win rate for risk of ruin
        let win_rate = if row.total > 0 {
            (Decimal::from(row.wins) / Decimal::from(row.total)) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // JNL-12 FR-4: SQL-side streak computation
        let streaks = self.fetch_streaks_sql(user_id, filter).await?;

        // JNL-13: Prefer snapshot-based drawdown (equity denominator), fall back to P&L-based
        let drawdown = if self.snapshot_svc.has_snapshots(user_id, filter.date_from, filter.date_to).await? {
            let (dd, dd_pct) = self.snapshot_svc
                .max_drawdown(user_id, filter.date_from, filter.date_to)
                .await?;
            DrawdownRow { max_drawdown: dd, max_drawdown_pct: dd_pct }
        } else {
            self.fetch_drawdown_sql(user_id, filter).await?
        };

        // JNL-12 FR-6: Best/worst day as scalar SQL
        let (worst_day, best_day) = self.fetch_day_extremes(user_id, filter).await?;
        let (worst_week, best_week) = self.fetch_rolling_extremes(user_id, filter, 7).await?;
        let (worst_month, best_month) = self.fetch_rolling_extremes(user_id, filter, 30).await?;

        let risk_of_ruin = compute_risk_of_ruin(win_rate);

        Ok(RiskStats {
            max_drawdown: drawdown.max_drawdown,
            max_drawdown_pct: drawdown.max_drawdown_pct,
            worst_day,
            worst_week,
            worst_month,
            best_day,
            best_week,
            best_month,
            avg_r_multiple: row.avg_r.unwrap_or(Decimal::ZERO),
            risk_of_ruin,
            current_streak: streaks.current_streak,
            best_streak: streaks.best_streak,
            worst_streak: streaks.worst_streak,
        })
    }

    // ── Internal query helpers ────────────────────────────────────────

    /// Single-pass aggregate over filtered trades.
    async fn aggregate_trades(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<TradeAggRow, sqlx::Error> {
        let row = sqlx::query_as::<_, TradeAggRow>(
            "SELECT \
                COALESCE(COUNT(*), 0) as total, \
                COALESCE(COUNT(*) FILTER (WHERE net_pnl > 0), 0) as wins, \
                COALESCE(COUNT(*) FILTER (WHERE net_pnl <= 0), 0) as losses, \
                COALESCE(SUM(net_pnl) FILTER (WHERE net_pnl > 0), 0) as gross_profit, \
                COALESCE(ABS(SUM(net_pnl) FILTER (WHERE net_pnl <= 0)), 0) as gross_loss, \
                COALESCE(AVG(net_pnl) FILTER (WHERE net_pnl > 0), 0) as avg_win, \
                COALESCE(AVG(net_pnl) FILTER (WHERE net_pnl <= 0), 0) as avg_loss, \
                COALESCE(MAX(net_pnl), 0) as largest_win, \
                COALESCE(MIN(net_pnl), 0) as largest_loss, \
                AVG(r_multiple) FILTER (WHERE r_multiple IS NOT NULL) as avg_r, \
                AVG(duration_secs) as avg_duration, \
                COALESCE(SUM(realized_pnl), 0) as total_pnl, \
                COALESCE(SUM(fees), 0) as total_fees, \
                COALESCE(SUM(net_pnl), 0) as net_pnl, \
                MIN(closed_at) as first_trade, \
                MAX(closed_at) as last_trade \
            FROM journal_trades \
            WHERE user_id = $1 \
                AND ($2::TEXT IS NULL OR exchange = $2) \
                AND ($3::TEXT IS NULL OR symbol = $3) \
                AND ($4::DATE IS NULL OR closed_at >= $4) \
                AND ($5::DATE IS NULL OR closed_at <= $5)",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(&filter.symbol)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// JNL-12 FR-4: Compute streaks entirely in SQL using gaps-and-islands pattern.
    /// No trade rows are loaded into Rust memory.
    async fn fetch_streaks_sql(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<StreakRow, sqlx::Error> {
        let row = sqlx::query_as::<_, StreakRow>(
            "WITH outcomes AS ( \
                SELECT net_pnl, \
                    CASE WHEN net_pnl > 0 THEN 1 ELSE 0 END as is_win, \
                    ROW_NUMBER() OVER (ORDER BY closed_at) as rn \
                FROM journal_trades \
                WHERE user_id = $1 \
                    AND ($2::TEXT IS NULL OR exchange = $2) \
                    AND ($3::TEXT IS NULL OR symbol = $3) \
                    AND ($4::DATE IS NULL OR closed_at >= $4) \
                    AND ($5::DATE IS NULL OR closed_at <= $5) \
            ), \
            groups AS ( \
                SELECT is_win, \
                    rn - ROW_NUMBER() OVER (PARTITION BY is_win ORDER BY rn) as grp, \
                    rn \
                FROM outcomes \
            ), \
            streaks AS ( \
                SELECT is_win, COUNT(*) as streak_len, \
                    MAX(rn) as last_rn \
                FROM groups GROUP BY is_win, grp \
            ), \
            max_rn AS (SELECT MAX(rn) as total FROM outcomes) \
            SELECT \
                COALESCE((SELECT CASE WHEN s.is_win = 1 THEN s.streak_len::INTEGER \
                                      ELSE -(s.streak_len::INTEGER) END \
                          FROM streaks s, max_rn m \
                          WHERE s.last_rn = m.total LIMIT 1), 0) as current_streak, \
                COALESCE((SELECT MAX(streak_len)::INTEGER FROM streaks WHERE is_win = 1), 0) as best_streak, \
                COALESCE((SELECT -(MAX(streak_len)::INTEGER) FROM streaks WHERE is_win = 0), 0) as worst_streak",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(&filter.symbol)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// JNL-12 FR-5: Compute max drawdown entirely in SQL.
    /// No trade rows are loaded into Rust memory.
    async fn fetch_drawdown_sql(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<DrawdownRow, sqlx::Error> {
        let row = sqlx::query_as::<_, DrawdownRow>(
            "WITH cumulative AS ( \
                SELECT \
                    SUM(net_pnl) OVER (ORDER BY closed_at) as cum_pnl \
                FROM journal_trades \
                WHERE user_id = $1 \
                    AND ($2::TEXT IS NULL OR exchange = $2) \
                    AND ($3::TEXT IS NULL OR symbol = $3) \
                    AND ($4::DATE IS NULL OR closed_at >= $4) \
                    AND ($5::DATE IS NULL OR closed_at <= $5) \
            ), \
            peaks AS ( \
                SELECT cum_pnl, \
                    MAX(cum_pnl) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) as peak \
                FROM cumulative \
            ) \
            SELECT \
                COALESCE(MAX(peak - cum_pnl), 0) as max_drawdown, \
                COALESCE(MAX(CASE WHEN peak > 0 THEN (peak - cum_pnl) / peak * 100 ELSE 0 END), 0) as max_drawdown_pct \
            FROM peaks",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(&filter.symbol)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// JNL-12 FR-6: Best/worst day as scalar SQL — returns one row, not Vec.
    async fn fetch_day_extremes(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
    ) -> Result<(Decimal, Decimal), sqlx::Error> {
        let row = sqlx::query_as::<_, DayExtremesRow>(
            "SELECT \
                COALESCE(MIN(net_pnl), 0) as worst_day, \
                COALESCE(MAX(net_pnl), 0) as best_day \
            FROM journal_daily_stats \
            WHERE user_id = $1 \
                AND ($2::TEXT IS NULL OR exchange = $2) \
                AND ($3::DATE IS NULL OR stat_date >= $3) \
                AND ($4::DATE IS NULL OR stat_date <= $4)",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .fetch_one(&self.pool)
        .await?;

        Ok((row.worst_day, row.best_day))
    }

    /// Fetch best/worst rolling N-day window from daily stats.
    /// MIN/MAX applied in SQL — returns a 2-tuple, not a Vec.
    async fn fetch_rolling_extremes(
        &self,
        user_id: Uuid,
        filter: &StatsFilter,
        window: i32,
    ) -> Result<(Decimal, Decimal), sqlx::Error> {
        let row: (Option<Decimal>, Option<Decimal>) = sqlx::query_as(
            "SELECT MIN(rolling_pnl), MAX(rolling_pnl) FROM ( \
                SELECT SUM(net_pnl) OVER ( \
                    ORDER BY stat_date \
                    ROWS BETWEEN ($5 - 1) PRECEDING AND CURRENT ROW \
                ) AS rolling_pnl \
                FROM journal_daily_stats \
                WHERE user_id = $1 \
                    AND ($2::TEXT IS NULL OR exchange = $2) \
                    AND ($3::DATE IS NULL OR stat_date >= $3) \
                    AND ($4::DATE IS NULL OR stat_date <= $4) \
            ) sub",
        )
        .bind(user_id)
        .bind(&filter.exchange)
        .bind(filter.date_from)
        .bind(filter.date_to)
        .bind(window)
        .fetch_one(&self.pool)
        .await?;

        let worst = row.0.unwrap_or(Decimal::ZERO);
        let best = row.1.unwrap_or(Decimal::ZERO);

        Ok((worst, best))
    }
}

// ── Unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ── compute_streaks ───────────────────────────────────────────────

    #[test]
    fn test_empty_trades_returns_zero_streaks() {
        let (current, best, worst) = compute_streaks(&[]);
        assert_eq!(current, 0);
        assert_eq!(best, 0);
        assert_eq!(worst, 0);
    }

    #[test]
    fn test_single_winning_trade() {
        let (current, best, worst) = compute_streaks(&[dec!(100)]);
        assert_eq!(current, 1);
        assert_eq!(best, 1);
        assert_eq!(worst, 0);
    }

    #[test]
    fn test_single_losing_trade() {
        let (current, best, worst) = compute_streaks(&[dec!(-50)]);
        assert_eq!(current, -1);
        assert_eq!(best, 0);
        assert_eq!(worst, -1);
    }

    #[test]
    fn test_all_wins() {
        let pnls = vec![dec!(10), dec!(20), dec!(30), dec!(5)];
        let (current, best, worst) = compute_streaks(&pnls);
        assert_eq!(current, 4);
        assert_eq!(best, 4);
        assert_eq!(worst, 0);
    }

    #[test]
    fn test_all_losses() {
        let pnls = vec![dec!(-10), dec!(-20), dec!(-5)];
        let (current, best, worst) = compute_streaks(&pnls);
        assert_eq!(current, -3);
        assert_eq!(best, 0);
        assert_eq!(worst, -3);
    }

    #[test]
    fn test_mixed_streaks() {
        // W W W L L W W W W L
        let pnls = vec![
            dec!(10),
            dec!(20),
            dec!(30),
            dec!(-5),
            dec!(-10),
            dec!(15),
            dec!(25),
            dec!(35),
            dec!(45),
            dec!(-1),
        ];
        let (current, best, worst) = compute_streaks(&pnls);
        assert_eq!(current, -1); // ends on a loss
        assert_eq!(best, 4);     // W W W W
        assert_eq!(worst, -2);   // L L
    }

    #[test]
    fn test_breakeven_counts_as_loss() {
        let pnls = vec![dec!(10), dec!(0), dec!(10)];
        let (current, best, worst) = compute_streaks(&pnls);
        assert_eq!(current, 1);
        assert_eq!(best, 1);
        assert_eq!(worst, -1);
    }

    // ── compute_max_drawdown ──────────────────────────────────────────

    #[test]
    fn test_empty_drawdown() {
        let (dd, pct) = compute_max_drawdown(&[]);
        assert_eq!(dd, Decimal::ZERO);
        assert_eq!(pct, Decimal::ZERO);
    }

    #[test]
    fn test_only_gains_no_drawdown() {
        let cum = vec![dec!(10), dec!(20), dec!(30)];
        let (dd, _) = compute_max_drawdown(&cum);
        assert_eq!(dd, Decimal::ZERO);
    }

    #[test]
    fn test_simple_drawdown() {
        // Goes up to 100, drops to 60, then recovers to 80
        let cum = vec![dec!(50), dec!(100), dec!(60), dec!(80)];
        let (dd, pct) = compute_max_drawdown(&cum);
        assert_eq!(dd, dec!(40));  // 100 -> 60
        assert_eq!(pct, dec!(40)); // 40/100 * 100
    }

    #[test]
    fn test_drawdown_from_negative_start() {
        // Start negative, never positive — peak is the least negative
        let cum = vec![dec!(-10), dec!(-5), dec!(-20)];
        let (dd, _) = compute_max_drawdown(&cum);
        assert_eq!(dd, dec!(15)); // -5 peak, drop to -20 = 15
    }

    // ── compute_risk_of_ruin ──────────────────────────────────────────

    #[test]
    fn test_100_pct_win_rate_zero_ruin() {
        assert_eq!(compute_risk_of_ruin(dec!(100)), Decimal::ZERO);
    }

    #[test]
    fn test_0_pct_win_rate_100_ruin() {
        assert_eq!(compute_risk_of_ruin(dec!(0)), Decimal::from(100));
    }

    #[test]
    fn test_50_pct_win_rate_risk_of_ruin() {
        let ror = compute_risk_of_ruin(dec!(50));
        // 0.5^20 * 100 = 0.00009536743... %
        assert!(ror > Decimal::ZERO);
        assert!(ror < dec!(0.001));
    }

    #[test]
    fn test_high_loss_rate_high_ruin() {
        let ror = compute_risk_of_ruin(dec!(20));
        // 0.8^20 * 100 ≈ 1.15%
        assert!(ror > dec!(1));
        assert!(ror < dec!(2));
    }

    // ── Performance stats edge cases (pure math) ──────────────────────

    #[test]
    fn test_profit_factor_no_losses() {
        // When gross_loss is zero but there are profits → capped at 999.99
        let gross_profit = dec!(500);
        let gross_loss = Decimal::ZERO;
        let pf_cap = Decimal::new(99999, 2);
        let pf = if gross_loss > Decimal::ZERO {
            (gross_profit / gross_loss).min(pf_cap)
        } else if gross_profit > Decimal::ZERO {
            pf_cap
        } else {
            Decimal::ZERO
        };
        assert_eq!(pf, dec!(999.99));
    }

    #[test]
    fn test_profit_factor_no_trades() {
        let gross_profit = Decimal::ZERO;
        let gross_loss = Decimal::ZERO;
        let pf = if gross_loss > Decimal::ZERO {
            gross_profit / gross_loss
        } else if gross_profit > Decimal::ZERO {
            Decimal::MAX
        } else {
            Decimal::ZERO
        };
        assert_eq!(pf, Decimal::ZERO);
    }

    #[test]
    fn test_profit_factor_normal() {
        let pf = dec!(3000) / dec!(1000);
        assert_eq!(pf, dec!(3));
    }

    #[test]
    fn test_expectancy_calculation() {
        // win_rate = 60%, avg_win = 200, loss_rate = 40%, avg_loss = -150
        let win_rate = dec!(60);
        let loss_rate = dec!(40);
        let avg_win = dec!(200);
        let avg_loss = dec!(-150); // stored as negative
        let hundred = dec!(100);

        let expectancy =
            (win_rate / hundred * avg_win) - (loss_rate / hundred * avg_loss.abs());
        // 0.6 * 200 - 0.4 * 150 = 120 - 60 = 60
        assert_eq!(expectancy, dec!(60));
    }

    #[test]
    fn test_win_rate_zero_trades() {
        let total = 0i64;
        let win_rate = if total > 0 {
            (Decimal::from(0i64) / Decimal::from(total)) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        assert_eq!(win_rate, Decimal::ZERO);
    }
}
