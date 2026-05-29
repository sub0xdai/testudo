// @anchor exchange:router:order
// @tags api

use actix_web::{web, HttpRequest, HttpResponse, Result};
use rust_decimal_macros::dec;
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    decision_loop::{DecisionInputBuilder, DecisionLoop, DecisionOrderSide, DecisionOrderType},
    log_route_action,
    middleware::AuthenticatedUser,
    types::{
        app::AppState,
        auth::ErrorResponse,
        routes::{
            CancelAllOrdersInput, CancelOrderInput, CreateOrderInput, GetOpenOrderInput,
            GetOpenOrdersInput, OrderSide,
        },
    },
    utils::auth_helpers::AuthContext,
};

use common_utils::{
    adapters::{execution_types::ExecutionMode, AccountStateBuilder},
    risk::{PgRiskConfigStorage, RiskConfig},
    services::pg_cache::PgCacheService,
    OrderSide as CommonOrderSide, OrderType, StandardOrderBuilder, TimeInForce,
};

/// Detect execution mode from request headers.
/// - `Authorization: Bearer <JWT>` -> Live
/// - `X-User-Id: <uuid>` -> Shadow
/// - Default: Shadow
fn detect_execution_mode(req: &HttpRequest) -> ExecutionMode {
    if req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("Bearer "))
        .unwrap_or(false)
    {
        // Check if X-User-Id is also present (extension sends both for paper trading)
        if req.headers().get("x-user-id").is_some()
            && req
                .headers()
                .get("x-execution-mode")
                .and_then(|v| v.to_str().ok())
                .map(|v| v == "live")
                .unwrap_or(false)
        {
            return ExecutionMode::Live;
        }
        // JWT-authenticated requests with explicit live mode header
        if req
            .headers()
            .get("x-execution-mode")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "live")
            .unwrap_or(false)
        {
            return ExecutionMode::Live;
        }
    }
    ExecutionMode::Shadow
}

