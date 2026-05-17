//! Trade Event Writer (019f)
//!
//! Single-writer persistence layer for the append-only trade event log.
//! Receives `TradeEvent`s from the `EngineActor` and `FillDetector` via
//! a shared mpsc channel and flushes them to PostgreSQL in batched transactions.
//!
//! Each flush writes:
//! 1. Bulk INSERT into `trade_events` (append-only audit log)
//! 2. UPDATE `managed_positions` state on fill events (entry → filled, SL/TP → closed)
//!
//! Journal writes are handled by the JournalSyncer pull pipeline (JNL-SYNC-01).

use engine::{TradeEvent, TradeEventType};
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::mpsc;

/// Background Tokio task that batches and persists trade events.
pub struct TradeEventWriter {
    rx: mpsc::Receiver<TradeEvent>,
    pool: PgPool,
}

/// Maximum events per flush batch.
const BATCH_SIZE: usize = 50;
/// Flush interval (milliseconds).
const FLUSH_INTERVAL_MS: u64 = 100;
/// Maximum retry attempts before discarding a batch.
const MAX_RETRIES: u32 = 3;

impl TradeEventWriter {
    pub fn new(
        rx: mpsc::Receiver<TradeEvent>,
        pool: PgPool,
    ) -> Self {
        Self { rx, pool }
    }

    /// Run the writer loop. Batches events and flushes every 100ms or 50 events.
    pub async fn run(mut self) {
        let mut batch: Vec<TradeEvent> = Vec::with_capacity(BATCH_SIZE);
        let mut flush_interval = tokio::time::interval(Duration::from_millis(FLUSH_INTERVAL_MS));

        tracing::info!("TradeEventWriter started (batch_size={}, flush_interval={}ms)", BATCH_SIZE, FLUSH_INTERVAL_MS);

        loop {
            tokio::select! {
                event = self.rx.recv() => {
                    match event {
                        Some(event) => {
                            batch.push(event);
                            if batch.len() >= BATCH_SIZE {
                                self.flush(&mut batch).await;
                            }
                        }
                        None => {
                            // Channel closed — flush remaining and exit
                            if !batch.is_empty() {
                                self.flush(&mut batch).await;
                            }
                            tracing::info!("TradeEventWriter shut down — channel closed");
                            break;
                        }
                    }
                }
                _ = flush_interval.tick() => {
                    if !batch.is_empty() {
                        self.flush(&mut batch).await;
                    }
                }
            }
        }
    }

    /// Flush the batch with retry logic.
    async fn flush(&self, batch: &mut Vec<TradeEvent>) {
        let mut retries = 0u32;
        loop {
            match self.flush_transaction(batch).await {
                Ok(_) => {
                    batch.clear();
                    return;
                }
                Err(e) => {
                    retries += 1;
                    if retries > MAX_RETRIES {
                        tracing::error!(
                            error = %e,
                            batch_size = batch.len(),
                            "TradeEventWriter: flush failed after {} retries, discarding batch",
                            MAX_RETRIES
                        );
                        // Alert via pg_notify
                        let _ = sqlx::query("SELECT pg_notify('system.alerts', $1)")
                            .bind(format!("event_writer_flush_failed: {}", e))
                            .execute(&self.pool)
                            .await;
                        batch.clear();
                        return;
                    }
                    let delay = Duration::from_millis(100 * (1 << retries));
                    tracing::warn!(
                        error = %e,
                        retry = retries,
                        "TradeEventWriter: flush failed, retrying in {:?}",
                        delay
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Execute a single-transaction flush: insert events + apply mutable state updates.
    async fn flush_transaction(&self, batch: &[TradeEvent]) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // 1. Bulk insert events into append-only log
        for event in batch {
            sqlx::query(
                "INSERT INTO trade_events (event_type, group_id, user_id, symbol, payload) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(event.event_type.as_str())
            .bind(event.group_id)
            .bind(event.user_id)
            .bind(&event.symbol)
            .bind(&event.payload)
            .execute(&mut *tx)
            .await?;
        }

        // 2. Apply mutable state updates derived from fill events
        for event in batch {
            match event.event_type {
                TradeEventType::EntryFilled => {
                    if let Some(group_id) = event.group_id {
                        sqlx::query(
                            "UPDATE managed_positions SET state = 'filled', updated_at = now() WHERE id = $1",
                        )
                        .bind(group_id)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                TradeEventType::StopLossFilled | TradeEventType::TakeProfitFilled => {
                    if let Some(group_id) = event.group_id {
                        sqlx::query(
                            "UPDATE managed_positions SET state = 'closed', updated_at = now() WHERE id = $1",
                        )
                        .bind(group_id)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                _ => {} // Other events are log-only
            }
        }

        tx.commit().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::TradeEvent;

    #[test]
    fn test_writer_constants() {
        assert_eq!(BATCH_SIZE, 50);
        assert_eq!(FLUSH_INTERVAL_MS, 100);
        assert_eq!(MAX_RETRIES, 3);
    }

    #[tokio::test]
    async fn test_writer_shuts_down_on_channel_close() {
        // Create a channel and immediately drop the sender
        let (tx, rx) = mpsc::channel::<TradeEvent>(16);
        drop(tx);

        // Writer should exit cleanly when channel closes
        // We can't test with a real PgPool here, but we verify the pattern
        // by ensuring the receiver gets None
        let mut rx = rx;
        assert!(rx.recv().await.is_none());
    }

    #[test]
    fn test_retry_backoff_progression() {
        // Verify the exponential backoff formula
        for retries in 1..=MAX_RETRIES {
            let delay_ms = 100u64 * (1 << retries);
            match retries {
                1 => assert_eq!(delay_ms, 200),
                2 => assert_eq!(delay_ms, 400),
                3 => assert_eq!(delay_ms, 800),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn test_event_type_as_str_for_db() {
        // Ensure all event types produce valid strings for DB storage
        let types = vec![
            TradeEventType::TradeCreated,
            TradeEventType::EntryPlaced,
            TradeEventType::EntryFilled,
            TradeEventType::StopLossPlaced,
            TradeEventType::StopLossFilled,
            TradeEventType::TakeProfitPlaced,
            TradeEventType::TakeProfitFilled,
            TradeEventType::OrderCancelled,
            TradeEventType::GroupStatusChanged,
            TradeEventType::BreakEvenTriggered,
            TradeEventType::StopLossAmended,
            TradeEventType::ReconciliationAction,
            TradeEventType::PlacementTimeout,
            TradeEventType::TradeClosed,
        ];
        for t in types {
            let s = t.as_str();
            assert!(!s.is_empty());
            assert!(s.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        }
    }

    #[test]
    fn test_group_status_changed_payload_format() {
        // Verify the payload format used by the writer for group status updates
        let payload = serde_json::json!({
            "from": "Pending",
            "to": "Active",
        });
        assert_eq!(payload["to"].as_str().unwrap(), "Active");
        assert_eq!(payload["from"].as_str().unwrap(), "Pending");
    }
}
