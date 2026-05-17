pub mod query;
pub mod types;

use query::insert_trade;
use sqlx::{Pool, Postgres};
use types::DatabaseRequests;

/// Handle database updates from PostgreSQL queue
pub async fn handle_db_updates_pg(db_request: DatabaseRequests, pg_pool: &Pool<Postgres>) {
    match db_request {
        DatabaseRequests::InsertTrade(db_data) => {
            println!("Received Trade {:?}", db_data);
            let _ = insert_trade(pg_pool, db_data).await;
        }
    }
}