/// Fetch market price from Binance ticker API.
async fn resolve_market_price(symbol: &str) -> Result<rust_decimal::Decimal, String> {
    let binance_symbol = common_utils::adapters::execution_types::symbol::to_binance(symbol);
    let url = format!(
        "https://fapi.binance.com/fapi/v2/ticker/price?symbol={}",
        binance_symbol
    );

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to fetch price: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Binance returned status {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse price response: {}", e))?;

    body.get("price")
        .and_then(|p| p.as_str())
        .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
        .ok_or_else(|| "No price in response".to_string())
}

/// POST /api/v1/order
/// Execute a new order on an external exchange
pub async fn execute_order(
    req: HttpRequest,
    user: AuthenticatedUser,
    body: web::Json<CreateOrderInput>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let order_input = body.into_inner();

    let auth_ctx = AuthContext::new(user);
    let authorized_user_id = auth_ctx.authorize_user_id(&order_input.user_id)?;

    let decision_side = match order_input.side {
        OrderSide::BUY => DecisionOrderSide::Long,
        OrderSide::SELL => DecisionOrderSide::Short,
    };

    let decision_order_type = if order_input.price.is_zero() {
        DecisionOrderType::Market
    } else {
        DecisionOrderType::Limit
    };

    // Resolve market price for market orders
    let entry_price = if order_input.price.is_zero() {
        match resolve_market_price(&order_input.market).await {
            Ok(price) => price,
            Err(_) => {
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "market_price_unavailable",
                    "message": "Could not determine current market price for this symbol",
                })));
            }
        }
    } else {
        order_input.price
    };

    let decision_input = DecisionInputBuilder::new()
        .user_id(authorized_user_id.user_id())
        .symbol(&order_input.market)
        .side(decision_side)
        .order_type(decision_order_type)
        .quantity(order_input.quantity)
        .entry_price(entry_price)
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build DecisionInput: {}", e);
            actix_web::error::ErrorBadRequest(format!("Invalid order parameters: {}", e))
        })?;

    // Detect execution mode from request headers
    let execution_mode = detect_execution_mode(&req);

    // Fetch real account state based on mode
    let account_state = match execution_mode {
        ExecutionMode::Shadow => {
            let balances = app_state.engine_handle.get_balances(authorized_user_id.user_id()).await;
            let usdt_balance = balances
                .iter()
                .find(|b| b.asset == "USDT")
                .map(|b| b.available + b.reserved)
                .unwrap_or(dec!(10000));
            let positions = app_state.engine_handle.get_positions(authorized_user_id.user_id()).await;
            let unrealized_pnl = app_state.engine_handle
                .get_unrealized_pnl(authorized_user_id.user_id())
                .await;

            AccountStateBuilder::new()
                .balance(usdt_balance)
                .open_positions(positions.len() as u32)
                .daily_pnl(unrealized_pnl)
                .starting_balance(dec!(10000))
                .build()
        }
        ExecutionMode::Live => {
            // Live mode: fallback to defaults until Binance account fetch is wired
            AccountStateBuilder::new().build()
        }
    };

    // Load risk config and run Decision Loop
    let cache = PgCacheService::new(app_state.pool.clone());
    let storage = PgRiskConfigStorage::new(cache);
    let risk_config = storage
        .load_or_default(authorized_user_id.user_id())
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(
                "Failed to load risk config for user {}: {}, using default",
                authorized_user_id.user_id(),
                e
            );
            RiskConfig::default()
        });

    let decision_loop = DecisionLoop::new(risk_config);
    let decision_result = decision_loop.execute(&decision_input, &account_state, None);

    if !decision_result.approved {
        let rejection_msg = decision_result
            .rejection
            .as_ref()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "Order rejected by risk management".to_string());

        tracing::warn!(
            "Order rejected for user {}: {}",
            authorized_user_id.user_id(),
            rejection_msg
        );

        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "order_rejected",
            "message": rejection_msg,
            "details": decision_result.rejection,
        })));
    }

    if !decision_result.warnings.is_empty() {
        tracing::info!(
            "Order approved with {} warnings for user {}",
            decision_result.warnings.len(),
            authorized_user_id.user_id()
        );
    }

    let position_size = decision_result
        .position_size
        .unwrap_or(order_input.quantity);

    let side = match order_input.side {
        OrderSide::BUY => CommonOrderSide::Buy,
        OrderSide::SELL => CommonOrderSide::Sell,
    };

    let mut builder = StandardOrderBuilder::new()
        .user_id(authorized_user_id.user_id())
        .symbol(&order_input.market)
        .side(side)
        .order_type(if order_input.price.is_zero() {
            OrderType::Market
        } else {
            OrderType::Limit
        })
        .quantity(position_size)
        .time_in_force(TimeInForce::GTC);

    if !order_input.price.is_zero() {
        builder = builder.price(order_input.price);
    }

    let standard_order = builder.build().map_err(|e| {
        tracing::error!("Failed to build StandardOrder: {}", e);
        actix_web::error::ErrorBadRequest(format!("Invalid order parameters: {}", e))
    })?;

    let order_result = app_state
        .execution_service
        .execute_order(&standard_order, execution_mode)
        .await;

    match order_result {
        Ok(response) => {
            let order_response = serde_json::json!({
                "order_id": response.order_id,
                "shadow_order_id": decision_result.shadow_order_id,
                "exchange_order_id": response.exchange_order_id,
                "status": response.status,
                "symbol": standard_order.symbol,
                "side": format!("{:?}", standard_order.side),
                "quantity": standard_order.quantity,
                "requested_quantity": order_input.quantity,
                "calculated_size": decision_result.position_size,
                "sizing_method": decision_result.sizing_method,
                "price": standard_order.price,
                "filled_quantity": response.filled_quantity,
                "remaining_quantity": response.remaining_quantity,
                "average_price": response.average_price,
                "execution_mode": format!("{:?}", execution_mode),
                "warnings": decision_result.warnings,
                "created_at": chrono::Utc::now(),
            });

            log_route_action!(
                "Executed order",
                authorized_user_id.user_id(),
                "order",
                standard_order.symbol
            );
            Ok(HttpResponse::Ok().json(order_response))
        }
        Err(e) => {
            tracing::error!(
                "Order execution failed for user {}: {}",
                authorized_user_id.user_id(),
                e
            );
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "execution_failed",
                "message": format!("Order execution failed: {}", e),
            })))
        }
    }
}

/// GET /api/v1/order
/// Get a specific open order
pub async fn get_open_order(
    req: HttpRequest,
    user: AuthenticatedUser,
    body: web::Json<GetOpenOrderInput>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let order_input = body.into_inner();

    let auth_ctx = AuthContext::new(user);
    let authorized_user_id = auth_ctx.authorize_user_id(&order_input.user_id)?;
    let order_id = auth_ctx.parse_resource_id(&order_input.order_id)?;

    let execution_mode = detect_execution_mode(&req);

    let status_result = app_state
        .execution_service
        .get_order_status(&order_id.to_string(), execution_mode)
        .await;

    match status_result {
        Ok(response) => {
            let order_response = serde_json::json!({
                "order_id": response.order_id,
                "user_id": authorized_user_id.user_id(),
                "symbol": order_input.market,
                "exchange_order_id": response.exchange_order_id,
                "status": response.status,
                "filled_quantity": response.filled_quantity,
                "remaining_quantity": response.remaining_quantity,
                "average_price": response.average_price,
            });

            log_route_action!(
                "Retrieved order",
                authorized_user_id.user_id(),
                "order",
                order_id.to_string()
            );
            Ok(HttpResponse::Ok().json(order_response))
        }
        Err(e) => {
            tracing::warn!("Order {} not found: {}", order_id, e);
            Ok(HttpResponse::NotFound().json(serde_json::json!({
                "error": "order_not_found",
                "message": format!("Order {} not found", order_id),
            })))
        }
    }
}

