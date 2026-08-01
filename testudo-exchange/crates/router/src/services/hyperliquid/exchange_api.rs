//! HL-03: HyperliquidExchangeApi — ExchangeApi trait implementation
//!
//! Native Rust SDK integration that slots in alongside `ShadowExchangeApi`
//! and `CexExchangeApi`. Bypasses the Node.js sidecar entirely.

// @anchor exchange:router:exchange_api
// @tags api

use alloy::signers::local::PrivateKeySigner;
use async_trait::async_trait;
use hyperliquid_sdk_rs::{
    types::{
        CancelRequest as HlCancelRequest, ExchangeDataStatus, Limit, OrderRequest as HlOrderRequest,
        OrderType as HlOrderType, Trigger,
    },
    ExchangeProvider, InfoProvider, Network,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

use super::auth::{AuthCache, AuthError, AuthMode, HyperliquidAuth};
use super::universe::AssetUniverse;
use crate::repositories::exchange_account::ExchangeAccountRepository;
use crate::types::exchange_names::{auth_modes, exchanges};
use crate::services::exchange_api::{
    AmendRequest, ApiOrderType, ExchangeApi, ExchangeApiError, OrderSide, PlaceOrderRequest,
    PlaceOrderResult, PositionInfo,
};

/// Namespace UUID for deterministic CLOID generation (UUID v5).
/// Uses the DNS namespace as a base — the input string is always
/// `testudo:{group_id}:{role}` so collisions are impossible.
const CLOID_NAMESPACE: Uuid = Uuid::NAMESPACE_DNS;

/// Native Hyperliquid exchange API implementation.
///
/// Implements the `ExchangeApi` trait using the Hyperliquid Rust SDK directly,
/// bypassing the Node.js sidecar. Credentials are loaded per-request from the
/// existing `ExchangeAccountRepository` and cached via `AuthCache`.
pub struct HyperliquidExchangeApi {
    info: InfoProvider,
    universe: Arc<AssetUniverse>,
    auth_cache: Arc<AuthCache>,
    account_repo: ExchangeAccountRepository,
    network: Network,
}

impl HyperliquidExchangeApi {
    pub fn new(
        universe: Arc<AssetUniverse>,
        auth_cache: Arc<AuthCache>,
        account_repo: ExchangeAccountRepository,
        network: Network,
    ) -> Self {
        let info = InfoProvider::new(network);
        Self {
            info,
            universe,
            auth_cache,
            account_repo,
            network,
        }
    }

    /// Load auth for a user's exchange account, returning the cached signer.
    async fn load_auth(
        &self,
        user_id: Uuid,
        exchange_account_id: Option<Uuid>,
    ) -> Result<HyperliquidAuth, ExchangeApiError> {
        let accounts = self
            .account_repo
            .list_by_user(user_id)
            .await
            .map_err(|e| ExchangeApiError::Internal(format!("Failed to list accounts: {}", e)))?;

        // Find the Hyperliquid account
        let account = if let Some(target_id) = exchange_account_id {
            accounts
                .iter()
                .find(|a| a.id == target_id)
                .ok_or_else(|| {
                    ExchangeApiError::Internal(format!("Exchange account {} not found", target_id))
                })?
        } else {
            accounts
                .iter()
                .find(|a| a.exchange_name.eq_ignore_ascii_case(exchanges::HYPERLIQUID))
                .or_else(|| accounts.first())
                .ok_or_else(|| {
                    ExchangeApiError::Internal("No exchange account configured".into())
                })?
        };

        // UXA-01: Return specific error for inactive agent wallets instead of generic NotFound
        if account.auth_mode == auth_modes::AGENT_WALLET && !account.is_active.unwrap_or(false) {
            return Err(ExchangeApiError::AgentWalletInactive { account_id: account.id });
        }

        let creds = self
            .account_repo
            .load_credentials(account.id, user_id)
            .await
            .map_err(|e| {
                ExchangeApiError::Internal(format!("Failed to load credentials: {}", e))
            })?;

        match creds.auth_mode.as_str() {
            auth_modes::AGENT_WALLET => {
                let wallet_addr = creds.wallet_address.ok_or_else(|| {
                    ExchangeApiError::Internal(
                        AuthError::MissingWalletAddress.to_string(),
                    )
                })?;
                self.auth_cache
                    .get_or_insert_agent(account.id, &creds.api_secret, &wallet_addr)
                    .await
                    .map_err(|e| ExchangeApiError::Internal(format!("Auth failed: {}", e)))
            }
            _ => {
                self.auth_cache
                    .get_or_insert(account.id, &creds.api_key, &creds.api_secret)
                    .await
                    .map_err(|e| ExchangeApiError::Internal(format!("Auth failed: {}", e)))
            }
        }
    }

    /// Build an `ExchangeProvider` from a cached signer.
    ///
    /// Always uses the plain constructor (no agent wrapping, no vault).
    /// Agent wallets sign with their own key; the HL API recovers the
    /// signer address and looks up the agent→user approval mapping.
    fn build_exchange(
        &self,
        auth: &HyperliquidAuth,
    ) -> ExchangeProvider<PrivateKeySigner> {
        match self.network {
            Network::Mainnet => ExchangeProvider::mainnet(auth.signer.clone()),
            Network::Testnet => ExchangeProvider::testnet(auth.signer.clone()),
        }
    }

    /// Transfer USDC between spot and perp accounts.
    /// `to_perp`: true = spot→perp, false = perp→spot.
    /// Returns Ok(true) on success, Ok(false) if HL rejected the transfer.
    pub async fn transfer_usdc(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        amount: &str,
        to_perp: bool,
    ) -> Result<bool, ExchangeApiError> {
        let auth = self.load_auth(user_id, Some(account_id)).await?;
        let exchange = self.build_exchange(&auth);

        // Parse amount as integer wei (USDC has 6 decimals)
        let dec: Decimal = amount.parse().map_err(|e| {
            ExchangeApiError::Internal(format!("Invalid amount: {}", e))
        })?;
        let usd_size = (dec * Decimal::from(1_000_000u64))
            .to_u64()
            .ok_or_else(|| ExchangeApiError::Internal("Amount too large".into()))?;

        // spot_transfer_to_perp uses the spotUser action which has correct field naming.
        // usd_class_transfer has a serde bug: sends to_perp (snake_case) but HL expects toPerp.
        let status = exchange
            .spot_transfer_to_perp(usd_size, to_perp)
            .await
            .map_err(|e| ExchangeApiError::Internal(format!("Transfer failed: {}", e)))?;
        let ok = status.is_ok();
        tracing::info!(
            "HL transfer: user={} account={} amount={} usd_size={} to_perp={} ok={}",
            user_id, account_id, amount, usd_size, to_perp, ok
        );
        Ok(ok)
    }
}

/// Generate a deterministic CLOID (UUID v5) from a client order ID string.
///
/// Input: `"testudo:{group_id}:{role}"` or any string.
/// Output: deterministic UUID suitable for Hyperliquid CLOID.
pub fn generate_cloid(client_order_id: &str) -> Uuid {
    Uuid::new_v5(&CLOID_NAMESPACE, client_order_id.as_bytes())
}

/// Build a Hyperliquid `OrderRequest` from a `PlaceOrderRequest`.
///
/// Handles market, limit, and stop-loss order type mapping.
/// Formats quantity to the correct number of decimal places using `sz_decimals`.
pub fn build_order_request(
    asset_index: u32,
    req: &PlaceOrderRequest,
    sz_decimals: u32,
) -> Result<HlOrderRequest, ExchangeApiError> {
    let is_buy = matches!(req.side, OrderSide::Buy);

    // Format size with correct decimal precision
    let sz = format_sz(req.quantity, sz_decimals);

    let cloid = req
        .client_order_id
        .as_ref()
        .map(|coid| generate_cloid(coid));

    let order = match req.order_type {
        ApiOrderType::Market => {
            // Market = aggressive IOC limit at a very unfavorable price
            // Hyperliquid doesn't have native market orders; use IOC limit.
            let slippage_price = if is_buy {
                // Buy: set limit price very high to ensure fill
                req.price
                    .unwrap_or_else(|| Decimal::new(999_999_999, 0))
                    .to_string()
            } else {
                // Sell: set limit price very low to ensure fill
                req.price
                    .unwrap_or_else(|| Decimal::new(1, 2)) // 0.01
                    .to_string()
            };
            HlOrderRequest::limit(asset_index, is_buy, slippage_price, &sz, "Ioc")
                .reduce_only(req.reduce_only)
                .with_cloid(cloid)
        }
        ApiOrderType::Limit => {
            let price = req
                .price
                .ok_or_else(|| ExchangeApiError::Internal("Limit order requires price".into()))?;
            HlOrderRequest::limit(asset_index, is_buy, price.to_string(), &sz, "Gtc")
                .reduce_only(req.reduce_only)
                .with_cloid(cloid)
        }
        ApiOrderType::StopLoss => {
            let trigger_px = req.stop_price.ok_or_else(|| {
                ExchangeApiError::Internal("StopLoss order requires stop_price".into())
            })?;
            let mut order = HlOrderRequest::trigger(
                asset_index,
                is_buy,
                trigger_px.to_string(),
                &sz,
                "sl",
                true, // is_market: stop-loss executes as market
            )
            .reduce_only(true) // SL is always reduce-only
            .with_cloid(cloid);
            order.limit_px = trigger_limit_px(&trigger_px, is_buy);
            order
        }
        ApiOrderType::TakeProfit => {
            let trigger_px = req.stop_price.ok_or_else(|| {
                ExchangeApiError::Internal("TakeProfit order requires stop_price".into())
            })?;
            let mut order = HlOrderRequest::trigger(
                asset_index,
                is_buy,
                trigger_px.to_string(),
                &sz,
                "tp",
                true, // is_market: take-profit executes as market
            )
            .reduce_only(true) // TP is always reduce-only
            .with_cloid(cloid);
            order.limit_px = trigger_limit_px(&trigger_px, is_buy);
            order
        }
    };

    Ok(order)
}

/// Compute the limit price for market trigger orders.
/// Hyperliquid requires a valid `limit_px` even when `isMarket = true`.
/// The SDK defaults to "0" which is rejected as "Order has invalid price."
/// Per HL SDK convention: set limit_px = trigger_px for market triggers.
fn trigger_limit_px(trigger_px: &Decimal, _is_buy: bool) -> String {
    trigger_px.normalize().to_string()
}

/// Format a quantity to the correct number of decimal places for Hyperliquid.
pub fn format_sz(quantity: Decimal, sz_decimals: u32) -> String {
    let scaled = quantity
        .round_dp(sz_decimals)
        .normalize();
    scaled.to_string()
}

/// Extract the order ID (OID) from a Hyperliquid exchange response.
pub fn extract_order_id(
    statuses: &[ExchangeDataStatus],
) -> Option<u64> {
    for status in statuses {
        match status {
            ExchangeDataStatus::Resting(r) => return Some(r.oid),
            ExchangeDataStatus::Filled(f) => return Some(f.oid),
            _ => {}
        }
    }
    None
}

/// HL-11 FR-1: Normalize ExchangeDataStatus to CCXT-compatible status string.
/// "closed" = filled/done, "open" = resting/waiting, "error:..." = rejected.
pub fn normalize_status(status: &ExchangeDataStatus) -> String {
    match status {
        ExchangeDataStatus::Filled(_) => "closed".to_string(),
        ExchangeDataStatus::Success => "closed".to_string(),
        ExchangeDataStatus::Resting(_) => "open".to_string(),
        ExchangeDataStatus::WaitingForTrigger => "open".to_string(),
        ExchangeDataStatus::WaitingForFill => "open".to_string(),
        ExchangeDataStatus::Error(msg) => format!("error:{}", msg),
    }
}

/// Extract the average fill price from a response (if filled).
pub fn extract_avg_price(statuses: &[ExchangeDataStatus]) -> Option<Decimal> {
    for status in statuses {
        if let ExchangeDataStatus::Filled(f) = status {
            return Decimal::from_str(&f.avg_px).ok();
        }
    }
    None
}

/// FIX-01: Parse a decimal from a string, returning an error on failure.
fn parse_decimal(s: &str) -> Result<Decimal, ExchangeApiError> {
    Decimal::from_str(s).map_err(|e| {
        ExchangeApiError::Exchange(format!("Failed to parse decimal '{}': {}", s, e))
    })
}

#[async_trait]
impl ExchangeApi for HyperliquidExchangeApi {
    async fn get_balance(
        &self,
        user_id: Uuid,
        _asset: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Decimal, ExchangeApiError> {
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let state = self
            .info
            .user_state(auth.query_address())
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Failed to fetch user state: {}", e)))?;

        let account_value = parse_decimal(&state.margin_summary.account_value)?;
        Ok(account_value)
    }

    async fn place_order(
        &self,
        mut req: PlaceOrderRequest,
    ) -> Result<PlaceOrderResult, ExchangeApiError> {
        let auth = self
            .load_auth(req.user_id, req.exchange_account_id)
            .await?;
        let exchange = self.build_exchange(&auth);

        // Resolve symbol to HL coin and asset index
        let coin = AssetUniverse::to_hl_coin(&req.symbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;
        let sz_decimals = self
            .universe
            .sz_decimals(coin)
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;

        // HL-11: For market orders without a price, query mid price and apply
        // 10% slippage band. HL rejects extreme prices (e.g. 0.01 for BTC).
        if req.order_type == ApiOrderType::Market && req.price.is_none() {
            if let Ok(mids) = self.info.all_mids().await {
                if let Some(mid_str) = mids.get(coin) {
                    if let Ok(mid) = Decimal::from_str(mid_str) {
                        let slippage = mid * Decimal::new(10, 2); // 10%
                        let is_buy = matches!(req.side, OrderSide::Buy);
                        req.price = Some(if is_buy { mid + slippage } else { mid - slippage });
                        tracing::debug!(
                            coin = %coin,
                            mid = %mid,
                            slippage_price = ?req.price,
                            is_buy = %is_buy,
                            "HL market order: using mid price with 10% slippage"
                        );
                    }
                }
            }
        }

        let mut hl_order = build_order_request(asset_index, &req, sz_decimals)?;
        let cloid = req.client_order_id.as_ref().map(|c| generate_cloid(c));

        // Fix CLOID format: Hyperliquid API requires "0x" prefix + 32 hex chars.
        // The SDK's with_cloid() formats without prefix, so we override here.
        // Must call place_order() directly (not place_order_with_cloid which re-formats).
        if let Some(ref cloid_str) = hl_order.cloid {
            if !cloid_str.starts_with("0x") {
                hl_order.cloid = Some(format!("0x{}", cloid_str));
            }
        }

        tracing::info!(
            coin = %coin,
            asset_index = %asset_index,
            is_buy = %hl_order.is_buy,
            limit_px = %hl_order.limit_px,
            sz = %hl_order.sz,
            reduce_only = %hl_order.reduce_only,
            order_type = ?hl_order.order_type,
            cloid = ?hl_order.cloid,
            auth_mode = ?auth.auth_mode,
            query_address = %auth.query_address(),
            "HyperliquidExchangeApi: placing order"
        );

        let response = exchange
            .place_order(&hl_order)
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Place order failed: {}", e)))?;

        let resp = response
            .into_result()
            .map_err(ExchangeApiError::Exchange)?;

        let statuses = resp
            .data
            .as_ref()
            .map(|d| d.statuses.as_slice())
            .unwrap_or(&[]);

        // Extract OID from response
        let order_id = if let Some(oid) = extract_order_id(statuses) {
            oid.to_string()
        } else if let Some(cloid_uuid) = cloid {
            // For trigger orders (WaitingForTrigger), query open orders to find OID
            match self.find_oid_by_cloid(&auth, cloid_uuid).await {
                Ok(oid) => oid.to_string(),
                Err(_) => {
                    // Fallback: return CLOID hex as ID
                    format!("cloid:{:032x}", cloid_uuid.as_u128())
                }
            }
        } else {
            // FR-7: No OID extracted and no CLOID — check for errors in statuses.
            // into_result() already confirmed the response envelope was OK,
            // so if there's no Error status, the exchange accepted the order
            // (e.g. atomically-filled market close, WaitingForFill, Success).
            let error_msg = statuses.iter().find_map(|s| {
                if let ExchangeDataStatus::Error(msg) = s {
                    Some(msg.clone())
                } else {
                    None
                }
            });
            if let Some(msg) = error_msg {
                return Err(ExchangeApiError::Exchange(msg));
            }
            "success".to_string()
        };

        let avg_price = extract_avg_price(statuses);
        // HL-11 FR-1: Normalize ExchangeDataStatus to CCXT-compatible strings
        // so downstream `is_filled` check (== "closed") works for immediate fills.
        let status = statuses.first().map(normalize_status);

        // HL-09 FR-1/FR-2: Place SL/TP as separate trigger orders after entry
        let close_is_buy = !matches!(req.side, OrderSide::Buy);
        let sz = format_sz(req.quantity, sz_decimals);
        let mut sl_order_id = None;
        let mut tp_order_id = None;

        if let Some(sl_trigger) = req.stop_loss_trigger {
            sl_order_id = self.place_trigger_order(
                &exchange, &auth, asset_index, close_is_buy,
                sl_trigger, &sz, "sl", req.client_order_id.as_deref(),
            ).await;
        }

        if let Some(tp_trigger) = req.take_profit_trigger {
            tp_order_id = self.place_trigger_order(
                &exchange, &auth, asset_index, close_is_buy,
                tp_trigger, &sz, "tp", req.client_order_id.as_deref(),
            ).await;
        }

        Ok(PlaceOrderResult {
            id: order_id,
            status,
            average: avg_price,
            stop_loss_order_id: sl_order_id,
            take_profit_order_id: tp_order_id,
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
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let exchange = self.build_exchange(&auth);

        let oid: u64 = parse_oid(order_id)?;

        let coin = AssetUniverse::to_hl_coin(symbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;
        let sz_decimals = self
            .universe
            .sz_decimals(coin)
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;

        // FIX-01: Amend safety — side must be specified
        let is_buy = match amend.side {
            Some(OrderSide::Buy) => true,
            Some(OrderSide::Sell) => false,
            None => {
                return Err(ExchangeApiError::Internal(
                    "Amend requires side".into(),
                ));
            }
        };

        // FIX-01: Amend safety — quantity must be non-zero
        let quantity = amend.new_quantity.or(amend.quantity).unwrap_or(Decimal::ZERO);
        if quantity == Decimal::ZERO {
            return Err(ExchangeApiError::Internal(
                "Amend requires non-zero quantity".into(),
            ));
        }
        let sz = format_sz(quantity, sz_decimals);

        let new_order = match amend.order_type {
            Some(ApiOrderType::StopLoss) => {
                let trigger_px = amend.new_stop_price.ok_or_else(|| {
                    ExchangeApiError::Internal("StopLoss amend requires stop_price".into())
                })?;
                let mut order = HlOrderRequest::trigger(
                    asset_index,
                    is_buy,
                    trigger_px.to_string(),
                    &sz,
                    "sl",
                    true,
                )
                .reduce_only(true);
                order.limit_px = trigger_limit_px(&trigger_px, is_buy);
                order
            }
            Some(ApiOrderType::TakeProfit) => {
                let trigger_px = amend.new_stop_price.ok_or_else(|| {
                    ExchangeApiError::Internal("TakeProfit amend requires stop_price".into())
                })?;
                let mut order = HlOrderRequest::trigger(
                    asset_index,
                    is_buy,
                    trigger_px.to_string(),
                    &sz,
                    "tp",
                    true,
                )
                .reduce_only(true);
                order.limit_px = trigger_limit_px(&trigger_px, is_buy);
                order
            }
            Some(ApiOrderType::Market) => {
                let price = if is_buy { "999999999" } else { "0.01" };
                HlOrderRequest::limit(asset_index, is_buy, price, &sz, "Ioc")
                    .reduce_only(amend.reduce_only)
            }
            Some(ApiOrderType::Limit) | None => {
                let price = amend.new_price.ok_or_else(|| {
                    ExchangeApiError::Internal("Limit amend requires price".into())
                })?;
                HlOrderRequest::limit(asset_index, is_buy, price.to_string(), &sz, "Gtc")
                    .reduce_only(amend.reduce_only)
            }
        };

        let response = exchange
            .modify_order(oid, new_order)
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Modify order failed: {}", e)))?;

        let resp = response
            .into_result()
            .map_err(ExchangeApiError::Exchange)?;

        let statuses = resp
            .data
            .as_ref()
            .map(|d| d.statuses.as_slice())
            .unwrap_or(&[]);

        // Modified orders keep the same OID on Hyperliquid
        let new_oid = extract_order_id(statuses).unwrap_or(oid);

        tracing::info!(
            old_oid = %oid,
            new_oid = %new_oid,
            symbol = %symbol,
            "HyperliquidExchangeApi: modify_order completed"
        );

        Ok(new_oid.to_string())
    }

    async fn cancel_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let exchange = self.build_exchange(&auth);

        let coin = AssetUniverse::to_hl_coin(symbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;

        // Handle both numeric OID and CLOID-based IDs
        if let Some(cloid_hex) = order_id.strip_prefix("cloid:") {
            let cloid = Uuid::from_u128(
                u128::from_str_radix(cloid_hex, 16).map_err(|e| {
                    ExchangeApiError::Internal(format!("Invalid CLOID: {}", e))
                })?,
            );
            exchange
                .cancel_order_by_cloid(asset_index, cloid)
                .await
                .map_err(|e| ExchangeApiError::Exchange(format!("Cancel failed: {}", e)))?
                .into_result()
                .map_err(ExchangeApiError::Exchange)?;
        } else {
            let oid: u64 = parse_oid(order_id)?;
            exchange
                .cancel_order(asset_index, oid)
                .await
                .map_err(|e| ExchangeApiError::Exchange(format!("Cancel failed: {}", e)))?
                .into_result()
                .map_err(ExchangeApiError::Exchange)?;
        }

        Ok(())
    }

    async fn cancel_all_orders(
        &self,
        user_id: Uuid,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let exchange = self.build_exchange(&auth);

        let coin = AssetUniverse::to_hl_coin(symbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;

        // Fetch all open orders for this user
        let open_orders = self
            .info
            .open_orders(auth.query_address())
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Failed to fetch open orders: {}", e)))?;

        // Filter to this symbol and build cancel requests
        let cancels: Vec<HlCancelRequest> = open_orders
            .iter()
            .filter(|o| o.coin.to_uppercase() == coin.to_uppercase())
            .map(|o| HlCancelRequest::new(asset_index, o.oid))
            .collect();

        if cancels.is_empty() {
            return Ok(());
        }

        tracing::info!(
            count = cancels.len(),
            symbol = %symbol,
            coin = %coin,
            "HyperliquidExchangeApi: bulk cancelling orders"
        );

        exchange
            .bulk_cancel(cancels)
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Bulk cancel failed: {}", e)))?
            .into_result()
            .map_err(ExchangeApiError::Exchange)?;

        Ok(())
    }

    async fn get_position(
        &self,
        user_id: Uuid,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Option<PositionInfo>, ExchangeApiError> {
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let state = self
            .info
            .user_state(auth.query_address())
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Failed to fetch user state: {}", e)))?;

        let coin = AssetUniverse::to_hl_coin(symbol);

        let pos = state.asset_positions.iter().find(|ap| {
            ap.position.coin.to_uppercase() == coin.to_uppercase()
        });

        let position_info = match pos {
            Some(ap) => {
                let szi = parse_decimal(&ap.position.szi)?;
                if szi == Decimal::ZERO {
                    None
                } else {
                    let side = if szi > Decimal::ZERO {
                        "long".to_string()
                    } else {
                        "short".to_string()
                    };

                    let entry_price = match ap.position.entry_px.as_ref() {
                        Some(s) => parse_decimal(s)?,
                        None => Decimal::ZERO,
                    };

                    Some(PositionInfo {
                        symbol: symbol.to_string(),
                        side,
                        quantity: szi.abs(),
                        entry_price,
                        unrealized_pnl: parse_decimal(&ap.position.unrealized_pnl)?,
                    })
                }
            }
            None => None,
        };

        Ok(position_info)
    }
}

impl HyperliquidExchangeApi {
    /// HL-09 FR-1/FR-2: Place a trigger order (SL or TP) and return the exchange order ID.
    /// Logs warnings on failure but never errors — SL/TP failure must not fail the entry.
    async fn place_trigger_order(
        &self,
        exchange: &ExchangeProvider<PrivateKeySigner>,
        auth: &HyperliquidAuth,
        asset_index: u32,
        close_is_buy: bool,
        trigger_px: Decimal,
        sz: &str,
        tpsl: &str,
        client_order_id_base: Option<&str>,
    ) -> Option<String> {
        // Generate CLOID with ":sl" or ":tp" suffix for tracking
        let cloid = client_order_id_base
            .map(|base| generate_cloid(&format!("{}:{}", base, tpsl)));

        let mut order = HlOrderRequest::trigger(
            asset_index, close_is_buy, trigger_px.to_string(), sz, tpsl, true,
        )
        .reduce_only(true)
        .with_cloid(cloid);

        // Fix limit_px: SDK defaults to "0" which HL rejects as "invalid price"
        order.limit_px = trigger_limit_px(&trigger_px, close_is_buy);

        // Fix CLOID 0x prefix
        if let Some(ref s) = order.cloid {
            if !s.starts_with("0x") {
                order.cloid = Some(format!("0x{}", s));
            }
        }

        tracing::info!(
            tpsl = %tpsl,
            trigger_px = %trigger_px,
            close_is_buy = %close_is_buy,
            cloid = ?order.cloid,
            "Placing {} trigger order", tpsl
        );

        // Log the full order for debugging
        tracing::info!(
            tpsl = %tpsl,
            limit_px = %order.limit_px,
            sz = %order.sz,
            is_buy = %order.is_buy,
            reduce_only = %order.reduce_only,
            order_type = ?order.order_type,
            "{} trigger order details", tpsl
        );

        match exchange.place_order(&order).await {
            Ok(response) => match response.into_result() {
                Ok(resp) => {
                    let statuses = resp.data.as_ref()
                        .map(|d| d.statuses.as_slice()).unwrap_or(&[]);
                    tracing::info!(
                        tpsl = %tpsl,
                        statuses = ?statuses,
                        "{} trigger response statuses", tpsl
                    );
                    // Try OID from response first
                    if let Some(oid) = extract_order_id(statuses) {
                        return Some(oid.to_string());
                    }
                    // CLOID fallback for WaitingForTrigger
                    if let Some(cloid_uuid) = cloid {
                        if let Ok(oid) = self.find_oid_by_cloid(auth, cloid_uuid).await {
                            return Some(oid.to_string());
                        }
                        return Some(format!("cloid:{:032x}", cloid_uuid.as_u128()));
                    }
                    tracing::warn!("{} trigger placed but no OID returned", tpsl);
                    None
                }
                Err(e) => { tracing::warn!("{} trigger rejected: {}", tpsl, e); None }
            },
            Err(e) => { tracing::warn!("{} trigger failed: {}", tpsl, e); None }
        }
    }

    /// Query frontend open orders to find an OID by CLOID.
    /// Used as fallback when place_order response doesn't include OID (trigger orders).
    async fn find_oid_by_cloid(
        &self,
        auth: &HyperliquidAuth,
        cloid: Uuid,
    ) -> Result<u64, ExchangeApiError> {
        let cloid_hex = format!("0x{:032x}", cloid.as_u128());

        let orders = self
            .info
            .frontend_open_orders(auth.query_address())
            .await
            .map_err(|e| {
                ExchangeApiError::Exchange(format!("Failed to fetch open orders: {}", e))
            })?;

        for order in &orders {
            if let Some(ref c) = order.cloid {
                // Match with or without 0x prefix for compatibility
                if *c == cloid_hex || format!("0x{}", c) == cloid_hex || *c == cloid_hex[2..] {
                    return Ok(order.oid);
                }
            }
        }

        Err(ExchangeApiError::Exchange(format!(
            "Order with CLOID {} not found in open orders",
            cloid_hex
        )))
    }
}

/// Parse a string order ID to u64.
fn parse_oid(order_id: &str) -> Result<u64, ExchangeApiError> {
    order_id
        .parse::<u64>()
        .map_err(|e| ExchangeApiError::Internal(format!("Invalid order ID '{}': {}", order_id, e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperliquid_sdk_rs::types::{FilledOrder, RestingOrder};
    use rust_decimal_macros::dec;

    // ==================== CLOID Tests ====================

    #[test]
    fn cloid_is_deterministic() {
        let id1 = generate_cloid("testudo:550e8400-e29b-41d4-a716-446655440000:entry");
        let id2 = generate_cloid("testudo:550e8400-e29b-41d4-a716-446655440000:entry");
        assert_eq!(id1, id2);
    }

    #[test]
    fn cloid_differs_by_role() {
        let entry = generate_cloid("testudo:550e8400-e29b-41d4-a716-446655440000:entry");
        let sl = generate_cloid("testudo:550e8400-e29b-41d4-a716-446655440000:sl");
        let tp = generate_cloid("testudo:550e8400-e29b-41d4-a716-446655440000:tp");
        assert_ne!(entry, sl);
        assert_ne!(entry, tp);
        assert_ne!(sl, tp);
    }

    #[test]
    fn cloid_differs_by_group() {
        let g1 = generate_cloid("testudo:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa:entry");
        let g2 = generate_cloid("testudo:bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb:entry");
        assert_ne!(g1, g2);
    }

    #[test]
    fn cloid_is_valid_uuid() {
        let cloid = generate_cloid("testudo:test:entry");
        // UUID v5 has version nibble = 5
        assert_eq!(cloid.get_version_num(), 5);
    }

    // ==================== Order Building Tests ====================

    #[test]
    fn build_limit_order() {
        let req = PlaceOrderRequest {
            user_id: Uuid::new_v4(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide::Buy,
            order_type: ApiOrderType::Limit,
            quantity: dec!(0.12345),
            price: Some(dec!(65000.5)),
            stop_price: None,
            leverage: 10,
            exchange_account_id: None,
            reduce_only: false,
            client_order_id: Some("testudo:test-id:entry".to_string()),
            stop_loss_trigger: None,
            take_profit_trigger: None,
        };

        let order = build_order_request(0, &req, 5).unwrap();
        assert_eq!(order.asset, 0);
        assert!(order.is_buy);
        assert_eq!(order.limit_px, "65000.5");
        assert_eq!(order.sz, "0.12345");
        assert!(!order.reduce_only);
        assert!(order.cloid.is_some());
        assert!(matches!(order.order_type, HlOrderType::Limit(Limit { ref tif }) if tif == "Gtc"));
    }

    #[test]
    fn build_market_order() {
        let req = PlaceOrderRequest {
            user_id: Uuid::new_v4(),
            symbol: "ETH_USDT".to_string(),
            side: OrderSide::Sell,
            order_type: ApiOrderType::Market,
            quantity: dec!(1.5),
            price: None,
            stop_price: None,
            leverage: 5,
            exchange_account_id: None,
            reduce_only: false,
            client_order_id: None,
            stop_loss_trigger: None,
            take_profit_trigger: None,
        };

        let order = build_order_request(1, &req, 4).unwrap();
        assert_eq!(order.asset, 1);
        assert!(!order.is_buy); // Sell
        assert_eq!(order.sz, "1.5");
        assert!(order.cloid.is_none());
        // Market = IOC limit
        assert!(matches!(order.order_type, HlOrderType::Limit(Limit { ref tif }) if tif == "Ioc"));
    }

    #[test]
    fn build_stop_loss_order() {
        let req = PlaceOrderRequest {
            user_id: Uuid::new_v4(),
            symbol: "SOL_USDT".to_string(),
            side: OrderSide::Sell,
            order_type: ApiOrderType::StopLoss,
            quantity: dec!(10),
            price: None,
            stop_price: Some(dec!(120.50)),
            leverage: 3,
            exchange_account_id: None,
            reduce_only: true,
            client_order_id: Some("testudo:test-id:sl".to_string()),
            stop_loss_trigger: None,
            take_profit_trigger: None,
        };

        let order = build_order_request(5, &req, 2).unwrap();
        assert_eq!(order.asset, 5);
        assert!(!order.is_buy); // Sell side
        assert_eq!(order.sz, "10");
        assert!(order.reduce_only); // SL is always reduce-only
        assert!(order.cloid.is_some());
        match &order.order_type {
            HlOrderType::Trigger(t) => {
                assert!(t.is_market);
                assert_eq!(t.trigger_px, "120.50");
                assert_eq!(t.tpsl, "sl");
            }
            _ => panic!("Expected trigger order type"),
        }
    }

    #[test]
    fn build_limit_order_without_price_fails() {
        let req = PlaceOrderRequest {
            user_id: Uuid::new_v4(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide::Buy,
            order_type: ApiOrderType::Limit,
            quantity: dec!(0.01),
            price: None, // Missing!
            stop_price: None,
            leverage: 1,
            exchange_account_id: None,
            reduce_only: false,
            client_order_id: None,
            stop_loss_trigger: None,
            take_profit_trigger: None,
        };

        let err = build_order_request(0, &req, 5).unwrap_err();
        assert!(matches!(err, ExchangeApiError::Internal(_)));
    }

    #[test]
    fn build_stop_loss_without_stop_price_fails() {
        let req = PlaceOrderRequest {
            user_id: Uuid::new_v4(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide::Sell,
            order_type: ApiOrderType::StopLoss,
            quantity: dec!(0.01),
            price: None,
            stop_price: None, // Missing!
            leverage: 1,
            exchange_account_id: None,
            reduce_only: true,
            client_order_id: None,
            stop_loss_trigger: None,
            take_profit_trigger: None,
        };

        let err = build_order_request(0, &req, 5).unwrap_err();
        assert!(matches!(err, ExchangeApiError::Internal(_)));
    }

    // ==================== Format Tests ====================

    #[test]
    fn format_sz_respects_decimals() {
        assert_eq!(format_sz(dec!(0.123456789), 5), "0.12346"); // rounds
        assert_eq!(format_sz(dec!(1.5), 2), "1.5");
        assert_eq!(format_sz(dec!(100), 0), "100");
        assert_eq!(format_sz(dec!(0.1), 8), "0.1");
    }

    // ==================== Response Extraction Tests ====================

    #[test]
    fn extract_oid_from_resting() {
        let statuses = vec![ExchangeDataStatus::Resting(RestingOrder { oid: 42 })];
        assert_eq!(extract_order_id(&statuses), Some(42));
    }

    #[test]
    fn extract_oid_from_filled() {
        let statuses = vec![ExchangeDataStatus::Filled(FilledOrder {
            total_sz: "0.5".to_string(),
            avg_px: "65000.0".to_string(),
            oid: 123,
        })];
        assert_eq!(extract_order_id(&statuses), Some(123));
    }

    #[test]
    fn extract_oid_from_waiting_for_trigger_returns_none() {
        let statuses = vec![ExchangeDataStatus::WaitingForTrigger];
        assert_eq!(extract_order_id(&statuses), None);
    }

    #[test]
    fn extract_avg_price_from_filled() {
        let statuses = vec![ExchangeDataStatus::Filled(FilledOrder {
            total_sz: "0.5".to_string(),
            avg_px: "65432.10".to_string(),
            oid: 1,
        })];
        assert_eq!(extract_avg_price(&statuses), Some(dec!(65432.10)));
    }

    #[test]
    fn extract_avg_price_from_resting_returns_none() {
        let statuses = vec![ExchangeDataStatus::Resting(RestingOrder { oid: 1 })];
        assert_eq!(extract_avg_price(&statuses), None);
    }

    // ==================== Parse OID Tests ====================

    #[test]
    fn parse_oid_valid() {
        assert_eq!(parse_oid("12345").unwrap(), 12345u64);
        assert_eq!(parse_oid("0").unwrap(), 0u64);
        assert_eq!(parse_oid("18446744073709551615").unwrap(), u64::MAX);
    }

    #[test]
    fn parse_oid_invalid() {
        assert!(parse_oid("not_a_number").is_err());
        assert!(parse_oid("").is_err());
        assert!(parse_oid("-1").is_err());
    }

    // ==================== HL-09: TakeProfit Order Tests ====================

    #[test]
    fn build_take_profit_order() {
        let req = PlaceOrderRequest {
            user_id: Uuid::new_v4(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide::Sell,
            order_type: ApiOrderType::TakeProfit,
            quantity: dec!(0.5),
            price: None,
            stop_price: Some(dec!(70000)),
            leverage: 3,
            exchange_account_id: None,
            reduce_only: true,
            client_order_id: Some("testudo:test-id:tp".to_string()),
            stop_loss_trigger: None,
            take_profit_trigger: None,
        };

        let order = build_order_request(0, &req, 5).unwrap();
        assert!(!order.is_buy); // Sell side
        assert_eq!(order.sz, "0.5");
        assert!(order.reduce_only);
        assert!(order.cloid.is_some());
        match &order.order_type {
            HlOrderType::Trigger(t) => {
                assert!(t.is_market);
                assert_eq!(t.trigger_px, "70000");
                assert_eq!(t.tpsl, "tp");
            }
            _ => panic!("Expected trigger order type"),
        }
    }

    #[test]
    fn build_take_profit_without_stop_price_fails() {
        let req = PlaceOrderRequest {
            user_id: Uuid::new_v4(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide::Sell,
            order_type: ApiOrderType::TakeProfit,
            quantity: dec!(0.01),
            price: None,
            stop_price: None, // Missing!
            leverage: 1,
            exchange_account_id: None,
            reduce_only: true,
            client_order_id: None,
            stop_loss_trigger: None,
            take_profit_trigger: None,
        };

        let err = build_order_request(0, &req, 5).unwrap_err();
        assert!(matches!(err, ExchangeApiError::Internal(_)));
    }

    // ==================== HL-09: Success Status Tests ====================

    #[test]
    fn extract_oid_from_success_returns_none() {
        let statuses = vec![ExchangeDataStatus::Success];
        assert_eq!(extract_order_id(&statuses), None);
    }

    // ==================== HL-09: SL/TP CLOID Tests ====================

    #[test]
    fn cloid_sl_tp_suffixes_are_unique() {
        let base = "testudo:550e8400-e29b-41d4-a716-446655440000:entry";
        let sl_cloid = generate_cloid(&format!("{}:sl", base));
        let tp_cloid = generate_cloid(&format!("{}:tp", base));
        assert_ne!(sl_cloid, tp_cloid);
    }

    #[test]
    fn cloid_sl_tp_are_deterministic() {
        let base = "testudo:test-group:entry";
        let sl1 = generate_cloid(&format!("{}:sl", base));
        let sl2 = generate_cloid(&format!("{}:sl", base));
        assert_eq!(sl1, sl2);
    }

    // ==================== HL-11: Status Normalization Tests ====================

    #[test]
    fn status_filled_maps_to_closed() {
        let status = ExchangeDataStatus::Filled(FilledOrder {
            total_sz: "1.0".to_string(),
            avg_px: "50000.0".to_string(),
            oid: 99,
        });
        assert_eq!(normalize_status(&status), "closed");
    }

    #[test]
    fn status_success_maps_to_closed() {
        assert_eq!(normalize_status(&ExchangeDataStatus::Success), "closed");
    }

    #[test]
    fn status_resting_maps_to_open() {
        let status = ExchangeDataStatus::Resting(RestingOrder { oid: 1 });
        assert_eq!(normalize_status(&status), "open");
    }

    #[test]
    fn status_waiting_for_trigger_maps_to_open() {
        assert_eq!(normalize_status(&ExchangeDataStatus::WaitingForTrigger), "open");
    }

    #[test]
    fn status_waiting_for_fill_maps_to_open() {
        assert_eq!(normalize_status(&ExchangeDataStatus::WaitingForFill), "open");
    }

    #[test]
    fn status_error_maps_to_error_prefix() {
        let status = ExchangeDataStatus::Error("insufficient margin".to_string());
        assert_eq!(normalize_status(&status), "error:insufficient margin");
    }
}
