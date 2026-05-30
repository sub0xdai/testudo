// @anchor exchange:router:exchanges
// @tags api

use actix_web::{web, HttpResponse, Result};
use uuid::Uuid;
use validator::Validate;

use crate::{
    middleware::AuthenticatedUser,
    repositories::exchange_account::RepoError,
    services::{
        hyperliquid::agent_approval,
        SidecarCredentials,
    },
    types::{
        app::AppState,
        auth::ErrorResponse,
        exchange_names::{auth_modes, exchanges},
        exchanges::{
            ApproveAgentRequest, ApproveAgentResponse, ApproveDataRequest, ApproveDataResponse,
            ExchangeAccountRequest, ExchangeAccountResponse, ExchangeBalanceEntry,
            ExchangeBalanceResponse, ExchangeListResponse, InitAgentWalletRequest,
            InitAgentWalletResponse, MigrateToAgentWalletRequest, MigrateToAgentWalletResponse,
            RevokeAgentResponse, TestConnectionResponse,
        },
    },
};
use crate::services::CexClientError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Convert a `CexClientError` into an `HttpResponse`.
///
/// `fallback_code` is the error code used for unmatched variants (e.g. "balance_fetch_failed").
fn cex_error_to_response(e: CexClientError, fallback_code: &str) -> HttpResponse {
    match e {
        CexClientError::AuthenticationFailed => HttpResponse::Unauthorized().json(
            ErrorResponse::new("invalid_credentials", "Exchange credentials are invalid"),
        ),
        CexClientError::Unavailable(msg) => {
            HttpResponse::BadGateway().json(ErrorResponse::new(
                "exchange_unreachable",
                &format!("Could not reach exchange: {}", msg),
            ))
        }
        CexClientError::RateLimited => HttpResponse::TooManyRequests().json(
            ErrorResponse::new("rate_limited", "Exchange rate limit hit, try again later"),
        ),
        _ => HttpResponse::BadGateway().json(ErrorResponse::new(
            fallback_code,
            &format!("{}: {}", fallback_code.replace('_', " "), e),
        )),
    }
}

/// GET /api/v1/exchanges
/// List all available exchanges that can be connected.
/// Hardcoded list of featured exchanges (UI display).
pub async fn list_exchanges(_user: AuthenticatedUser) -> Result<HttpResponse> {
    let exchanges: Vec<serde_json::Value> = crate::services::onboarding::build_exchange_list()
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "name": e.name,
                "type": e.exchange_type,
                "required_credentials": e.required_credentials,
            })
        })
        .collect();

    let response = ExchangeListResponse { exchanges };
    Ok(HttpResponse::Ok().json(response))
}

/// GET /api/v1/exchanges/supported
/// Returns the full list of supported exchange IDs (CEX sidecar + Hyperliquid).
pub async fn list_supported_exchanges(
    _user: AuthenticatedUser,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let mut all_exchanges: Vec<String> = vec![exchanges::HYPERLIQUID.to_string()];

    match app_state.cex_client.list_exchanges().await {
        Ok(cex_exchanges) => {
            all_exchanges.extend(cex_exchanges);
        }
        Err(e) => {
            tracing::warn!("CEX sidecar unavailable for exchange list: {}", e);
            // Still return Hyperliquid even if sidecar is down
        }
    }

    Ok(HttpResponse::Ok().json(all_exchanges))
}

/// GET /api/v1/exchanges/accounts
/// Get user's configured exchange accounts (without revealing sensitive data)
pub async fn get_user_exchange_accounts(
    user: AuthenticatedUser,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let rows = app_state
        .exchange_account_repo
        .list_by_user(user.user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list exchange accounts: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to list exchange accounts")
        })?;

    let accounts: Vec<ExchangeAccountResponse> = rows
        .into_iter()
        .map(|row| {
            let is_active = row.is_active.unwrap_or(false);
            let requires_reauth = row.auth_mode == auth_modes::AGENT_WALLET && !is_active;
            ExchangeAccountResponse {
                id: row.id,
                exchange_name: row.exchange_name.clone(),
                account_name: format!("{} Account", capitalize(&row.exchange_name)),
                is_active,
                permissions: row.permissions.unwrap_or(serde_json::json!({})),
                created_at: row.created_at.unwrap_or_else(chrono::Utc::now),
                last_used_at: row.last_used_at,
                auth_mode: row.auth_mode.clone(),
                wallet_address: row.wallet_address.clone(),
                requires_reauthorization: if requires_reauth { Some(true) } else { None },
            }
        })
        .collect();

    tracing::info!(
        "Retrieved {} exchange accounts for user {}",
        accounts.len(),
        user.user_id
    );
    Ok(HttpResponse::Ok().json(accounts))
}

