//! PostgreSQL-based queue, pub/sub, and cache implementation
//!
//! This crate provides a unified data layer using PostgreSQL to replace Redis:
//! - Queues: Using SKIP LOCKED for concurrent job processing
//! - Pub/Sub: Using LISTEN/NOTIFY for real-time messaging
//! - Cache: Using UNLOGGED tables with per-query TTL checks

// @anchor exchange:pg_queue:lib
// @tags infra

pub mod cache;
pub mod errors;
pub mod listen;
pub mod notify;
pub mod queue;
pub mod request_response;

pub use cache::CacheRepository;
pub use errors::{PgQueueError, Result};
pub use listen::{ListenerService, Notification};
pub use notify::NotifyService;
pub use queue::{Job, JobStatus, QueueName, QueueRepository};
pub use request_response::{RequestResponseService, RequestWrapper};

// Re-export sqlx types for convenience
pub use sqlx::PgPool;

/// Main manager combining all PostgreSQL queue/pubsub/cache functionality
#[derive(Clone)]
pub struct PgQueueManager {
    pub queue: QueueRepository,
    pub notify: NotifyService,
    pub cache: CacheRepository,
    pub request_response: RequestResponseService,
    pool: PgPool,
}

impl PgQueueManager {
    /// Create a new PgQueueManager with the given pool
    pub fn new(pool: PgPool) -> Self {
        let queue = QueueRepository::new(pool.clone());
        Self {
            notify: NotifyService::new(pool.clone()),
            cache: CacheRepository::new(pool.clone()),
            request_response: RequestResponseService::new(pool.clone(), queue.clone()),
            queue,
            pool,
        }
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new listener service for pub/sub
    pub async fn create_listener(&self) -> Result<ListenerService> {
        ListenerService::new(&self.pool).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_name_variants() {
        assert_eq!(QueueName::Orders.table_name(), "queue_orders");
        assert_eq!(QueueName::Users.table_name(), "queue_users");
        assert_eq!(QueueName::Database.table_name(), "queue_database");
        assert_eq!(QueueName::TradeImports.table_name(), "queue_imports");
    }
}
