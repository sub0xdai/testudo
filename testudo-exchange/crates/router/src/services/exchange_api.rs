//! Exchange API Trait
//!
//! Defines the `ExchangeApi` trait for position management operations.
//! This is separate from `ExchangeAdapter` (which handles order routing).
//! ExchangeApi focuses on the management operations needed by the trade
//! manager: balance queries, order placement, amendment, and cancellation.

// @anchor exchange:router:exchange_api
// @tags api

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Errors from exchange API operations.
#[derive(Debug, Error)]
pub enum ExchangeApiError {
    #[error("Order not found: {0}")]
    OrderNotFound(String),
    #[error("Insufficient balance: need {required}, have {available}")]
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
    },
    #[error("Exchange error: {0}")]
    Exchange(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Agent wallet inactive: account {account_id} needs re-authorization")]
    AgentWalletInactive { account_id: Uuid },
}

/// Request to place an order via the exchange API.
#[derive(Debug, Clone)]
pub struct PlaceOrderRequest {
    pub user_id: Uuid,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: ApiOrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    /// Leverage for Binance Futures (1-125). Ignored by Shadow engine.
    pub leverage: u8,
    /// EXT-16 FR-3: Optional exchange account ID for multi-account routing.
    /// When present, routes to specific account; otherwise uses first account.
    pub exchange_account_id: Option<Uuid>,
    /// EXT-21: reduce-only flag for SL/TP orders (close position, don't open new).
    pub reduce_only: bool,
    /// EXT-24 FR-5: Optional clientOrderId for defense-in-depth identification.
    pub client_order_id: Option<String>,
    /// EXT-31: Bracket order — attached stop-loss trigger price.
    /// When set, the exchange activates the SL only after the entry fills.
    pub stop_loss_trigger: Option<Decimal>,
    /// EXT-31: Bracket order — attached take-profit trigger price.
    /// When set, the exchange activates the TP only after the entry fills.
    pub take_profit_trigger: Option<Decimal>,
}

/// Result from placing an order, including bracket order child IDs.
#[derive(Debug, Clone)]
pub struct PlaceOrderResult {
    pub id: String,
    /// Order status from exchange (e.g. "open", "closed").
    pub status: Option<String>,
    /// Average fill price (present when order is filled).
    pub average: Option<Decimal>,
    /// EXT-31: Exchange-assigned stop-loss order ID (bracket orders only).
    pub stop_loss_order_id: Option<String>,
    /// EXT-31: Exchange-assigned take-profit order ID (bracket orders only).
    pub take_profit_order_id: Option<String>,
}

/// Order side for exchange API operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type for exchange API operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiOrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,
}

/// Request to amend an existing order.
///
/// For CEX (live) exchanges, amend is implemented as cancel+replace.
/// The `order_type`, `side`, `quantity`, and `reduce_only` fields are
/// required for the replacement order placement.
#[derive(Debug, Clone)]
pub struct AmendRequest {
    pub new_price: Option<Decimal>,
    pub new_stop_price: Option<Decimal>,
    pub new_quantity: Option<Decimal>,
    /// Order type for the replacement order (e.g. StopLoss for SL amendments).
    pub order_type: Option<ApiOrderType>,
    /// Side for the replacement order (opposite of position side for SL).
    pub side: Option<OrderSide>,
    /// Original order quantity (used when new_quantity is None).
    pub quantity: Option<Decimal>,
    /// Whether the replacement order is reduce-only (true for SL/TP).
    pub reduce_only: bool,
}

/// Information about an open position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub symbol: String,
    pub side: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub unrealized_pnl: Decimal,
}

/// Exchange API trait for trade management operations.
///
/// Separate from `ExchangeAdapter` which handles order routing.
/// This trait focuses on the operations needed by TradeManagerService:
/// balance lookups, order placement/amendment/cancellation, and position queries.
///
/// All methods accept an optional `exchange_account_id` for multi-account routing.
/// Shadow implementations ignore it; CEX uses it to load the correct credentials.
#[async_trait]
pub trait ExchangeApi: Send + Sync {
    /// Get balance for a specific asset.
    async fn get_balance(
        &self,
        user_id: Uuid,
        asset: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Decimal, ExchangeApiError>;

    /// Place a new order. Returns entry order ID and optional bracket child IDs.
    async fn place_order(&self, req: PlaceOrderRequest) -> Result<PlaceOrderResult, ExchangeApiError>;

    /// Amend an existing order (cancel + replace for shadow, native amend for Binance).
    async fn amend_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        symbol: &str,
        amend: AmendRequest,
        exchange_account_id: Option<Uuid>,
    ) -> Result<String, ExchangeApiError>;