/// DELETE /api/v1/order
/// Cancel a specific order
pub async fn cancel_order(
    req: HttpRequest,
    user: AuthenticatedUser,
    body: web::Json<CancelOrderInput>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let order_input = body.into_inner();

    let user_id = Uuid::from_str(&order_input.user_id)
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid user ID format"))?;

    if user_id != user.user_id {
        return Ok(HttpResponse::Forbidden().json(ErrorResponse::new(
            "forbidden",
            "Cannot cancel orders for other users",
        )));
    }

    let execution_mode = detect_execution_mode(&req);

    let cancel_result = app_state
        .execution_service
        .cancel_order(&order_input.order_id, execution_mode)
        .await;

    match cancel_result {
        Ok(()) => {
            let response = serde_json::json!({
                "order_id": order_input.order_id,
                "status": "CANCELLED",
                "cancelled_at": chrono::Utc::now(),
                "message": "Order cancelled successfully"
            });

            tracing::info!(
                "Cancelled order {} for user {}",
                order_input.order_id,
                user.user_id
            );
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::warn!("Failed to cancel order {}: {}", order_input.order_id, e);
            Ok(HttpResponse::NotFound().json(serde_json::json!({
                "error": "cancel_failed",
                "message": format!("Failed to cancel order: {}", e),
            })))
        }
    }
}

/// GET /api/v1/orders
/// Get all open orders for the authenticated user
pub async fn get_open_orders(
    req: HttpRequest,
    user: AuthenticatedUser,
    body: web::Json<GetOpenOrdersInput>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let order_input = body.into_inner();

    let user_id = Uuid::from_str(&order_input.user_id)
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid user ID format"))?;

    if user_id != user.user_id {
        return Ok(HttpResponse::Forbidden().json(ErrorResponse::new(
            "forbidden",
            "Cannot access orders for other users",
        )));
    }

    let execution_mode = detect_execution_mode(&req);

    match execution_mode {
        ExecutionMode::Shadow => {
            let open_orders = app_state.engine_handle.get_open_orders(user_id).await;

            let orders: Vec<serde_json::Value> = open_orders
                .into_iter()
                .map(|o| {
                    serde_json::json!({
                        "order_id": o.id,
                        "symbol": o.symbol,
                        "side": format!("{:?}", o.side),
                        "order_type": format!("{:?}", o.order_type),
                        "quantity": o.quantity.to_string(),
                        "price": o.price.map(|p| p.to_string()),
                        "status": format!("{:?}", o.status),
                        "filled_quantity": o.filled_quantity.to_string(),
                        "remaining_quantity": (o.quantity - o.filled_quantity).to_string(),
                        "created_at": o.created_at,
                        "exchange": "shadow"
                    })
                })
                .collect();

            tracing::info!(
                "Retrieved {} open orders for user {}",
                orders.len(),
                user.user_id
            );
            Ok(HttpResponse::Ok().json(orders))
        }
        ExecutionMode::Live => {
            // Live mode: would call Binance GET /fapi/v1/openOrders
            // For now, return empty list (Binance adapter handles this via ExecutionService)
            Ok(HttpResponse::Ok().json(serde_json::json!([])))
        }
    }
}

/// DELETE /api/v1/orders
/// Cancel all open orders for the authenticated user
pub async fn cancel_all_orders(
    req: HttpRequest,
    user: AuthenticatedUser,
    body: web::Json<CancelAllOrdersInput>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let order_input = body.into_inner();

    let user_id = Uuid::from_str(&order_input.user_id)
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid user ID format"))?;

    if user_id != user.user_id {
        return Ok(HttpResponse::Forbidden().json(ErrorResponse::new(
            "forbidden",
            "Cannot cancel orders for other users",
        )));
    }

    let execution_mode = detect_execution_mode(&req);

    match execution_mode {
        ExecutionMode::Shadow => {
            let open_orders = app_state.engine_handle.get_open_orders(user_id).await;
            let mut cancelled = Vec::new();
            for order in &open_orders {
                match app_state.engine_handle.cancel_order(user_id, order.id).await {
                    Ok(o) => cancelled.push(serde_json::json!({
                        "order_id": o.id,
                        "status": "CANCELLED",
                    })),
                    Err(e) => {
                        tracing::warn!("Failed to cancel order {}: {}", order.id, e);
                    }
                }
            }

            let response = serde_json::json!({
                "cancelled_count": cancelled.len(),
                "cancelled_orders": cancelled,
                "cancelled_at": chrono::Utc::now(),
                "message": format!("{} orders cancelled", cancelled.len())
            });

            tracing::info!(
                "Cancelled {} orders for user {} on market {}",
                cancelled.len(),
                user.user_id,
                order_input.market
            );
            Ok(HttpResponse::Ok().json(response))
        }
        ExecutionMode::Live => {
            // Live mode: would call Binance DELETE /fapi/v1/allOpenOrders
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "cancelled_count": 0,
                "cancelled_orders": [],
                "cancelled_at": chrono::Utc::now(),
                "message": "Live bulk cancel not yet implemented"
            })))
        }
    }
}
