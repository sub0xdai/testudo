// @anchor exchange:db-processor:main
// @tags infra

use db_processor::{handle_db_updates_pg, types::DatabaseRequests};
use pg_queue::{PgQueueManager, QueueName};
use sqlx_postgres::PostgresDb;
use std::time::Duration;
pub mod query;
pub mod seed;
pub mod types;

#[tokio::main]
async fn main() {
    let postgres = PostgresDb::new().await.unwrap();
    let pg_pool = postgres.get_pg_connection().unwrap();
    println!("Postgres connection pool ready!");

    // Create PgQueueManager for queue operations
    let pg_queue = PgQueueManager::new(pg_pool.clone());
    println!("PostgreSQL queue manager ready!");

    // Set up listener for queue notifications
    let mut listener = match pg_queue.create_listener().await {
        Ok(l) => l,
        Err(e) => {
            println!("Failed to create listener: {:?}", e);
            return;
        }
    };

    if let Err(e) = listener.listen(QueueName::Database.channel_name()).await {
        println!("Failed to listen on queue_database: {:?}", e);
        return;
    }

    println!("Listening on queue: {}", QueueName::Database);

    loop {
        // Try to claim a job
        match pg_queue
            .queue
            .pop::<DatabaseRequests>(QueueName::Database)
            .await
        {
            Ok(Some(job)) => {
                handle_db_updates_pg(job.payload, &pg_pool).await;

                // Mark job as completed
                if let Err(e) = pg_queue.queue.complete(QueueName::Database, job.id).await {
                    println!("Failed to complete job {}: {:?}", job.id, e);
                }

                // Continue immediately to check for more jobs
                continue;
            }
            Ok(None) => {
                // No jobs available, wait for notification
            }
            Err(e) => {
                println!("Error popping from database queue: {:?}", e);
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        }

        // Wait for notification (with timeout to handle edge cases)
        match listener.recv_timeout(Duration::from_secs(5)).await {
            Ok(Some(_notification)) => {
                // Notification received, loop back to try claiming a job
            }
            Ok(None) => {
                // Timeout, check for any missed jobs
            }
            Err(e) => {
                println!("Listener error: {:?}", e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}
