//! JNL-13: Balance Snapshot Service
//!
//! Captures and queries account equity snapshots for true equity curves
//! and accurate max drawdown calculations. Snapshots are taken at trade
//! boundaries (TradeClosed events) via exchange API balance fetch.

// @anchor exchange:router:balance_snapshot
// @tags api

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

use std::sync::Arc;

use super::cex_client::{CexClient, SidecarCredentials};

/// A single balance snapshot row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BalanceSnapshot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub exchange_account_id: Uuid,
    pub equity: Decimal,
    pub available: Decimal,
    pub snapshot_at: DateTime<Utc>,
}

/// Daily equity point aggregated from snapshots (last snapshot per day).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DailyEquityRow {
    pub snapshot_date: NaiveDate,
    pub equity: Decimal,
}

pub struct BalanceSnapshotService {
    pool: PgPool,
}

impl BalanceSnapshotService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a balance snapshot. Called after a trade closes.
    pub async fn insert(
        &self,
        user_id: Uuid,
        exchange_account_id: Uuid,
        equity: Decimal,
        available: Decimal,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO balance_snapshots \
             (user_id, exchange_account_id, equity, available, snapshot_at) \
             VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(user_id)
        .bind(exchange_account_id)
        .bind(equity)
        .bind(available)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if any snapshots exist for a user (optionally filtered by date range).
    pub async fn has_snapshots(
        &self,
        user_id: Uuid,
        date_from: Option<NaiveDate>,
        date_to: Option<NaiveDate>,
    ) -> Result<bool, sqlx::Error> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM balance_snapshots \
             WHERE user_id = $1 \
               AND ($2::DATE IS NULL OR snapshot_at >= $2::DATE) \
               AND ($3::DATE IS NULL OR snapshot_at <= ($3::DATE + INTERVAL '1 day'))",
        )
        .bind(user_id)
        .bind(date_from)
        .bind(date_to)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0 > 0)
    }

    /// Fetch daily equity values (last snapshot per day), ordered by date.
    pub async fn daily_equity(
        &self,
        user_id: Uuid,
        date_from: Option<NaiveDate>,
        date_to: Option<NaiveDate>,
    ) -> Result<Vec<DailyEquityRow>, sqlx::Error> {
        sqlx::query_as::<_, DailyEquityRow>(
            "SELECT \
                 snapshot_at::DATE as snapshot_date, \
                 (ARRAY_AGG(equity ORDER BY snapshot_at DESC))[1] as equity \
             FROM balance_snapshots \
             WHERE user_id = $1 \
               AND ($2::DATE IS NULL OR snapshot_at >= $2::DATE) \
               AND ($3::DATE IS NULL OR snapshot_at <= ($3::DATE + INTERVAL '1 day')) \
             GROUP BY snapshot_at::DATE \
             ORDER BY snapshot_date",
        )
        .bind(user_id)
        .bind(date_from)
        .bind(date_to)
        .fetch_all(&self.pool)
        .await
    }

    /// Compute max drawdown from snapshots (equity-based denominator).
    /// Returns (max_drawdown_abs, max_drawdown_pct).
    pub async fn max_drawdown(
        &self,
        user_id: Uuid,
        date_from: Option<NaiveDate>,
        date_to: Option<NaiveDate>,
    ) -> Result<(Decimal, Decimal), sqlx::Error> {
        let row: (Decimal, Decimal) = sqlx::query_as(
            "WITH daily AS ( \
                 SELECT \
                     snapshot_at::DATE as d, \
                     (ARRAY_AGG(equity ORDER BY snapshot_at DESC))[1] as equity \
                 FROM balance_snapshots \
                 WHERE user_id = $1 \
                   AND ($2::DATE IS NULL OR snapshot_at >= $2::DATE) \
                   AND ($3::DATE IS NULL OR snapshot_at <= ($3::DATE + INTERVAL '1 day')) \
                 GROUP BY snapshot_at::DATE \
             ), \
             peaks AS ( \
                 SELECT equity, \
                     MAX(equity) OVER (ORDER BY d ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) as peak \
                 FROM daily \
             ) \
             SELECT \
                 COALESCE(MAX(peak - equity), 0), \
                 COALESCE(MAX(CASE WHEN peak > 0 THEN (peak - equity) / peak * 100 ELSE 0 END), 0) \
             FROM peaks",
        )
        .bind(user_id)
        .bind(date_from)
        .bind(date_to)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Resolve exchange_account_id from (user_id, exchange_name).
    /// Returns None if no matching active account exists.
    pub async fn resolve_account_id(
        &self,
        user_id: Uuid,
        exchange_name: &str,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM exchange_accounts \
             WHERE user_id = $1 AND exchange_name = $2 AND is_active = true \
             LIMIT 1",
        )
        .bind(user_id)
        .bind(exchange_name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0))
    }

    /// Get starting_balance for an exchange account.
    pub async fn get_starting_balance(
        &self,
        exchange_account_id: Uuid,
    ) -> Result<Option<Decimal>, sqlx::Error> {
        let row: Option<(Option<Decimal>,)> = sqlx::query_as(
            "SELECT starting_balance FROM exchange_accounts WHERE id = $1",
        )
        .bind(exchange_account_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| r.0))
    }

    /// Fire-and-forget snapshot capture. Fetches balance from exchange and inserts snapshot.
    /// Logs errors but never panics — snapshot failure must not affect trade persistence.
    pub async fn capture_snapshot(
        pool: PgPool,
        cex_client: Arc<CexClient>,
        user_id: Uuid,
        exchange_name: String,
        creds: SidecarCredentials,
    ) {
        let svc = Self::new(pool);

        let account_id = match svc.resolve_account_id(user_id, &exchange_name).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                tracing::debug!(
                    user_id = %user_id,
                    exchange = %exchange_name,
                    "BalanceSnapshot: no active account found, skipping"
                );
                return;
            }
            Err(e) => {
                tracing::warn!(
                    user_id = %user_id,
                    error = %e,
                    "BalanceSnapshot: failed to resolve account, skipping"
                );
                return;
            }
        };

        let balance = match cex_client
            .fetch_balance(&exchange_name, &creds, false, "future")
            .await
        {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    user_id = %user_id,
                    exchange = %exchange_name,
                    error = %e,
                    "BalanceSnapshot: failed to fetch balance, skipping"
                );
                return;
            }
        };

        let (equity, available) = balance
            .iter()
            .find(|b| b.asset == "USDT" || b.asset == "USD")
            .map(|b| {
                let total = Decimal::from_str(&b.total).unwrap_or(Decimal::ZERO);
                let free = Decimal::from_str(&b.free).unwrap_or(Decimal::ZERO);
                (total, free)
            })
            .unwrap_or((Decimal::ZERO, Decimal::ZERO));

        if equity == Decimal::ZERO {
            tracing::debug!(
                user_id = %user_id,
                exchange = %exchange_name,
                "BalanceSnapshot: zero equity, skipping"
            );
            return;
        }

        if let Err(e) = svc.insert(user_id, account_id, equity, available).await {
            tracing::warn!(
                user_id = %user_id,
                exchange = %exchange_name,
                error = %e,
                "BalanceSnapshot: failed to insert snapshot"
            );
        } else {
            tracing::info!(
                user_id = %user_id,
                exchange = %exchange_name,
                equity = %equity,
                "BalanceSnapshot: captured"
            );
        }
    }
}

/// JNL-SYNC-01: Spawn a fire-and-forget balance snapshot capture.
/// Called from JournalSyncer after new fills are synced for a CCXT account.
pub fn spawn_balance_snapshot(
    pool: PgPool,
    cex_client: Arc<CexClient>,
    exchange_repo: Arc<crate::repositories::exchange_account::ExchangeAccountRepository>,
    user_id: Uuid,
    account_id: Uuid,
    exchange: String,
) {
    tokio::spawn(async move {
        let decrypted = match exchange_repo.load_credentials(account_id, user_id).await {
            Ok(c) => c,
            Err(_) => return,
        };
        let creds = SidecarCredentials {
            api_key: decrypted.api_key,
            secret: decrypted.api_secret,
            password: decrypted.passphrase,
        };
        BalanceSnapshotService::capture_snapshot(pool, cex_client, user_id, exchange, creds).await;
    });
}