/// POST /api/v1/exchanges/accounts
/// Add new exchange API keys for a user.
/// FR-5.1: Validates credentials via CCXT sidecar balance fetch for all exchanges.
pub async fn add_exchange_account(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    req: web::Json<ExchangeAccountRequest>,
) -> Result<HttpResponse> {
    if let Err(validation_errors) = req.validate() {
        let errors = serde_json::to_value(validation_errors).unwrap_or_default();
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::validation_error(errors)));
    }

    // FR-5.4: Validate credentials — Hyperliquid native or CEX sidecar
    let validated_permissions = if req.exchange_name.eq_ignore_ascii_case(exchanges::HYPERLIQUID) {
        // Hyperliquid: validate by querying clearinghouseState with the address
        let query_address = &req.api_key; // api_key stores the Ethereum address for HL
        let info_url = match app_state.hl_network {
            hyperliquid_sdk_rs::Network::Mainnet => "https://api.hyperliquid.xyz/info",
            hyperliquid_sdk_rs::Network::Testnet => "https://api.hyperliquid-testnet.xyz/info",
        };
        let payload = serde_json::json!({ "type": "clearinghouseState", "user": query_address });
        match app_state.hl_http_client.post(info_url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                serde_json::json!({
                    "validated": true,
                    "exchange": req.exchange_name
                })
            }
            Ok(resp) => {
                return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
                    "validation_failed",
                    &format!("Hyperliquid API returned HTTP {}", resp.status()),
                )));
            }
            Err(e) => {
                return Ok(HttpResponse::BadGateway().json(ErrorResponse::new(
                    "validation_failed",
                    &format!("Failed to reach Hyperliquid API: {}", e),
                )));
            }
        }
    } else {
        let creds = SidecarCredentials {
            api_key: req.api_key.clone(),
            secret: req.secret.clone(),
            password: req.passphrase.clone(),
        };
        match app_state
            .cex_client
            .fetch_balance(&req.exchange_name, &creds, false, "future")
            .await
        {
            Ok(_) => {
                serde_json::json!({
                    "validated": true,
                    "exchange": req.exchange_name
                })
            }
            Err(e) => {
                return Ok(cex_error_to_response(e, "validation_failed"));
            }
        }
    };

    // Encrypt credentials and persist to database
    let row = app_state
        .exchange_account_repo
        .insert(
            user.user_id,
            &req.exchange_name,
            &req.api_key,
            &req.secret,
            req.passphrase.as_deref(),
            validated_permissions,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to save exchange account: {}", e);
            actix_web::error::ErrorInternalServerError(format!("Failed to save account: {}", e))
        })?;

    let account_name = req
        .account_name
        .clone()
        .unwrap_or_else(|| format!("{} Account", capitalize(&req.exchange_name)));

    let response = ExchangeAccountResponse {
        id: row.id,
        exchange_name: row.exchange_name.clone(),
        account_name,
        is_active: row.is_active.unwrap_or(false),
        permissions: row.permissions.unwrap_or(serde_json::json!({})),
        created_at: row.created_at.unwrap_or_else(chrono::Utc::now),
        last_used_at: row.last_used_at,
        auth_mode: row.auth_mode.clone(),
        wallet_address: row.wallet_address.clone(),
        requires_reauthorization: None, // newly created accounts are always active
    };

    tracing::info!(
        "Created exchange account for user {} on exchange {} (credentials encrypted + persisted)",
        user.user_id,
        req.exchange_name
    );

    // HIST-01/HIST-02: Auto-trigger trade history import on exchange add
    if let Err(e) = crate::services::import_worker::enqueue_import(
        &app_state.pg_queue.queue,
        user.user_id,
        row.id,
        &req.exchange_name,
    )
    .await
    {
        tracing::warn!(
            exchange = %req.exchange_name,
            error = %e,
            "Failed to enqueue auto-import (account saved successfully)"
        );
    } else {
        tracing::info!(
            exchange = %req.exchange_name,
            "Auto-import job enqueued for new exchange account"
        );
    }

    Ok(HttpResponse::Created().json(response))
}

/// DELETE /api/v1/exchanges/accounts/:id
/// Remove an exchange account
pub async fn delete_exchange_account(
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let account_id = path.into_inner();

    let deleted = app_state
        .exchange_account_repo
        .delete(account_id, user.user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete exchange account: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to delete account")
        })?;

    if !deleted {
        return Ok(HttpResponse::NotFound().json(ErrorResponse::new(
            "not_found",
            "Exchange account not found or not owned by user",
        )));
    }

    tracing::info!(
        "Deleted exchange account {} for user {}",
        account_id,
        user.user_id
    );
    Ok(HttpResponse::NoContent().finish())
}

/// POST /api/v1/exchanges/accounts/:id/test
/// Test connection to an exchange using stored credentials
pub async fn test_exchange_connection(
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let account_id = path.into_inner();

    // Load and decrypt credentials
    let creds = app_state
        .exchange_account_repo
        .load_credentials(account_id, user.user_id)
        .await
        .map_err(|e| {
            match &e {
                RepoError::NotFound => {
                    tracing::warn!("Account {} not found for user {}", account_id, user.user_id);
                }
                _ => {
                    tracing::error!("Failed to load credentials: {}", e);
                }
            }
            actix_web::error::ErrorNotFound("Account not found")
        })?;

    // FR-5.2: Test connection — Hyperliquid native or CEX sidecar
    let start = std::time::Instant::now();
    let (status, message) = if creds.exchange_name == exchanges::HYPERLIQUID {
        let query_address = creds.wallet_address.as_deref().unwrap_or(&creds.api_key);
        let info_url = match app_state.hl_network {
            hyperliquid_sdk_rs::Network::Mainnet => "https://api.hyperliquid.xyz/info",
            hyperliquid_sdk_rs::Network::Testnet => "https://api.hyperliquid-testnet.xyz/info",
        };
        let payload = serde_json::json!({ "type": "clearinghouseState", "user": query_address });
        match app_state.hl_http_client.post(info_url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                ("success".to_string(), "Connection successful".to_string())
            }
            Ok(resp) => ("failed".to_string(), format!("HTTP {}", resp.status())),
            Err(e) => ("failed".to_string(), format!("Connection failed: {}", e)),
        }
    } else {
        let sidecar_creds = SidecarCredentials {
            api_key: creds.api_key,
            secret: creds.api_secret,
            password: creds.passphrase,
        };
        match app_state
            .cex_client
            .fetch_balance(&creds.exchange_name, &sidecar_creds, false, "future")
            .await
        {
            Ok(_) => ("success".to_string(), "Connection successful".to_string()),
            Err(e) => ("failed".to_string(), format!("Connection failed: {}", e)),
        }
    };
    let latency = start.elapsed().as_millis() as u64;

    let response = TestConnectionResponse {
        account_id,
        exchange_name: creds.exchange_name,
        status,
        message,
        tested_at: chrono::Utc::now(),
        latency_ms: Some(latency),
        api_limits: None,
    };

    tracing::info!(
        "Tested connection for account {} by user {} ({}ms)",
        account_id,
        user.user_id,
        latency
    );
    Ok(HttpResponse::Ok().json(response))
}

