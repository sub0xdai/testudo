use crate::errors::Result;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::PgPool;

/// Job status values used in queue tables
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Processing => "processing",
            JobStatus::Completed => "completed",
        }
    }
}

/// Supported queue names matching the migration tables
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueName {
    Orders,
    Users,
    Database,
    TradeImports,
}

impl QueueName {
    pub fn table_name(&self) -> &'static str {
        match self {
            QueueName::Orders => "queue_orders",
            QueueName::Users => "queue_users",
            QueueName::Database => "queue_database",
            QueueName::TradeImports => "queue_imports",
        }
    }

    /// Channel name for LISTEN/NOTIFY (same as table name by convention)
    pub fn channel_name(&self) -> &'static str {
        self.table_name()
    }
}

impl std::fmt::Display for QueueName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.table_name())
    }
}

/// A job retrieved from the queue
#[derive(Debug)]
pub struct Job<T> {
    pub id: i64,
    pub payload: T,
}

/// Queue repository for push/pop operations using SKIP LOCKED
#[derive(Clone)]
pub struct QueueRepository {
    pool: PgPool,
}

impl QueueRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Push a job to the queue
    pub async fn push<T: Serialize>(&self, queue: QueueName, payload: &T) -> Result<i64> {
        let json = serde_json::to_value(payload)?;

        let row: (i64,) = sqlx::query_as(&format!(
            "INSERT INTO {} (payload) VALUES ($1) RETURNING id",
            queue.table_name()
        ))
        .bind(json)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Pop a job from the queue using SKIP LOCKED for concurrent safety.
    /// Returns None if no pending jobs are available.
    pub async fn pop<T: DeserializeOwned>(&self, queue: QueueName) -> Result<Option<Job<T>>> {
        let table = queue.table_name();
        let pending = JobStatus::Pending.as_str();
        let processing = JobStatus::Processing.as_str();

        let row: Option<(i64, serde_json::Value)> = sqlx::query_as(&format!(
            r#"
            UPDATE {table} SET status = '{processing}', processed_at = NOW()
            WHERE id = (
                SELECT id FROM {table} WHERE status = '{pending}'
                ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1
            )
            RETURNING id, payload
            "#
        ))
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((id, payload)) => {
                let parsed: T = serde_json::from_value(payload)?;
                Ok(Some(Job {
                    id,
                    payload: parsed,
                }))
            }
            None => Ok(None),
        }
    }

    /// Mark a job as completed
    pub async fn complete(&self, queue: QueueName, job_id: i64) -> Result<()> {
        sqlx::query(&format!(
            "UPDATE {} SET status = '{}' WHERE id = $1",
            queue.table_name(),
            JobStatus::Completed.as_str()
        ))
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark a job as failed, resetting it to pending for retry
    pub async fn fail(&self, queue: QueueName, job_id: i64) -> Result<()> {
        sqlx::query(&format!(
            "UPDATE {} SET status = '{}', processed_at = NULL WHERE id = $1",
            queue.table_name(),
            JobStatus::Pending.as_str()
        ))
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get the count of pending jobs in a queue
    pub async fn pending_count(&self, queue: QueueName) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(&format!(
            "SELECT COUNT(*) FROM {} WHERE status = '{}'",
            queue.table_name(),
            JobStatus::Pending.as_str()
        ))
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_name_table() {
        assert_eq!(QueueName::Orders.table_name(), "queue_orders");
        assert_eq!(QueueName::Users.table_name(), "queue_users");
        assert_eq!(QueueName::Database.table_name(), "queue_database");
        assert_eq!(QueueName::TradeImports.table_name(), "queue_imports");
    }

    #[test]
    fn test_channel_name_delegates_to_table_name() {
        assert_eq!(QueueName::Orders.channel_name(), QueueName::Orders.table_name());
        assert_eq!(QueueName::Users.channel_name(), QueueName::Users.table_name());
    }

    #[test]
    fn test_queue_name_display() {
        assert_eq!(format!("{}", QueueName::Orders), "queue_orders");
    }

    #[test]
    fn test_job_status_strings() {
        assert_eq!(JobStatus::Pending.as_str(), "pending");
        assert_eq!(JobStatus::Processing.as_str(), "processing");
        assert_eq!(JobStatus::Completed.as_str(), "completed");
    }
}
