use crate::{types::engine::UserRequests, Engine};
use pg_queue::PgQueueManager;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Handle user requests from PostgreSQL queue
/// Uses pg_notify for responses instead of Redis pub/sub
pub async fn handle_user_pg(
    payload: serde_json::Value,
    pg_queue: &Arc<PgQueueManager>,
    engine: Arc<Mutex<Engine>>,
) {
    match serde_json::from_value::<UserRequests>(payload) {
        Ok(user) => match user {
            UserRequests::CreateUser(user) => {
                println!("Create User: {:?}", user);
                let pubsub_id = match user.pubsub_id {
                    Some(id) => id.to_string(),
                    None => {
                        tracing::warn!("CreateUser missing pubsub_id, cannot respond");
                        return;
                    }
                };

                let mut engine_guard = engine.lock().await;
                engine_guard.init_user_balance(user.user_id.as_str());
                drop(engine_guard);

                let create_user_json = serde_json::json!({
                    "status": "Created User",
                    "user_id": user.user_id,
                });

                if let Ok(response_string) = serde_json::to_string(&create_user_json) {
                    let _ = pg_queue.notify.notify(&pubsub_id, &response_string).await;
                }

                println!("Successfully created user!")
            }
        },
        Err(err) => {
            println!("Failed to deserialize user request: {:?}", err);
        }
    }
}
