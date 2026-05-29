// @anchor exchange:pg_queue:errors
// @tags infra

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PgQueueError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Queue not found: {0}")]
    QueueNotFound(String),

    #[error("Listener error: {0}")]
    Listener(String),

    #[error("Timeout waiting for response")]
    Timeout,
}

pub type Result<T> = std::result::Result<T, PgQueueError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PgQueueError::QueueNotFound("orders".to_string());
        assert_eq!(err.to_string(), "Queue not found: orders");

        let err = PgQueueError::Timeout;
        assert_eq!(err.to_string(), "Timeout waiting for response");
    }

    #[test]
    fn test_error_from_serde() {
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err: PgQueueError = json_err.into();
        assert!(matches!(err, PgQueueError::Serialization(_)));
    }
}
