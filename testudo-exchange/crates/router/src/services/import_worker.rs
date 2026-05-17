//! HIST-01: Exchange Trade History Import Worker
//!
//! Background worker that fetches trade history from connected exchanges
//! and persists closing fills to `journal_trades` via the existing journal pipeline.
//!
//! Phase 1: Hyperliquid via native SDK (`user_fills_by_time`)
//! Phase 2: CEX via CCXT sidecar (`fetchMyTrades`)

use alloy::primitives::Address;
use chrono::{DateTime, Utc};
use hyperliquid_sdk_rs::{InfoProvider, Network};
use pg_queue::{QueueName, QueueRepository};
use crate::repositories::exchange_account::ExchangeAccountRepository;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::cex_history;
use super::hl_fill_journal;
use super::journal_service::{JournalService, RecordOutcome, TradeCloseEvent};

/// Maximum fills per HL API response
const HL_PAGE_SIZE: usize = 2000;

/// Delay between pagination requests to avoid rate limiting
const PAGINATION_DELAY: Duration = Duration::from_millis(100);

/// Import job payload — stored in pg_queue as JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportJobPayload {
    pub user_id: Uuid,
    pub account_id: Uuid,
    pub exchange_name: String,
    pub start_time_ms: i64,
    pub end_time_ms: i64,
}

/// Result of a completed import job
#[derive(Debug, Default)]
pub struct ImportResult {
    pub trades_imported: u64,
    /// Non-closing / spot / zero-qty / otherwise structurally unimportable fills.
    pub trades_skipped: u64,
    /// HIST-03: fills rejected by the partial unique index because they were
    /// already journaled in a prior import run. Split from `trades_skipped` so
    /// operators can distinguish "nothing to import" from "already imported".
    pub trades_skipped_duplicate: u64,
    pub errors: u64,
}

/// HIST-03: outcome of processing a single fill/reconstructed-trade into the journal.
/// Lets callers bump the right counter without string-matching error messages.
#[derive(Debug)]
enum ProcessOutcome {
    Imported,
    Duplicate,
    StructuralSkip,
}

/// Import worker that processes jobs from the `queue_imports` table.
pub struct ImportWorker {
    queue: QueueRepository,
    exchange_repo: ExchangeAccountRepository,
    journal: Arc<JournalService>,
    hl_network: Network,
    pool: sqlx::PgPool,
    http_client: reqwest::Client,
}

impl ImportWorker {
    pub fn new(
        queue: QueueRepository,
        exchange_repo: ExchangeAccountRepository,
        journal: Arc<JournalService>,
        hl_network: Network,
        pool: sqlx::PgPool,
    ) -> Self {
        Self {
            queue,
            exchange_repo,
            journal,
            hl_network,
            pool,
            http_client: reqwest::Client::new(),
        }
    }

