//! HL-03: HyperliquidExchangeApi — ExchangeApi trait implementation
//!
//! Native Rust SDK integration that slots in alongside `ShadowExchangeApi`
//! and `CexExchangeApi`. Bypasses the Node.js sidecar entirely.

// @anchor exchange:router:exchange_api
// @tags api

use alloy<IP_ADDRESS>signers<IP_ADDRESS>local<IP_ADDRESS>PrivateKeySigner;
use async_trait<IP_ADDRESS>async_trait;
use hyperliquid_sdk_rs<IP_ADDRESS>{
    types<IP_ADDRESS>{
        CancelRequest as HlCancelRequest, ExchangeDataStatus, <LOCATION>, OrderRequest as HlOrderRequest,
        OrderType as HlOrderType, Trigger,
    },
    ExchangeProvider, InfoProvider, Network,
};
use rust_decimal<IP_ADDRESS>Decimal;
use std<IP_ADDRESS>str<IP_ADDRESS><PERSON>;
use std<IP_ADDRESS>sync<IP_ADDRESS>Arc;
use uuid<IP_ADDRESS>Uuid;

use super<IP_ADDRESS>auth<IP_ADDRESS>{AuthCache, AuthError, AuthMode, HyperliquidAuth};
use super<IP_ADDRESS>universe<IP_ADDRESS>AssetUniverse;
use crate<IP_ADDRESS>repositories<IP_ADDRESS>exchange_account<IP_ADDRESS>ExchangeAccountRepository;
use crate<IP_ADDRESS>types<IP_ADDRESS>exchange_names<IP_ADDRESS>{auth_modes, exchanges};
use crate<IP_ADDRESS>services<IP_ADDRESS>exchange_api<IP_ADDRESS>{
    AmendRequest, ApiOrderType, ExchangeApi, ExchangeApiError, OrderSide, PlaceOrderRequest,
    PlaceOrderResult, <LOCATION>,
};

/// Namespace UUID for deterministic CLOID generation (UUID <US_DRIVER_LICENSE>).
/// Uses the DNS namespace as a base — the input string is always
/// `testudo:{group_id}:{role}` so collisions are impossible.
const CLOID_NAMESPACE: Uuid = <PERSON>;

/// Native Hyperliquid exchange API implementation.
///
/// Implements the `ExchangeApi` trait using the Hyperliquid Rust SDK directly,
/// bypassing the <LOCATION> sidecar. Credentials are loaded per-request from the
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
        let info = InfoProvider<IP_ADDRESS>new(network);
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
        user_id: <PERSON>,
        exchange_account_id: Option<Uuid>,
    ) -> Result<HyperliquidAuth, ExchangeApiError> {
        let accounts = self
            .account_repo
            .list_by_user(user_id)
            .await
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Internal(format!("Failed to list accounts: {}", e)))?;

        // Find the Hyperliquid account
        let account = if let Some(target_id) = exchange_account_id {
            accounts
                .iter()
                .find(|a| <URL> == target_id)
                .ok_or_else(|| {
                    ExchangeApiError<IP_ADDRESS>Internal(format!("Exchange account {} not found", target_id))
                })?
        } else {
            accounts
                .iter()
                .find(|a| <URL>_name.eq_ignore_ascii_case(exchanges<IP_ADDRESS>HYPERLIQUID))
                .or_else(|| <URL>rst())
                .ok_or_else(|| {
                    ExchangeApiError<IP_ADDRESS>Internal("No exchange account configured".into())
                })?
        };

        // UXA-01: Return specific error for inactive agent wallets instead of generic NotFound
        if <URL>th_mode == auth_modes<IP_ADDRESS>AGENT_WALLET && !<URL>_active.unwrap_or(false) {
            return Err(ExchangeApiError<IP_ADDRESS>AgentWalletInactive { account_id: <URL> });
        }

        let creds = self
            .account_repo
            .load_credentials(<URL>, user_id)
            .await
            .map_err(|e| {
                ExchangeApiError<IP_ADDRESS>Internal(format!("Failed to load credentials: {}", e))
            })?;

        match <URL>th_<URL>_str() {
            auth_modes<IP_ADDRESS>AGENT_WALLET => {
                let wallet_addr = creds.wallet_address.ok_or_else(|| {
                    ExchangeApiError<IP_ADDRESS>Internal(
                        AuthError<IP_ADDRESS><URL>_string(),
                    )
                })?;
                <URL>th_cache
                    .get_or_insert_agent(<URL>, &creds.api_secret, &wallet_addr)
                    .await
                    .map_err(|e| ExchangeApiError<IP_ADDRESS>Internal(format!("Auth failed: {}", e)))
            }
            _ => {
                <URL>th_cache
                    .get_or_insert(<URL>, &creds.api_key, &creds.api_secret)
                    .await
                    .map_err(|e| ExchangeApiError<IP_ADDRESS>Internal(format!("Auth failed: {}", e)))
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
        match <URL>work {
            Network<IP_ADDRESS>Mainnet => ExchangeProvider<IP_ADDRESS>mainnet(<URL>one()),
            Network<IP_ADDRESS>Testnet => ExchangeProvider<IP_ADDRESS>testnet(<URL>one()),
        }
    }

    /// Transfer USDC between spot and perp accounts.
    /// `to_perp`: true = spot→perp, false = perp→spot.
    pub async fn transfer_usdc(
        &self,
        user_id: <PERSON>,
        account_id: <PERSON>,
        amount: &str,
        to_perp: bool,
    ) -> Result<bool, ExchangeApiError> {
        let auth = self.load_auth(user_id, Some(account_id)).await?;
        let exchange = self.build_exchange(&auth);

        if to_perp {
            // Spot→Perp: usd_class_transfer via EIP-712 user-signed action.
            // This uses send_user_action (not send_l1_action) so chain fields +
            // nonce are properly included in the signed payload.
            let status = exchange
                .usd_class_transfer(amount, true)
                .await
                .map_err(|e| ExchangeApiError::Internal(format!("Transfer failed: {}", e)))?;
            let ok = status.is_ok();
            tracing::info!("HL spot→perp: amount={} ok={}", amount, ok);
            Ok(ok)
        } else {
            let status = exchange
                .usd_transfer(auth.signer.address(), amount)
                .await
                .map_err(|e| ExchangeApiError::Internal(format!("Transfer failed: {}", e)))?;
            let ok = status.is_ok();
            tracing::info!("HL perp→spot: ok={}", ok);
            Ok(ok)
        }
    }
}