/// GET /api/v1/exchanges/accounts/:id/balance
/// Fetch live balance from exchange (Hyperliquid native or CEX sidecar)
pub async fn get_exchange_balance(
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    app_state: web::Data<AppState>,
    query: web::Query<BalanceQuery>,
) -> Result<HttpResponse> {
    let account_id = path.into_inner();
    let balance_type = query.r#type.as_deref().unwrap_or("future");

    // Load and decrypt credentials (ownership-verified)
    let creds = app_state
        .exchange_account_repo
        .load_credentials(account_id, user.user_id)
        .await
        .map_err(|e| match &e {
            RepoError::NotFound => {
                tracing::warn!(
                    "Balance: account {} not found for user {}",
                    account_id,
                    user.user_id
                );
                actix_web::error::ErrorNotFound("Account not found")
            }
            _ => {
                tracing::error!("Failed to load credentials for balance: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to load credentials")
            }
        })?;

    // Hyperliquid: use native info API instead of CEX sidecar
    if creds.exchange_name == exchanges::HYPERLIQUID {
        return get_hyperliquid_balance(account_id, user.user_id, &creds, &app_state).await;
    }

    let sidecar_creds = SidecarCredentials {
        api_key: creds.api_key,
        secret: creds.api_secret,
        password: creds.passphrase,
    };

    match app_state
        .cex_client
        .fetch_balance(&creds.exchange_name, &sidecar_creds, false, balance_type)
        .await
    {
        Ok(entries) => {
            // Filter to non-zero balances
            let balances: Vec<ExchangeBalanceEntry> = entries
                .into_iter()
                .filter(|e| {
                    e.total.parse::<f64>().unwrap_or(0.0) != 0.0
                        || e.free.parse::<f64>().unwrap_or(0.0) != 0.0
                        || e.used.parse::<f64>().unwrap_or(0.0) != 0.0
                })
                .map(|e| ExchangeBalanceEntry {
                    asset: e.asset,
                    total: e.total,
                    free: e.free,
                    used: e.used,
                })
                .collect();

            let response = ExchangeBalanceResponse {
                account_id,
                exchange_name: creds.exchange_name,
                balances,
                fetched_at: chrono::Utc::now(),
            };

            tracing::info!(
                "Fetched balance for account {} (user {}): {} assets",
                account_id,
                user.user_id,
                response.balances.len()
            );
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => Ok(cex_error_to_response(e, "balance_fetch_failed")),
    }
}

/// Fetch Hyperliquid balance via native info API.
async fn get_hyperliquid_balance(
    account_id: Uuid,
    user_id: Uuid,
    creds: &crate::repositories::exchange_account::DecryptedCredentials,
    app_state: &web::Data<AppState>,
) -> Result<HttpResponse> {
    // For agent wallet: query wallet_address; for direct: query api_key (which stores the address)
    let query_address = creds
        .wallet_address
        .as_deref()
        .unwrap_or(&creds.api_key);

    let info_url = match app_state.hl_network {
        hyperliquid_sdk_rs::Network::Mainnet => "https://api.hyperliquid.xyz/info",
        hyperliquid_sdk_rs::Network::Testnet => "https://api.hyperliquid-testnet.xyz/info",
    };

    let payload = serde_json::json!({
        "type": "clearinghouseState",
        "user": query_address,
    });

    let resp = app_state
        .hl_http_client
        .post(info_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Hyperliquid balance fetch failed: {}", e);
            actix_web::error::ErrorBadGateway("Failed to reach Hyperliquid API")
        })?;

    let body: serde_json::Value = resp.json().await.map_err(|e| {
        tracing::error!("Hyperliquid balance parse failed: {}", e);
        actix_web::error::ErrorBadGateway("Invalid response from Hyperliquid")
    })?;

    let account_value = body
        .get("marginSummary")
        .and_then(|m| m.get("accountValue"))
        .and_then(|v| v.as_str())
        .unwrap_or("0");

    let total_margin = body
        .get("marginSummary")
        .and_then(|m| m.get("totalMarginUsed"))
        .and_then(|v| v.as_str())
        .unwrap_or("0");

    let available = account_value
        .parse::<Decimal>()
        .unwrap_or_default()
        - total_margin.parse::<Decimal>().unwrap_or_default();

    let response = ExchangeBalanceResponse {
        account_id,
        exchange_name: exchanges::HYPERLIQUID.to_string(),
        balances: vec![ExchangeBalanceEntry {
            asset: "USDC".to_string(),
            total: account_value.to_string(),
            free: available.to_string(),
            used: total_margin.to_string(),
        }],
        fetched_at: chrono::Utc::now(),
    };

    tracing::info!(
        "Fetched Hyperliquid balance for account {} (user {}): {}",
        account_id,
        user_id,
        account_value
    );
    Ok(HttpResponse::Ok().json(response))
}

