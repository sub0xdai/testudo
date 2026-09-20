//! HL-03: HyperliquidExchangeApi — ExchangeApi trait implementation
//!
//! Native Rust SDK integration that slots in alongside `ShadowExchangeApi`
//! and `CexExchangeApi`. Bypasses the Node.js sidecar entirely.

// @anchor exchange:router:exchange_api
// @tags api

use alloy::primitives::Address;
use async_trait::async_trait;
use hypersdk::hypercore::{
    types::{
        BatchCancel, BatchCancelCloid, BatchModify, BatchOrder, CancelByCloid, Modify,
        OrderGrouping, OrderRequest as HlOrderRequest, OrderResponseStatus,
        OrderTypePlacement, TimeInForce, TpSl,
    },
    Cancel as HlCancel, Cloid, HttpClient, NonceHandler, OidOrCloid,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

use super::auth::{AuthCache, AuthError, AuthMode, HyperliquidAuth};
use crate::types::hl_network::HlNetwork;
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
/// Implements the `ExchangeApi` trait using `hypersdk` directly, bypassing the
/// Node.js sidecar. Credentials are loaded per-request from the existing
/// `ExchangeAccountRepository` and cached via `AuthCache`.
///
/// `hypersdk` no longer generates nonces internally, so this struct owns a
/// [`NonceHandler`] and threads a fresh nonce into every signed action. The
/// handler is atomically monotonic, so one per router is correct and sharing it
/// across concurrent requests is what keeps nonces unique.
pub struct HyperliquidExchangeApi {
    client: HttpClient,
    nonces: NonceHandler,
    universe: Arc<AssetUniverse>,
    auth_cache: Arc<AuthCache>,
    account_repo: ExchangeAccountRepository,
    network: HlNetwork,
}

impl HyperliquidExchangeApi {
    pub fn new(
        universe: Arc<AssetUniverse>,
        auth_cache: Arc<AuthCache>,
        account_repo: ExchangeAccountRepository,
        network: HlNetwork,
    ) -> Self {
        Self {
            client: HttpClient::new(network.chain()),
            nonces: NonceHandler::default(),
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

    /// Transfer USDC between spot and perp accounts.
    ///
    /// Transfer USDC between the spot and perpetual accounts.
    ///
    /// `to_perp` true moves spot to perp, false moves perp to spot. Both
    /// directions use `usdClassTransfer`, which carries the `hyperliquidChain`
    /// and `signatureChainId` fields the old `spotUser`/`classTransfer` action
    /// omitted and which the exchange rejected it for.
    ///
    /// The old perp to spot path was not a class transfer either: it sent a
    /// `UsdSend` addressed to the signer's own address, which returns `ok`
    /// without moving funds between classes.
    //
    // @waiver TB-01: the two real invariants here, a non-zero query address and
    // a positive decimal amount, are asserted below and inside
    // `parse_transfer_amount`. A third would restate one of those.
    pub async fn transfer_usdc(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        amount: &str,
        to_perp: bool,
    ) -> Result<bool, ExchangeApiError> {
        let auth = self.load_auth(user_id, Some(account_id)).await?;
        debug_assert!(
            auth.query_address() != Address::ZERO,
            "a loaded auth must resolve to a non-zero query address"
        );
        let decimal = parse_transfer_amount(amount)?;
        let nonce = self.nonces.next();

        self.client
            .usd_class_transfer(&auth.signer, decimal, to_perp, nonce, None, None)
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Transfer failed: {e}")))?;

        tracing::info!(
            amount = %decimal,
            to_perp,
            network = ?self.network,
            "HL USDC class transfer submitted"
        );
        Ok(true)
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
/// Rounds quantity to the market's `sz_decimals`.
pub fn build_order_request(
    asset_index: u32,
    req: &PlaceOrderRequest,
    sz_decimals: u32,
) -> Result<HlOrderRequest, ExchangeApiError> {
    let is_buy = matches!(req.side, OrderSide::Buy);

    // Round size to the market's decimal precision
    let sz = round_sz(req.quantity, sz_decimals);

    let cloid = req
        .client_order_id
        .as_ref()
        .map(|coid| cloid_from_uuid(generate_cloid(coid)))
        .unwrap_or(Cloid::ZERO);

    let order = match req.order_type {
        ApiOrderType::Market => {
            // Market = aggressive IOC limit at a very unfavorable price.
            // Hyperliquid has no native market order; use IOC limit.
            let slippage_price = if is_buy {
                // Buy: set limit price very high to ensure fill
                req.price.unwrap_or_else(|| Decimal::new(999_999_999, 0))
            } else {
                // Sell: set limit price very low to ensure fill
                req.price.unwrap_or_else(|| Decimal::new(1, 2)) // 0.01
            };
            HlOrderRequest {
                asset: asset_index as usize,
                is_buy,
                limit_px: slippage_price,
                sz,
                reduce_only: req.reduce_only,
                order_type: OrderTypePlacement::Limit {
                    tif: TimeInForce::Ioc,
                },
                cloid,
            }
        }
        ApiOrderType::Limit => {
            let price = req
                .price
                .ok_or_else(|| ExchangeApiError::Internal("Limit order requires price".into()))?;
            HlOrderRequest {
                asset: asset_index as usize,
                is_buy,
                limit_px: price,
                sz,
                reduce_only: req.reduce_only,
                order_type: OrderTypePlacement::Limit {
                    tif: TimeInForce::Gtc,
                },
                cloid,
            }
        }
        ApiOrderType::StopLoss => {
            let trigger_px = req.stop_price.ok_or_else(|| {
                ExchangeApiError::Internal("StopLoss order requires stop_price".into())
            })?;
            trigger_order(asset_index, is_buy, trigger_px, sz, TpSl::Sl, cloid)
        }
        ApiOrderType::TakeProfit => {
            let trigger_px = req.stop_price.ok_or_else(|| {
                ExchangeApiError::Internal("TakeProfit order requires stop_price".into())
            })?;
            trigger_order(asset_index, is_buy, trigger_px, sz, TpSl::Tp, cloid)
        }
    };

    Ok(order)
}

/// Build a trigger order.
///
/// Both SL and TP execute as market on trigger, and Hyperliquid rejects the
/// action unless `limit_px` is a valid price, so it is set to the trigger price.
fn trigger_order(
    asset_index: u32,
    is_buy: bool,
    trigger_px: Decimal,
    sz: Decimal,
    tpsl: TpSl,
    cloid: Cloid,
) -> HlOrderRequest {
    let limit_px = trigger_limit_px(&trigger_px);
    HlOrderRequest {
        asset: asset_index as usize,
        is_buy,
        limit_px,
        sz,
        reduce_only: true, // SL and TP are always reduce-only
        order_type: OrderTypePlacement::Trigger {
            is_market: true,
            trigger_px,
            tpsl,
        },
        cloid,
    }
}

/// Convert a router CLOID UUID into the SDK's 128-bit client order ID.
///
/// Big-endian, matching `format!("{:032x}", uuid.as_u128())` and therefore the
/// `cloid:<hex>` strings this module already emits.
pub fn cloid_from_uuid(cloid: Uuid) -> Cloid {
    Cloid::from(*cloid.as_bytes())
}

/// Compute the limit price for market trigger orders.
/// Per HL SDK convention: `limit_px = trigger_px` for market triggers.
fn trigger_limit_px(trigger_px: &Decimal) -> Decimal {
    trigger_px.normalize()
}

/// Choose a conservative price tick from the mid price.
fn tick_size_from_mid(mid: Decimal) -> Decimal {
    // Conservative: use 1 decimal place for prices > 1000, 2 for > 100
    if mid >= Decimal::from(10_000u32) { Decimal::new(1, 0) }
    else if mid >= Decimal::from(1_000u32) { Decimal::new(1, 1) }
    else if mid >= Decimal::from(100u32) { Decimal::new(1, 2) }
    else if mid >= Decimal::ONE { Decimal::new(1, 3) }
    else { Decimal::new(1, 5) }
}

/// Round a quantity to the number of decimal places Hyperliquid accepts.
///
/// `hypersdk` serialises the size with `decimal_normalized`, which strips
/// trailing zeros, so a plain `Decimal` is the wire form.
pub fn round_sz(quantity: Decimal, sz_decimals: u32) -> Decimal {
    quantity.round_dp(sz_decimals).normalize()
}

/// Parse a USDC amount for a spot/perp class transfer.
///
/// `usd_class_transfer` takes a decimal USDC amount. The vendored crate took a
/// `u64` in micro-units instead, so the old call site multiplied by 1_000_000
/// before handing the value over. Carrying that conversion across the swap
/// moves a million times the intended amount, which is why the unit is pinned
/// by a test rather than left to review.
//
// @waiver TB-01: a parse plus a range check, with no branch that can fail
// without returning an error. The assertions available would restate the range
// check; `transfer_amount_is_decimal_usdc_not_micro_units` pins the unit that
// actually matters.
fn parse_transfer_amount(amount: &str) -> Result<Decimal, ExchangeApiError> {
    let decimal: Decimal = amount.parse().map_err(|e| {
        ExchangeApiError::Internal(format!("Invalid transfer amount '{amount}': {e}"))
    })?;
    if decimal <= Decimal::ZERO {
        return Err(ExchangeApiError::Internal(format!(
            "Transfer amount must be positive, got '{amount}'"
        )));
    }
    Ok(decimal)
}

/// Extract the order ID (OID) from a Hyperliquid exchange response.
pub fn extract_order_id(statuses: &[OrderResponseStatus]) -> Option<u64> {
    statuses.iter().find_map(OrderResponseStatus::oid)
}

/// HL-11 FR-1: Normalize the order status to a CCXT-compatible status string.
/// "closed" = filled/done, "open" = resting/waiting, "error:..." = rejected.
pub fn normalize_status(status: &OrderResponseStatus) -> String {
    match status {
        OrderResponseStatus::Filled { .. } => "closed".to_string(),
        OrderResponseStatus::Success => "closed".to_string(),
        OrderResponseStatus::Resting { .. } => "open".to_string(),
        OrderResponseStatus::WaitingForTrigger => "open".to_string(),
        OrderResponseStatus::WaitingForFill => "open".to_string(),
        OrderResponseStatus::Error(msg) => format!("error:{}", msg),
    }
}

/// Extract the average fill price from a response (if any status filled).
pub fn extract_avg_price(statuses: &[OrderResponseStatus]) -> Option<Decimal> {
    statuses.iter().find_map(|status| match status {
        OrderResponseStatus::Filled { avg_px, .. } => Some(*avg_px),
        _ => None,
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
        let query_addr = auth.query_address();
        let info_url = self.network.info_url();

        // For unified accounts, spotClearinghouseState is the source of truth
        // for trading balance across spot and perps.
        let spot_total = {
            let client = reqwest::Client::new();
            let spot_payload = serde_json::json!({
                "type": "spotClearinghouseState",
                "user": format!("{:#x}", query_addr),
            });
            let resp = client.post(info_url).json(&spot_payload).send().await;
            let body: serde_json::Value = match resp {
                Ok(r) => r.json().await.unwrap_or_default(),
                Err(_) => serde_json::Value::Null,
            };
            body.get("balances")
                .and_then(|b| b.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| {
                            e.get("total")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse::<Decimal>().ok())
                        })
                        .sum::<Decimal>()
                })
                .unwrap_or_default()
        };

        if spot_total > Decimal::ZERO {
            return Ok(spot_total);
        }

        // Fall back to perp clearinghouse
        let state = self.client.clearinghouse_state(query_addr, None).await
            .map_err(|e| ExchangeApiError::Exchange(format!("Failed to fetch user state: {}", e)))?;
        Ok(state.margin_summary.account_value)
    }

    async fn place_order(
        &self,
        mut req: PlaceOrderRequest,
    ) -> Result<PlaceOrderResult, ExchangeApiError> {
        let auth = self
            .load_auth(req.user_id, req.exchange_account_id)
            .await?;

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
            if let Ok(mids) = self.client.all_mids(None).await {
                if let Some(mid) = mids.get(coin) {
                    let slip = *mid * Decimal::new(10, 2); // 10%
                    let is_buy = matches!(req.side, OrderSide::Buy);
                    req.price = Some(if is_buy { *mid + slip } else { *mid - slip });
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

        let mut hl_order = build_order_request(asset_index, &req, sz_decimals)?;

        // Round limit price to HL tick size derived from the mid price.
        if let Ok(mids) = self.client.all_mids(None).await {
            if let Some(mid) = mids.get(coin) {
                let tick = tick_size_from_mid(*mid);
                hl_order.limit_px = (hl_order.limit_px / tick).round_dp(0) * tick;
            }
        }
        let cloid_uuid = req.client_order_id.as_ref().map(|c| generate_cloid(c));

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

        let nonce = self.nonces.next();
        let statuses = self
            .client
            .place(
                &auth.signer,
                BatchOrder {
                    orders: vec![hl_order],
                    grouping: OrderGrouping::Na,
                    builder: None,
                },
                nonce,
                None,
                None,
            )
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Place order failed: {}", e)))?;

        debug_assert!(
            statuses.len() <= 1,
            "a single-order batch must yield at most one status"
        );

        // Extract OID from response
        let order_id = if let Some(oid) = extract_order_id(&statuses) {
            oid.to_string()
        } else {
            // Check for error statuses before falling back to CLOID.
            let error_msg = statuses.iter().find_map(|s| s.error());
            if let Some(msg) = error_msg {
                return Err(ExchangeApiError::Exchange(msg.to_string()));
            }
            if let Some(cloid_uuid) = cloid_uuid {
                match self.find_oid_by_cloid(&auth, cloid_uuid).await {
                    Ok(oid) => oid.to_string(),
                    Err(_) => format!("cloid:{:032x}", cloid_uuid.as_u128()),
                }
            } else {
                "success".to_string()
            }
        };

        let avg_price = extract_avg_price(&statuses);
        // HL-11 FR-1: Normalize the order status to CCXT-compatible strings
        // so downstream `is_filled` check (== "closed") works for immediate fills.
        let status = statuses.first().map(normalize_status);

        // HL-09 FR-1/FR-2: Place SL/TP as separate trigger orders after entry
        let close_is_buy = !matches!(req.side, OrderSide::Buy);
        let sz = round_sz(req.quantity, sz_decimals);
        let mut sl_order_id = None;
        let mut tp_order_id = None;

        // Round trigger prices to HL tick size
        let tick = self.client.all_mids(None).await.ok()
            .and_then(|mids| mids.get(coin).copied())
            .map(tick_size_from_mid)
            .unwrap_or(Decimal::new(1, 2)); // default 0.01

        if let Some(sl_trigger) = req.stop_loss_trigger {
            let sl_rounded = (sl_trigger / tick).round_dp(0) * tick;
            sl_order_id = self.place_trigger_order(
                &auth, asset_index, close_is_buy,
                sl_rounded, sz, TpSl::Sl, req.client_order_id.as_deref(),
            ).await;
        }

        if let Some(tp_trigger) = req.take_profit_trigger {
            let tp_rounded = (tp_trigger / tick).round_dp(0) * tick;
            tp_order_id = self.place_trigger_order(
                &auth, asset_index, close_is_buy,
                tp_rounded, sz, TpSl::Tp, req.client_order_id.as_deref(),
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
        let sz = round_sz(quantity, sz_decimals);

        let new_order = match amend.order_type {
            Some(ApiOrderType::StopLoss) => {
                let trigger_px = amend.new_stop_price.ok_or_else(|| {
                    ExchangeApiError::Internal("StopLoss amend requires stop_price".into())
                })?;
                trigger_order(asset_index, is_buy, trigger_px, sz, TpSl::Sl, Cloid::ZERO)
            }
            Some(ApiOrderType::TakeProfit) => {
                let trigger_px = amend.new_stop_price.ok_or_else(|| {
                    ExchangeApiError::Internal("TakeProfit amend requires stop_price".into())
                })?;
                trigger_order(asset_index, is_buy, trigger_px, sz, TpSl::Tp, Cloid::ZERO)
            }
            Some(ApiOrderType::Market) => {
                let price = if is_buy {
                    Decimal::new(999_999_999, 0)
                } else {
                    Decimal::new(1, 2)
                };
                HlOrderRequest {
                    asset: asset_index as usize,
                    is_buy,
                    limit_px: price,
                    sz,
                    reduce_only: amend.reduce_only,
                    order_type: OrderTypePlacement::Limit {
                        tif: TimeInForce::Ioc,
                    },
                    cloid: Cloid::ZERO,
                }
            }
            Some(ApiOrderType::Limit) | None => {
                let price = amend.new_price.ok_or_else(|| {
                    ExchangeApiError::Internal("Limit amend requires price".into())
                })?;
                HlOrderRequest {
                    asset: asset_index as usize,
                    is_buy,
                    limit_px: price,
                    sz,
                    reduce_only: amend.reduce_only,
                    order_type: OrderTypePlacement::Limit {
                        tif: TimeInForce::Gtc,
                    },
                    cloid: Cloid::ZERO,
                }
            }
        };

        let nonce = self.nonces.next();
        let statuses = self
            .client
            .modify(
                &auth.signer,
                BatchModify {
                    modifies: vec![Modify {
                        oid: OidOrCloid::Left(oid),
                        order: new_order,
                    }],
                    always_place: false,
                },
                nonce,
                None,
                None,
            )
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Modify order failed: {}", e)))?;

        // Modified orders keep the same OID on Hyperliquid
        let new_oid = extract_order_id(&statuses).unwrap_or(oid);

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

        let coin = AssetUniverse::to_hl_coin(symbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;
        let nonce = self.nonces.next();

        // Handle both numeric OID and CLOID-based IDs
        if let Some(cloid_hex) = order_id.strip_prefix("cloid:") {
            let cloid = cloid_from_uuid(Uuid::from_u128(
                u128::from_str_radix(cloid_hex, 16).map_err(|e| {
                    ExchangeApiError::Internal(format!("Invalid CLOID: {}", e))
                })?,
            ));
            self.client
                .cancel_by_cloid(
                    &auth.signer,
                    BatchCancelCloid {
                        cancels: vec![CancelByCloid {
                            asset: asset_index,
                            cloid,
                        }],
                        fast: false,
                    },
                    nonce,
                    None,
                    None,
                )
                .await
                .map_err(|e| ExchangeApiError::Exchange(format!("Cancel failed: {}", e)))?;
        } else {
            let oid: u64 = parse_oid(order_id)?;
            self.client
                .cancel(
                    &auth.signer,
                    BatchCancel {
                        cancels: vec![HlCancel {
                            asset: asset_index as usize,
                            oid,
                        }],
                        fast: false,
                    },
                    nonce,
                    None,
                    None,
                )
                .await
                .map_err(|e| ExchangeApiError::Exchange(format!("Cancel failed: {}", e)))?;
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

        let coin = AssetUniverse::to_hl_coin(symbol);
        let asset_index = self
            .universe
            .resolve(coin)
            .map_err(|e| ExchangeApiError::Exchange(e.to_string()))?;

        // Fetch all open orders for this user
        let open_orders = self
            .client
            .open_orders(auth.query_address(), None)
            .await
            .map_err(|e| {
                ExchangeApiError::Exchange(format!("Failed to fetch open orders: {}", e))
            })?;

        // Filter to this symbol and build cancel requests
        let cancels: Vec<HlCancel> = open_orders
            .iter()
            .filter(|o| o.coin.to_uppercase() == coin.to_uppercase())
            .map(|o| HlCancel {
                asset: asset_index as usize,
                oid: o.oid,
            })
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

        let nonce = self.nonces.next();
        self.client
            .cancel(
                &auth.signer,
                BatchCancel {
                    cancels,
                    fast: false,
                },
                nonce,
                None,
                None,
            )
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Bulk cancel failed: {}", e)))?;

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
            .client
            .clearinghouse_state(auth.query_address(), None)
            .await
            .map_err(|e| ExchangeApiError::Exchange(format!("Failed to fetch user state: {}", e)))?;

        let coin = AssetUniverse::to_hl_coin(symbol);

        let pos = state.asset_positions.iter().find(|ap| {
            ap.position.coin.to_uppercase() == coin.to_uppercase()
        });

        let position_info = match pos {
            Some(ap) => {
                let szi = ap.position.szi;
                if szi == Decimal::ZERO {
                    None
                } else {
                    let side = if szi > Decimal::ZERO {
                        "long".to_string()
                    } else {
                        "short".to_string()
                    };

                    Some(PositionInfo {
                        symbol: symbol.to_string(),
                        side,
                        quantity: szi.abs(),
                        entry_price: ap.position.entry_px.unwrap_or(Decimal::ZERO),
                        unrealized_pnl: ap.position.unrealized_pnl,
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
    #[allow(clippy::too_many_arguments)]
    async fn place_trigger_order(
        &self,
        auth: &HyperliquidAuth,
        asset_index: u32,
        close_is_buy: bool,
        trigger_px: Decimal,
        sz: Decimal,
        tpsl: TpSl,
        client_order_id_base: Option<&str>,
    ) -> Option<String> {
        let tpsl_label = match tpsl {
            TpSl::Sl => "sl",
            TpSl::Tp => "tp",
        };

        // Generate CLOID with ":sl" or ":tp" suffix for tracking
        let cloid_uuid = client_order_id_base
            .map(|base| generate_cloid(&format!("{}:{}", base, tpsl_label)));
        let cloid = cloid_uuid.map(cloid_from_uuid).unwrap_or(Cloid::ZERO);

        let order = trigger_order(asset_index, close_is_buy, trigger_px, sz, tpsl, cloid);

        tracing::info!(
            tpsl = %tpsl_label,
            trigger_px = %trigger_px,
            close_is_buy = %close_is_buy,
            limit_px = %order.limit_px,
            sz = %order.sz,
            is_buy = %order.is_buy,
            reduce_only = %order.reduce_only,
            order_type = ?order.order_type,
            cloid = ?order.cloid,
            "{} trigger order details", tpsl_label
        );

        let nonce = self.nonces.next();
        let placed = self
            .client
            .place(
                &auth.signer,
                BatchOrder {
                    orders: vec![order],
                    grouping: OrderGrouping::Na,
                    builder: None,
                },
                nonce,
                None,
                None,
            )
            .await;

        match placed {
            Ok(statuses) => {
                tracing::info!(
                    tpsl = %tpsl_label,
                    statuses = ?statuses,
                    "{} trigger response statuses", tpsl_label
                );
                // A rejected trigger comes back as a per-status error inside an
                // otherwise successful response, so it has to be surfaced here.
                if let Some(err) = statuses.iter().find_map(OrderResponseStatus::error) {
                    tracing::warn!("{} trigger rejected: {}", tpsl_label, err);
                }
                // Try OID from response first
                if let Some(oid) = extract_order_id(&statuses) {
                    return Some(oid.to_string());
                }
                // CLOID fallback for WaitingForTrigger
                if let Some(cloid_uuid) = cloid_uuid {
                    if let Ok(oid) = self.find_oid_by_cloid(auth, cloid_uuid).await {
                        return Some(oid.to_string());
                    }
                    return Some(format!("cloid:{:032x}", cloid_uuid.as_u128()));
                }
                tracing::warn!("{} trigger placed but no OID returned", tpsl_label);
                None
            }
            Err(e) => {
                tracing::warn!("{} trigger failed: {}", tpsl_label, e);
                None
            }
        }
    }

    /// Query open orders to find an OID by CLOID.
    /// Used as fallback when place response doesn't include OID (trigger orders).
    async fn find_oid_by_cloid(
        &self,
        auth: &HyperliquidAuth,
        cloid: Uuid,
    ) -> Result<u64, ExchangeApiError> {
        let wanted = cloid_from_uuid(cloid);

        let orders = self
            .client
            .open_orders(auth.query_address(), None)
            .await
            .map_err(|e| {
                ExchangeApiError::Exchange(format!("Failed to fetch open orders: {}", e))
            })?;

        for order in &orders {
            if order.cloid == Some(wanted) {
                return Ok(order.oid);
            }
        }

        Err(ExchangeApiError::Exchange(format!(
            "Order with CLOID {:#x} not found in open orders",
            wanted
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
    use alloy::primitives::B256;
    use hypersdk::hypercore::{
        signing::agent_signing_hash,
        types::{Action, Cancel, OrderTypePlacement, UsdClassTransferAction},
        Chain,
    };
    use rust_decimal_macros::dec;
    use serde_json::json;

    /// A hand-chosen CLOID so the wire string is checkable by eye.
    const FIXTURE_CLOID_VALUE: u128 = 0xdead_beef;

    fn fixture_cloid() -> Cloid {
        Cloid::from(FIXTURE_CLOID_VALUE.to_be_bytes())
    }

    fn limit_request() -> PlaceOrderRequest {
        PlaceOrderRequest {
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
            client_order_id: None,
            stop_loss_trigger: None,
            take_profit_trigger: None,
        }
    }

    /// The order the signing fixture pins. Values are fixed so the digest is.
    fn fixture_order() -> HlOrderRequest {
        HlOrderRequest {
            asset: 0,
            is_buy: true,
            limit_px: dec!(50000),
            sz: dec!(0.1),
            reduce_only: false,
            order_type: OrderTypePlacement::Limit {
                tif: TimeInForce::Gtc,
            },
            cloid: Cloid::ZERO,
        }
    }

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

    #[test]
    fn cloid_uuid_is_the_big_endian_hex_hyperliquid_expects() {
        // The same UUID reaches the exchange two ways: as the `cloid:` fallback
        // order id this module prints, and as the 128-bit value on the wire.
        // They must agree, or a cancel by cloid can never find its order.
        let uuid = Uuid::from_u128(FIXTURE_CLOID_VALUE);
        assert_eq!(
            format!("{:032x}", uuid.as_u128()),
            "000000000000000000000000deadbeef"
        );
        assert_eq!(
            format!("{:#x}", cloid_from_uuid(uuid)),
            "0x000000000000000000000000deadbeef"
        );
    }

    // ==================== Order Building Tests ====================

    #[test]
    fn build_limit_order() {
        let order = build_order_request(0, &limit_request(), 5).unwrap();
        assert_eq!(order.asset, 0);
        assert!(order.is_buy);
        assert_eq!(order.limit_px, dec!(65000.5));
        assert_eq!(order.sz, dec!(0.12345));
        assert!(!order.reduce_only);
        assert!(order.cloid.is_zero());
        assert!(matches!(
            order.order_type,
            OrderTypePlacement::Limit { tif: TimeInForce::Gtc }
        ));
    }

    #[test]
    fn build_market_order() {
        let mut req = limit_request();
        req.symbol = "ETH_USDT".to_string();
        req.side = OrderSide::Sell;
        req.order_type = ApiOrderType::Market;
        req.quantity = dec!(1.5);
        req.price = None;

        let order = build_order_request(1, &req, 4).unwrap();
        assert_eq!(order.asset, 1);
        assert!(!order.is_buy); // Sell
        assert_eq!(order.sz, dec!(1.5));
        assert!(order.cloid.is_zero());
        // Market = IOC limit
        assert!(matches!(
            order.order_type,
            OrderTypePlacement::Limit { tif: TimeInForce::Ioc }
        ));
    }

    #[test]
    fn build_stop_loss_order() {
        let mut req = limit_request();
        req.symbol = "SOL_USDT".to_string();
        req.side = OrderSide::Sell;
        req.order_type = ApiOrderType::StopLoss;
        req.quantity = dec!(10);
        req.price = None;
        req.stop_price = Some(dec!(120.50));
        req.reduce_only = true;
        req.client_order_id = Some("testudo:test-id:sl".to_string());

        let order = build_order_request(5, &req, 2).unwrap();
        assert_eq!(order.asset, 5);
        assert!(!order.is_buy); // Sell side
        assert_eq!(order.sz, dec!(10));
        assert!(order.reduce_only); // SL is always reduce-only
        assert!(!order.cloid.is_zero());
        match &order.order_type {
            OrderTypePlacement::Trigger {
                is_market,
                trigger_px,
                tpsl,
            } => {
                assert!(*is_market);
                assert_eq!(*trigger_px, dec!(120.50));
                assert_eq!(*tpsl, TpSl::Sl);
            }
            other => panic!("Expected trigger order type, got {other:?}"),
        }
        // Hyperliquid rejects a trigger whose limit price is zero.
        assert_eq!(order.limit_px, dec!(120.5));
    }

    #[test]
    fn build_limit_order_without_price_fails() {
        let mut req = limit_request();
        req.price = None; // Missing!

        let err = build_order_request(0, &req, 5).unwrap_err();
        assert!(matches!(err, ExchangeApiError::Internal(_)));
    }

    #[test]
    fn build_stop_loss_without_stop_price_fails() {
        let mut req = limit_request();
        req.side = OrderSide::Sell;
        req.order_type = ApiOrderType::StopLoss;
        req.stop_price = None; // Missing!

        let err = build_order_request(0, &req, 5).unwrap_err();
        assert!(matches!(err, ExchangeApiError::Internal(_)));
    }

    // ==================== Size Rounding Tests ====================

    #[test]
    fn round_sz_respects_decimals() {
        assert_eq!(round_sz(dec!(0.123456789), 5), dec!(0.12346)); // rounds
        assert_eq!(round_sz(dec!(1.5), 2), dec!(1.5));
        assert_eq!(round_sz(dec!(100), 0), dec!(100));
        assert_eq!(round_sz(dec!(0.1), 8), dec!(0.1));
    }

    #[test]
    fn tick_size_tracks_the_mid_price_band() {
        assert_eq!(tick_size_from_mid(dec!(50000)), dec!(1));
        assert_eq!(tick_size_from_mid(dec!(5000)), dec!(0.1));
        assert_eq!(tick_size_from_mid(dec!(500)), dec!(0.01));
        assert_eq!(tick_size_from_mid(dec!(5)), dec!(0.001));
        assert_eq!(tick_size_from_mid(dec!(0.5)), dec!(0.00001));
    }

    // ==================== Response Extraction Tests ====================

    #[test]
    fn extract_oid_from_resting() {
        let statuses = vec![OrderResponseStatus::Resting {
            oid: 42,
            cloid: None,
        }];
        assert_eq!(extract_order_id(&statuses), Some(42));
    }

    #[test]
    fn extract_oid_from_filled() {
        let statuses = vec![OrderResponseStatus::Filled {
            total_sz: dec!(0.5),
            avg_px: dec!(65000),
            oid: 123,
        }];
        assert_eq!(extract_order_id(&statuses), Some(123));
    }

    #[test]
    fn extract_oid_from_waiting_for_trigger_returns_none() {
        let statuses = vec![OrderResponseStatus::WaitingForTrigger];
        assert_eq!(extract_order_id(&statuses), None);
    }

    #[test]
    fn extract_avg_price_from_filled() {
        let statuses = vec![OrderResponseStatus::Filled {
            total_sz: dec!(0.5),
            avg_px: dec!(65432.10),
            oid: 1,
        }];
        assert_eq!(extract_avg_price(&statuses), Some(dec!(65432.10)));
    }

    #[test]
    fn extract_avg_price_from_resting_returns_none() {
        let statuses = vec![OrderResponseStatus::Resting {
            oid: 1,
            cloid: None,
        }];
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
        let mut req = limit_request();
        req.side = OrderSide::Sell;
        req.order_type = ApiOrderType::TakeProfit;
        req.quantity = dec!(0.5);
        req.price = None;
        req.stop_price = Some(dec!(70000));
        req.reduce_only = true;
        req.client_order_id = Some("testudo:test-id:tp".to_string());

        let order = build_order_request(0, &req, 5).unwrap();
        assert!(!order.is_buy); // Sell side
        assert_eq!(order.sz, dec!(0.5));
        assert!(order.reduce_only);
        assert!(!order.cloid.is_zero());
        match &order.order_type {
            OrderTypePlacement::Trigger {
                is_market,
                trigger_px,
                tpsl,
            } => {
                assert!(*is_market);
                assert_eq!(*trigger_px, dec!(70000));
                assert_eq!(*tpsl, TpSl::Tp);
            }
            other => panic!("Expected trigger order type, got {other:?}"),
        }
    }

    #[test]
    fn build_take_profit_without_stop_price_fails() {
        let mut req = limit_request();
        req.side = OrderSide::Sell;
        req.order_type = ApiOrderType::TakeProfit;
        req.stop_price = None; // Missing!

        let err = build_order_request(0, &req, 5).unwrap_err();
        assert!(matches!(err, ExchangeApiError::Internal(_)));
    }

    // ==================== HL-09: Success Status Tests ====================

    #[test]
    fn extract_oid_from_success_returns_none() {
        let statuses = vec![OrderResponseStatus::Success];
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
        let status = OrderResponseStatus::Filled {
            total_sz: dec!(1.0),
            avg_px: dec!(50000),
            oid: 99,
        };
        assert_eq!(normalize_status(&status), "closed");
    }

    #[test]
    fn status_success_maps_to_closed() {
        assert_eq!(normalize_status(&OrderResponseStatus::Success), "closed");
    }

    #[test]
    fn status_resting_maps_to_open() {
        let status = OrderResponseStatus::Resting {
            oid: 1,
            cloid: None,
        };
        assert_eq!(normalize_status(&status), "open");
    }

    #[test]
    fn status_waiting_for_trigger_maps_to_open() {
        assert_eq!(
            normalize_status(&OrderResponseStatus::WaitingForTrigger),
            "open"
        );
    }

    #[test]
    fn status_waiting_for_fill_maps_to_open() {
        assert_eq!(
            normalize_status(&OrderResponseStatus::WaitingForFill),
            "open"
        );
    }

    #[test]
    fn status_error_maps_to_error_prefix() {
        let status = OrderResponseStatus::Error("insufficient margin".to_string());
        assert_eq!(normalize_status(&status), "error:insufficient margin");
    }

    // ==================== HL-12 CP-3: wire-shape body assertions ====================
    //
    // Hyperliquid hashes the same bytes it reads as JSON, and it reads short
    // keys. Nothing else in this crate observes the serialized names, because
    // they are only ever produced by serde, so these assertions are the only
    // guard against a rename shipping straight to the exchange.

    #[test]
    fn order_body_carries_the_short_keys_and_normalised_decimals() {
        let value = serde_json::to_value(Action::Order(BatchOrder {
            orders: vec![fixture_order()],
            grouping: OrderGrouping::Na,
            builder: None,
        }))
        .expect("order action serializes");

        assert_eq!(value["type"], json!("order"));
        assert_eq!(value["grouping"], json!("na"));
        let order = &value["orders"][0];
        assert_eq!(order["a"], json!(0));
        assert_eq!(order["b"], json!(true));
        assert_eq!(order["p"], json!("50000"));
        assert_eq!(order["s"], json!("0.1"));
        assert_eq!(order["r"], json!(false));
        assert_eq!(order["t"]["limit"]["tif"], json!("Gtc"));
        // The zero cloid is omitted, matching the Python SDK's hashing.
        assert!(order.get("c").is_none(), "zero cloid must be omitted");
        assert!(
            order.get("limitPx").is_none(),
            "the long field name must never reach the wire; got {value:?}"
        );
    }

    #[test]
    fn order_body_carries_a_non_zero_cloid_as_full_width_hex() {
        let mut order = fixture_order();
        order.cloid = fixture_cloid();
        let value = serde_json::to_value(Action::Order(BatchOrder {
            orders: vec![order],
            grouping: OrderGrouping::Na,
            builder: None,
        }))
        .expect("order action serializes");

        assert_eq!(
            value["orders"][0]["c"],
            json!("0x000000000000000000000000deadbeef")
        );
    }

    #[test]
    fn trigger_body_carries_the_lowercase_tpsl_tag() {
        let mut order = fixture_order();
        order.order_type = OrderTypePlacement::Trigger {
            is_market: true,
            trigger_px: dec!(120.5),
            tpsl: TpSl::Sl,
        };
        let value = serde_json::to_value(Action::Order(BatchOrder {
            orders: vec![order],
            grouping: OrderGrouping::Na,
            builder: None,
        }))
        .expect("trigger action serializes");

        let trigger = &value["orders"][0]["t"]["trigger"];
        assert_eq!(trigger["isMarket"], json!(true));
        assert_eq!(trigger["triggerPx"], json!("120.5"));
        assert_eq!(trigger["tpsl"], json!("sl"));
    }

    #[test]
    fn cancel_body_omits_the_fast_flag() {
        let value = serde_json::to_value(Action::Cancel(BatchCancel {
            cancels: vec![HlCancel {
                asset: 3,
                oid: 42,
            }],
            fast: false,
        }))
        .expect("cancel action serializes");

        assert_eq!(value["type"], json!("cancel"));
        assert_eq!(value["cancels"][0], json!({ "a": 3, "o": 42 }));
        // The exchange rejects an action hashed with `f: false`.
        assert!(value.get("f").is_none(), "fast must be omitted when false");
    }

    #[test]
    fn cancel_by_cloid_body_carries_the_asset_and_hex_cloid() {
        let value = serde_json::to_value(Action::CancelByCloid(BatchCancelCloid {
            cancels: vec![CancelByCloid {
                asset: 3,
                cloid: fixture_cloid(),
            }],
            fast: false,
        }))
        .expect("cancelByCloid action serializes");

        assert_eq!(value["type"], json!("cancelByCloid"));
        assert_eq!(value["cancels"][0]["asset"], json!(3));
        assert_eq!(
            value["cancels"][0]["cloid"],
            json!("0x000000000000000000000000deadbeef")
        );
    }

    #[test]
    fn modify_body_nests_the_replacement_order() {
        let value = serde_json::to_value(Action::BatchModify(BatchModify {
            modifies: vec![Modify {
                oid: OidOrCloid::Left(7),
                order: fixture_order(),
            }],
            always_place: false,
        }))
        .expect("modify action serializes");

        assert_eq!(value["type"], json!("batchModify"));
        assert_eq!(value["modifies"][0]["oid"], json!(7));
        assert_eq!(value["modifies"][0]["order"]["p"], json!("50000"));
        assert!(
            value.get("a").is_none(),
            "alwaysPlace must be omitted when false"
        );
    }

    // ==================== HL-12 CP-3: signing fixtures ====================
    //
    // Pin the input to the signature, never the signature. An ECDSA signature
    // over a digest is not unique, and alloy 2 derives its deterministic nonce
    // differently from alloy 0.1. The vendored crate's expected signature
    // `fa8a41f6...` does not survive the alloy bump and would fail against a
    // correct implementation, so a fixture pinned to it would be actively
    // harmful: the fix that makes it pass damages a money path.

    /// Nonce for the signing fixtures. Fixed so the digest is.
    const FIXTURE_NONCE: u64 = 1_770_000_000_000;

    /// The exchange-verified digest for `Agent(source, connectionId)` on
    /// mainnet, measured against the vendored crate's own test vector before
    /// the swap. This is the one value here with an external reference: it
    /// proves the EIP-712 domain and the `Agent` struct encode identically in
    /// both crates, which closed the spec's headline 422 risk.
    #[test]
    fn agent_signing_digest_matches_the_external_vector() {
        let connection_id: B256 =
            "0xde6c4037798a4434ca03cd05f00e3b803126221375cd1e7eaaaf041768be06eb"
                .parse()
                .expect("connection id");

        assert_eq!(
            format!("{:#x}", agent_signing_hash(Chain::Mainnet, connection_id)),
            "0x6c43321e0f67bbafa52f8ad0cca68ce5e6700cac04fd5f8ccc1df6d4d3e799b0"
        );
    }

    /// Locks the msgpack encoding of the fixture order.
    ///
    /// The digest above proves the outer `Agent` layer matches the exchange;
    /// this value is the inner layer, and together they pin the whole chain:
    /// msgpack action -> keccak -> `Agent` -> EIP-712 -> digest. If a future
    /// hypersdk release changes the action encoding, this fails here rather
    /// than at trade time.
    #[test]
    fn order_signing_input_is_stable() {
        let action = Action::Order(BatchOrder {
            orders: vec![fixture_order()],
            grouping: OrderGrouping::Na,
            builder: None,
        });

        let connection_id = action
            .hash(FIXTURE_NONCE, None, None)
            .expect("action hashes");
        assert_eq!(
            format!("{:#x}", connection_id),
            "0x80a7fa73795d92fa7ef42bac59658a57660f92c3e2b5ccdf827265cb0396ecf0"
        );

        let prehash = action
            .prehash(FIXTURE_NONCE, None, None, Chain::Mainnet)
            .expect("action prehashes");
        assert_eq!(prehash, agent_signing_hash(Chain::Mainnet, connection_id));
        assert_eq!(
            format!("{:#x}", prehash),
            "0x5262c55da753cec89aca4f45e385b848eb7c3bc9a4bd01ea89dadd09267cabaa"
        );
    }

    /// The signature is not pinned, but it must recover to the signing key.
    ///
    /// This is what the exchange does, so it is the property that actually has
    /// to hold across the alloy bump.
    #[test]
    fn signed_order_recovers_to_the_signing_key() {
        let signer = alloy::signers::local::PrivateKeySigner::random();
        let action = Action::Order(BatchOrder {
            orders: vec![fixture_order()],
            grouping: OrderGrouping::Na,
            builder: None,
        });

        let request = action
            .sign_sync(&signer, FIXTURE_NONCE, None, None, Chain::Mainnet)
            .expect("action signs");

        assert_eq!(
            request.recover(Chain::Mainnet).expect("signature recovers"),
            signer.address()
        );
    }

    // ==================== Transfer Tests ====================

    /// The transfer amount is decimal USDC, never micro-units.
    ///
    /// This is the highest-severity hazard in the migration: the vendored crate
    /// took a `u64` in micro-units, so the old call site multiplied by
    /// 1_000_000. `usd_class_transfer` takes a decimal. If the conversion came
    /// back, every transfer would move a million times the intended amount.
    #[test]
    fn transfer_amount_is_decimal_usdc_not_micro_units() {
        assert_eq!(parse_transfer_amount("12.5").expect("parses"), dec!(12.5));
        assert_eq!(parse_transfer_amount("1000").expect("parses"), dec!(1000));
        assert_eq!(
            parse_transfer_amount("0.000001").expect("parses"),
            dec!(0.000001)
        );
        assert_ne!(
            parse_transfer_amount("12.5").expect("parses"),
            dec!(12_500_000),
            "a micro-unit conversion turned 12.5 USDC into 12.5 million"
        );
    }

    #[test]
    fn transfer_amount_rejects_zero_negative_and_unparseable() {
        assert!(parse_transfer_amount("0").is_err());
        assert!(parse_transfer_amount("-1").is_err());
        assert!(parse_transfer_amount("not-a-number").is_err());
    }

    /// Pins the wire body, replacing the `ClassTransfer` guard this migration
    /// deleted. The old action omitted `hyperliquidChain` and
    /// `signatureChainId`, which is why the transfer was rejected.
    #[test]
    fn usd_class_transfer_body_carries_the_chain_fields_and_decimal_amount() {
        let nonce = 1_770_000_000_000u64;
        let action = Action::UsdClassTransfer(UsdClassTransferAction {
            signature_chain_id: Chain::Mainnet.arbitrum_id().to_string(),
            hyperliquid_chain: Chain::Mainnet,
            amount: "12.5".to_string(),
            to_perp: true,
            nonce,
        });
        let body = serde_json::to_value(&action).expect("action serialises");

        assert_eq!(body["type"], "usdClassTransfer");
        assert_eq!(body["hyperliquidChain"], "Mainnet");
        // Hex, not decimal: 42161 is 0xa4b1. Hyperliquid documents the hex form,
        // and `agent_approval.rs` formats it the same way for MetaMask-signed
        // approvals, so this is the convention the exchange expects.
        assert_eq!(body["signatureChainId"], "0xa4b1");
        assert_eq!(body["amount"], "12.5");
        assert_eq!(body["toPerp"], true);
        assert_eq!(body["nonce"], nonce);
    }
}
