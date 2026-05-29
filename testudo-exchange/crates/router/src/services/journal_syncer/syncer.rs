//! JournalSyncer — per-(user, account) tokio task for the pull-based journal pipeline.

// @anchor exchange:router:syncer
// @tags api

use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::{mpsc, Notify};
use tokio::time;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use common_utils::journal::reconstruct_trades;
use crate::repositories::exchange_account::ExchangeAccountRepository;
use crate::repositories::raw_fills::RawFillRepository;
use crate::services::journal_service::JournalService;
use crate::services::ManagementEvent;
use super::{FillSource, SyncError};

/// How many consecutive failures before emitting a ManagementEvent and backing off to max.
const CONSECUTIVE_FAILURE_ALERT_THRESHOLD: u32 = 10;

pub struct JournalSyncer {
    pub user_id: Uuid,
    pub account_id: Uuid,
    pub exchange_label: String,
    pub interval_secs: u64,
    pub source: Arc<dyn FillSource>,
    pub pool: PgPool,
    pub raw_fill_repo: RawFillRepository,
    pub exchange_account_repo: ExchangeAccountRepository,
    pub journal_service: Arc<JournalService>,
    pub notify: Arc<Notify>,
    pub event_tx: Option<mpsc::Sender<ManagementEvent>>,
    /// JNL-SYNC-01: Optional CCXT client for post-sync balance snapshot capture.
    pub cex_client: Option<Arc<crate::services::CexClient>>,
}

impl JournalSyncer {
    pub async fn run(self, shutdown: CancellationToken) {
        let base_interval = std::time::Duration::from_secs(self.interval_secs);
        let mut interval = time::interval(base_interval);
        interval.tick().await; // skip first immediate tick

        let mut consecutive_failures: u32 = 0;
        let mut backoff_secs: u64 = self.interval_secs;

        loop {
            let delay = std::time::Duration::from_secs(backoff_secs);
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!(
                        user_id = %self.user_id,
                        account_id = %self.account_id,
                        "JournalSyncer shutting down"
                    );
                    break;
                }
                _ = self.notify.notified() => {
                    tracing::debug!(
                        user_id = %self.user_id,
                        exchange = %self.exchange_label,
                        "JournalSyncer: manual sync triggered"
                    );
                }
                _ = time::sleep(delay) => {}
            }

            match self.tick().await {
                Ok(new_count) => {
                    consecutive_failures = 0;
                    backoff_secs = self.interval_secs;
                    if new_count > 0 {
                        tracing::info!(
                            user_id = %self.user_id,
                            exchange = %self.exchange_label,
                            new_fills = new_count,
                            "JournalSyncer: sync complete"
                        );
                    }
                    // Reset interval after a successful run so the next tick is on schedule.
                    interval = time::interval(base_interval);
                    interval.tick().await;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::warn!(
                        user_id = %self.user_id,
                        exchange = %self.exchange_label,
                        error = %e,
                        consecutive_failures,
                        "JournalSyncer: sync failed, watermark unchanged"
                    );

                    if consecutive_failures >= CONSECUTIVE_FAILURE_ALERT_THRESHOLD {
                        if let Some(ref tx) = self.event_tx {
                            let _ = tx.try_send(ManagementEvent {
                                user_id: self.user_id,
                                event_type: "journal_sync_failing".to_string(),
                                symbol: self.exchange_label.clone(),
                                detail: format!(
                                    "Journal sync for {} failed {} consecutive times: {}",
                                    self.exchange_label, consecutive_failures, e
                                ),
                            });
                        }
                    }

                    // Exponential backoff: 30s → 60s → 120s → 240s → 300s cap
                    backoff_secs = (backoff_secs * 2).min(300);
                }
            }
        }
    }

    pub(crate) async fn tick(&self) -> Result<usize, SyncError> {
        let watermark = self
            .exchange_account_repo
            .get_last_synced_exec_time(self.account_id)
            .await
            .map_err(|e| SyncError::Other(e.to_string()))?
            .unwrap_or_else(|| Utc::now() - Duration::days(90));

        let fills = self
            .source
            .fetch_since(self.user_id, self.account_id, watermark)
            .await?;

        if fills.is_empty() {
            return Ok(0);
        }

        let new_count = self
            .raw_fill_repo
            .upsert_many(&fills)
            .await
            .map_err(|e| SyncError::Other(e.to_string()))?;

        // Advance watermark to max exec_time in this batch.
        if let Some(max_ts) = fills.iter().map(|f| f.exec_time).max() {
            self.exchange_account_repo
                .set_last_synced_exec_time(self.account_id, max_ts)
                .await
                .map_err(|e| SyncError::Other(e.to_string()))?;
        }

        // Reconstruct all round trips for this account and upsert.
        let all_fills = self
            .raw_fill_repo
            .fetch_for_account(self.user_id, &self.exchange_label)
            .await
            .map_err(|e| SyncError::Other(e.to_string()))?;

        let trades = reconstruct_trades(&all_fills);
        if !trades.is_empty() {
            self.journal_service
                .upsert_many_pull_sync(&trades)
                .await
                .map_err(|e| SyncError::Other(e.to_string()))?;
        }

        // JNL-SYNC-01: Fire balance snapshot if new fills arrived and we have a CCXT client.
        if new_count > 0 {
            if let Some(ref cex_client) = self.cex_client {
                crate::services::balance_snapshot::spawn_balance_snapshot(
                    self.pool.clone(),
                    Arc::clone(cex_client),
                    Arc::new(self.exchange_account_repo.clone()),
                    self.user_id,
                    self.account_id,
                    self.exchange_label.clone(),
                );
            }
        }

        Ok(new_count)
    }
}

/// Convenience constructor — all fields wired via this to avoid field mismatches.
pub struct JournalSyncerBuilder {
    pub user_id: Uuid,
    pub account_id: Uuid,
    pub exchange_label: String,
    pub interval_secs: u64,
    pub source: Arc<dyn FillSource>,
    pub pool: PgPool,
    pub exchange_account_repo: ExchangeAccountRepository,
    pub journal_service: Arc<JournalService>,
    pub notify: Arc<Notify>,
    pub event_tx: Option<mpsc::Sender<ManagementEvent>>,
    /// JNL-SYNC-01: Optional CCXT client for post-sync balance snapshot capture.
    pub cex_client: Option<Arc<crate::services::CexClient>>,
}

impl JournalSyncerBuilder {
    pub fn build(self) -> JournalSyncer {
        let pool = self.pool.clone();
        JournalSyncer {
            user_id: self.user_id,
            account_id: self.account_id,
            exchange_label: self.exchange_label.clone(),
            interval_secs: self.interval_secs,
            source: self.source,
            pool,
            raw_fill_repo: RawFillRepository::new(self.pool),
            exchange_account_repo: self.exchange_account_repo,
            journal_service: self.journal_service,
            notify: self.notify,
            event_tx: self.event_tx,
            cex_client: self.cex_client,
        }
    }
}
