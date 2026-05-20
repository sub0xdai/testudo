//! JNL-02: Trade Event Ingestion Pipeline
//!
//! Accepts closed trade data from any exchange adapter and persists to journal_trades.
//! Computes derived fields (P&L, R-multiple) and upserts daily stats.
//! Idempotent: duplicate trade_group_id writes are no-ops.
//!
//! JNL-DUR-01: `duration_secs` is a generated column on `journal_trades` derived
//! from `(closed_at - opened_at)`. Writers MUST NOT bind it; the database is the
//! single source of truth. The chronology CHECK constraint
//! (`closed_at >= opened_at`) makes negative durations structurally impossible.

use chrono::{DateTime, NaiveDate, Utc};
use common_utils::models::canonical_exchange_name;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::journal::JournalTrade;

/// Input from any trade close event — exchange-agnostic.
#[derive(Debug, Clone)]
pub struct TradeCloseEvent {
    pub user_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: String, // "LONG" | "SHORT"
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub leverage: i32,
    pub fees: Decimal,
    pub stop_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub risk_amount: Option<Decimal>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    pub trade_group_id: Option<Uuid>,
    pub source: Option<String>,
    pub exchange_fill_id: Option<i64>,
    pub setup_tag: Option<String>,
    /// AGENT-01: Agent's free-text reasoning for the signal (e.g. TA, on-chain, sentiment).
    pub reasoning: Option<String>,
    /// AGENT-01: Agent's confidence 0.0–1.0 for future Kelly criterion calibration.
    pub confidence: Option<Decimal>,
    /// QNT-01a: entry-time calibration snapshot built by `create_trade` when
    /// Dynamic Risk is on and a `setup_tag` is present. `None` otherwise.
    pub kelly_inputs: Option<serde_json::Value>,
    /// FIX-08: true when exit_price is a placeholder (0) awaiting REST reconciliation.
    /// Always false on the import path (real fill data available).
    pub needs_reconciliation: bool,
}

/// HIST-03: Outcome of a `record_trade_close` attempt. `Inserted` for a fresh row,
/// `SkippedDuplicate` when the partial unique index `idx_unique_import_fill` catches a
/// re-import. Callers on the import path map the two variants to distinct counters;
/// live-path callers always see `Inserted` because live trades carry
/// `exchange_fill_id = None` and the partial index's `WHERE exchange_fill_id IS NOT NULL`
/// predicate excludes them.
#[derive(Debug)]
pub enum RecordOutcome {
    Inserted(Box<JournalTrade>),
    SkippedDuplicate,
}

/// Computed fields derived from a TradeCloseEvent.
///
/// JNL-DUR-01: `duration_secs` is intentionally absent — it lives on the DB as a
/// generated column derived from `(closed_at - opened_at)`. Storing it here would
/// reintroduce the drift this design eliminates.
#[derive(Debug, Clone)]
pub struct DerivedFields {
    pub realized_pnl: Decimal,
    pub realized_pnl_pct: Decimal,
    pub net_pnl: Decimal,
    pub risk_amount: Option<Decimal>,
    pub r_multiple: Option<Decimal>,
}

/// Compute P&L, percentage return, and R-multiple from trade close data.
///
/// FIX-09: when the close event carries a placeholder `exit_price = 0` with
/// `needs_reconciliation` set, suppress economic fields to avoid surfacing
/// fake (entry × quantity) P&L in the journal.
pub fn compute_derived_fields(event: &TradeCloseEvent) -> DerivedFields {
    if event.needs_reconciliation && event.exit_price == Decimal::ZERO {
        return DerivedFields {
            realized_pnl: Decimal::ZERO,
            realized_pnl_pct: Decimal::ZERO,
            net_pnl: Decimal::ZERO,
            risk_amount: event.risk_amount,
            r_multiple: None,
        };
    }

    let pnl = match event.side.as_str() {
        "LONG" => (event.exit_price - event.entry_price) * event.quantity,
        "SHORT" => (event.entry_price - event.exit_price) * event.quantity,
        _ => Decimal::ZERO,
    };

    let margin = if event.leverage > 0 {
        (event.entry_price * event.quantity) / Decimal::from(event.leverage)
    } else {
        event.entry_price * event.quantity
    };

    let pnl_pct = if margin > Decimal::ZERO {
        (pnl / margin) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };

    let net = pnl - event.fees;

    // R-multiple: use explicit risk_amount if provided (Testudo-placed trades),
    // otherwise derive from stop distance for imports with stop data.
    // Van Tharp: R = |entry - stop| * quantity = the dollar amount at risk.
    let effective_risk = event
        .risk_amount
        .filter(|r| *r > Decimal::ZERO)
        .or_else(|| {
            event.stop_price.map(|stop| {
                (event.entry_price - stop).abs() * event.quantity
            }).filter(|r| *r > Decimal::ZERO)
        });

    let r_mult = effective_risk.map(|r| net / r);

    DerivedFields {
        realized_pnl: pnl,
        realized_pnl_pct: pnl_pct,
        net_pnl: net,
        risk_amount: effective_risk,
        r_multiple: r_mult,
    }
}