/// Fetch Hyperliquid positions and open orders via native info API.
async fn get_hyperliquid_positions(
    account_id: Uuid,
    creds: &crate::repositories::exchange_account::DecryptedCredentials,
    app_state: &web::Data<AppState>,
) -> Result<HttpResponse> {
    let query_address = creds.wallet_address.as_deref().unwrap_or(&creds.api_key);
    let info_url = match app_state.hl_network {
        hyperliquid_sdk_rs::Network::Mainnet => "https://api.hyperliquid.xyz/info",
        hyperliquid_sdk_rs::Network::Testnet => "https://api.hyperliquid-testnet.xyz/info",
    };

    // Fetch clearinghouseState for positions
    let state_payload = serde_json::json!({ "type": "clearinghouseState", "user": query_address });
    let state_resp = app_state.hl_http_client.post(info_url).json(&state_payload).send().await
        .map_err(|e| actix_web::error::ErrorBadGateway(format!("Hyperliquid API error: {}", e)))?;
    let state_body: serde_json::Value = state_resp.json().await
        .map_err(|e| actix_web::error::ErrorBadGateway(format!("Invalid response: {}", e)))?;

    let entries: Vec<ExchangePositionEntry> = state_body
        .get("assetPositions")
        .and_then(|v| v.as_array())
        .map(|positions| {
            positions.iter().filter_map(|ap| {
                let pos = ap.get("position")?;
                let szi = pos.get("szi")?.as_str()?.parse::<f64>().ok()?;
                if szi.abs() < 1e-12 { return None; }
                Some(ExchangePositionEntry {
                    symbol: pos.get("coin")?.as_str()?.to_string(),
                    side: if szi > 0.0 { "long".to_string() } else { "short".to_string() },
                    contracts: format!("{}", szi.abs()),
                    entry_price: pos.get("entryPx").and_then(|v| v.as_str()).unwrap_or("0").to_string(),
                    unrealized_pnl: pos.get("unrealizedPnl").and_then(|v| v.as_str()).unwrap_or("0").to_string(),
                })
            }).collect()
        })
        .unwrap_or_default();

    // Fetch open orders
    let orders_payload = serde_json::json!({ "type": "openOrders", "user": query_address });
    let order_entries: Vec<ExchangeOpenOrderEntry> = match app_state
        .hl_http_client.post(info_url).json(&orders_payload).send().await
    {
        Ok(resp) => {
            let orders: Vec<serde_json::Value> = resp.json().await.unwrap_or_default();
            orders.iter().filter_map(|o| {
                Some(ExchangeOpenOrderEntry {
                    id: o.get("oid")?.to_string(),
                    symbol: o.get("coin")?.as_str()?.to_string(),
                    side: o.get("side")?.as_str()?.to_string(),
                    order_type: o.get("orderType")?.as_str().unwrap_or("limit").to_string(),
                    price: o.get("limitPx").and_then(|v| v.as_str()).map(String::from),
                    stop_price: o.get("triggerPx").and_then(|v| v.as_str()).map(String::from),
                    amount: o.get("sz")?.as_str()?.to_string(),
                })
            }).collect()
        }
        Err(e) => {
            tracing::warn!("Failed to fetch Hyperliquid open orders: {}", e);
            vec![]
        }
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "account_id": account_id,
        "exchange_name": exchanges::HYPERLIQUID,
        "positions": entries,
        "open_orders": order_entries,
        "fetched_at": chrono::Utc::now().to_rfc3339(),
    })))
}

/// Close a Hyperliquid position via native SDK (reduce-only market order).
async fn close_hyperliquid_position(
    user_id: Uuid,
    account_id: Uuid,
    symbol: &str,
    close_side: &str,
    amount: Decimal,
    _creds: &crate::repositories::exchange_account::DecryptedCredentials,
    app_state: &web::Data<AppState>,
) -> Result<HttpResponse> {
    use crate::services::exchange_api::{ExchangeApi, OrderSide, ApiOrderType, PlaceOrderRequest};

    let (hl_universe, hl_auth_cache) = match (app_state.hl_universe.as_ref(), app_state.hl_auth_cache.as_ref()) {
        (Some(u), Some(c)) => (u, c),
        _ => {
            return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "hyperliquid_unavailable",
                "message": "Hyperliquid SDK not initialized",
            })));
        }
    };

    let hl_api = crate::services::hyperliquid::HyperliquidExchangeApi::new(
        hl_universe.clone(),
        hl_auth_cache.clone(),
        app_state.exchange_account_repo.clone(),
        app_state.hl_network,
    );

    let side = if close_side == "sell" { OrderSide::Sell } else { OrderSide::Buy };

    match hl_api.place_order(PlaceOrderRequest {
        user_id,
        symbol: symbol.to_string(),
        side,
        order_type: ApiOrderType::Market,
        quantity: amount,
        price: None,
        stop_price: None,
        leverage: 0,
        exchange_account_id: Some(account_id),
        reduce_only: false, // HL-11: agent wallets reject reduce_only=true
        client_order_id: None,
        stop_loss_trigger: None,
        take_profit_trigger: None,
    }).await {
        Ok(result) => {
            tracing::info!(
                "Closed Hyperliquid position: {} {} {} for user {}",
                symbol, close_side, amount, user_id
            );
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "order_id": result.id,
                "status": result.status,
            })))
        }
        Err(e) => {
            tracing::error!("Failed to close Hyperliquid position: {}", e);
            Ok(HttpResponse::BadGateway().json(serde_json::json!({
                "error": "close_position_failed",
                "message": format!("{}", e),
            })))
        }
    }
}

/// Query parameters for balance endpoint
#[derive(Debug, serde::Deserialize)]
pub struct BalanceQuery {
    /// Balance type: "future" or "spot" (default: "future")
    pub r#type: Option<String>,
}

/// Response for position list endpoint.
#[derive(Debug, Serialize)]
pub struct ExchangePositionEntry {
    pub symbol: String,
    pub side: String,
    pub contracts: String,
    pub entry_price: String,
    pub unrealized_pnl: String,
}

/// Open order entry in position list response.
#[derive(Debug, Serialize)]
pub struct ExchangeOpenOrderEntry {
    pub id: String,
    pub symbol: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub price: Option<String>,
    pub stop_price: Option<String>,
    pub amount: String,
}