/// Generate a deterministic CLOID (UUID <US_DRIVER_LICENSE>) from a client order ID string.
///
/// Input: `"testudo:{group_id}:{role}"` or any string.
/// Output: deterministic UUID suitable for Hyperliquid CLOID.
pub fn generate_cloid(client_order_id: &str) -> Uuid {
    Uuid<IP_ADDRESS>new_v5(&CLOID_NAMESPACE, client_order_<URL>_bytes())
}

/// Build a Hyperliquid `OrderRequest` from a `PlaceOrderRequest`.
///
/// Handles market, limit, and stop-loss order type mapping.
/// Formats quantity to the correct number of decimal places using `sz_decimals`.
pub fn build_order_request(
    asset_index: <US_DRIVER_LICENSE>,
    req: &PlaceOrderRequest,
    sz_decimals: <US_DRIVER_LICENSE>,
) -> Result<HlOrderRequest, ExchangeApiError> {
    let is_buy = matches!(<URL>de, OrderSide<IP_ADDRESS>Buy);

    // Format size with correct decimal precision
    let sz = format_sz(req.quantity, sz_decimals);

    let cloid = req
        .client_order_id
        .as_ref()
        .map(|coid| generate_cloid(coid));

    let order = match req.order_type {
        ApiOrderType<IP_ADDRESS>Market => {
            // Market = aggressive IOC limit at a very unfavorable price
            // Hyperliquid doesn't have native market orders; use IOC limit.
            let slippage_price = if is_buy {
                // Buy: set limit price very high to ensure fill
                <URL>ice
                    .unwrap_or_else(|| Decimal<IP_ADDRESS>new(999_999_999, 0))
                    .to_string()
            } else {
                // Sell: set limit price very low to ensure fill
                <URL>ice
                    .unwrap_or_else(|| Decimal<IP_ADDRESS>new(1, 2)) // 0.01
                    .to_string()
            };
            HlOrderRequest<IP_ADDRESS>limit(asset_index, is_buy, slippage_price, &sz, "Ioc")
                .reduce_only(<URL>uce_only)
                .with_cloid(cloid)
        }
        ApiOrderType<IP_ADDRESS>Limit => {
            let price = req
                .price
                .ok_or_else(|| ExchangeApiError<IP_ADDRESS>Internal("Limit order requires price".into()))?;
            HlOrderRequest<IP_ADDRESS>limit(asset_index, is_buy, <URL>_string(), &sz, "Gtc")
                .reduce_only(<URL>uce_only)
                .with_cloid(cloid)
        }
        ApiOrderType<IP_ADDRESS>StopLoss => {
            let trigger_px = <URL>op_price.ok_or_else(|| {
                ExchangeApiError<IP_ADDRESS>Internal("StopLoss order requires stop_price".into())
            })?;
            let mut order = HlOrderRequest<IP_ADDRESS>trigger(
                asset_index,
                is_buy,
                trigger_<URL>_string(),
                &sz,
                "sl",
                true, // is_market: stop-loss executes as market
            )
            .reduce_only(true) // SL is always reduce-only
            .with_cloid(cloid);
            <URL>mit_px = trigger_limit_px(&trigger_px, is_buy);
            order
        }
        ApiOrderType<IP_ADDRESS>TakeProfit => {
            let trigger_px = <URL>op_price.ok_or_else(|| {
                ExchangeApiError<IP_ADDRESS>Internal("TakeProfit order requires stop_price".into())
            })?;
            let mut order = HlOrderRequest<IP_ADDRESS>trigger(
                asset_index,
                is_buy,
                trigger_<URL>_string(),
                &sz,
                "tp",
                true, // is_market: take-profit executes as market
            )
            .reduce_only(true) // TP is always reduce-only
            .with_cloid(cloid);
            <URL>mit_px = trigger_limit_px(&trigger_px, is_buy);
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
    trigger_<URL>rmalize().to_string()
}

/// Format a quantity to the correct number of decimal places for Hyperliquid.
pub fn format_sz(quantity: Decimal, sz_decimals: <US_DRIVER_LICENSE>) -> String {
    let scaled = quantity
        .round_dp(sz_decimals)
        .normalize();
    <URL>_string()
}

/// Extract the order ID (OID) from a Hyperliquid exchange response.
pub fn extract_order_id(
    statuses: &[ExchangeDataStatus],
) -> Option<<US_DRIVER_LICENSE>> {
    for status in statuses {
        match status {
            ExchangeDataStatus<IP_ADDRESS>Resting(r) => return Some(r.oid),
            ExchangeDataStatus<IP_ADDRESS>Filled(f) => return Some(f.oid),
            _ => {}
        }
    }
    None
}

/// HL-11 FR-1: Normalize ExchangeDataStatus to CCXT-compatible status string.
/// "closed" = filled/done, "open" = resting/waiting, "error:..." = rejected.
pub fn normalize_status(status: &ExchangeDataStatus) -> String {
    match status {
        ExchangeDataStatus<IP_ADDRESS>Filled(_) => "closed".to_string(),
        ExchangeDataStatus<IP_ADDRESS>Success => "closed".to_string(),
        ExchangeDataStatus<IP_ADDRESS>Resting(_) => "open".to_string(),
        ExchangeDataStatus<IP_ADDRESS>WaitingForTrigger => "open".to_string(),
        ExchangeDataStatus<IP_ADDRESS>WaitingForFill => "open".to_string(),
        ExchangeDataStatus<IP_ADDRESS>Error(msg) => format!("error:{}", msg),
    }
}

/// Extract the average fill price from a response (if filled).
pub fn extract_avg_price(statuses: &[ExchangeDataStatus]) -> Option<Decimal> {
    for status in statuses {
        if let ExchangeDataStatus<IP_ADDRESS>Filled(f) = status {
            return Decimal<IP_ADDRESS>from_str(&f.avg_px).ok();
        }
    }
    None
}

/// FIX-01: Parse a decimal from a string, returning an error on failure.
fn parse_decimal(s: &str) -> Result<Decimal, ExchangeApiError> {
    Decimal<IP_ADDRESS>from_str(s).map_err(|e| {
        ExchangeApiError<IP_ADDRESS>Exchange(format!("Failed to parse decimal '{}': {}", s, e))
    })
}

#[async_trait]
impl ExchangeApi for HyperliquidExchangeApi {
    async fn get_balance(
        &self,
        user_id: <PERSON>,
        _asset: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Decimal, ExchangeApiError> {
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let state = self
            .info
            .user_state(auth.query_address())
            .await
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(format!("Failed to fetch user state: {}", e)))?;

        let account_value = parse_decimal(&<URL>rgin_<URL>count_value)?;
        Ok(account_value)
    }

    async fn place_order(
        &self,
        mut req: PlaceOrderRequest,
    ) -> Result<PlaceOrderResult, ExchangeApiError> {
        let auth = self
            .load_auth(<URL>er_id, <URL>_account_id)
            .await?;
        let exchange = <PERSON>);

        // Resolve symbol to HL coin and asset index
        let coin = AssetUniverse<IP_ADDRESS>to_hl_coin(&<URL>mbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(<URL>_string()))?;
        let sz_decimals = self
            .universe
            .sz_decimals(coin)
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(<URL>_string()))?;

        // HL-11: For market orders without a price, query mid price and apply
        // 10% slippage band. HL rejects extreme prices (e.g. 0.01 for BTC).
        if req.order_type == ApiOrderType<IP_ADDRESS>Market && <URL>_none() {
            if let Ok(mids) = <URL>l_mids().await {
                if let Some(mid_str) = <URL>t(coin) {
                    if let <LOCATION>) = Decimal<IP_ADDRESS>from_str(mid_str) {
                        let slippage = mid * Decimal<IP_ADDRESS>new(10, 2); // 10%
                        let is_buy = matches!(<URL>de, OrderSide<IP_ADDRESS>Buy);
                        <URL>ice = Some(if is_buy { mid + slippage } else { mid - slippage });
                        tracing<IP_ADDRESS>debug!(
                            coin = %coin,
                            mid = %<DATE_TIME> = ?<URL>ice,
                            is_buy = %is_buy,
                            "HL market order: using mid price with 10% slippage"
                        );
                    }
                }
            }
        }

        let mut hl_order = build_order_request(asset_index, &req, sz_decimals)?;
        let cloid = <URL>ient_order_<URL>_ref().map(|c| generate_cloid(c));

        // Fix CLOID format: Hyperliquid API requires "0x" prefix + 32 hex chars.
        // The SDK's with_cloid() formats without prefix, so we override here.
        // Must call place_order() directly (not place_order_with_cloid which re-formats).
        if let <LOCATION> cloid_str) = hl_<URL>oid {
            if !cloid_<URL>arts_with("0x") {
                hl_<URL>oid = Some(format!("0x{}", cloid_str));
            }
        }

        tracing<IP_ADDRESS>info!(
            coin = %coin,
            asset_index = %asset_index,
            is_buy = %hl_<URL>_buy,
            limit_px = %hl_<URL>mit_px,
            sz = %hl_<URL>,
            reduce_only = %hl_<URL>uce_only,
            order_type = ?hl_order.order_type,
            cloid = ?hl_<URL>oid,
            auth_mode = ?<URL>th_mode,
            query_address = %auth.query_address(),
            "HyperliquidExchangeApi: placing order"
        );

        let response = exchange
            .place_order(&hl_order)
            .await
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(format!("Place order failed: {}", e)))?;

        let resp = response
            .into_result()
            .map_err(ExchangeApiError<IP_ADDRESS>Exchange)?;

        let statuses = resp
            .data
            .as_ref()
            .map(|d| <URL>_slice())
            .unwrap_or(&[]);

        // Extract OID from response
        let order_id = if let Some(oid) = extract_order_id(statuses) {
            <URL>_string()
        } else if let Some(cloid_uuid) = cloid {
            // For trigger orders (WaitingForTrigger), query open orders to find OID
            match <URL>nd_oid_by_cloid(&auth, cloid_uuid).await {
                Ok(oid) => <URL>_string(),
                Err(_) => {
                    // Fallback: return CLOID hex as ID
                    format!("cloid:{:032x}", cloid_<URL>_u128())
                }
            }
        } else {
            // FR-7: No OID extracted and no CLOID — check for errors in statuses.
            // into_result() already confirmed the response envelope was OK,
            // so if there's no Error status, the exchange accepted the order
            // (e.g. atomically-filled market close, WaitingForFill, Success).
            let error_msg = <URL>er().find_map(|s| {
                if let ExchangeDataStatus<IP_ADDRESS>Error(msg) = s {
                    Some(<URL>one())
                } else {
                    None
                }
            });
            if let <PERSON>) = error_msg {
                return Err(ExchangeApiError<IP_ADDRESS>Exchange(msg));
            }
            "success".to_string()
        };

        let avg_price = <PERSON>);
        // HL-11 FR-1: Normalize ExchangeDataStatus to CCXT-compatible strings
        // so downstream `is_filled` check (== "closed") works for immediate fills.
        let status = <URL>rst().map(normalize_status);

        // HL-09 FR-1/FR-2: Place SL/TP as separate trigger orders after entry
        let close_is_buy = !matches!(<URL>de, OrderSide<IP_ADDRESS>Buy);
        let sz = format_sz(req.quantity, sz_decimals);
        let mut sl_order_id = None;
        let mut tp_order_id = None;

        if let Some(sl_trigger) = <URL>op_loss_trigger {
            sl_order_id = <URL>_trigger_order(
                &exchange, &auth, asset_index, close_is_buy,
                sl_trigger, &sz, "sl", <URL>ient_order_<URL>_deref(),
            ).await;
        }

        if let Some(tp_trigger) = req.take_profit_trigger {
            tp_order_id = <URL>_trigger_order(
                &exchange, &auth, asset_index, close_is_buy,
                tp_trigger, &sz, "tp", <URL>ient_order_<URL>_deref(),
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
        user_id: <PERSON>,
        order_id: &str,
        symbol: &str,
        amend: AmendRequest,
        exchange_account_id: Option<Uuid>,
    ) -> Result<String, ExchangeApiError> {
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let exchange = <PERSON>);

        let oid: <US_DRIVER_LICENSE> = parse_oid(order_id)?;

        let coin = AssetUniverse<IP_ADDRESS>to_hl_coin(symbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(<URL>_string()))?;
        let sz_decimals = self
            .universe
            .sz_decimals(coin)
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(<URL>_string()))?;

        // FIX-01: Amend safety — side must be specified
        let is_buy = match <URL>de {
            Some(OrderSide<IP_ADDRESS>Buy) => true,
            Some(OrderSide<IP_ADDRESS>Sell) => false,
            None => {
                return Err(ExchangeApiError<IP_ADDRESS>Internal(
                    "Amend requires side".into(),
                ));
            }
        };

        // FIX-01: Amend safety — quantity must be non-zero
        let quantity = <URL>_quantity.or(amend.quantity).unwrap_or(Decimal<IP_ADDRESS>ZERO);
        if quantity == Decimal<IP_ADDRESS>ZERO {
            return Err(ExchangeApiError<IP_ADDRESS>Internal(
                "Amend requires non-zero quantity".into(),
            ));
        }
        let sz = format_sz(quantity, sz_decimals);

        let new_order = match amend.order_type {
            Some(ApiOrderType<IP_ADDRESS>StopLoss) => {
                let trigger_px = <URL>_stop_price.ok_or_else(|| {
                    ExchangeApiError<IP_ADDRESS>Internal("StopLoss amend requires stop_price".into())
                })?;
                let mut order = HlOrderRequest<IP_ADDRESS>trigger(
                    asset_index,
                    is_buy,
                    trigger_<URL>_string(),
                    &sz,
                    "sl",
                    true,
                )
                .reduce_only(true);
                <URL>mit_px = trigger_limit_px(&trigger_px, is_buy);
                order
            }
            Some(ApiOrderType<IP_ADDRESS>TakeProfit) => {
                let trigger_px = <URL>_stop_price.ok_or_else(|| {
                    ExchangeApiError<IP_ADDRESS>Internal("TakeProfit amend requires stop_price".into())
                })?;
                let mut order = HlOrderRequest<IP_ADDRESS>trigger(
                    asset_index,
                    is_buy,
                    trigger_<URL>_string(),
                    &sz,
                    "tp",
                    true,
                )
                .reduce_only(true);
                <URL>mit_px = trigger_limit_px(&trigger_px, is_buy);
                order
            }
            Some(ApiOrderType<IP_ADDRESS>Market) => {
                let price = if is_buy { "<US_ITIN>" } else { "0.01" };
                HlOrderRequest<IP_ADDRESS>limit(asset_index, is_buy, price, &sz, "Ioc")
                    .reduce_only(<URL>uce_only)
            }
            Some(ApiOrderType<IP_ADDRESS>Limit) | None => {
                let price = <URL>_price.ok_or_else(|| {
                    ExchangeApiError<IP_ADDRESS>Internal("Limit amend requires price".into())
                })?;
                HlOrderRequest<IP_ADDRESS>limit(asset_index, is_buy, <URL>_string(), &sz, "Gtc")
                    .reduce_only(<URL>uce_only)
            }
        };

        let response = exchange
            .modify_order(oid, new_order)
            .await
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(format!("Modify order failed: {}", e)))?;

        let resp = response
            .into_result()
            .map_err(ExchangeApiError<IP_ADDRESS>Exchange)?;

        let statuses = resp
            .data
            .as_ref()
            .map(|d| <URL>_slice())
            .unwrap_or(&[]);

        // Modified orders keep the same OID on Hyperliquid
        let new_oid = extract_order_id(statuses).unwrap_or(oid);

        tracing<IP_ADDRESS>info!(
            old_oid = %oid,
            new_oid = %new_oid,
            symbol = %symbol,
            "HyperliquidExchangeApi: modify_order completed"
        );

        Ok(new_<URL>_string())
    }

    async fn cancel_order(
        &self,
        user_id: <PERSON>,
        order_id: &str,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let exchange = <PERSON>);

        let coin = AssetUniverse<IP_ADDRESS>to_hl_coin(symbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(<URL>_string()))?;

        // Handle both numeric OID and CLOID-based IDs
        if let <NRP>) = order_<URL>rip_prefix("cloid:") {
            let cloid = <PERSON><IP_ADDRESS>from_u128(
                <US_DRIVER_LICENSE><IP_ADDRESS>from_str_radix(cloid_hex, 16).map_err(|e| {
                    ExchangeApiError<IP_ADDRESS>Internal(format!("Invalid CLOID: {}", e))
                })?,
            );
            exchange
                .cancel_order_by_cloid(asset_index, cloid)
                .await
                .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(format!("Cancel failed: {}", e)))?
                .into_result()
                .map_err(ExchangeApiError<IP_ADDRESS>Exchange)?;
        } else {
            let oid: <US_DRIVER_LICENSE> = parse_oid(order_id)?;
            exchange
                .cancel_order(asset_index, oid)
                .await
                .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(format!("Cancel failed: {}", e)))?
                .into_result()
                .map_err(ExchangeApiError<IP_ADDRESS>Exchange)?;
        }

        Ok(())
    }

    async fn cancel_all_orders(
        &self,
        user_id: <PERSON>,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let exchange = <PERSON>);

        let coin = AssetUniverse<IP_ADDRESS>to_hl_coin(symbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(<URL>_string()))?;

        // Fetch all open orders for this user
        let open_orders = self
            .info
            .open_orders(auth.query_address())
            .await
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(format!("Failed to fetch open orders: {}", e)))?;

        // Filter to this symbol and build cancel requests
        let cancels: <PERSON><HlCancelRequest> = open_orders
            .iter()
            .filter(|o| <URL>_uppercase() == <URL>_uppercase())
            .map(|o| HlCancelRequest<IP_ADDRESS>new(asset_index, o.oid))
            .collect();

        if <URL>_empty() {
            return Ok(());
        }

        tracing<IP_ADDRESS>info!(
            count = cancels.len(),
            symbol = %symbol,
            coin = %coin,
            "HyperliquidExchangeApi: bulk cancelling orders"
        );

        exchange
            .bulk_cancel(cancels)
            .await
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(format!("Bulk cancel failed: {}", e)))?
            .into_result()
            .map_err(ExchangeApiError<IP_ADDRESS>Exchange)?;

        Ok(())
    }

    async fn get_position(
        &self,
        user_id: <PERSON>,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Option<PositionInfo>, ExchangeApiError> {
        let auth = self.load_auth(user_id, exchange_account_id).await?;
        let state = self
            .info
            .user_state(auth.query_address())
            .await
            .map_err(|e| ExchangeApiError<IP_ADDRESS>Exchange(format!("Failed to fetch user state: {}", e)))?;

        let coin = AssetUniverse<IP_ADDRESS>to_hl_coin(symbol);

        let pos = <PERSON>
            <URL>_uppercase() == <URL>_uppercase()
        });

        let position_info = match pos {
            Some(ap) => {
                let szi = parse_decimal(&<URL>i)?;
                if szi == Decimal<IP_ADDRESS>ZERO {
                    None
                } else {
                    let side = if szi > Decimal<IP_ADDRESS>ZERO {
                        "long".to_string()
                    } else {
                        "short".to_string()
                    };

                    let entry_price = match <PERSON>() {
                        Some(s) => parse_decimal(s)?,
                        None => Decimal<IP_ADDRESS>ZERO,
                    };

                    Some(PositionInfo {
                        symbol: <URL>_string(),
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
        asset_index: <US_DRIVER_LICENSE>,
        close_is_buy: bool,
        trigger_px: Decimal,
        sz: &str,
        <PERSON>: &str,
        client_order_id_base: Option<&str>,
    ) -> Option<String> {
        // Generate CLOID with ":sl" or ":tp" suffix for tracking
        let cloid = client_order_id_base
            .map(|base| generate_cloid(&format!("{}:{}", base, <LOCATION>)));

        let mut order = HlOrderRequest<IP_ADDRESS>trigger(
            asset_index, close_is_buy, trigger_<URL>_string(), sz, <LOCATION>, true,
        )
        .reduce_only(true)
        .with_cloid(cloid);

        // Fix limit_px: SDK defaults to "0" which HL rejects as "invalid price"
        <URL>mit_px = trigger_limit_px(&trigger_px, close_is_buy);

        // Fix CLOID 0x prefix
        if let <PERSON>) = <URL>oid {
            if !<URL>arts_with("0x") {
                <URL>oid = Some(format!("0x{}", s));
            }
        }

        tracing<IP_ADDRESS>info!(
            <LOCATION> = %<LOCATION>,
            trigger_px = %trigger_px,
            close_is_buy = %close_is_buy,
            cloid = ?<URL>oid,
            "Placing {} trigger order", <PERSON>
        );

        // Log the full order for debugging
        tracing<IP_ADDRESS>info!(
            <LOCATION> = %<LOCATION>,
            limit_px = %<URL>mit_px,
            sz = %<URL>,
            is_buy = %<URL>_buy,
            reduce_only = %<URL>uce_only,
            order_type = ?order.order_type,
            "{} trigger order details", <PERSON>
        );

        match <URL>_order(&order).await {
            Ok(response) => match <URL>o_result() {
                Ok(resp) => {
                    let statuses = <URL>_ref()
                        .map(|d| <URL>_slice()).unwrap_or(&[]);
                    tracing<IP_ADDRESS>info!(
                        <LOCATION> = %<LOCATION>,
                        statuses = ?statuses,
                        "{} trigger response statuses", <LOCATION>
                    );
                    // Try OID from response first
                    if let Some(oid) = extract_order_id(statuses) {
                        return <NRP>());
                    }
                    // CLOID fallback for WaitingForTrigger
                    if let Some(cloid_uuid) = <PERSON>
                        if let Ok(oid) = <URL>nd_oid_by_cloid(auth, cloid_uuid).await {
                            return Some(<URL>_string());
                        }
                        return Some(format!("cloid:{:032x}", cloid_<URL>_u128()));
                    }
                    tracing<IP_ADDRESS>warn!("{} trigger placed but no OID returned", <LOCATION>);
                    None
                }
                Err(e) => { tracing<IP_ADDRESS>warn!("{} trigger rejected: {}", <PERSON>, e); None }
            },
            Err(e) => { tracing<IP_ADDRESS>warn!("{} trigger failed: {}", <PERSON>, e); None }
        }
    }

    /// Query frontend open orders to find an OID by CLOID.
    /// Used as fallback when place_order response doesn't include OID (trigger orders).
    async fn find_oid_by_cloid(
        &self,
        auth: &HyperliquidAuth,
        cloid: Uuid,
    ) -> Result<<US_DRIVER_LICENSE>, ExchangeApiError> {
        let cloid_hex = format!("0x{:032x}", <URL>_u128());

        let orders = self
            .info
            .frontend_open_orders(auth.query_address())
            .await
            .map_err(|e| {
                ExchangeApiError<IP_ADDRESS>Exchange(format!("Failed to fetch open orders: {}", e))
            })?;

        for order in &orders {
            if let <LOCATION> c) = <URL>oid {
                // Match with or without 0x prefix for compatibility
                if *c == cloid_hex || format!("0x{}", c) == cloid_hex || *c == cloid_hex[2..] {
                    return Ok(order.oid);
                }
            }
        }

        Err(ExchangeApiError<IP_ADDRESS>Exchange(format!(
            "Order with CLOID {} not found in open orders",
            cloid_hex
        )))
    }
}