    /// Run the import worker loop until shutdown is signalled.
    pub async fn run(self, shutdown: CancellationToken) {
        tracing::info!("ImportWorker started");

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("ImportWorker shutting down");
                    return;
                }
                result = self.queue.pop::<ImportJobPayload>(QueueName::TradeImports) => {
                    match result {
                        Ok(Some(job)) => {
                            let job_id = job.id;
                            tracing::info!(
                                job_id,
                                exchange = %job.payload.exchange_name,
                                user_id = %job.payload.user_id,
                                "Processing import job"
                            );

                            match self.process_job(&job.payload).await {
                                Ok(result) => {
                                    self.queue.complete(QueueName::TradeImports, job_id).await.ok();
                                    tracing::info!(
                                        job_id,
                                        imported = result.trades_imported,
                                        skipped = result.trades_skipped,
                                        skipped_duplicate = result.trades_skipped_duplicate,
                                        errors = result.errors,
                                        "Import job completed"
                                    );

                                    // HIST-01 T8: Notify user via WebSocket
                                    self.notify_user(
                                        &job.payload.user_id,
                                        &job.payload.exchange_name,
                                        &result,
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    tracing::error!(job_id, error = %e, "Import job failed");
                                    // Mark as completed (not pending) to prevent infinite retry
                                    self.queue.complete(QueueName::TradeImports, job_id).await.ok();
                                }
                            }
                        }
                        Ok(None) => {
                            // No pending jobs — wait before polling again
                            tokio::select! {
                                _ = shutdown.cancelled() => return,
                                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Import queue pop error");
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        }
    }

    /// Send import_complete notification to user via pg_notify → ws-stream pipeline.
    async fn notify_user(&self, user_id: &Uuid, exchange: &str, result: &ImportResult) {
        let channel = format!("order.{}", user_id);
        let payload = serde_json::json!({
            "stream": channel,
            "data": {
                "e": "import_complete",
                "exchange": exchange,
                "trades_imported": result.trades_imported,
                "trades_skipped": result.trades_skipped,
                "trades_skipped_duplicate": result.trades_skipped_duplicate,
            }
        });

        if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
            .bind(&channel)
            .bind(payload.to_string())
            .execute(&self.pool)
            .await
        {
            tracing::warn!(
                user_id = %user_id,
                error = %e,
                "Failed to send import_complete notification"
            );
        }
    }

    async fn process_job(&self, payload: &ImportJobPayload) -> Result<ImportResult, ImportError> {
        match payload.exchange_name.as_str() {
            "hyperliquid" => self.import_hyperliquid(payload).await,
            _ => self.import_cex(payload).await,
        }
    }

    /// Import closing fills from Hyperliquid via `user_fills_by_time`.
    async fn import_hyperliquid(
        &self,
        payload: &ImportJobPayload,
    ) -> Result<ImportResult, ImportError> {
        // Load credentials to get wallet address
        let creds = self
            .exchange_repo
            .load_credentials(payload.account_id, payload.user_id)
            .await
            .map_err(|e| ImportError::CredentialLoad(e.to_string()))?;

        let wallet_address = creds
            .wallet_address
            .as_deref()
            .ok_or_else(|| ImportError::CredentialLoad("No wallet address for HL account".into()))?;

        let address = Address::from_str(wallet_address)
            .map_err(|e| ImportError::CredentialLoad(format!("Invalid wallet address: {e}")))?;

        let info = InfoProvider::new(self.hl_network);

        let mut result = ImportResult::default();

        let mut cursor = payload.start_time_ms as u64;
        let end_time = payload.end_time_ms as u64;
        // Track the most recent "Open Long/Short" timestamp per coin
        // so closing fills can compute actual trade duration.
        let mut open_times: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

        loop {
            let fills = info
                .user_fills_by_time(address, cursor, Some(end_time), None)
                .await
                .map_err(|e| ImportError::ApiFetch(format!("HL fills query failed: {e}")))?;

            if fills.is_empty() {
                break;
            }

            let page_size = fills.len();

            // Build a map of the most recent open time per coin for duration calculation.
            // "Open Long"/"Open Short" fills record when a position was entered.
            // Closing fills can then look up the corresponding open time.
            for fill in &fills {
                if fill.coin.starts_with('@') {
                    continue;
                }
                if fill.dir.starts_with("Open") {
                    open_times.insert(fill.coin.clone(), fill.time);
                }
            }

            for fill in &fills {
                // Skip spot fills (prefixed with @)
                if fill.coin.starts_with('@') {
                    continue;
                }

                // Skip non-closing fills
                if fill.closed_pnl == "0" || fill.closed_pnl == "0.0" {
                    continue;
                }

                let open_time = open_times.get(&fill.coin).copied();
                match self.process_hl_fill(fill, payload.user_id, open_time).await {
                    Ok(ProcessOutcome::Imported) => result.trades_imported += 1,
                    Ok(ProcessOutcome::Duplicate) => result.trades_skipped_duplicate += 1,
                    Ok(ProcessOutcome::StructuralSkip) => result.trades_skipped += 1,
                    Err(e) => {
                        tracing::warn!(
                            tid = fill.tid,
                            coin = %fill.coin,
                            error = %e,
                            "Failed to import fill"
                        );
                        result.errors += 1;
                    }
                }
            }

            // Advance cursor past last fill
            if let Some(last) = fills.last() {
                cursor = last.time + 1;
            }

            // Stop if we got fewer than a full page
            if page_size < HL_PAGE_SIZE {
                break;
            }

            tokio::time::sleep(PAGINATION_DELAY).await;
        }

        Ok(result)
    }

    /// Process a single HL closing fill into a journal trade.
    /// `open_time_ms`: timestamp of the corresponding open fill for duration calc.
    async fn process_hl_fill(
        &self,
        fill: &hyperliquid_sdk_rs::types::info_types::UserFillByTime,
        user_id: Uuid,
        open_time_ms: Option<u64>,
    ) -> Result<ProcessOutcome, ImportError> {
        let event = match hl_fill_journal::build_trade_close_event(fill, user_id, open_time_ms, "import_hl") {
            Some(e) => e,
            None => return Ok(ProcessOutcome::StructuralSkip),
        };

        // HIST-03: dedup is now structural via ON CONFLICT DO NOTHING inside
        // `record_trade_close` — no more error-string matching.
        match self.journal.record_trade_close(event).await {
            Ok(RecordOutcome::Inserted(_)) => Ok(ProcessOutcome::Imported),
            Ok(RecordOutcome::SkippedDuplicate) => Ok(ProcessOutcome::Duplicate),
            Err(e) => Err(ImportError::Database(e.to_string())),
        }
    }

    /// HIST-02: Import trade history from CEX via direct REST API calls.
    async fn import_cex(
        &self,
        payload: &ImportJobPayload,
    ) -> Result<ImportResult, ImportError> {
        let creds = self
            .exchange_repo
            .load_credentials(payload.account_id, payload.user_id)
            .await
            .map_err(|e| ImportError::CredentialLoad(e.to_string()))?;

        let fills = cex_history::fetch_trade_history(
            &self.http_client,
            &payload.exchange_name,
            &creds.api_key,
            &creds.api_secret,
            creds.passphrase.as_deref(),
            payload.start_time_ms,
            payload.end_time_ms,
        )
        .await
        .map_err(|e| ImportError::ApiFetch(e.to_string()))?;

        // Split fills: those with closed_pnl can be imported directly (like HL),
        // those without need reconstruction from raw entry/exit fill pairs.
        let (pnl_fills, raw_fills): (Vec<_>, Vec<_>) = fills
            .into_iter()
            .partition(|f| f.closed_pnl.is_some());

        tracing::info!(
            exchange = %payload.exchange_name,
            pnl_fills = pnl_fills.len(),
            raw_fills = raw_fills.len(),
            "Fetched fills from exchange REST API"
        );

        let mut result = ImportResult::default();

        // Direct import for fills with realized PnL (entry price derived from PnL)
        for fill in &pnl_fills {
            let pnl = fill.closed_pnl.unwrap();
            let qty = fill.quantity;
            if qty == Decimal::ZERO {
                result.trades_skipped += 1;
                continue;
            }

            // HIST-03 FR-5: require a parseable numeric fill ID. Dropping the old
            // `unwrap_or(fill.timestamp as i64)` fallback prevents synthetic keys
            // from drifting across endpoint versions and defeating the partial
            // unique index.
            let Ok(exchange_fill_id) = fill.id.parse::<i64>() else {
                tracing::warn!(
                    fill_id = %fill.id,
                    symbol = %fill.symbol,
                    timestamp = fill.timestamp,
                    exchange = %payload.exchange_name,
                    "HIST-03: unparseable CEX fill ID — skipping fill to avoid synthetic timestamp-based dedup key"
                );
                result.errors += 1;
                continue;
            };

            // Closing sell = was LONG, closing buy = was SHORT
            let side = if fill.side == "sell" { "LONG" } else { "SHORT" };

            // Derive entry price: LONG entry = exit - pnl/qty, SHORT entry = exit + pnl/qty
            let entry_price = if side == "LONG" {
                fill.price - (pnl / qty)
            } else {
                fill.price + (pnl / qty)
            };

            let fill_time = timestamp_to_datetime(fill.timestamp as u64);

            let event = TradeCloseEvent {
                user_id: payload.user_id,
                exchange: payload.exchange_name.clone(),
                symbol: fill.symbol.clone(),
                side: side.to_string(),
                entry_price,
                exit_price: fill.price,
                quantity: qty,
                leverage: 1,
                fees: fill.fee,
                stop_price: None,
                target_price: None,
                risk_amount: None,
                opened_at: fill_time, // best we have — no open time in closing fill
                closed_at: fill_time,
                trade_group_id: None,
                source: Some("import_ccxt".to_string()),
                exchange_fill_id: Some(exchange_fill_id),
                setup_tag: None,
                kelly_inputs: None,
                needs_reconciliation: false,
            };

            // HIST-03: dedup via ON CONFLICT DO NOTHING; no error-string matching.
            match self.journal.record_trade_close(event).await {
                Ok(RecordOutcome::Inserted(_)) => result.trades_imported += 1,
                Ok(RecordOutcome::SkippedDuplicate) => result.trades_skipped_duplicate += 1,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to import PnL fill");
                    result.errors += 1;
                }
            }
        }

        // Reconstruction for fills without PnL data (entry/exit pair matching)
        let completed = reconstruct_positions(&raw_fills);
        for trade in completed {
            match self
                .record_reconstructed_trade(
                    &trade,
                    payload.user_id,
                    &payload.exchange_name,
                )
                .await
            {
                Ok(ProcessOutcome::Imported) => result.trades_imported += 1,
                Ok(ProcessOutcome::Duplicate) => result.trades_skipped_duplicate += 1,
                Ok(ProcessOutcome::StructuralSkip) => result.trades_skipped += 1,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to import reconstructed trade");
                    result.errors += 1;
                }
            }
        }

        Ok(result)
    }

    /// Record a reconstructed trade from CCXT fills.
    async fn record_reconstructed_trade(
        &self,
        trade: &ReconstructedTrade,
        user_id: Uuid,
        exchange_name: &str,
    ) -> Result<ProcessOutcome, ImportError> {
        let event = TradeCloseEvent {
            user_id,
            exchange: exchange_name.to_string(),
            symbol: trade.symbol.clone(),
            side: trade.side.clone(),
            entry_price: trade.entry_price,
            exit_price: trade.exit_price,
            quantity: trade.quantity,
            leverage: 1,
            fees: trade.total_fees,
            stop_price: None,
            target_price: None,
            risk_amount: None,
            opened_at: trade.opened_at,
            closed_at: trade.closed_at,
            trade_group_id: None,
            source: Some("import_ccxt".to_string()),
            exchange_fill_id: Some(trade.last_fill_id),
            setup_tag: None,
            kelly_inputs: None,
            needs_reconciliation: false,
        };

        // HIST-03: dedup via ON CONFLICT DO NOTHING; no error-string matching.
        match self.journal.record_trade_close(event).await {
            Ok(RecordOutcome::Inserted(_)) => Ok(ProcessOutcome::Imported),
            Ok(RecordOutcome::SkippedDuplicate) => Ok(ProcessOutcome::Duplicate),
            Err(e) => Err(ImportError::Database(e.to_string())),
        }
    }
}

/// A completed round-trip trade reconstructed from raw fills.
#[derive(Debug)]
struct ReconstructedTrade {
    symbol: String,
    side: String, // "LONG" | "SHORT"
    entry_price: Decimal,
    exit_price: Decimal,
    quantity: Decimal,
    total_fees: Decimal,
    opened_at: DateTime<Utc>,
    closed_at: DateTime<Utc>,
    last_fill_id: i64,
}

/// Reconstruct completed position round-trips from raw fills.
/// Uses symbol-net tracking: when net position crosses zero, emit a completed trade.
///
/// JNL-DUR-02: fills are sorted chronologically before iteration. Bybit's
/// `fetchMyTrades` returns fills newest-first; without a sort the loop saw the
/// closing fill of trade N+1 before the opening fill of trade N, paired them
/// against each other, and produced rows with `closed_at < opened_at`.
/// Dedup is unaffected: each emitted trade keys on the closing fill's
/// `exchange_fill_id` via the `idx_unique_import_fill` partial unique index,
/// so re-running this function over the same fill stream is idempotent.
fn reconstruct_positions(fills: &[cex_history::CexFill]) -> Vec<ReconstructedTrade> {
    use std::collections::HashMap;

    struct OpenPosition {
        side: String,
        entry_fills: Vec<(Decimal, Decimal, Decimal)>, // (price, qty, fee)
        total_qty: Decimal,
        opened_at: DateTime<Utc>,
    }

    let mut positions: HashMap<String, OpenPosition> = HashMap::new();
    let mut completed: Vec<ReconstructedTrade> = Vec::new();

    // JNL-DUR-02: sort fills oldest-first so Open precedes Close per coin.
    // Stable sort: ties in `timestamp` preserve API order (matters for
    // sub-millisecond fills that share a timestamp).
    let mut ordered: Vec<&cex_history::CexFill> = fills.iter().collect();
    ordered.sort_by_key(|f| f.timestamp);

    for fill in ordered {
        let price = fill.price;
        let amount = fill.quantity;
        if amount <= Decimal::ZERO {
            continue;
        }
        let fee = fill.fee;
        let fill_time = timestamp_to_datetime(fill.timestamp as u64);
        let symbol = fill.symbol.clone();
        let fill_side = fill.side.to_lowercase();

        if let Some(pos) = positions.get_mut(&symbol) {
            let is_closing = (pos.side == "LONG" && fill_side == "sell")
                || (pos.side == "SHORT" && fill_side == "buy");

            if is_closing {
                if amount >= pos.total_qty {
                    // HIST-03 FR-5: require a parseable numeric fill ID to dedup
                    // the reconstructed trade. Without a stable key, re-imports
                    // would silently produce duplicate rows.
                    let Ok(last_fill_id) = fill.id.parse::<i64>() else {
                        tracing::warn!(
                            fill_id = %fill.id,
                            symbol = %symbol,
                            timestamp = fill.timestamp,
                            "HIST-03: unparseable closing-fill ID — dropping reconstructed trade"
                        );
                        positions.remove(&symbol);
                        continue;
                    };

                    // Full close (or overclose)
                    let close_qty = pos.total_qty;
                    let entry_price = weighted_avg_price(&pos.entry_fills);
                    let entry_fees: Decimal = pos.entry_fills.iter().map(|(_, _, f)| f).sum();

                    // JNL-DUR-02 defense-in-depth: with the chronological sort above this
                    // branch is unreachable. If it ever fires, the journal_trades_chronology
                    // CHECK constraint would reject the insert downstream — surface the
                    // mispairing here with full context instead of as an opaque DB error.
                    if fill_time < pos.opened_at {
                        tracing::warn!(
                            symbol = %symbol,
                            opened_at = ?pos.opened_at,
                            closed_at = ?fill_time,
                            close_fill_id = %fill.id,
                            "JNL-DUR-02: rejected chronology-violating reconstructed trade"
                        );
                        positions.remove(&symbol);
                        continue;
                    }

                    completed.push(ReconstructedTrade {
                        symbol: symbol.clone(),
                        side: pos.side.clone(),
                        entry_price,
                        exit_price: price,
                        quantity: close_qty,
                        total_fees: entry_fees + fee,
                        opened_at: pos.opened_at,
                        closed_at: fill_time,
                        last_fill_id,
                    });

                    let remainder = amount - close_qty;
                    positions.remove(&symbol);

                    // If overclose, open reverse position
                    if remainder > Decimal::ZERO {
                        let new_side = if fill_side == "buy" {
                            "LONG"
                        } else {
                            "SHORT"
                        };
                        positions.insert(
                            symbol,
                            OpenPosition {
                                side: new_side.to_string(),
                                entry_fills: vec![(price, remainder, Decimal::ZERO)],
                                total_qty: remainder,
                                opened_at: fill_time,
                            },
                        );
                    }
                } else {
                    // Partial close — reduce position but don't emit trade yet
                    pos.total_qty -= amount;
                    // We track the fee but don't emit until full close
                    pos.entry_fills.push((price, Decimal::ZERO, fee)); // zero qty marker for fee tracking
                }
            } else {
                // Same direction — scaling in
                pos.entry_fills.push((price, amount, fee));
                pos.total_qty += amount;
            }
        } else {
            // No open position — open new
            let side = if fill_side == "buy" { "LONG" } else { "SHORT" };
            positions.insert(
                symbol,
                OpenPosition {
                    side: side.to_string(),
                    entry_fills: vec![(price, amount, fee)],
                    total_qty: amount,
                    opened_at: fill_time,
                },
            );
        }
    }

    completed
}

/// Weighted average entry price from fills.
fn weighted_avg_price(fills: &[(Decimal, Decimal, Decimal)]) -> Decimal {
    let mut total_cost = Decimal::ZERO;
    let mut total_qty = Decimal::ZERO;
    for (price, qty, _fee) in fills {
        if *qty > Decimal::ZERO {
            total_cost += price * qty;
            total_qty += qty;
        }
    }
    if total_qty > Decimal::ZERO {
        total_cost / total_qty
    } else {
        Decimal::ZERO
    }
}

use super::hl_fill_journal::timestamp_to_datetime;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("Unsupported exchange: {0}")]
    UnsupportedExchange(String),
    #[error("Failed to load credentials: {0}")]
    CredentialLoad(String),
    #[error("API fetch failed: {0}")]
    ApiFetch(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Database error: {0}")]
    Database(String),
}