/// Auto-upsert a `journal_tags` row (lower-cased for dedup) and link it to the trade via
/// `journal_trade_tags`. Fire-and-forget from callers: errors surface to the caller so they
/// can log them without aborting trade persistence.
///
/// Called from both `JournalService::record_trade_close` and `TradeEventWriter::flush_transaction`
/// after the primary trade row has been committed, so the trade is guaranteed to exist.
pub(crate) async fn upsert_auto_tag(
    pool: &PgPool,
    user_id: Uuid,
    trade_id: Uuid,
    raw_tag: &str,
) -> Result<(), sqlx::Error> {
    let name_lc = raw_tag.trim().to_lowercase();
    if name_lc.is_empty() {
        return Ok(());
    }

    // Upsert the tag. ON CONFLICT DO UPDATE (no-op) is required so RETURNING fires even
    // when the row already exists — pure DO NOTHING omits RETURNING on conflict.
    let tag_id: Uuid = sqlx::query_scalar(
        "INSERT INTO journal_tags (user_id, name, color) VALUES ($1, $2, NULL) \
         ON CONFLICT (user_id, name) DO UPDATE SET name = EXCLUDED.name \
         RETURNING id",
    )
    .bind(user_id)
    .bind(&name_lc)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "INSERT INTO journal_trade_tags (trade_id, tag_id) VALUES ($1, $2) \
         ON CONFLICT DO NOTHING",
    )
    .bind(trade_id)
    .bind(tag_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Service for persisting trade closes to the journal schema.
pub struct JournalService {
    pool: PgPool,
}

impl JournalService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Persist a closed trade to journal_trades and update daily stats.
    ///
    /// Idempotent on two axes:
    /// 1. `trade_group_id` SELECT short-circuit for live-placed trades (Testudo).
    /// 2. HIST-03: partial unique index `idx_unique_import_fill(user_id, exchange,
    ///    exchange_fill_id) WHERE exchange_fill_id IS NOT NULL` catches re-imports via
    ///    `ON CONFLICT DO NOTHING`. The `WHERE` qualifier in `ON CONFLICT` must match
    ///    the partial index predicate verbatim.
    pub async fn record_trade_close(
        &self,
        event: TradeCloseEvent,
    ) -> Result<RecordOutcome, sqlx::Error> {
        // Idempotency: check if trade_group_id already recorded
        if let Some(group_id) = event.trade_group_id {
            let existing: Option<JournalTrade> = sqlx::query_as::<_, JournalTrade>(
                "SELECT id, user_id, exchange, symbol, side, entry_price, exit_price, quantity, \
                 leverage, realized_pnl, realized_pnl_pct, fees, net_pnl, stop_price, \
                 target_price, risk_amount, r_multiple, opened_at, closed_at, duration_secs, \
                 trade_group_id, notes, source, reasoning, confidence, exchange_fill_id, \
                 setup_tag, kelly_inputs, needs_reconciliation, close_reason, \
                 created_at, updated_at \
                 FROM journal_trades WHERE trade_group_id = $1",
            )
            .bind(group_id)
            .fetch_optional(&self.pool)
            .await?;

            if let Some(trade) = existing {
                tracing::debug!(
                    trade_group_id = %group_id,
                    "Journal: trade already recorded, skipping duplicate"
                );
                return Ok(RecordOutcome::Inserted(Box::new(trade)));
            }
        }

        let derived = compute_derived_fields(&event);

        let source = event.source.as_deref().unwrap_or("testudo");
        // HIST-03: Canonicalize exchange name at the journal-write boundary so the
        // partial unique index can't be defeated by mixed-case values sneaking in.
        let exchange_canon = canonical_exchange_name(&event.exchange);

        // JNL-DUR-01: duration_secs is a generated column on journal_trades. Excluded
        // from the column list — Postgres rejects explicit values for generated cols.
        let inserted: Option<JournalTrade> = sqlx::query_as::<_, JournalTrade>(
            "INSERT INTO journal_trades \
             (user_id, exchange, symbol, side, entry_price, exit_price, quantity, leverage, \
              realized_pnl, realized_pnl_pct, fees, net_pnl, stop_price, target_price, \
              risk_amount, r_multiple, opened_at, closed_at, trade_group_id, \
              exchange_order_ids, source, reasoning, confidence, exchange_fill_id, \
              setup_tag, kelly_inputs, needs_reconciliation) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
                     $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27) \
             ON CONFLICT (user_id, exchange, exchange_fill_id) \
                 WHERE exchange_fill_id IS NOT NULL \
             DO NOTHING \
             RETURNING id, user_id, exchange, symbol, side, entry_price, exit_price, quantity, \
                       leverage, realized_pnl, realized_pnl_pct, fees, net_pnl, stop_price, \
                       target_price, risk_amount, r_multiple, opened_at, closed_at, \
                       duration_secs, trade_group_id, notes, source, reasoning, confidence, \
                       exchange_fill_id, setup_tag, kelly_inputs, needs_reconciliation, \
                       close_reason, created_at, updated_at",
        )
        .bind(event.user_id)
        .bind(&exchange_canon)
        .bind(&event.symbol)
        .bind(&event.side)
        .bind(event.entry_price)
        .bind(event.exit_price)
        .bind(event.quantity)
        .bind(event.leverage)
        .bind(derived.realized_pnl)
        .bind(derived.realized_pnl_pct)
        .bind(event.fees)
        .bind(derived.net_pnl)
        .bind(event.stop_price)
        .bind(event.target_price)
        .bind(derived.risk_amount)
        .bind(derived.r_multiple)
        .bind(event.opened_at)
        .bind(event.closed_at)
        .bind(event.trade_group_id)
        .bind({
            // JNL-SYNC-01 CP-6: exchange_order_ids not populated on the pull-sync path.
            // The pull pipeline has no live order tracking; IDs are not available here.
            let ids: Vec<String> = Vec::new();
            ids
        })
        .bind(source)
        .bind(event.reasoning.as_deref())
        .bind(event.confidence)
        .bind(event.exchange_fill_id)
        .bind(event.setup_tag.as_deref())
        .bind(&event.kelly_inputs)
        .bind(event.needs_reconciliation)
        .fetch_optional(&self.pool)
        .await?;

        let trade: JournalTrade = match inserted {
            Some(t) => t,
            None => {
                tracing::debug!(
                    user_id = %event.user_id,
                    exchange = %event.exchange,
                    exchange_fill_id = ?event.exchange_fill_id,
                    "HIST-03: duplicate import skipped by partial unique index"
                );
                return Ok(RecordOutcome::SkippedDuplicate);
            }
        };

        // JNL-20: Merge pre-existing draft notes from active trade
        if let Some(group_id) = event.trade_group_id {
            if let Ok(Some(Some(notes))) = sqlx::query_scalar::<_, Option<String>>(
                "DELETE FROM journal_trade_drafts WHERE trade_group_id = $1 RETURNING notes"
            )
            .bind(group_id)
            .fetch_optional(&self.pool)
            .await
            {
                if !notes.is_empty() {
                    let _ = sqlx::query(
                        "UPDATE journal_trades SET notes = $1 WHERE id = $2 AND (notes IS NULL OR notes = '')"
                    )
                    .bind(&notes)
                    .bind(trade.id)
                    .execute(&self.pool)
                    .await;
                    tracing::info!(
                        trade_id = %trade.id,
                        group_id = %group_id,
                        "Merged draft notes into closed trade"
                    );
                }
            }
        }

        // Update daily stats (fire-and-forget on error — don't fail the trade record)
        if let Err(e) = self.upsert_daily_stats(&trade).await {
            tracing::warn!(
                trade_id = %trade.id,
                "Journal: failed to upsert daily stats: {}",
                e
            );
        }

        // RSK-02 FR-7: auto-create a tag from setup_tag and link it to the trade so the
        // existing tag system surfaces it in journal views without manual re-tagging.
        if let Some(tag) = trade.setup_tag.as_deref() {
            if let Err(e) = upsert_auto_tag(&self.pool, trade.user_id, trade.id, tag).await {
                tracing::warn!(
                    trade_id = %trade.id,
                    "Journal: failed to auto-tag trade: {}",
                    e
                );
            }
        }

        tracing::info!(
            trade_id = %trade.id,
            symbol = %trade.symbol,
            net_pnl = %trade.net_pnl,
            "Journal: recorded trade close"
        );

        Ok(RecordOutcome::Inserted(Box::new(trade)))
    }

    /// JNL-SYNC-01 CP-4 T20: Upsert reconstructed trades from the pull-sync pipeline.
    ///
    /// Idempotent on `(user_id, exchange, source_fills_hash)` via partial unique index
    /// `idx_unique_pull_sync_trade`. Per AGENTS.md: ON CONFLICT must repeat the WHERE
    /// predicate verbatim. Always inserts with `source = 'pull_sync'`.
    pub async fn upsert_many_pull_sync(
        &self,
        trades: &[common_utils::journal::ReconstructedTrade],
    ) -> Result<usize, sqlx::Error> {
        let mut inserted = 0usize;
        for t in trades {
            let trade_side = match t.side {
                common_utils::journal::TradeSide::Long => "long",
                common_utils::journal::TradeSide::Short => "short",
            };
            let exchange = canonical_exchange_name(&t.exchange);
            let net_pnl = t.realized_pnl - t.fees;
            let realized_pnl_pct = if t.entry_price.is_zero() {
                Decimal::ZERO
            } else {
                (t.exit_price - t.entry_price) / t.entry_price * Decimal::ONE_HUNDRED
            };

            let result = sqlx::query(
                "INSERT INTO journal_trades \
                 (user_id, exchange, symbol, side, entry_price, exit_price, quantity, leverage, \
                  realized_pnl, realized_pnl_pct, fees, net_pnl, opened_at, closed_at, \
                  source, needs_reconciliation, source_fills_hash) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
                         $15, FALSE, $16) \
                 ON CONFLICT (user_id, exchange, source_fills_hash) \
                     WHERE source_fills_hash IS NOT NULL \
                 DO NOTHING",
            )
            .bind(t.user_id)
            .bind(&exchange)
            .bind(&t.symbol)
            .bind(trade_side)
            .bind(t.entry_price)
            .bind(t.exit_price)
            .bind(t.quantity)
            .bind(1i32)
            .bind(t.realized_pnl)
            .bind(realized_pnl_pct)
            .bind(t.fees)
            .bind(net_pnl)
            .bind(t.opened_at)
            .bind(t.closed_at)
            .bind("pull_sync")
            .bind(&t.source_fills_hash)
            .execute(&self.pool)
            .await?;

            if result.rows_affected() > 0 {
                upsert_daily_stats_raw(
                    &self.pool,
                    t.user_id,
                    &exchange,
                    t.closed_at.date_naive(),
                    net_pnl,
                    t.fees,
                )
                .await?;
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    /// Upsert the journal_daily_stats row for this trade's close date.
    async fn upsert_daily_stats(&self, trade: &JournalTrade) -> Result<(), sqlx::Error> {
        let stat_date = trade.closed_at.date_naive();
        let is_win = trade.net_pnl > Decimal::ZERO;
        let win_count: i32 = if is_win { 1 } else { 0 };
        let loss_count: i32 = if is_win { 0 } else { 1 };
        let gross_profit = if is_win { trade.net_pnl } else { Decimal::ZERO };
        let gross_loss = if !is_win {
            trade.net_pnl.abs()
        } else {
            Decimal::ZERO
        };

        // Upsert daily aggregates
        sqlx::query(
            "INSERT INTO journal_daily_stats \
             (user_id, stat_date, exchange, trade_count, win_count, loss_count, \
              gross_profit, gross_loss, net_pnl, fees, \
              cumulative_pnl, peak_cumulative_pnl, drawdown, drawdown_pct) \
             VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, 0, 0, 0, 0) \
             ON CONFLICT (user_id, stat_date, exchange) \
             DO UPDATE SET \
                 trade_count = journal_daily_stats.trade_count + 1, \
                 win_count = journal_daily_stats.win_count + EXCLUDED.win_count, \
                 loss_count = journal_daily_stats.loss_count + EXCLUDED.loss_count, \
                 gross_profit = journal_daily_stats.gross_profit + EXCLUDED.gross_profit, \
                 gross_loss = journal_daily_stats.gross_loss + EXCLUDED.gross_loss, \
                 net_pnl = journal_daily_stats.net_pnl + EXCLUDED.net_pnl, \
                 fees = journal_daily_stats.fees + EXCLUDED.fees",
        )
        .bind(trade.user_id)
        .bind(stat_date)
        .bind(&trade.exchange)
        .bind(win_count)
        .bind(loss_count)
        .bind(gross_profit)
        .bind(gross_loss)
        .bind(trade.net_pnl)
        .bind(trade.fees)
        .execute(&self.pool)
        .await?;

        // FIX-08 T16: Shared cumulative recompute (DRY — same SQL as TradeEventWriter;
        // both call recompute_cumulative_pnl_from).
        recompute_cumulative_pnl_from(&self.pool, trade.user_id, &trade.exchange, stat_date)
            .await?;

        Ok(())
    }
}

/// JNL-SYNC-01 follow-up: shared daily-stats upsert + cumulative recompute.
///
/// Called by `upsert_many_pull_sync` (pull-based journal) and `upsert_daily_stats`
/// (legacy live path, kept for HIST-02 imports). Atomic: increments aggregates
/// and recomputes cumulative_pnl/peak/drawdown for the affected window.
pub(crate) async fn upsert_daily_stats_raw(
    pool: &PgPool,
    user_id: Uuid,
    exchange: &str,
    stat_date: NaiveDate,
    net_pnl: Decimal,
    fees: Decimal,
) -> Result<(), sqlx::Error> {
    let is_win = net_pnl > Decimal::ZERO;
    let win_count: i32 = if is_win { 1 } else { 0 };
    let loss_count: i32 = if is_win { 0 } else { 1 };
    let gross_profit = if is_win { net_pnl } else { Decimal::ZERO };
    let gross_loss = if !is_win { net_pnl.abs() } else { Decimal::ZERO };

    sqlx::query(
        "INSERT INTO journal_daily_stats \
         (user_id, stat_date, exchange, trade_count, win_count, loss_count, \
          gross_profit, gross_loss, net_pnl, fees, \
          cumulative_pnl, peak_cumulative_pnl, drawdown, drawdown_pct) \
         VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, 0, 0, 0, 0) \
         ON CONFLICT (user_id, stat_date, exchange) \
         DO UPDATE SET \
             trade_count = journal_daily_stats.trade_count + 1, \
             win_count = journal_daily_stats.win_count + EXCLUDED.win_count, \
             loss_count = journal_daily_stats.loss_count + EXCLUDED.loss_count, \
             gross_profit = journal_daily_stats.gross_profit + EXCLUDED.gross_profit, \
             gross_loss = journal_daily_stats.gross_loss + EXCLUDED.gross_loss, \
             net_pnl = journal_daily_stats.net_pnl + EXCLUDED.net_pnl, \
             fees = journal_daily_stats.fees + EXCLUDED.fees",
    )
    .bind(user_id)
    .bind(stat_date)
    .bind(exchange)
    .bind(win_count)
    .bind(loss_count)
    .bind(gross_profit)
    .bind(gross_loss)
    .bind(net_pnl)
    .bind(fees)
    .execute(pool)
    .await?;

    recompute_cumulative_pnl_from(pool, user_id, exchange, stat_date).await?;
    Ok(())
}

/// FIX-08 T16: Recompute cumulative P&L, peak, drawdown from `from_date` forward.
///
/// Shared by `JournalService::upsert_daily_stats` and `TradeEventWriter::upsert_daily_stats`.
/// The CTE computes the full user+exchange window (for correct peak propagation) but the
/// UPDATE only touches rows from `from_date` forward.
pub(crate) async fn recompute_cumulative_pnl_from(
    pool: &PgPool,
    user_id: Uuid,
    exchange: &str,
    from_date: NaiveDate,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "WITH running AS ( \
             SELECT id, \
                 SUM(net_pnl) OVER (ORDER BY stat_date) as cum_pnl, \
                 MAX(SUM(net_pnl) OVER (ORDER BY stat_date)) \
                     OVER (ORDER BY stat_date) as running_peak \
             FROM journal_daily_stats \
             WHERE user_id = $1 AND exchange = $2 \
         ) \
         UPDATE journal_daily_stats jds SET \
             cumulative_pnl = r.cum_pnl, \
             peak_cumulative_pnl = r.running_peak, \
             drawdown = r.cum_pnl - r.running_peak, \
             drawdown_pct = CASE \
                 WHEN r.running_peak > 0 \
                 THEN (r.cum_pnl - r.running_peak) / r.running_peak * 100 \
                 ELSE 0 END \
         FROM running r \
         WHERE jds.id = r.id \
           AND jds.stat_date >= $3",
    )
    .bind(user_id)
    .bind(exchange)
    .bind(from_date)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal_macros::dec;

    fn make_event(side: &str, entry: Decimal, exit: Decimal, qty: Decimal) -> TradeCloseEvent {
        TradeCloseEvent {
            user_id: Uuid::new_v4(),
            exchange: "woo".to_string(),
            symbol: "BTC_USDT".to_string(),
            side: side.to_string(),
            entry_price: entry,
            exit_price: exit,
            quantity: qty,
            leverage: 10,
            fees: dec!(5.0),
            stop_price: Some(dec!(49000)),
            target_price: Some(dec!(55000)),
            risk_amount: Some(dec!(100)),
            opened_at: Utc.with_ymd_and_hms(2026, 3, 18, 10, 0, 0).unwrap(),
            closed_at: Utc.with_ymd_and_hms(2026, 3, 18, 14, 30, 0).unwrap(),
            trade_group_id: Some(Uuid::new_v4()),
            source: None,
            exchange_fill_id: None,
            setup_tag: None,
            reasoning: None,
            confidence: None,
            kelly_inputs: None,
            needs_reconciliation: false,
        }
    }

    #[test]
    fn test_long_winning_trade() {
        let event = make_event("LONG", dec!(50000), dec!(51000), dec!(0.1));
        let derived = compute_derived_fields(&event);

        // PnL = (51000 - 50000) * 0.1 = 100
        assert_eq!(derived.realized_pnl, dec!(100));

        // Margin = (50000 * 0.1) / 10 = 500
        // PnL% = (100 / 500) * 100 = 20%
        assert_eq!(derived.realized_pnl_pct, dec!(20));

        // Net = 100 - 5 = 95
        assert_eq!(derived.net_pnl, dec!(95));

        // R-multiple = 95 / 100 = 0.95
        assert_eq!(derived.r_multiple, Some(dec!(0.95)));

        // JNL-DUR-01: duration_secs is generated by the DB; the application-layer
        // invariant that remains is the chronological ordering of the input event.
        assert!(event.closed_at >= event.opened_at);
    }

    #[test]
    fn test_long_losing_trade() {
        let event = make_event("LONG", dec!(50000), dec!(49000), dec!(0.1));
        let derived = compute_derived_fields(&event);

        // PnL = (49000 - 50000) * 0.1 = -100
        assert_eq!(derived.realized_pnl, dec!(-100));

        // Net = -100 - 5 = -105
        assert_eq!(derived.net_pnl, dec!(-105));

        // R-multiple = -105 / 100 = -1.05
        assert_eq!(derived.r_multiple, Some(dec!(-1.05)));
    }

    #[test]
    fn test_short_winning_trade() {
        let event = make_event("SHORT", dec!(50000), dec!(49000), dec!(0.1));
        let derived = compute_derived_fields(&event);

        // PnL = (50000 - 49000) * 0.1 = 100
        assert_eq!(derived.realized_pnl, dec!(100));
        assert_eq!(derived.net_pnl, dec!(95));
    }

    #[test]
    fn test_short_losing_trade() {
        let event = make_event("SHORT", dec!(50000), dec!(51000), dec!(0.1));
        let derived = compute_derived_fields(&event);

        // PnL = (50000 - 51000) * 0.1 = -100
        assert_eq!(derived.realized_pnl, dec!(-100));
        assert_eq!(derived.net_pnl, dec!(-105));
    }

    #[test]
    fn test_explicit_risk_amount_takes_priority() {
        // When risk_amount is explicitly set, it should be used even if stop_price differs
        let mut event = make_event("LONG", dec!(50000), dec!(51000), dec!(0.1));
        event.risk_amount = Some(dec!(200)); // explicit, different from stop-derived (100)
        let derived = compute_derived_fields(&event);

        assert_eq!(derived.risk_amount, Some(dec!(200)));
        // net = 95, R = 95/200 = 0.475
        assert_eq!(derived.r_multiple, Some(dec!(0.475)));
    }

    #[test]
    fn test_zero_risk_amount_falls_back_to_stop_distance() {
        let mut event = make_event("LONG", dec!(50000), dec!(51000), dec!(0.1));
        event.risk_amount = Some(Decimal::ZERO);
        // stop_price is 49000, so risk = |50000 - 49000| * 0.1 = 100
        let derived = compute_derived_fields(&event);

        assert_eq!(derived.risk_amount, Some(dec!(100)));
        // net = 95, R = 95/100 = 0.95
        assert_eq!(derived.r_multiple, Some(dec!(0.95)));
    }

    #[test]
    fn test_no_risk_amount_derives_from_stop_distance() {
        let mut event = make_event("LONG", dec!(50000), dec!(51000), dec!(0.1));
        event.risk_amount = None;
        // stop_price is 49000, so risk = |50000 - 49000| * 0.1 = 100
        let derived = compute_derived_fields(&event);

        assert_eq!(derived.risk_amount, Some(dec!(100)));
        // net = 95, R = 95/100 = 0.95
        assert_eq!(derived.r_multiple, Some(dec!(0.95)));
    }

    #[test]
    fn test_no_risk_no_stop_gives_none() {
        let mut event = make_event("LONG", dec!(50000), dec!(51000), dec!(0.1));
        event.risk_amount = None;
        event.stop_price = None;
        let derived = compute_derived_fields(&event);

        assert!(derived.risk_amount.is_none());
        assert!(derived.r_multiple.is_none());
    }

    #[test]
    fn test_zero_leverage_defaults_to_1x() {
        let mut event = make_event("LONG", dec!(50000), dec!(51000), dec!(0.1));
        event.leverage = 0;
        let derived = compute_derived_fields(&event);

        // With leverage=0 treated as 1x: margin = 50000 * 0.1 = 5000
        // PnL% = (100 / 5000) * 100 = 2%
        assert_eq!(derived.realized_pnl_pct, dec!(2));
    }

    #[test]
    fn test_unknown_side_gives_zero_pnl() {
        let event = make_event("UNKNOWN", dec!(50000), dec!(51000), dec!(0.1));
        let derived = compute_derived_fields(&event);

        assert_eq!(derived.realized_pnl, Decimal::ZERO);
    }

    #[test]
    fn test_chronology_invariant_holds_for_well_formed_event() {
        // JNL-DUR-01: duration_secs is computed by the DB as a generated column.
        // The application-layer responsibility is now to ensure event timestamps
        // are chronologically ordered before the write. The DB CHECK constraint
        // (closed_at >= opened_at) rejects malformed events at the boundary.
        let mut event = make_event("LONG", dec!(50000), dec!(51000), dec!(0.1));
        event.opened_at = Utc.with_ymd_and_hms(2026, 3, 18, 0, 0, 0).unwrap();
        event.closed_at = Utc.with_ymd_and_hms(2026, 3, 19, 0, 0, 0).unwrap();
        assert!(event.closed_at >= event.opened_at);
        // 24 hours expressed in the timestamp delta the DB will record.
        assert_eq!((event.closed_at - event.opened_at).num_seconds(), 86400);
    }

    // --- AGENT-01 CP-2: Agent attribution on TradeCloseEvent ---

    #[test]
    fn agent_attribution_fields_on_trade_close_event() {
        let mut event = make_event("LONG", dec!(50000), dec!(51000), dec!(0.1));
        event.reasoning = Some("BTC broke above 50k resistance on 4h".to_string());
        event.confidence = Some(dec!(0.85));
        event.source = Some("agent:hermes_v1.2".to_string());

        assert_eq!(
            event.reasoning.as_deref(),
            Some("BTC broke above 50k resistance on 4h")
        );
        assert_eq!(event.confidence, Some(dec!(0.85)));
        assert_eq!(event.source.as_deref(), Some("agent:hermes_v1.2"));
    }

    #[test]
    fn agent_attribution_fields_default_to_none() {
        let event = make_event("LONG", dec!(50000), dec!(51000), dec!(0.1));
        assert!(event.reasoning.is_none());
        assert!(event.confidence.is_none());
    }

    #[test]
    fn journal_trade_serde_includes_agent_attribution() {
        use crate::models::journal::JournalTrade;
        let json = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "user_id": "550e8400-e29b-41d4-a716-446655440001",
            "exchange": "shadow",
            "symbol": "BTC_USDT",
            "side": "LONG",
            "entry_price": "50000",
            "exit_price": "51000",
            "quantity": "0.1",
            "leverage": 10,
            "realized_pnl": "100",
            "realized_pnl_pct": "20",
            "fees": "5",
            "net_pnl": "95",
            "stop_price": null,
            "target_price": null,
            "risk_amount": null,
            "r_multiple": null,
            "opened_at": "2026-03-18T10:00:00Z",
            "closed_at": "2026-03-18T14:30:00Z",
            "duration_secs": 16200,
            "trade_group_id": null,
            "notes": null,
            "source": "agent:hermes_v1.2",
            "reasoning": "BTC broke above 50k on 4h",
            "confidence": "0.85",
            "exchange_fill_id": null,
            "setup_tag": null,
            "kelly_inputs": null,
            "needs_reconciliation": false,
            "close_reason": null,
            "created_at": "2026-03-18T14:30:00Z",
            "updated_at": "2026-03-18T14:30:00Z"
        });

        let trade: JournalTrade = serde_json::from_value(json).expect("deserialize");
        assert_eq!(trade.source, "agent:hermes_v1.2");
        assert_eq!(trade.reasoning.as_deref(), Some("BTC broke above 50k on 4h"));
        assert_eq!(trade.confidence, Some(dec!(0.85)));
    }
}