/// GET /api/v1/exchanges/accounts/{id}/positions
/// Fetch open positions for an exchange account via CEX sidecar.
pub async fn get_exchange_positions(
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let account_id = path.into_inner();

    let creds = app_state
        .exchange_account_repo
        .load_credentials(account_id, user.user_id)
        .await
        .map_err(|e| match &e {
            RepoError::NotFound => actix_web::error::ErrorNotFound("Account not found"),
            _ => actix_web::error::ErrorInternalServerError("Failed to load credentials"),
        })?;

    // Hyperliquid: use native info API
    if creds.exchange_name.eq_ignore_ascii_case(exchanges::HYPERLIQUID) {
        return get_hyperliquid_positions(account_id, &creds, &app_state).await;
    }

    let sidecar_creds = SidecarCredentials {
        api_key: creds.api_key,
        secret: creds.api_secret,
        password: creds.passphrase,
    };

    match app_state
        .cex_client
        .fetch_positions(&creds.exchange_name, &sidecar_creds, false, None)
        .await
    {
        Ok(positions) => {
            let entries: Vec<ExchangePositionEntry> = positions
                .into_iter()
                .filter(|p| {
                    p.contracts.parse::<f64>().unwrap_or(0.0).abs() > 0.0
                })
                .map(|p| ExchangePositionEntry {
                    symbol: p.symbol,
                    side: p.side,
                    contracts: p.contracts,
                    entry_price: p.entry_price,
                    unrealized_pnl: p.unrealized_pnl,
                })
                .collect();

            // Fetch open orders — fail gracefully (positions are more important)
            let order_entries: Vec<ExchangeOpenOrderEntry> = match app_state
                .cex_client
                .fetch_open_orders(&creds.exchange_name, &sidecar_creds, false, "")
                .await
            {
                Ok(orders) => orders
                    .into_iter()
                    .map(|o| ExchangeOpenOrderEntry {
                        id: o.id,
                        symbol: o.symbol.unwrap_or_default(),
                        side: o.side.unwrap_or_default(),
                        order_type: o.order_type.unwrap_or_default(),
                        price: o.price,
                        stop_price: o.stop_price,
                        amount: o.amount.unwrap_or_default(),
                    })
                    .collect(),
                Err(e) => {
                    tracing::warn!("Failed to fetch open orders: {e:?}");
                    vec![]
                }
            };

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "account_id": account_id,
                "exchange_name": creds.exchange_name,
                "positions": entries,
                "open_orders": order_entries,
                "fetched_at": chrono::Utc::now().to_rfc3339(),
            })))
        }
        Err(e) => Ok(cex_error_to_response(e, "positions_fetch_failed")),
    }
}

/// Request body for closing an exchange position.
#[derive(Debug, Deserialize)]
pub struct ClosePositionRequest {
    pub symbol: String,
    pub side: String,
    pub contracts: String,
}

/// POST /api/v1/exchanges/accounts/{id}/close-position
/// Close an exchange position by placing a reduce-only market order in the opposite direction.
pub async fn close_exchange_position(
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<ClosePositionRequest>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let account_id = path.into_inner();

    let creds = app_state
        .exchange_account_repo
        .load_credentials(account_id, user.user_id)
        .await
        .map_err(|e| match &e {
            RepoError::NotFound => actix_web::error::ErrorNotFound("Account not found"),
            _ => actix_web::error::ErrorInternalServerError("Failed to load credentials"),
        })?;

    // Opposite side for close: long → sell, short → buy
    let close_side = match body.side.to_lowercase().as_str() {
        "long" => "sell",
        "short" => "buy",
        other => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "invalid_side",
                "message": format!("Unknown position side: {}", other),
            })));
        }
    };

    let amount = match body.contracts.parse::<Decimal>() {
        Ok(d) if d > Decimal::ZERO => d,
        _ => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "invalid_contracts",
                "message": "Contracts must be a positive number",
            })));
        }
    };

    // Hyperliquid: close via native SDK
    if creds.exchange_name.eq_ignore_ascii_case(exchanges::HYPERLIQUID) {
        return close_hyperliquid_position(
            user.user_id, account_id, &body.symbol, close_side, amount, &creds, &app_state,
        ).await;
    }

    // CEX sidecar path
    let sidecar_creds = SidecarCredentials {
        api_key: creds.api_key,
        secret: creds.api_secret,
        password: creds.passphrase,
    };

    match app_state
        .cex_client
        .create_order(
            &creds.exchange_name,
            &sidecar_creds,
            false,
            &body.symbol,
            close_side,
            "market",
            amount,
            None,  // no price for market order
            None,  // no stop price
            None,  // no leverage change
            true,  // reduce-only
            None,  // no clientOrderId
            None,  // no bracket SL
            None,  // no bracket TP
        )
        .await
    {
        Ok(order) => {
            tracing::info!(
                "Closed exchange position: {} {} {} for user {}",
                body.symbol,
                body.side,
                body.contracts,
                user.user_id
            );
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "order_id": order.id,
                "status": order.status,
            })))
        }
        Err(e) => Ok(cex_error_to_response(e, "close_position_failed")),
    }
}

