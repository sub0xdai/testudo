//! Hyperliquid fill source — calls info.user_fills_by_time REST endpoint.
//!
//! Stores ALL fills (opening and closing) in raw_fills — reconstruct_trades
//! derives round trips from net-qty crossings, so filtering by closed_pnl here
//! would break opening fill inclusion.

use alloy::primitives::Address;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use common_utils::journal::{FillSide, RawFill};
use hyperliquid_sdk_rs::{types::info_types::UserFillByTime, InfoProvider, Network};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use crate::repositories::exchange_account::ExchangeAccountRepository;
use super::{FillSource, SyncError};

pub struct HyperliquidFillSource {
    info: InfoProvider,
    exchange_account_repo: ExchangeAccountRepository,
    label: String,
}

impl HyperliquidFillSource {
    pub fn new(network: Network, exchange_account_repo: ExchangeAccountRepository) -> Self {
        Self {
            info: InfoProvider::new(network),
            exchange_account_repo,
            label: "hyperliquid".to_string(),
        }
    }
}

#[async_trait]
impl FillSource for HyperliquidFillSource {
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

        let wallet_str = creds
            .wallet_address
            .ok_or_else(|| SyncError::Credential("HL account missing wallet_address".to_string()))?;

        let user_address: Address = wallet_str
            .parse()
            .map_err(|e| SyncError::Credential(format!("invalid wallet address: {e}")))?;

        let since_ms = since.timestamp_millis() as u64;

        let hl_fills = self
            .info
            .user_fills_by_time(user_address, since_ms, None, None)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("429") || msg.to_lowercase().contains("rate") {
                    SyncError::RateLimit
                } else {
                    SyncError::Network(msg)
                }
            })?;

        tracing::debug!(
            user_id = %user_id,
            fill_count = hl_fills.len(),
            since_ms,
            "HyperliquidFillSource: fetched fills"
        );

        let fills = hl_fills
            .iter()
            .filter_map(|f| convert_hl_fill(f, user_id))
            .collect();

        Ok(fills)
    }

    fn exchange_label(&self) -> &str {
        &self.label
    }
}

/// Convert a Hyperliquid `UserFillByTime` into a `RawFill`.
///
/// Returns `None` for spot fills (coin starts with '@') and unparseable fields.
/// Does NOT filter by `closed_pnl` — raw_fills stores all fills; reconstruct_trades
/// derives closed round trips from net-qty crossings.
pub(crate) fn convert_hl_fill(fill: &UserFillByTime, user_id: Uuid) -> Option<RawFill> {
    // Skip spot fills (coin starts with '@')
    if fill.coin.starts_with('@') {
        return None;
    }

    let side = match fill.side.as_str() {
        "B" => FillSide::Buy,
        "A" => FillSide::Sell,
        other => {
            tracing::warn!(tid = fill.tid, side = %other, "HL fill: unknown side — skipping");
            return None;
        }
    };

    let price = Decimal::from_str(&fill.px).ok()?;
    let qty = Decimal::from_str(&fill.sz).ok()?;
    if qty == Decimal::ZERO {
        return None;
    }
    let fee = Decimal::from_str(&fill.fee).unwrap_or(Decimal::ZERO);

    let exec_time = Utc.timestamp_millis_opt(fill.time as i64).single()?;

    Some(RawFill {
        user_id,
        exchange: "hyperliquid".to_string(),
        exec_id: fill.tid.to_string(),
        symbol: format!("{}_USDT", fill.coin),
        side,
        price,
        qty,
        fee,
        fee_asset: fill.fee_token.clone(),
        exec_time,
        order_id: Some(fill.oid.to_string()),
        raw_json: serde_json::to_value(fill).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn assert_send_sync<T: Send + Sync>() {}

    fn make_fill(coin: &str, side: &str, px: &str, sz: &str, tid: u64, time: u64) -> UserFillByTime {
        UserFillByTime {
            closed_pnl: "0".to_string(),
            coin: coin.to_string(),
            crossed: true,
            dir: "Open Long".to_string(),
            hash: "0xabc".to_string(),
            oid: 12345,
            px: px.to_string(),
            side: side.to_string(),
            start_position: "0".to_string(),
            sz: sz.to_string(),
            time,
            fee: "0.5".to_string(),
            fee_token: "USDC".to_string(),
            tid,
            cloid: None,
        }
    }

    #[test]
    fn hl_fill_source_is_send_sync() {
        assert_send_sync::<HyperliquidFillSource>();
    }

    #[test]
    fn converts_buy_fill() {
        let fill = make_fill("BTC", "B", "50000", "0.1", 1001, 1700000000000);
        let user_id = Uuid::new_v4();
        let raw = convert_hl_fill(&fill, user_id).expect("should convert");

        assert_eq!(raw.exec_id, "1001");
        assert_eq!(raw.symbol, "BTC_USDT");
        assert_eq!(raw.side, FillSide::Buy);
        assert_eq!(raw.price, dec!(50000));
        assert_eq!(raw.qty, dec!(0.1));
        assert_eq!(raw.fee, dec!(0.5));
        assert_eq!(raw.fee_asset, "USDC");
        assert_eq!(raw.exchange, "hyperliquid");
        assert_eq!(raw.order_id, Some("12345".to_string()));
    }

    #[test]
    fn converts_sell_fill() {
        let fill = make_fill("ETH", "A", "3000", "1.0", 2002, 1700000001000);
        let user_id = Uuid::new_v4();
        let raw = convert_hl_fill(&fill, user_id).expect("should convert");

        assert_eq!(raw.side, FillSide::Sell);
        assert_eq!(raw.symbol, "ETH_USDT");
    }

    #[test]
    fn skips_spot_fills() {
        let fill = make_fill("@1", "B", "1.0", "100", 3003, 1700000002000);
        let user_id = Uuid::new_v4();
        assert!(convert_hl_fill(&fill, user_id).is_none());
    }

    #[test]
    fn skips_zero_qty() {
        let fill = make_fill("BTC", "B", "50000", "0", 4004, 1700000003000);
        let user_id = Uuid::new_v4();
        assert!(convert_hl_fill(&fill, user_id).is_none());
    }

    #[test]
    fn stores_opening_fills_without_closed_pnl_filter() {
        // Opening fills have closed_pnl = "0"; we must NOT skip them.
        let mut fill = make_fill("BTC", "B", "50000", "0.1", 5005, 1700000004000);
        fill.closed_pnl = "0".to_string();
        let user_id = Uuid::new_v4();
        let raw = convert_hl_fill(&fill, user_id);
        assert!(raw.is_some(), "opening fills (closed_pnl=0) must be stored");
    }

    #[test]
    fn uses_tid_as_exec_id() {
        let fill = make_fill("SOL", "B", "150", "5.0", 99999, 1700000005000);
        let user_id = Uuid::new_v4();
        let raw = convert_hl_fill(&fill, user_id).unwrap();
        assert_eq!(raw.exec_id, "99999");
    }

    #[test]
    fn unknown_side_returns_none() {
        let fill = make_fill("BTC", "X", "50000", "0.1", 6006, 1700000006000);
        let user_id = Uuid::new_v4();
        assert!(convert_hl_fill(&fill, user_id).is_none());
    }
}
