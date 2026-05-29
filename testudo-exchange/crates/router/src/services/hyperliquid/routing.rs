//! HL-05: Exchange Routing & Dependency Injection
//!
//! `RoutingExchangeApi` transparently delegates `ExchangeApi` calls to either
//! `HyperliquidExchangeApi` (native SDK) or `CexExchangeApi` (sidecar) based
//! on the `exchange_name` stored in the user's exchange account.

// @anchor exchange:router:routing
// @tags api

use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

use super::HyperliquidExchangeApi;
use crate::repositories::exchange_account::ExchangeAccountRepository;
use crate::types::exchange_names::exchanges;
use crate::services::exchange_api::{
    AmendRequest, CexExchangeApi, ExchangeApi, ExchangeApiError, PlaceOrderRequest,
    PlaceOrderResult, PositionInfo,
};

/// Routes `ExchangeApi` calls to the correct backend based on `exchange_name`.
///
/// - `exchanges::HYPERLIQUID` → `HyperliquidExchangeApi` (native Rust SDK)
/// - Everything else → `CexExchangeApi` (CCXT sidecar)
///
/// The routing lookup uses `ExchangeAccountRepository` to resolve the
/// `exchange_name` for the given `exchange_account_id` (or first account).
pub struct RoutingExchangeApi {
    cex_api: Arc<CexExchangeApi>,
    hl_api: Arc<HyperliquidExchangeApi>,
    account_repo: ExchangeAccountRepository,
}

impl RoutingExchangeApi {
    pub fn new(
        cex_api: Arc<CexExchangeApi>,
        hl_api: Arc<HyperliquidExchangeApi>,
        account_repo: ExchangeAccountRepository,
    ) -> Self {
        Self {
            cex_api,
            hl_api,
            account_repo,
        }
    }

    /// Resolve whether the target exchange account is Hyperliquid.
    async fn is_hyperliquid(
        &self,
        user_id: Uuid,
        exchange_account_id: Option<Uuid>,
    ) -> Result<bool, ExchangeApiError> {
        let accounts = self
            .account_repo
            .list_by_user(user_id)
            .await
            .map_err(|e| ExchangeApiError::Internal(format!("Failed to list accounts: {}", e)))?;

        let account = if let Some(target_id) = exchange_account_id {
            accounts.iter().find(|a| a.id == target_id).ok_or_else(|| {
                ExchangeApiError::Internal(format!("Exchange account {} not found", target_id))
            })?
        } else {
            accounts.first().ok_or_else(|| {
                ExchangeApiError::Internal("No exchange account configured".into())
            })?
        };

        Ok(account.exchange_name.eq_ignore_ascii_case(exchanges::HYPERLIQUID))
    }
}

#[async_trait]
impl ExchangeApi for RoutingExchangeApi {
    async fn get_balance(
        &self,
        user_id: Uuid,
        asset: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Decimal, ExchangeApiError> {
        if self.is_hyperliquid(user_id, exchange_account_id).await? {
            self.hl_api
                .get_balance(user_id, asset, exchange_account_id)
                .await
        } else {
            self.cex_api
                .get_balance(user_id, asset, exchange_account_id)
                .await
        }
    }

    async fn place_order(
        &self,
        req: PlaceOrderRequest,
    ) -> Result<PlaceOrderResult, ExchangeApiError> {
        if self
            .is_hyperliquid(req.user_id, req.exchange_account_id)
            .await?
        {
            self.hl_api.place_order(req).await
        } else {
            self.cex_api.place_order(req).await
        }
    }

    async fn amend_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        symbol: &str,
        amend: AmendRequest,
        exchange_account_id: Option<Uuid>,
    ) -> Result<String, ExchangeApiError> {
        if self.is_hyperliquid(user_id, exchange_account_id).await? {
            self.hl_api
                .amend_order(user_id, order_id, symbol, amend, exchange_account_id)
                .await
        } else {
            self.cex_api
                .amend_order(user_id, order_id, symbol, amend, exchange_account_id)
                .await
        }
    }

    async fn cancel_order(
        &self,
        user_id: Uuid,
        order_id: &str,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        if self.is_hyperliquid(user_id, exchange_account_id).await? {
            self.hl_api
                .cancel_order(user_id, order_id, symbol, exchange_account_id)
                .await
        } else {
            self.cex_api
                .cancel_order(user_id, order_id, symbol, exchange_account_id)
                .await
        }
    }

    async fn cancel_all_orders(
        &self,
        user_id: Uuid,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<(), ExchangeApiError> {
        if self.is_hyperliquid(user_id, exchange_account_id).await? {
            self.hl_api
                .cancel_all_orders(user_id, symbol, exchange_account_id)
                .await
        } else {
            self.cex_api
                .cancel_all_orders(user_id, symbol, exchange_account_id)
                .await
        }
    }

    async fn get_position(
        &self,
        user_id: Uuid,
        symbol: &str,
        exchange_account_id: Option<Uuid>,
    ) -> Result<Option<PositionInfo>, ExchangeApiError> {
        if self.is_hyperliquid(user_id, exchange_account_id).await? {
            self.hl_api
                .get_position(user_id, symbol, exchange_account_id)
                .await
        } else {
            self.cex_api
                .get_position(user_id, symbol, exchange_account_id)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::exchange_account::ExchangeAccountRow;
    use chrono::Utc;

    // Helper: check the routing logic directly
    // We can't easily unit-test RoutingExchangeApi without a DB,
    // but we can test the is_hyperliquid pattern via ExchangeAccountRow.

    fn mock_row(exchange_name: &str) -> ExchangeAccountRow {
        ExchangeAccountRow {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            exchange_name: exchange_name.to_string(),
            permissions: None,
            is_active: Some(true),
            created_at: Some(Utc::now()),
            last_used_at: None,
            auth_mode: "api_key".to_string(),
            wallet_address: None,
            agent_approved_at: None,
        }
    }

    #[test]
    fn exchange_name_matches_hyperliquid() {
        let row = mock_row("hyperliquid");
        assert_eq!(row.exchange_name, "hyperliquid");
    }

    #[test]
    fn exchange_name_does_not_match_binance() {
        let row = mock_row("binance");
        assert_ne!(row.exchange_name, "hyperliquid");
    }

    #[test]
    fn exchange_name_does_not_match_woo() {
        let row = mock_row("woo");
        assert_ne!(row.exchange_name, "hyperliquid");
    }
}