    /// Cancel an order.
    async fn cancel_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError>;

    /// Cancel ALL open orders for a symbol. Defense-in-depth fallback.
    /// Default no-op for Shadow engine (no exchange orders to clean up).
    async fn cancel_all_orders(
        &self,
        _user_id: Uuid,
        _symbol: &str,
        _exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        Ok(())
    }

    /// Get position info for a symbol.
    async fn get_position(
        &self,
        user_id: Uuid,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Option<PositionInfo>, ExchangeApiError>;
}

/// Shadow engine implementation of ExchangeApi.
///
/// Uses cancel_order_no_cascade + place_order_no_group for amend (swap pattern).
/// Migrated to EngineHandle (019b): all operations routed through the actor.
pub struct ShadowExchangeApi {
    engine: engine::EngineHandle,
}

impl ShadowExchangeApi {
    pub fn new(engine: engine::EngineHandle) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl ExchangeApi for ShadowExchangeApi {
    async fn get_balance(
        &self,
        user_id: Uuid,
        asset: &str,
        _exchange_account_id: Option<Uuid>,
    ) -> Result<Decimal, ExchangeApiError> {
        let balances = self.engine.get_balances(user_id).await;
        let balance = balances
            .iter()
            .find(|b| b.asset == asset)
            .map(|b| b.available + b.reserved)
            .unwrap_or(Decimal::ZERO);
        Ok(balance)
    }

    async fn place_order(&self, req: PlaceOrderRequest) -> Result<PlaceOrderResult, ExchangeApiError> {
        let side = match req.side {
            OrderSide::Buy => engine::ShadowOrderSide::Buy,
            OrderSide::Sell => engine::ShadowOrderSide::Sell,
        };

        let order_type = match req.order_type {
            ApiOrderType::Market => engine::ShadowOrderType::Market,
            ApiOrderType::Limit => engine::ShadowOrderType::Limit,
            ApiOrderType::StopLoss => engine::ShadowOrderType::StopLoss,
            ApiOrderType::TakeProfit => engine::ShadowOrderType::TakeProfit,
        };

        let mut order = engine::ShadowOrder::new(
            req.user_id,
            req.symbol,
            side,
            order_type,
            req.quantity,
            req.price,
            req.stop_price,
            None,
        );
        order.mark_risk_validated();

        match self.engine.place_order_no_group(req.user_id, order).await {
            Ok(placed) => Ok(PlaceOrderResult {
                id: placed.id.to_string(),
                status: None,
                average: None,
                stop_loss_order_id: None,
                take_profit_order_id: None,
            }),
            Err(e) => Err(ExchangeApiError::Exchange(e.to_string())),
        }
    }

    async fn amend_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        _symbol: &str,
        amend: AmendRequest,
        _exchange_account_id: Option<Uuid>,
    ) -> Result<String, ExchangeApiError> {
        let order_uuid = Uuid::parse_str(order_id)
            .map_err(|e| ExchangeApiError::Internal(format!("Invalid order ID: {}", e)))?;

        // Get original order info before cancellation
        let order = self
            .engine
            .get_order(order_uuid)
            .await
            .ok_or_else(|| ExchangeApiError::OrderNotFound(order_id.to_string()))?;
        let (symbol, side, order_type, orig_qty, orig_price, orig_stop) = (
            order.symbol.clone(),
            order.side,
            order.order_type,
            order.quantity,
            order.price,
            order.stop_price,
        );

        // Cancel the old order (no cascade)
        self.engine
            .cancel_order_no_cascade(user_id, order_uuid)
            .await
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;

        // Place replacement order with amended values
        let new_qty = amend.new_quantity.unwrap_or(orig_qty);
        let new_price = amend.new_price.or(orig_price);
        let new_stop = amend.new_stop_price.or(orig_stop);

        let mut new_order = engine::ShadowOrder::new(
            user_id, symbol, side, order_type, new_qty, new_price, new_stop, None,
        );
        new_order.mark_risk_validated();

        match self
            .engine
            .place_order_no_group(user_id, new_order)
            .await
        {
            Ok(placed) => Ok(placed.id.to_string()),
            Err(e) => Err(ExchangeApiError::Exchange(e.to_string())),
        }
    }