/// Parse a string order ID to <US_DRIVER_LICENSE>.
fn parse_oid(order_id: &str) -> Result<<US_DRIVER_LICENSE>, ExchangeApiError> {
    order_id
        .parse<IP_ADDRESS><<US_DRIVER_LICENSE>>()
        .map_err(|e| ExchangeApiError<IP_ADDRESS>Internal(format!("Invalid order ID '{}': {}", order_id, e)))
}

#[cfg(test)]
mod tests {
    use super<IP_ADDRESS>*;
    use hyperliquid_sdk_rs<IP_ADDRESS>types<IP_ADDRESS>{FilledOrder, RestingOrder};
    use rust_decimal_macros<IP_ADDRESS>;

    // ==================== CLOID Tests ====================

    #[test]
    fn cloid_is_deterministic() {
        let id1 = generate_cloid("testudo:550e8400-e29b-41d4-<US_DRIVER_LICENSE>-<US_BANK_NUMBER>:entry");
        let id2 = generate_cloid("testudo:550e8400-e29b-41d4-<US_DRIVER_LICENSE>-<US_BANK_NUMBER>:entry");
        assert_eq!(id1, id2);
    }

    #[test]
    fn cloid_differs_by_role() {
        let entry = generate_cloid("testudo:550e8400-e29b-41d4-<US_DRIVER_LICENSE>-<US_BANK_NUMBER>:entry");
        let sl = generate_cloid("testudo:550e8400-e29b-41d4-<US_DRIVER_LICENSE>-<US_BANK_NUMBER>:sl");
        let tp = generate_cloid("testudo:550e8400-e29b-41d4-<US_DRIVER_LICENSE>-<US_BANK_NUMBER>:tp");
        assert_ne!(entry, sl);
        assert_ne!(entry, tp);
        assert_ne!(sl, tp);
    }

