//! CCXT fill source — wraps the sidecar POST /trades/since endpoint.

// @anchor exchange:router:ccxt
// @tags api

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use common_utils::journal::{FillSide, RawFill};
use common_utils::models::canonical_exchange_name;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

use crate::repositories::exchange_account::ExchangeAccountRepository;
use crate::services::cex_client::{CexClient, SidecarCredentials};
use super::{FillSource, SyncError};

pub struct CcxtFillSource {
    cex_client: Arc<CexClient>,
    exchange_account_repo: ExchangeAccountRepository,
    sandbox: bool,
    label: String,
}

impl CcxtFillSource {
    pub fn new(
        cex_client: Arc<CexClient>,
        exchange_account_repo: ExchangeAccountRepository,
        sandbox: bool,
        exchange_label: impl Into<String>,
    ) -> Self {
        Self {
            cex_client,
            exchange_account_repo,
            sandbox,
            label: exchange_label.into(),
        }
    }
}

#[async_trait]
impl FillSource for CcxtFillSource {
    async fn fetch_since(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<RawFill>, SyncError> {
        let creds = self
            .exchange_account_repo
            .load_credentials(account_id, user_id)
            .await
            .map_err(|e| SyncError::Credential(e.to_string()))?;

        let exchange_id = canonical_exchange_name(&creds.exchange_name);
        let sidecar_creds = SidecarCredentials {
            api_key: creds.api_key,
            secret: creds.api_secret,
            password: creds.passphrase,
        };
        let since_ms = since.timestamp_millis();

        let items = self
            .cex_client
            .fetch_trades_since(&exchange_id, &sidecar_creds, self.sandbox, since_ms, None, None)
            .await
            .map_err(|e| {
                if e.to_string().contains("Rate limit") || e.to_string().contains("rate limit") {
                    SyncError::RateLimit
                } else {
                    SyncError::Network(e.to_string())
                }
            })?;

        let mut fills = Vec::with_capacity(items.len());
        for item in items {
            let side = match item.side.to_lowercase().as_str() {
                "buy" => FillSide::Buy,
                "sell" => FillSide::Sell,
                other => {
                    tracing::warn!(exec_id = %item.exec_id, side = %other, "unknown fill side — skipping");
                    continue;
                }
            };

            let price = Decimal::from_str(&item.price)
                .map_err(|e| SyncError::Deser(format!("price: {e}")))?;
            let qty = Decimal::from_str(&item.qty)
                .map_err(|e| SyncError::Deser(format!("qty: {e}")))?;
            let fee = Decimal::from_str(&item.fee).unwrap_or(Decimal::ZERO);

            let exec_time = Utc
                .timestamp_millis_opt(item.exec_time_ms)
                .single()
                .ok_or_else(|| SyncError::Deser(format!("invalid exec_time_ms: {}", item.exec_time_ms)))?;

            fills.push(RawFill {
                user_id,
                exchange: exchange_id.clone(),
                exec_id: item.exec_id,
                symbol: item.symbol,
                side,
                price,
                qty,
                fee,
                fee_asset: item.fee_asset,
                exec_time,
                order_id: item.order_id,
                raw_json: item.raw_json,
            });
        }

        Ok(fills)
    }

    fn exchange_label(&self) -> &str {
        &self.label
    }
}