    async fn cancel_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        _symbol: &str,
        _exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        let order_uuid = Uuid::parse_str(order_id)
            .map_err(|e| ExchangeApiError::Internal(format!("Invalid order ID: {}", e)))?;

        self.engine
            .cancel_order_no_cascade(user_id, order_uuid)
            .await
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;
        Ok(())
    }

    async fn get_position(
        &self,
        user_id: Uuid,
        symbol: &str,
        _exchange_account_id: Option<Uuid>,
    ) -> Result<Option<PositionInfo>, ExchangeApiError> {
        let positions = self.engine.get_positions(user_id).await;
        let pos = positions.iter().find(|p| p.symbol == symbol);
        Ok(pos.map(|p| PositionInfo {
            symbol: p.symbol.clone(),
            side: format!("{:?}", p.side),
            quantity: p.size,
            entry_price: p.entry_price,
            unrealized_pnl: p.unrealized_pnl,
        }))
    }
}

/// CEX Sidecar implementation of ExchangeApi (CEX-07 safe-cex migration).
///
/// Routes all live trading operations through the safe-cex gateway,
/// supporting any configured exchange (Binance, WooX, Bybit, etc.).
/// Credentials are fetched per-request from `ExchangeAccountRepository`.
pub struct CexExchangeApi {
    cex_client: std::sync::Arc<crate::services::cex_client::CexClient>,
    account_repo: crate::repositories::exchange_account::ExchangeAccountRepository,
    sandbox: bool,
}

impl CexExchangeApi {
    pub fn new(
        cex_client: std::sync::Arc<crate::services::cex_client::CexClient>,
        account_repo: crate::repositories::exchange_account::ExchangeAccountRepository,
        sandbox: bool,
    ) -> Self {
        Self {
            cex_client,
            account_repo,
            sandbox,
        }
    }

