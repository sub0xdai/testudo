use actix_web::web::Data;
use db_processor::query::get_klines_timeseries_data;

use std::time::Instant;

use crate::types::{app::AppState, routes::GetKlinesInput};

pub async fn get_klines(
    query: actix_web::web::Query<GetKlinesInput>,
    app_state: Data<AppState>,
) -> actix_web::HttpResponse {
    let starttime = Instant::now();
    let mut klines_input = query.into_inner();

    klines_input.interval = match klines_input.interval.as_str() {
        "1y" | "1Y" => "year".to_string(),
        "1m" | "1M" => "month".to_string(),
        "1w" | "1W" => "week".to_string(),
        "1d" | "1D" => "day".to_string(),
        "1h" | "1H" => "hour".to_string(),
        "1min" | "1MIN" => "minute".to_string(),
        _ => "week".to_string(),
    };

    println!("Get Klines: {}", klines_input.symbol);

    let pg_pool = match app_state.postgres_db.get_pg_connection() {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            println!("Timeout: {:?}", starttime.elapsed());
            return actix_web::HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "database_error", "message": "Database connection unavailable"}));
        }
    };

    let klines = match get_klines_timeseries_data(
        &pg_pool,
        klines_input.symbol,
        klines_input.interval,
        klines_input.start_time,
    )
    .await
    {
        Ok(klines) => klines,
        Err(e) => {
            tracing::error!("Failed to fetch klines from database: {}", e);
            println!("Timeout: {:?}", starttime.elapsed());
            return actix_web::HttpResponse::InternalServerError().json(
                serde_json::json!({"error": "query_error", "message": "Failed to fetch klines"}),
            );
        }
    };

    println!("Timeout: {:?}", starttime.elapsed());

    actix_web::HttpResponse::Ok().json(klines)
}
