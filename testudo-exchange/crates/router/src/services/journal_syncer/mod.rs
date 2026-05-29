//! Journal Syncer — pull-based journal pipeline (JNL-SYNC-01)
//!
//! REST-polling model: each `JournalSyncer` task runs per `(user_id, exchange_account_id)` on a
//! fixed cadence, upserts raw fills to `raw_fills`, then calls
//! `reconstruct_trades` to project closed round trips into `journal_trades`.

// @anchor exchange:router:mod
// @tags api

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common_utils::journal::RawFill;
use thiserror::Error;
use uuid::Uuid;

pub mod ccxt;
pub mod hyperliquid;
pub mod syncer;

#[cfg(test)]
mod integration_tests;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Deserialization error: {0}")]
    Deser(String),
    #[error("Rate limit")]
    RateLimit,
    #[error("Credential error: {0}")]
    Credential(String),
    #[error("Other: {0}")]
    Other(String),
}

#[async_trait]
pub trait FillSource: Send + Sync {
    async fn fetch_since(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<RawFill>, SyncError>;

    fn exchange_label(&self) -> &str;
}