    /// Look up the user's exchange account and decrypt credentials.
    /// EXT-16 FR-3.6: If `exchange_account_id` is present, use that specific account.
    /// FR-3.7: Otherwise, fall back to first account for backwards compatibility.
    async fn load_credentials(
        &self,
        user_id: Uuid,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(String, crate::services::cex_client::SidecarCredentials), ExchangeApiError> {
        let accounts =
            self.account_repo.list_by_user(user_id).await.map_err(|e| {
                ExchangeApiError::Internal(format!("Failed to list accounts: {}", e))
            })?;

        let account = if let Some(target_id) = exchange_account_id {
            accounts.iter().find(|a| a.id == target_id).ok_or_else(|| {
                ExchangeApiError::Internal(format!("Exchange account {} not found", target_id))
            })?
        } else {
            // Filter to only active accounts for CEX fallback
            accounts.iter().find(|a| a.is_active.unwrap_or(false)).ok_or_else(|| {
                ExchangeApiError::Internal("No exchange account configured".into())
            })?
        };

        // UXA-01: Return specific error for inactive agent wallets
        if account.auth_mode == crate::types::exchange_names::auth_modes::AGENT_WALLET
            && !account.is_active.unwrap_or(false)
        {
            return Err(ExchangeApiError::AgentWalletInactive { account_id: account.id });
        }

        let creds = self
            .account_repo
            .load_credentials(account.id, user_id)
            .await
            .map_err(|e| {
                ExchangeApiError::Internal(format!("Failed to load credentials: {}", e))
            })?;

        let sidecar_creds = crate::services::cex_client::SidecarCredentials {
            api_key: creds.api_key,
            secret: creds.api_secret,
            password: creds.passphrase,
        };

        Ok((creds.exchange_name, sidecar_creds))
    }
}

/// Convert internal symbol format `BTC_USDT` to CEX format `BTCUSDT` (strip underscore).
pub fn to_cex_symbol(internal: &str) -> String {
    internal.replace('_', "")
}

/// Convert CEX symbol `BTCUSDT` back to internal format `BTC_USDT`.
pub fn from_cex_symbol(cex: &str) -> String {
    for quote in &["USDT", "USDC", "BUSD"] {
        if let Some(base) = cex.strip_suffix(quote) {
            if !base.is_empty() {
                return format!("{}_{}", base, quote);
            }
        }
    }
    cex.to_string()
}

fn map_cex_error(e: crate::services::cex_client::CexClientError) -> ExchangeApiError {
    use crate::services::cex_client::CexClientError;
    match e {
        CexClientError::AuthenticationFailed => {
            ExchangeApiError::Exchange("Authentication failed".into())
        }
        CexClientError::InsufficientFunds => ExchangeApiError::InsufficientBalance {
            required: Decimal::ZERO,
            available: Decimal::ZERO,
        },
        CexClientError::OrderNotFound(id) => ExchangeApiError::OrderNotFound(id),
        CexClientError::RateLimited => {
            ExchangeApiError::Exchange("Rate limited by exchange".into())
        }
        CexClientError::Unavailable(msg) => {
            ExchangeApiError::Exchange(format!("Sidecar unavailable: {}", msg))
        }
        CexClientError::ExchangeError(msg) => ExchangeApiError::Exchange(msg),
        CexClientError::WebSocketError(msg) => {
            ExchangeApiError::Exchange(format!("WebSocket error: {}", msg))
        }
    }
}

#[async_trait]
impl ExchangeApi for CexExchangeApi {
    async fn get_balance(
        &self,
        user_id: Uuid,
        _asset: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Decimal, ExchangeApiError> {
        let (exchange_id, creds) = self.load_credentials(user_id, exchange_account_id).await?;

        let balances = self
            .cex_client
            .fetch_balance(&exchange_id, &creds, self.sandbox, "future")
            .await
            .map_err(map_cex_error)?;

        let usdt = match balances.iter().find(|b| b.asset == "USDT") {
            Some(b) => crate::services::cex_client::parse_decimal(&b.total)
                .map_err(ExchangeApiError::Exchange)?,
            None => Decimal::ZERO,
        };

        Ok(usdt)
    }

    async fn place_order(&self, req: PlaceOrderRequest) -> Result<PlaceOrderResult, ExchangeApiError> {
        let (exchange_id, creds) = self
            .load_credentials(req.user_id, req.exchange_account_id)
            .await?;

        let cex_symbol = to_cex_symbol(&req.symbol);
        let side = match req.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };
        let order_type = match req.order_type {
            ApiOrderType::Market => "market",
            ApiOrderType::Limit => "limit",
            ApiOrderType::StopLoss => "market",
            ApiOrderType::TakeProfit => "limit",
        };
        let leverage = if req.leverage > 0 {
            Some(req.leverage)
        } else {
            None
        };

        let result = self
            .cex_client
            .create_order(
                &exchange_id,
                &creds,
                self.sandbox,
                &cex_symbol,
                side,
                order_type,
                req.quantity,
                req.price,
                req.stop_price,
                leverage,
                req.reduce_only,
                req.client_order_id,
                req.stop_loss_trigger,
                req.take_profit_trigger,
            )
            .await
            .map_err(map_cex_error)?;

        Ok(PlaceOrderResult {
            id: result.id,
            status: result.status,
            average: result.average.as_ref().and_then(|s| s.parse().ok()),
            stop_loss_order_id: result.stop_loss_order_id,
            take_profit_order_id: result.take_profit_order_id,
        })
    }