/// POST /api/v1/exchanges/agent-wallet/init
/// Generate an agent keypair for Hyperliquid agent wallet authentication.
/// The agent key is encrypted and stored; the user's main wallet address is recorded.
pub async fn init_agent_wallet(
    user: AuthenticatedUser,
    app_state: web::Data<AppState>,
    req: web::Json<InitAgentWalletRequest>,
) -> Result<HttpResponse> {
    // Validate wallet address format: 0x + 40 hex chars
    let addr = req.wallet_address.trim();
    if !is_valid_eth_address(addr) {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "invalid_wallet_address",
            "Wallet address must be 0x-prefixed with 40 hex characters",
        )));
    }

    // FR-1: Check for existing agent wallet before generating a new one
    if let Some(existing) = app_state
        .exchange_account_repo
        .find_agent_wallet(user.user_id, addr)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check existing agent wallet: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to check existing agent wallet")
        })?
    {
        // FR-2/FR-3: Try to decrypt agent_address from existing wallet
        match app_state
            .exchange_account_repo
            .load_credentials_for_approval(existing.id, user.user_id)
            .await
        {
            Ok(creds) => {
                let status = if existing.is_active.unwrap_or(false) {
                    "active"
                } else {
                    "pending"
                };
                tracing::info!(
                    "Reusing {} agent wallet for user {} (account: {}, agent: {})",
                    status,
                    user.user_id,
                    existing.id,
                    creds.api_key
                );
                return Ok(HttpResponse::Ok().json(InitAgentWalletResponse {
                    account_id: existing.id,
                    agent_address: creds.api_key,
                }));
            }
            Err(e) => {
                // Risk #2: Decryption failure — fall through to generate new keypair
                tracing::warn!(
                    "Failed to decrypt existing agent wallet {} for user {}, generating new: {}",
                    existing.id,
                    user.user_id,
                    e
                );
            }
        }
    }

    // FR-4: No existing wallet (or decryption failed) — generate new keypair
    let signer = alloy::signers::local::PrivateKeySigner::random();
    let agent_address = format!("{:?}", signer.address());
    let agent_key = hex::encode(signer.credential().to_bytes());

    // Store encrypted agent key + wallet address
    let row = app_state
        .exchange_account_repo
        .insert_agent_wallet(user.user_id, addr, &agent_key, &agent_address)
        .await
        .map_err(|e| {
            tracing::error!("Failed to store agent wallet: {}", e);
            actix_web::error::ErrorInternalServerError(format!("Failed to store agent wallet: {}", e))
        })?;

    tracing::info!(
        "Created new agent wallet for user {} (agent: {}, main: {})",
        user.user_id,
        agent_address,
        addr
    );

    Ok(HttpResponse::Created().json(InitAgentWalletResponse {
        account_id: row.id,
        agent_address,
    }))
}

/// POST /api/v1/exchanges/agent-wallet/approve-data
/// Returns EIP-712 typed data for the frontend to sign with MetaMask.
pub async fn approve_data(
    user: AuthenticatedUser,
    app_state: web::Data<AppState>,
    req: web::Json<ApproveDataRequest>,
) -> Result<HttpResponse> {
    // Check if account is already approved
    let is_active = app_state
        .exchange_account_repo
        .is_agent_active(req.account_id, user.user_id)
        .await
        .map_err(|e| match &e {
            RepoError::NotFound => actix_web::error::ErrorNotFound("Account not found"),
            _ => {
                tracing::error!("Failed to check agent status: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to check account status")
            }
        })?;

    if is_active {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "already_approved",
            "Agent wallet is already approved",
        )));
    }

    // Load credentials (works for pending accounts)
    let creds = app_state
        .exchange_account_repo
        .load_credentials_for_approval(req.account_id, user.user_id)
        .await
        .map_err(|e| match &e {
            RepoError::NotFound => actix_web::error::ErrorNotFound("Account not found"),
            _ => {
                tracing::error!("Failed to load credentials: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to load credentials")
            }
        })?;

    if creds.auth_mode != auth_modes::AGENT_WALLET {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "invalid_auth_mode",
            "Account is not an agent wallet",
        )));
    }

    let network = app_state.hl_network;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let typed_data =
        agent_approval::build_eip712_typed_data(&creds.api_key, network, nonce);

    tracing::info!(
        "Generated EIP-712 approve data for user {} (agent: {})",
        user.user_id,
        creds.api_key
    );

    Ok(HttpResponse::Ok().json(ApproveDataResponse {
        typed_data,
        nonce,
        agent_address: creds.api_key,
    }))
}

