#![allow(clippy::type_complexity)]
#![allow(clippy::module_inception)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::len_zero)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::unwrap_or_default)]
#![allow(deprecated)]

// @anchor exchange:engine:main
// @tags domain

pub mod engine;
pub mod order;
pub mod shadow;
pub mod types;
pub mod user;

use engine::engine::Engine;
use order::handle_order_pg;
use pg_queue::{PgQueueManager, QueueName};
use sqlx_postgres::PostgresDb;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task;
use user::handle_user_pg;

#[tokio::main]
async fn main() {
    let postgres = PostgresDb::new().await.unwrap();
    let pg_pool = postgres.get_pg_connection().unwrap();
    println!("Postgres connection pool ready!");

    // Create PgQueueManager for queue operations
    let pg_queue = Arc::new(PgQueueManager::new(pg_pool.clone()));
    println!("PostgreSQL queue manager ready!");

    // Use Arc and Mutex to safely share engine across tasks
    let engine = Arc::new(Mutex::new(Engine::new()));
    engine.lock().await.init_engine(&pg_pool).await
        .expect("Failed to initialize engine from database");
    engine.lock().await.init_user_balance("test_user");

    // Spawn a task to handle orders using PostgreSQL LISTEN/NOTIFY
    let pg_queue_orders = Arc::clone(&pg_queue);
    let engine_orders = Arc::clone(&engine);
    let orders_handle = task::spawn(async move {
        consume_queue_loop(
            pg_queue_orders,
            engine_orders,
            QueueName::Orders,
            |pg_queue, engine, payload| async move {
                handle_order_pg(payload, &pg_queue, engine).await;
            },
        )
        .await;
    });

    // Spawn a task to handle users using PostgreSQL LISTEN/NOTIFY
    let pg_queue_users = Arc::clone(&pg_queue);
    let engine_users = Arc::clone(&engine);
    let users_handle = task::spawn(async move {
        consume_queue_loop(
            pg_queue_users,
            engine_users,
            QueueName::Users,
            |pg_queue, engine, payload| async move {
                handle_user_pg(payload, &pg_queue, engine).await;
            },
        )
        .await;
    });

    // Await both tasks to run concurrently
    if let Err(e) = orders_handle.await {
        println!("Error in the orders task: {:?}", e);
    }

    if let Err(e) = users_handle.await {
        println!("Error in the users task: {:?}", e);
    }
}

/// Generic queue consumer loop with LISTEN/NOTIFY wake mechanism
async fn consume_queue_loop<F, Fut>(
    pg_queue: Arc<PgQueueManager>,
    engine: Arc<Mutex<Engine>>,
    queue_name: QueueName,
    handler: F,
) where
    F: Fn(Arc<PgQueueManager>, Arc<Mutex<Engine>>, serde_json::Value) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    // Set up listener for queue notifications
    let mut listener = match pg_queue.create_listener().await {
        Ok(l) => l,
        Err(e) => {
            println!("Failed to create listener for {}: {:?}", queue_name, e);
            return;
        }
    };

    if let Err(e) = listener.listen(queue_name.channel_name()).await {
        println!("Failed to listen on {}: {:?}", queue_name.channel_name(), e);
        return;
    }

    println!("Listening on queue: {}", queue_name);

    loop {
        // Try to claim a job
        match pg_queue.queue.pop::<serde_json::Value>(queue_name).await {
            Ok(Some(job)) => {
                let engine_clone = Arc::clone(&engine);
                let pg_queue_clone = Arc::clone(&pg_queue);

                // Process the job
                handler(pg_queue_clone.clone(), engine_clone, job.payload).await;

                // Mark job as completed
                if let Err(e) = pg_queue_clone.queue.complete(queue_name, job.id).await {
                    println!("Failed to complete job {}: {:?}", job.id, e);
                }

                // Continue immediately to check for more jobs
                continue;
            }
            Ok(None) => {
                // No jobs available, wait for notification
            }
            Err(e) => {
                println!("Error popping from {} queue: {:?}", queue_name, e);
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
                println!("Listener error on {}: {:?}", queue_name, e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}
