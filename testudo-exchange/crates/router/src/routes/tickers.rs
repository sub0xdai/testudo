// @anchor exchange:router:tickers
// @tags api

use crate::types::app::AppState;
use actix_web::web::Data;

use db_processor::query::get_tickers_from_db;

use std::time::Instant;

pub async fn get_tickers(app_state: Data<AppState>) -> actix_web::HttpResponse {
    let starttime = Instant::now();

    println!("Get Tickers:");

    let pg_pool = match app_state.postgres_db.get_pg_connection() {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            println!("Time: {:?}", starttime.elapsed());
            return actix_web::HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "database_error", "message": "Database connection unavailable"}));
        }
    };

    let tickers = match get_tickers_from_db(&pg_pool).await {
        Ok(tickers) => tickers,
        Err(e) => {
            tracing::error!("Failed to fetch tickers from database: {}", e);
            println!("Time: {:?}", starttime.elapsed());
            return actix_web::HttpResponse::InternalServerError().json(
                serde_json::json!({"error": "query_error", "message": "Failed to fetch tickers"}),
            );
        }
    };

    println!("Time: {:?}", starttime.elapsed());

    actix_web::HttpResponse::Ok().json(tickers)
}