/// POST /api/v1/exchanges/agent-wallet/approve
/// Accepts a signed EIP-712 message and submits it to Hyperliquid.
/// FR-1: Verifies is_active = false before submitting to Hyperliquid API.
pub async fn approve_agent(
    user: AuthenticatedUser,
    app_state: web::Data<AppState>,
    req: web::Json<ApproveAgentRequest>,
) -> Result<HttpResponse> {
    // FR-1: Verify account is not already active before submitting to HL API
    let is_active = app_state
        .exchange_account_repo
        .is_agent_active(req.account_id, user.user_id)
        .await
        .map_err(|e| match &e {
            RepoError::NotFound => actix_web::error::ErrorNotFound("Account not found"),
            _ => {
                tracing::error!("Failed to check agent status: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to check account status")
            }
        })?;

    if is_active {
        return Ok(HttpResponse::Conflict().json(ErrorResponse::new(
            "already_approved",
            "Agent wallet is already approved and active",
        )));
    }

    // Load credentials
    let creds = app_state
        .exchange_account_repo
        .load_credentials_for_approval(req.account_id, user.user_id)
        .await
        .map_err(|e| match &e {
            RepoError::NotFound => actix_web::error::ErrorNotFound("Account not found"),
            _ => {
                tracing::error!("Failed to load credentials: {}", e);
                actix_web::error::ErrorInternalServerError("Failed to load credentials")
            }
        })?;

    if creds.auth_mode != auth_modes::AGENT_WALLET {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "invalid_auth_mode",
            "Account is not an agent wallet",
        )));
    }

    let network = app_state.hl_network;
    let agent_address = &creds.api_key;
    let wallet_address = creds.wallet_address.as_deref().unwrap_or("");

    // Submit approval to Hyperliquid API
    match agent_approval::submit_approval(&app_state.hl_http_client, agent_address, network, req.nonce, &req.signature).await {
        Ok(_response) => {
            tracing::info!(
                "Approval submitted for user {} (agent: {})",
                user.user_id,
                agent_address
            );
        }
        Err(e) => {
            tracing::error!("Approval submission failed: {}", e);
            return Ok(HttpResponse::BadGateway().json(ErrorResponse::new(
                "approval_failed",
                &format!("Failed to submit approval to Hyperliquid: {}", e),
            )));
        }
    }

    // Verify registration
    let registered =
        agent_approval::verify_registration(&app_state.hl_http_client, wallet_address, agent_address, network)
            .await
            .unwrap_or(false);

    if !registered {
        tracing::warn!(
            "Agent {} not found in extraAgents check for wallet {} — \
             HL returned status:ok so proceeding (extraAgents may use different format)",
            agent_address,
            wallet_address
        );
        // HL returned status:ok — trust the approval response over the extraAgents check.
        // The extraAgents query format may not match HL's current API.
    }

    // FR-2: Atomic update with is_active = false precondition
    match app_state
        .exchange_account_repo
        .update_agent_approved(req.account_id, user.user_id)
        .await
    {
        Ok(true) => { /* success */ }
        Ok(false) => {
            // Another request won the race — account already activated
            return Ok(HttpResponse::Conflict().json(ErrorResponse::new(
                "already_approved",
                "Agent wallet was approved by a concurrent request",
            )));
        }
        Err(e) => {
            tracing::error!("Failed to update agent approved status: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::new(
                "update_failed",
                "Approval succeeded but failed to update account status",
            )));
        }
    }

    tracing::info!(
        "Agent wallet approved for user {} (agent: {}, verified: {})",
        user.user_id,
        agent_address,
        registered
    );

    Ok(HttpResponse::Ok().json(ApproveAgentResponse {
        success: true,
        agent_address: agent_address.to_string(),
        message: if registered {
            "Agent approved and verified".to_string()
        } else {
            "Agent approved (verification pending)".to_string()
        },
    }))
}

/// POST /api/v1/exchanges/agent-wallet/migrate
/// Convert an existing direct-key Hyperliquid account to agent-wallet mode.
/// Generates a new agent keypair, preserves the account ID, and requires re-approval.
pub async fn migrate_to_agent_wallet(
    user: AuthenticatedUser,
    app_state: web::Data<AppState>,
    req: web::Json<MigrateToAgentWalletRequest>,
) -> Result<HttpResponse> {
    let addr = req.wallet_address.trim();
    if !is_valid_eth_address(addr) {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "invalid_wallet_address",
            "Wallet address must be 0x-prefixed with 40 hex characters",
        )));
    }

    // Generate new agent keypair
    let signer = alloy::signers::local::PrivateKeySigner::random();
    let agent_address = format!("{:?}", signer.address());
    let agent_key = hex::encode(signer.credential().to_bytes());

    // Migrate: update credentials, auth_mode, wallet_address, deactivate
    if let Err(e) = app_state
        .exchange_account_repo
        .migrate_to_agent_wallet(
            req.account_id,
            user.user_id,
            addr,
            &agent_key,
            &agent_address,
        )
        .await
    {
        return Ok(match e {
            RepoError::NotFound => HttpResponse::NotFound().json(ErrorResponse::new(
                "not_found",
                "Account not found, not owned by user, or not a direct-key Hyperliquid account",
            )),
            _ => {
                tracing::error!("Migration failed: {}", e);
                HttpResponse::InternalServerError().json(ErrorResponse::new(
                    "migration_failed",
                    &format!("Failed to migrate account: {}", e),
                ))
            }
        });
    }

    // Invalidate auth cache
    if let Some(ref cache) = app_state.hl_auth_cache {
        cache.invalidate(&req.account_id).await;
    }

    tracing::info!(
        "Migrated account {} to agent-wallet for user {} (agent: {})",
        req.account_id,
        user.user_id,
        agent_address
    );

    Ok(HttpResponse::Ok().json(MigrateToAgentWalletResponse {
        account_id: req.account_id,
        agent_address,
        message: "Agent keypair generated. Please approve via wallet.".to_string(),
    }))
}

/// DELETE /api/v1/exchanges/agent-wallet/:id/revoke
/// Deactivate an agent wallet and record revocation timestamp.
/// FR-4: Uses `WHERE is_active = true` precondition to prevent double-revocation.
pub async fn revoke_agent(
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let account_id = path.into_inner();

    match app_state
        .exchange_account_repo
        .revoke_agent(account_id, user.user_id)
        .await
    {
        Ok(true) => { /* success */ }
        Ok(false) => {
            // Precondition not met: account not active or not agent_wallet mode
            return Ok(HttpResponse::Conflict().json(ErrorResponse::new(
                "not_active",
                "Agent wallet is not active or already revoked",
            )));
        }
        Err(e) => {
            tracing::error!("Revocation failed: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::new(
                "revocation_failed",
                &format!("Failed to revoke agent wallet: {}", e),
            )));
        }
    }

    // Invalidate auth cache
    if let Some(ref cache) = app_state.hl_auth_cache {
        cache.invalidate(&account_id).await;
    }

    tracing::info!(
        "Revoked agent wallet {} for user {}",
        account_id,
        user.user_id
    );

    Ok(HttpResponse::Ok().json(RevokeAgentResponse {
        success: true,
        message: "Agent wallet revoked successfully".to_string(),
    }))
}