    async fn amend_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        symbol: &str,
        amend: AmendRequest,
        exchange_account_id: Option<Uuid>,
    ) -> Result<String, ExchangeApiError> {
        let (exchange_id, creds) = self.load_credentials(user_id, exchange_account_id).await?;
        let cex_symbol = to_cex_symbol(symbol);

        // Cancel+replace: many exchanges (WOO X) don't support editOrder for
        // algo/stop orders. Cancel the old order first, then place a replacement.
        self.cex_client
            .cancel_order(&exchange_id, &creds, self.sandbox, order_id, &cex_symbol)
            .await
            .map_err(map_cex_error)?;

        let order_type = match amend.order_type {
            Some(ApiOrderType::StopLoss) | Some(ApiOrderType::Market) => "market",
            Some(ApiOrderType::TakeProfit) | Some(ApiOrderType::Limit) | None => "limit",
        };
        let side = match amend.side {
            Some(OrderSide::Buy) => "buy",
            Some(OrderSide::Sell) => "sell",
            None => "buy",
        };
        let quantity = amend.new_quantity.or(amend.quantity).unwrap_or(Decimal::ZERO);

        let result = self
            .cex_client
            .create_order(
                &exchange_id,
                &creds,
                self.sandbox,
                &cex_symbol,
                side,
                order_type,
                quantity,
                amend.new_price,
                amend.new_stop_price,
                None,  // leverage (already set on exchange)
                amend.reduce_only,
                None,  // clientOrderId
                None,  // stop_loss_trigger (bracket)
                None,  // take_profit_trigger (bracket)
            )
            .await
            .map_err(map_cex_error)?;

        tracing::info!(
            old_id = %order_id,
            new_id = %result.id,
            symbol = %symbol,
            order_type = %order_type,
            side = %side,
            "CexExchangeApi: cancel+replace amend completed"
        );

        Ok(result.id)
    }

    async fn cancel_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        let (exchange_id, creds) = self.load_credentials(user_id, exchange_account_id).await?;

        let cex_symbol = to_cex_symbol(symbol);