/// Enqueue an import job for a given exchange account.
/// Called from the exchange routes when credentials are saved, or from the import endpoint.
pub async fn enqueue_import(
    queue: &QueueRepository,
    user_id: Uuid,
    account_id: Uuid,
    exchange_name: &str,
) -> Result<i64, pg_queue::PgQueueError> {
    let now = Utc::now().timestamp_millis();
    let ninety_days_ago = now - (90 * 24 * 60 * 60 * 1000);

    let payload = ImportJobPayload {
        user_id,
        account_id,
        exchange_name: exchange_name.to_string(),
        start_time_ms: ninety_days_ago,
        end_time_ms: now,
    };

    queue.push(QueueName::TradeImports, &payload).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_timestamp_to_datetime() {
        let dt = timestamp_to_datetime(1681222254710);
        assert_eq!(dt.timestamp(), 1681222254);
        assert_eq!(dt.timestamp_subsec_millis(), 710);
    }

    #[test]
    fn test_entry_price_derivation_long() {
        // Long trade: exit=51000, pnl=100, qty=0.1
        // entry = 51000 - (100 / 0.1) = 51000 - 1000 = 50000
        let exit = dec!(51000);
        let pnl = dec!(100);
        let qty = dec!(0.1);
        let entry = exit - (pnl / qty);
        assert_eq!(entry, dec!(50000));
    }

    #[test]
    fn test_entry_price_derivation_short() {
        // Short trade: exit=49000, pnl=100, qty=0.1
        // entry = 49000 + (100 / 0.1) = 49000 + 1000 = 50000
        let exit = dec!(49000);
        let pnl = dec!(100);
        let qty = dec!(0.1);
        let entry = exit + (pnl / qty);
        assert_eq!(entry, dec!(50000));
    }

    #[test]
    fn test_entry_price_derivation_losing_long() {
        // Long trade: exit=49000, pnl=-100, qty=0.1
        // entry = 49000 - (-100 / 0.1) = 49000 + 1000 = 50000
        let exit = dec!(49000);
        let pnl = dec!(-100);
        let qty = dec!(0.1);
        let entry = exit - (pnl / qty);
        assert_eq!(entry, dec!(50000));
    }

    #[test]
    fn test_entry_price_derivation_losing_short() {
        // Short trade: exit=51000, pnl=-100, qty=0.1
        // entry = 51000 + (-100 / 0.1) = 51000 - 1000 = 50000
        let exit = dec!(51000);
        let pnl = dec!(-100);
        let qty = dec!(0.1);
        let entry = exit + (pnl / qty);
        assert_eq!(entry, dec!(50000));
    }

    #[test]
    fn test_ninety_day_window() {
        let now = Utc::now().timestamp_millis();
        let ninety_days_ago = now - (90 * 24 * 60 * 60 * 1000);
        let diff_days = (now - ninety_days_ago) / (24 * 60 * 60 * 1000);
        assert_eq!(diff_days, 90);
    }

    /// JNL-DUR-02: Bybit's `fetchMyTrades` returns fills newest-first. If
    /// `reconstruct_positions` doesn't sort first, it pairs the close of trade N
    /// with the open of trade N+1 (which appears earlier in the input). The
    /// resulting `ReconstructedTrade` then has `closed_at < opened_at` and the
    /// journal_trades_chronology CHECK constraint rejects it on insert.
    #[test]
    fn reconstruct_positions_orders_fills_chronologically() {
        // Single round-trip: long entry at T=100, exit at T=200.
        // Presented to the reconstructor in reverse (close first, open second).
        let fills = vec![
            cex_history::CexFill {
                id: "200".to_string(),
                symbol: "BTC/USDT:USDT".to_string(),
                side: "sell".to_string(),
                price: dec!(51000),
                quantity: dec!(0.1),
                fee: dec!(1),
                closed_pnl: None,
                timestamp: 200,
            },
            cex_history::CexFill {
                id: "100".to_string(),
                symbol: "BTC/USDT:USDT".to_string(),
                side: "buy".to_string(),
                price: dec!(50000),
                quantity: dec!(0.1),
                fee: dec!(1),
                closed_pnl: None,
                timestamp: 100,
            },
        ];

        let trades = reconstruct_positions(&fills);
        assert_eq!(trades.len(), 1, "one round-trip expected");
        let t = &trades[0];
        assert_eq!(t.side, "LONG");
        assert_eq!(t.entry_price, dec!(50000));
        assert_eq!(t.exit_price, dec!(51000));
        assert!(
            t.closed_at >= t.opened_at,
            "chronology invariant: closed_at ({:?}) must be >= opened_at ({:?})",
            t.closed_at,
            t.opened_at,
        );
        assert_eq!((t.closed_at - t.opened_at).num_milliseconds(), 100);
        // Closing fill ID is the dedup key — must come from the close, not the open.
        assert_eq!(t.last_fill_id, 200);
    }

    /// JNL-DUR-02: sequential round-trips interleaved with newest-first ordering
    /// must each pair their own open/close, not cross-pair.
    #[test]
    fn reconstruct_positions_two_round_trips_in_reverse_order() {
        // Round-trip A: open T=100, close T=200.
        // Round-trip B: open T=300, close T=400.
        // Reverse-ordered (Bybit shape): [B-close, B-open, A-close, A-open].
        let f = |id: &str, side: &str, price: Decimal, ts: i64| cex_history::CexFill {
            id: id.to_string(),
            symbol: "ETH/USDT:USDT".to_string(),
            side: side.to_string(),
            price,
            quantity: dec!(1),
            fee: dec!(0),
            closed_pnl: None,
            timestamp: ts,
        };
        let fills = vec![
            f("400", "sell", dec!(2100), 400),
            f("300", "buy",  dec!(2000), 300),
            f("200", "sell", dec!(1100), 200),
            f("100", "buy",  dec!(1000), 100),
        ];

        let trades = reconstruct_positions(&fills);
        assert_eq!(trades.len(), 2);
        for t in &trades {
            assert!(t.closed_at >= t.opened_at);
        }
        // Trades emitted in chronological order after the sort.
        assert_eq!(trades[0].entry_price, dec!(1000));
        assert_eq!(trades[0].exit_price, dec!(1100));
        assert_eq!(trades[0].last_fill_id, 200);
        assert_eq!(trades[1].entry_price, dec!(2000));
        assert_eq!(trades[1].exit_price, dec!(2100));
        assert_eq!(trades[1].last_fill_id, 400);
    }
}
