// @anchor exchange:pg_queue:request_response
// @tags infra

use crate::errors::{PgQueueError, Result};
use crate::listen::ListenerService;
use crate::queue::{QueueName, QueueRepository};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

/// Service for request-response pattern using PostgreSQL.
/// Uses queue for requests and LISTEN/NOTIFY for responses.
#[derive(Clone)]
pub struct RequestResponseService {
    pool: PgPool,
    queue: QueueRepository,
}

impl RequestResponseService {
    pub fn new(pool: PgPool, queue: QueueRepository) -> Self {
        Self { pool, queue }
    }

    /// Push a request to a queue and wait for a response
    /// The response is expected to be published to the request_responses table
    /// with a trigger that sends a NOTIFY
    pub async fn push_and_wait<Req, Resp>(
        &self,
        queue: QueueName,
        request: &Req,
        timeout_duration: Duration,
    ) -> Result<Resp>
    where
        Req: Serialize,
        Resp: DeserializeOwned,
    {
        let request_id = Uuid::new_v4();
        let channel = format!("response_{}", request_id);

        // Set up listener before pushing to avoid race condition
        let mut listener = ListenerService::new(&self.pool).await?;
        listener.listen(&channel).await?;

        // Wrap request with ID for correlation
        let wrapped = RequestWrapper {
            request_id,
            payload: serde_json::to_value(request)?,
        };

        // Push to queue
        self.queue.push(queue, &wrapped).await?;

        // Wait for response notification
        match timeout(timeout_duration, listener.recv()).await {
            Ok(Ok(_notification)) => {
                // Notification received, fetch the response from the table
                let response = self.fetch_response::<Resp>(&request_id).await?;
                Ok(response)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(PgQueueError::Timeout),
        }
    }

    /// Store a response for a request (called by the processor)
    pub async fn store_response<T: Serialize>(
        &self,
        request_id: &Uuid,
        response: &T,
    ) -> Result<()> {
        let json = serde_json::to_value(response)?;

        sqlx::query("INSERT INTO request_responses (request_id, response) VALUES ($1, $2)")
            .bind(request_id)
            .bind(json)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Fetch a stored response
    async fn fetch_response<T: DeserializeOwned>(&self, request_id: &Uuid) -> Result<T> {
        let row: (serde_json::Value,) =
            sqlx::query_as("SELECT response FROM request_responses WHERE request_id = $1")
                .bind(request_id)
                .fetch_one(&self.pool)
                .await?;

        let parsed: T = serde_json::from_value(row.0)?;
        Ok(parsed)
    }

    /// Clean up old responses (housekeeping)
    pub async fn cleanup_old_responses(&self, older_than: Duration) -> Result<u64> {
        let cutoff = chrono::Utc::now()
            - chrono::Duration::from_std(older_than)
                .map_err(|e| PgQueueError::Listener(e.to_string()))?;

        let result = sqlx::query("DELETE FROM request_responses WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

/// Wrapper for requests to include correlation ID
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct RequestWrapper {
    pub request_id: Uuid,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_wrapper_serialization() {
        let request_id = Uuid::new_v4();
        let payload = serde_json::json!({"action": "test"});

        let wrapper = RequestWrapper {
            request_id,
            payload: payload.clone(),
        };

        let serialized = serde_json::to_string(&wrapper).unwrap();
        let deserialized: RequestWrapper = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.request_id, request_id);
        assert_eq!(deserialized.payload, payload);
    }

    #[test]
    fn test_response_channel_format() {
        let request_id = Uuid::new_v4();
        let channel = format!("response_{}", request_id);
        assert!(channel.starts_with("response_"));
    }
}