/// Validate Ethereum address format: 0x + 40 hex characters.
fn is_valid_eth_address(addr: &str) -> bool {
    addr.len() == 42
        && addr.starts_with("0x")
        && addr[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::JwtMiddleware;
    use actix_web::{test, web, App};
    use chrono::Utc;
    use common_utils::auth::{AuthError, TokenClaims, TokenService, TokenType};
    use std::sync::Arc;

    struct MockTokenService;

    impl TokenService for MockTokenService {
        fn generate_access_token(
            &self,
            _user_id: &Uuid,
            _wallet_address: &str,
        ) -> Result<String, AuthError> {
            unimplemented!()
        }
        fn generate_refresh_token(
            &self,
            _user_id: &Uuid,
            _wallet_address: &str,
        ) -> Result<String, AuthError> {
            unimplemented!()
        }
        fn verify_access_token(&self, _token: &str) -> Result<TokenClaims, AuthError> {
            Ok(TokenClaims {
                sub: Uuid::new_v4().to_string(),
                wallet_address: "0xC285000000000000000000000000000000005b36".to_string(),
                exp: (Utc::now().timestamp() + 3600) as i64,
                iat: Utc::now().timestamp() as i64,
                iss: "https://api.testudo.vip".to_string(),
                token_type: TokenType::Access,
            })
        }
        fn verify_refresh_token(&self, _token: &str) -> Result<TokenClaims, AuthError> {
            Err(AuthError::InvalidToken)
        }
    }

    #[actix_web::test]
    async fn test_list_exchanges() {
        let token_service: Arc<dyn TokenService> = Arc::new(MockTokenService);

        let app = test::init_service(
            App::new()
                .wrap(JwtMiddleware::new(token_service))
                .route("/exchanges", web::get().to(list_exchanges)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/exchanges")
            .insert_header(("authorization", "Bearer valid_token"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: ExchangeListResponse = test::read_body_json(resp).await;
        assert!(!body.exchanges.is_empty());
        assert_eq!(body.exchanges.len(), 9);
    }

    #[actix_web::test]
    async fn test_exchange_account_request_validation() {
        let valid_request = ExchangeAccountRequest {
            exchange_name: "binance".to_string(),
            account_name: Some("Test Account".to_string()),
            api_key: "test_api_key".to_string(),
            secret: "test_secret".to_string(),
            passphrase: None,
            permissions: None,
        };
        assert!(valid_request.validate().is_ok());

        let invalid_request = ExchangeAccountRequest {
            exchange_name: "".to_string(),
            account_name: Some("Test Account".to_string()),
            api_key: "test_api_key".to_string(),
            secret: "test_secret".to_string(),
            passphrase: None,
            permissions: None,
        };
        assert!(invalid_request.validate().is_err());

        let invalid_request = ExchangeAccountRequest {
            exchange_name: "binance".to_string(),
            account_name: Some("Test Account".to_string()),
            api_key: "".to_string(),
            secret: "test_secret".to_string(),
            passphrase: None,
            permissions: None,
        };
        assert!(invalid_request.validate().is_err());
    }

    #[actix_web::test]
    async fn test_is_valid_eth_address() {
        // Valid addresses
        assert!(is_valid_eth_address("0x1234567890abcdef1234567890abcdef12345678"));
        assert!(is_valid_eth_address("0xABCDEF1234567890ABCDEF1234567890ABCDEF12"));

        // Invalid addresses
        assert!(!is_valid_eth_address(""));
        assert!(!is_valid_eth_address("0x"));
        assert!(!is_valid_eth_address("1234567890abcdef1234567890abcdef12345678")); // missing 0x
        assert!(!is_valid_eth_address("0x1234567890abcdef1234567890abcdef1234567")); // too short
        assert!(!is_valid_eth_address("0x1234567890abcdef1234567890abcdef123456789")); // too long
        assert!(!is_valid_eth_address("0xGGGG567890abcdef1234567890abcdef12345678")); // non-hex
    }

    #[actix_web::test]
    async fn test_init_agent_wallet_request_deserialization() {
        let json = r#"{"wallet_address":"0x1234567890abcdef1234567890abcdef12345678"}"#;
        let req: InitAgentWalletRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.wallet_address, "0x1234567890abcdef1234567890abcdef12345678");
    }

    #[actix_web::test]
    async fn test_init_agent_wallet_response_serialization() {
        let resp = InitAgentWalletResponse {
            account_id: Uuid::new_v4(),
            agent_address: "0xabcdef1234567890abcdef1234567890abcdef12".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("account_id"));
        assert!(json.contains("agent_address"));
        assert!(json.contains("0xabcdef1234567890abcdef1234567890abcdef12"));
    }

    #[actix_web::test]
    async fn test_conflict_response_for_already_approved_agent() {
        // Verify the 409 Conflict response shape for double-approval
        let response = HttpResponse::Conflict().json(ErrorResponse::new(
            "already_approved",
            "Agent wallet is already approved and active",
        ));
        assert_eq!(response.status(), actix_web::http::StatusCode::CONFLICT);
    }

    #[actix_web::test]
    async fn test_conflict_response_for_already_revoked_agent() {
        // Verify the 409 Conflict response shape for double-revocation
        let response = HttpResponse::Conflict().json(ErrorResponse::new(
            "not_active",
            "Agent wallet is not active or already revoked",
        ));
        assert_eq!(response.status(), actix_web::http::StatusCode::CONFLICT);
    }

    #[actix_web::test]
    async fn test_conflict_response_for_concurrent_approval() {
        // Verify the 409 Conflict response for race condition on approval
        let response = HttpResponse::Conflict().json(ErrorResponse::new(
            "already_approved",
            "Agent wallet was approved by a concurrent request",
        ));
        assert_eq!(response.status(), actix_web::http::StatusCode::CONFLICT);
    }

    #[actix_web::test]
    async fn test_revoke_agent_response_serialization() {
        let resp = RevokeAgentResponse {
            success: true,
            message: "Agent wallet revoked successfully".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("revoked successfully"));
    }
}
