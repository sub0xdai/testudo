use actix_web::web::Data;
use db_processor::query::get_trades_from_db;

use std::time::Instant;

use crate::types::{app::AppState, routes::GetTradesInput};

/// Trade cache TTL in seconds (FR-3.1.1: 5 second TTL)
const TRADE_CACHE_TTL_SECS: u64 = 5;

/// Build cache key for trades (FR-3.1.2: format `trades:{symbol}:{limit}`)
fn build_trade_cache_key(symbol: &str, limit: u32) -> String {
    format!("trades:{}:{}", symbol, limit)
}

/// GET /trades endpoint with PostgreSQL caching
///
/// Implements FR-3.1.1 through FR-3.1.3 from 006-performance-overhaul:
/// - PostgreSQL cache with 5s TTL (migrated from Redis)
/// - Cache key format: `trades:{symbol}:{limit}`
/// - Cache is invalidated on new trade insertion (handled by insert_trade)
pub async fn get_trades(
    query: actix_web::web::Query<GetTradesInput>,
    app_state: Data<AppState>,
) -> actix_web::HttpResponse {
    let starttime = Instant::now();
    let market_data = query.into_inner();
    let symbol = market_data.symbol.clone();
    let limit = 100u32; // Default limit, could be made configurable

    tracing::debug!(symbol = %symbol, "get_trades");

    // FR-3.1.1: Try PostgreSQL cache first
    let cache_key = build_trade_cache_key(&symbol, limit);
    if let Ok(Some(cached)) = app_state.pg_queue.cache.get::<String>(&cache_key).await {
        tracing::debug!(cache_key = %cache_key, elapsed = ?starttime.elapsed(), "cache_hit");
        return actix_web::HttpResponse::Ok()
            .content_type("application/json")
            .body(cached);
    }

    // Cache miss - fetch from database
    let pg_pool = match app_state.postgres_db.get_pg_connection() {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to get database connection: {}", e);
            tracing::debug!(elapsed = ?starttime.elapsed(), "db_connection_failed");
            return actix_web::HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "database_error", "message": "Database connection unavailable"}));
        }
    };

    let trades = match get_trades_from_db(&pg_pool, symbol.clone()).await {
        Ok(trades) => trades,
        Err(e) => {
            tracing::error!("Failed to fetch trades from database: {}", e);
            tracing::debug!(elapsed = ?starttime.elapsed(), "query_failed");
            return actix_web::HttpResponse::InternalServerError().json(
                serde_json::json!({"error": "query_error", "message": "Failed to fetch trades"}),
            );
        }
    };

    // Serialize and cache the result
    let json_response = match serde_json::to_string(&trades) {
        Ok(json) => json,
        Err(e) => {
            tracing::error!("Failed to serialize trades: {}", e);
            return actix_web::HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "serialization_error", "message": "Failed to serialize response"}));
        }
    };

    // Store in PostgreSQL cache (fire-and-forget, don't block on cache write)
    let cache_result = app_state
        .pg_queue
        .cache
        .set(&cache_key, &json_response, TRADE_CACHE_TTL_SECS)
        .await;
    if let Err(e) = cache_result {
        tracing::warn!("Failed to cache trades: {}", e);
    }

    tracing::debug!(cache_key = %cache_key, elapsed = ?starttime.elapsed(), "cache_miss");

    actix_web::HttpResponse::Ok()
        .content_type("application/json")
        .body(json_response)
}
