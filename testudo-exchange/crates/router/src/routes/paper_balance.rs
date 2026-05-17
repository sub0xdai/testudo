//! Paper Trading Balance Routes
//!
//! Provides API endpoints for fetching and managing shadow/paper trading account balances.
//! These are virtual balances managed by the ShadowEngine for paper trading.

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;
use uuid::Uuid;

use crate::routes::trade_management::{ApiResponse, TradeManagementState};

/// Balance response matching frontend Balance interface
#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub asset: String,
    pub available: String,
    pub locked: String,
}

/// Extract user_id from X-User-Id header
fn extract_user_id(req: &HttpRequest) -> Result<Uuid, HttpResponse> {
    match req.headers().get("X-User-Id") {
        Some(value) => match value.to_str().ok().and_then(|s| Uuid::parse_str(s).ok()) {
            Some(id) => Ok(id),
            None => Err(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid user ID format"
            }))),
        },
        None => Err(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "X-User-Id header required"
        }))),
    }
}

/// GET /api/v1/paper/balances
///
/// Returns the user's paper trading balances from the ShadowEngine.
/// Auto-initializes new users with 10,000 USDT (lazy initialization).
pub async fn get_paper_balances(
    req: HttpRequest,
    state: web::Data<TradeManagementState>,
) -> HttpResponse {
    let user_id = match extract_user_id(&req) {
        Ok(id) => id,
        Err(response) => return response,
    };

    // Lazy initialization: if user doesn't exist, initialize with defaults
    if !state.engine_handle.user_exists(user_id).await {
        let _ = state.engine_handle.init_user(user_id).await;
    }

    // Get balances (now guaranteed to exist)
    let balances = state.engine_handle.get_balances(user_id).await;

    let response: Vec<BalanceResponse> = balances
        .iter()
        .map(|b| BalanceResponse {
            asset: b.asset.clone(),
            available: b.available.to_string(),
            locked: b.reserved.to_string(),
        })
        .collect();

    HttpResponse::Ok().json(ApiResponse::success(response))
}

/// POST /api/v1/paper/reset
///
/// Resets the user's paper trading account to default balance (10,000 USDT).
/// Use this to start fresh or recover from a blown paper account.
pub async fn reset_paper_balance(
    req: HttpRequest,
    state: web::Data<TradeManagementState>,
) -> HttpResponse {
    let user_id = match extract_user_id(&req) {
        Ok(id) => id,
        Err(response) => return response,
    };

    // Reset user's balances to defaults
    let _ = state.engine_handle.reset_user(user_id).await;

    // Return the new balances
    let balances = state.engine_handle.get_balances(user_id).await;

    let response: Vec<BalanceResponse> = balances
        .iter()
        .map(|b| BalanceResponse {
            asset: b.asset.clone(),
            available: b.available.to_string(),
            locked: b.reserved.to_string(),
        })
        .collect();

    HttpResponse::Ok().json(serde_json::json!({
        "message": "Paper trading balance reset to 10,000 USDT",
        "balances": response
    }))
}