        self.cex_client
            .cancel_order(&exchange_id, &creds, self.sandbox, order_id, &cex_symbol)
            .await
            .map_err(map_cex_error)?;
        Ok(())
    }

    async fn cancel_all_orders(
        &self,
        user_id: Uuid,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        let (exchange_id, creds) = self.load_credentials(user_id, exchange_account_id).await?;
        let cex_symbol = to_cex_symbol(symbol);
        self.cex_client
            .cancel_all_orders(&exchange_id, &creds, self.sandbox, &cex_symbol)
            .await
            .map_err(map_cex_error)?;
        Ok(())
    }

    async fn get_position(
        &self,
        user_id: Uuid,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Option<PositionInfo>, ExchangeApiError> {
        let (exchange_id, creds) = self.load_credentials(user_id, exchange_account_id).await?;

        let cex_symbol = to_cex_symbol(symbol);
        let positions = self
            .cex_client
            .fetch_positions(&exchange_id, &creds, self.sandbox, Some(&cex_symbol))
            .await
            .map_err(map_cex_error)?;

        let pos = match positions.first() {
            Some(p) => {
                let contracts = crate::services::cex_client::parse_decimal(&p.contracts)
                    .map_err(ExchangeApiError::Exchange)?;
                if contracts == Decimal::ZERO {
                    None
                } else {
                    Some(PositionInfo {
                        symbol: symbol.to_string(),
                        side: p.side.clone(),
                        quantity: contracts.abs(),
                        entry_price: crate::services::cex_client::parse_decimal(&p.entry_price)
                            .map_err(ExchangeApiError::Exchange)?,
                        unrealized_pnl: crate::services::cex_client::parse_decimal(&p.unrealized_pnl)
                            .map_err(ExchangeApiError::Exchange)?,
                    })
                }
            }
            None => None,
        };

        Ok(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{EngineActor, EngineHandle, ShadowEngine};
    use rust_decimal_macros::dec;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn create_test_handle() -> EngineHandle {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        handle
    }

    #[tokio::test]
    async fn test_shadow_get_balance() {
        let handle = create_test_handle();
        let api = ShadowExchangeApi::new(handle.clone());
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let balance = api.get_balance(user_id, "USDT", None).await.unwrap();
        assert_eq!(balance, dec!(10000));
    }

    #[tokio::test]
    async fn test_shadow_get_balance_unknown_asset() {
        let handle = create_test_handle();
        let api = ShadowExchangeApi::new(handle.clone());
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let balance = api.get_balance(user_id, "UNKNOWN", None).await.unwrap();
        assert_eq!(balance, Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_shadow_place_order() {
        let handle = create_test_handle();
        let api = ShadowExchangeApi::new(handle.clone());
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let order_id = api
            .place_order(PlaceOrderRequest {
                user_id,
                symbol: "BTC_USDT".to_string(),
                side: OrderSide::Buy,
                order_type: ApiOrderType::Limit,
                quantity: dec!(0.01),
                price: Some(dec!(50000)),
                stop_price: None,
                leverage: 1,
                exchange_account_id: None,
                reduce_only: false,
                client_order_id: None,
                stop_loss_trigger: None,
                take_profit_trigger: None,
            })
            .await
            .unwrap();

        assert!(!order_id.id.is_empty());
        // Verify order exists in engine
        let open = handle.get_open_orders(user_id).await;
        assert_eq!(open.len(), 1);
    }

    #[tokio::test]
    async fn test_shadow_amend_order() {
        let handle = create_test_handle();
        let api = ShadowExchangeApi::new(handle.clone());
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        // Place initial order
        let result = api
            .place_order(PlaceOrderRequest {
                user_id,
                symbol: "BTC_USDT".to_string(),
                side: OrderSide::Buy,
                order_type: ApiOrderType::Limit,
                quantity: dec!(0.01),
                price: Some(dec!(50000)),
                stop_price: None,
                leverage: 1,
                exchange_account_id: None,
                reduce_only: false,
                client_order_id: None,
                stop_loss_trigger: None,
                take_profit_trigger: None,
            })
            .await
            .unwrap();

        // Amend to new price
        let new_order_id = api
            .amend_order(
                user_id,
                &result.id,
                "BTC_USDT",
                AmendRequest {
                    new_price: Some(dec!(49000)),
                    new_stop_price: None,
                    new_quantity: None,
                    order_type: Some(ApiOrderType::Limit),
                    side: Some(OrderSide::Buy),
                    quantity: Some(dec!(0.01)),
                    reduce_only: false,
                },
                None,
            )
            .await
            .unwrap();

        // New order should have different ID
        assert_ne!(result.id, new_order_id);

        // Should still have 1 open order
        let open = handle.get_open_orders(user_id).await;
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].price, Some(dec!(49000)));
    }

    #[tokio::test]
    async fn test_shadow_cancel_order() {
        let handle = create_test_handle();
        let api = ShadowExchangeApi::new(handle.clone());
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let result = api
            .place_order(PlaceOrderRequest {
                user_id,
                symbol: "BTC_USDT".to_string(),
                side: OrderSide::Buy,
                order_type: ApiOrderType::Limit,
                quantity: dec!(0.01),
                price: Some(dec!(50000)),
                stop_price: None,
                leverage: 1,
                exchange_account_id: None,
                reduce_only: false,
                client_order_id: None,
                stop_loss_trigger: None,
                take_profit_trigger: None,
            })
            .await
            .unwrap();

        api.cancel_order(user_id, &result.id, "BTC_USDT", None).await.unwrap();

        let open = handle.get_open_orders(user_id).await;
        assert_eq!(open.len(), 0);
    }

    #[tokio::test]
    async fn test_shadow_get_position_none() {
        let handle = create_test_handle();
        let api = ShadowExchangeApi::new(handle.clone());
        let user_id = Uuid::new_v4();

        handle.init_user(user_id).await.unwrap();

        let pos = api.get_position(user_id, "BTC_USDT", None).await.unwrap();
        assert!(pos.is_none());
    }

    // ==================== Symbol Conversion Tests ====================

    #[test]
    fn test_to_cex_symbol_btc() {
        assert_eq!(to_cex_symbol("BTC_USDT"), "BTCUSDT");
    }

    #[test]
    fn test_to_cex_symbol_eth() {
        assert_eq!(to_cex_symbol("ETH_USDT"), "ETHUSDT");
    }

    #[test]
    fn test_to_cex_symbol_sol() {
        assert_eq!(to_cex_symbol("SOL_USDT"), "SOLUSDT");
    }

    #[test]
    fn test_to_cex_symbol_passthrough() {
        assert_eq!(to_cex_symbol("INVALID"), "INVALID");
    }

    #[test]
    fn test_from_cex_symbol() {
        assert_eq!(from_cex_symbol("BTCUSDT"), "BTC_USDT");
        assert_eq!(from_cex_symbol("ETHUSDT"), "ETH_USDT");
    }

    #[test]
    fn test_cex_symbol_roundtrip() {
        let internal = "BTC_USDT";
        let cex = to_cex_symbol(internal);
        let back = from_cex_symbol(&cex);
        assert_eq!(back, internal);
    }
}