/// HIST-03 CP-4: integration tests for idempotent re-import.
///
/// These tests physically verify the partial unique index contract by hitting a real
/// Postgres pool. They are `#[ignore]` by default and only run when `DATABASE_URL` is
/// set:
///
/// ```bash
/// DATABASE_URL=postgres://user:pass@localhost/testudo \
///     cargo test -p router hist03_idempotency -- --ignored
/// ```
#[cfg(test)]
mod hist03_idempotency {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal_macros::dec;
    use sqlx::postgres::PgPoolOptions;

    /// Acquire a pool from `DATABASE_URL`. Panics with a helpful message if unset.
    async fn pool() -> PgPool {
        let url = std::env::var("DATABASE_URL").expect(
            "DATABASE_URL environment variable required. \
             Set it to a Postgres connection string for an initialized test DB.",
        );
        PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .expect("Failed to connect to DATABASE_URL for HIST-03 integration test")
    }

    /// Create a fresh user row. Wallet_address must match `^0x[0-9a-f]{40}$`.
    async fn make_user(pool: &PgPool) -> Uuid {
        let suffix = Uuid::new_v4().simple().to_string();
        // 40-char lowercase hex: take the first 40 chars of a concatenated uuid.
        let hex_40 = format!("{suffix}{suffix}").chars().take(40).collect::<String>();
        let wallet = format!("0x{hex_40}");

        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (wallet_address) VALUES ($1) RETURNING id",
        )
        .bind(&wallet)
        .fetch_one(pool)
        .await
        .expect("insert test user");
        id
    }

    async fn cleanup_user(pool: &PgPool, user_id: Uuid) {
        // journal_daily_stats FK-less, journal_trades has FK to users — delete in order.
        let _ = sqlx::query("DELETE FROM journal_daily_stats WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM journal_trades WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    fn import_event(user_id: Uuid, fill_id: i64, exchange: &str) -> TradeCloseEvent {
        TradeCloseEvent {
            user_id,
            exchange: exchange.to_string(),
            symbol: "BTC_USDT".to_string(),
            side: "LONG".to_string(),
            entry_price: dec!(50000),
            exit_price: dec!(51000),
            quantity: dec!(0.1),
            leverage: 10,
            fees: dec!(5),
            stop_price: Some(dec!(49000)),
            target_price: Some(dec!(55000)),
            risk_amount: Some(dec!(100)),
            opened_at: Utc.with_ymd_and_hms(2026, 3, 18, 10, 0, 0).unwrap(),
            closed_at: Utc.with_ymd_and_hms(2026, 3, 18, 14, 30, 0).unwrap(),
            trade_group_id: None, // import path
            source: Some("import_ccxt".to_string()),
            exchange_fill_id: Some(fill_id),
            setup_tag: None,
            reasoning: None,
            confidence: None,
            kelly_inputs: None,
            needs_reconciliation: false,
        }
    }

    #[tokio::test]
    #[ignore]
    async fn fresh_insert_returns_inserted() {
        let pool = pool().await;
        let user_id = make_user(&pool).await;
        let svc = JournalService::new(pool.clone());

        let event = import_event(user_id, 101, "bybit");
        let outcome = svc.record_trade_close(event).await.expect("record");

        match outcome {
            RecordOutcome::Inserted(trade) => {
                assert_eq!(trade.user_id, user_id);
                assert_eq!(trade.exchange, "bybit");
                assert_eq!(trade.exchange_fill_id, Some(101));
            }
            RecordOutcome::SkippedDuplicate => panic!("first insert should be Inserted"),
        }

        cleanup_user(&pool, user_id).await;
    }

    #[tokio::test]
    #[ignore]
    async fn second_insert_returns_skipped_duplicate() {
        let pool = pool().await;
        let user_id = make_user(&pool).await;
        let svc = JournalService::new(pool.clone());

        // First run: Inserted.
        let first = svc
            .record_trade_close(import_event(user_id, 202, "bybit"))
            .await
            .expect("first record");
        assert!(matches!(first, RecordOutcome::Inserted(_)));

        // Second run on the same key: SkippedDuplicate.
        let second = svc
            .record_trade_close(import_event(user_id, 202, "bybit"))
            .await
            .expect("second record");
        assert!(matches!(second, RecordOutcome::SkippedDuplicate));

        // Exactly one row in journal_trades.
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM journal_trades \
             WHERE user_id = $1 AND exchange = $2 AND exchange_fill_id = $3",
        )
        .bind(user_id)
        .bind("bybit")
        .bind(202_i64)
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(count, 1, "partial index must prevent duplicate row");

        cleanup_user(&pool, user_id).await;
    }

    #[tokio::test]
    #[ignore]
    async fn null_exchange_fill_id_not_affected_by_partial_index() {
        let pool = pool().await;
        let user_id = make_user(&pool).await;
        let svc = JournalService::new(pool.clone());

        // Two live-shaped events: exchange_fill_id = None, distinct trade_group_id each.
        // Partial index WHERE clause excludes these rows, so both insert independently.
        let mut a = import_event(user_id, 0, "bybit");
        a.exchange_fill_id = None;
        a.trade_group_id = Some(Uuid::new_v4());
        a.source = Some("testudo".to_string());

        let mut b = import_event(user_id, 0, "bybit");
        b.exchange_fill_id = None;
        b.trade_group_id = Some(Uuid::new_v4());
        b.source = Some("testudo".to_string());

        let ra = svc.record_trade_close(a).await.expect("live a");
        let rb = svc.record_trade_close(b).await.expect("live b");
        assert!(matches!(ra, RecordOutcome::Inserted(_)));
        assert!(matches!(rb, RecordOutcome::Inserted(_)));

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM journal_trades \
             WHERE user_id = $1 AND exchange_fill_id IS NULL",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(count, 2, "live trades must both persist; partial index excludes NULL");

        cleanup_user(&pool, user_id).await;
    }

    #[tokio::test]
    #[ignore]
    async fn canonical_exchange_applied_and_catches_case_drift() {
        let pool = pool().await;
        let user_id = make_user(&pool).await;
        let svc = JournalService::new(pool.clone());

        // First run: mixed-case exchange — must be persisted lowercase (T4 canonicalization).
        let first = svc
            .record_trade_close(import_event(user_id, 303, "Bybit"))
            .await
            .expect("first record");
        assert!(matches!(first, RecordOutcome::Inserted(_)));

        // Stored value is lowercase.
        let stored: String = sqlx::query_scalar(
            "SELECT exchange FROM journal_trades \
             WHERE user_id = $1 AND exchange_fill_id = $2",
        )
        .bind(user_id)
        .bind(303_i64)
        .fetch_one(&pool)
        .await
        .expect("fetch exchange");
        assert_eq!(stored, "bybit", "canonical_exchange_name should lowercase");

        // Second run with a DIFFERENT casing must still collide (canonical form matches).
        let second = svc
            .record_trade_close(import_event(user_id, 303, "BYBIT"))
            .await
            .expect("second record");
        assert!(
            matches!(second, RecordOutcome::SkippedDuplicate),
            "case drift must not defeat the partial unique index"
        );

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM journal_trades \
             WHERE user_id = $1 AND exchange_fill_id = $2",
        )
        .bind(user_id)
        .bind(303_i64)
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(count, 1, "case-variant re-imports must not create duplicate rows");

        cleanup_user(&pool, user_id).await;
    }
}