    #[test]
    fn cloid_differs_by_group() {
        let <US_DRIVER_LICENSE> = generate_cloid("testudo:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa:entry");
        let <US_DRIVER_LICENSE> = generate_cloid("testudo:bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb:entry");
        assert_ne!(<US_DRIVER_LICENSE>, <US_DRIVER_LICENSE>);
    }

    #[test]
    fn cloid_is_valid_uuid() {
        let cloid = generate_cloid("testudo:test:entry");
        // UUID <US_DRIVER_LICENSE> has version nibble = 5
        assert_eq!(<URL>t_version_num(), 5);
    }

    // ==================== Order Building Tests ====================

    #[test]
    fn build_limit_order() {
        let req = PlaceOrderRequest {
            user_id: <LOCATION>(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide<IP_ADDRESS>Buy,
            order_type: ApiOrderType<IP_ADDRESS>Limit,
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

        let order = build_order_request(0, &req, <DATE_TIME>();
        assert_eq!(<URL>set, 0);
        assert!(<URL>_buy);
        assert_eq!(<URL>mit_px, "65000.5");
        assert_eq!(<URL>, "0.12345");
        assert!(!<URL>uce_only);
        assert!(<URL>_some());
        assert!(matches!(order.order_type, HlOrderType<IP_ADDRESS>Limit(Limit { ref tif }) if tif == "Gtc"));
    }

    #[test]
    fn build_market_order() {
        let req = PlaceOrderRequest {
            user_id: <LOCATION>(),
            symbol: "ETH_USDT".to_string(),
            side: OrderSide<IP_ADDRESS>Sell,
            order_type: ApiOrderType<IP_ADDRESS>Market,
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
        assert_eq!(<URL>set, 1);
        assert!(!<URL>_buy); // Sell
        assert_eq!(<URL>, "1.5");
        assert!(<URL>_none());
        // Market = IOC limit
        assert!(matches!(order.order_type, HlOrderType<IP_ADDRESS>Limit(Limit { ref tif }) if tif == "Ioc"));
    }

    #[test]
    fn build_stop_loss_order() {
        let req = PlaceOrderRequest {
            user_id: <LOCATION>(),
            symbol: "SOL_USDT".to_string(),
            side: OrderSide<IP_ADDRESS>Sell,
            order_type: ApiOrderType<IP_ADDRESS>StopLoss,
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
        assert_eq!(<URL>set, 5);
        assert!(!<URL>_buy); // Sell side
        assert_eq!(<URL>, "10");
        assert!(<URL>uce_only); // SL is always reduce-only
        assert!(<URL>_some());
        match <LOCATION> {
            HlOrderType<IP_ADDRESS>Trigger(t) => {
                assert!(<URL>_market);
                assert_eq!(<URL>igger_px, "120.50");
                assert_eq!(<URL>sl, "sl");
            }
            _ => panic!("Expected trigger order type"),
        }
    }

    #[test]
    fn build_limit_order_without_price_fails() {
        let req = PlaceOrderRequest {
            user_id: <LOCATION>(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide<IP_ADDRESS>Buy,
            order_type: ApiOrderType<IP_ADDRESS>Limit,
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
        assert!(matches!(err, ExchangeApiError<IP_ADDRESS>Internal(_)));
    }

    #[test]
    fn build_stop_loss_without_stop_price_fails() {
        let req = PlaceOrderRequest {
            user_id: <LOCATION>(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide<IP_ADDRESS>Sell,
            order_type: ApiOrderType<IP_ADDRESS>StopLoss,
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
        assert!(matches!(err, ExchangeApiError<IP_ADDRESS>Internal(_)));
    }

    // ==================== Format Tests ====================

    #[test]
    fn format_sz_respects_decimals() {
        assert_eq!(format_sz(dec!(0.<US_PASSPORT>), 5), "0.12346"); // rounds
        assert_eq!(format_sz(dec!(1.5), 2), "1.5");
        assert_eq!(format_sz(dec!(100), 0), "100");
        assert_eq!(format_sz(dec!(0.1), <PERSON>, "0.1");
    }

    // ==================== Response Extraction Tests ====================

    #[test]
    fn extract_oid_from_resting() {
        let statuses = vec![ExchangeDataStatus<IP_ADDRESS>Resting(RestingOrder { oid: 42 })];
        assert_eq!(extract_order_id(&statuses), Some(42));
    }

    #[test]
    fn extract_oid_from_filled() {
        let statuses = vec![ExchangeDataStatus<IP_ADDRESS>Filled(FilledOrder {
            total_sz: "0.5".to_string(),
            avg_px: "65000.0".to_string(),
            oid: 123,
        })];
        assert_eq!(extract_order_id(&statuses), Some(123));
    }

    #[test]
    fn extract_oid_from_waiting_for_trigger_returns_none() {
        let statuses = vec![ExchangeDataStatus<IP_ADDRESS>WaitingForTrigger];
        assert_eq!(extract_order_id(&statuses), None);
    }

    #[test]
    fn extract_avg_price_from_filled() {
        let statuses = vec![ExchangeDataStatus<IP_ADDRESS>Filled(FilledOrder {
            total_sz: "0.5".to_string(),
            avg_px: "65432.10".to_string(),
            oid: 1,
        })];
        assert_eq!(extract_avg_price(&statuses), Some(dec!(65432.10)));
    }

    #[test]
    fn extract_avg_price_from_resting_returns_none() {
        let statuses = vec![ExchangeDataStatus<IP_ADDRESS>Resting(RestingOrder { oid: 1 })];
        assert_eq!(extract_avg_price(&statuses), None);
    }

    // ==================== Parse OID Tests ====================

    #[test]
    fn parse_oid_valid() {
        <PERSON>(), <DATE_TIME>);
        assert_eq!(parse_oid("0").unwrap(), 0u64);
        assert_eq!(parse_oid("18446744073709551615").unwrap(), <US_DRIVER_LICENSE><IP_ADDRESS><PERSON>);
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
            user_id: <LOCATION>(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide<IP_ADDRESS>Sell,
            order_type: ApiOrderType<IP_ADDRESS>TakeProfit,
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

        let order = build_order_request(0, &req, <DATE_TIME>();
        assert!(!<URL>_buy); // Sell side
        assert_eq!(<URL>, "0.5");
        assert!(<URL>uce_only);
        assert!(<URL>_some());
        match <LOCATION> {
            HlOrderType<IP_ADDRESS>Trigger(t) => {
                assert!(<URL>_market);
                assert_eq!(<URL>igger_px, "70000");
                assert_eq!(<URL>sl, "tp");
            }
            _ => panic!("Expected trigger order type"),
        }
    }

    #[test]
    fn build_take_profit_without_stop_price_fails() {
        let req = PlaceOrderRequest {
            user_id: <LOCATION>(),
            symbol: "BTC_USDT".to_string(),
            side: OrderSide<IP_ADDRESS>Sell,
            order_type: ApiOrderType<IP_ADDRESS>TakeProfit,
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
        assert!(matches!(err, ExchangeApiError<IP_ADDRESS>Internal(_)));
    }

    // ==================== HL-09: Success Status Tests ====================

    #[test]
    fn extract_oid_from_success_returns_none() {
        let statuses = vec![ExchangeDataStatus<IP_ADDRESS>Success];
        assert_eq!(extract_order_id(&statuses), None);
    }

    // ==================== HL-09: SL/TP CLOID Tests ====================

    #[test]
    fn cloid_sl_tp_suffixes_are_unique() {
        let base = "testudo:550e8400-e29b-41d4-<US_DRIVER_LICENSE>-<US_BANK_NUMBER>:entry";
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
        let status = ExchangeDataStatus<IP_ADDRESS>Filled(FilledOrder {
            total_sz: "1.0".to_string(),
            avg_px: "50000.0".to_string(),
            oid: 99,
        });
        assert_eq!(normalize_status(&status), "closed");
    }

    #[test]
    fn status_success_maps_to_closed() {
        assert_eq!(normalize_status(&ExchangeDataStatus<IP_ADDRESS>Success), "closed");
    }

    #[test]
    fn status_resting_maps_to_open() {
        let status = ExchangeDataStatus<IP_ADDRESS>Resting(RestingOrder { oid: 1 });
        assert_eq!(normalize_status(&status), "open");
    }

    #[test]
    fn status_waiting_for_trigger_maps_to_open() {
        assert_eq!(normalize_status(&ExchangeDataStatus<IP_ADDRESS>WaitingForTrigger), "open");
    }

    #[test]
    fn status_waiting_for_fill_maps_to_open() {
        assert_eq!(normalize_status(&ExchangeDataStatus<IP_ADDRESS>WaitingForFill), "open");
    }

    #[test]
    fn status_error_maps_to_error_prefix() {
        let status = ExchangeDataStatus<IP_ADDRESS>Error("insufficient margin".to_string());
        assert_eq!(normalize_status(&status), "error:insufficient margin");
    }
}
