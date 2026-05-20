//! REL-02: Shared HL fill → journal trade conversion.
//!
//! Extracted from `import_worker::process_hl_fill()` so that both the
//! batch import worker and the live REST poll loop in `ws_fills` can
//! convert Hyperliquid closing fills into `TradeCloseEvent` without
//! duplicating logic.

use chrono::{DateTime, TimeZone, Utc};
use hyperliquid_sdk_rs::types::info_types::UserFillByTime;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use super::journal_service::TradeCloseEvent;

/// Build a `TradeCloseEvent` from an HL closing fill.
///
/// Returns `None` for non-closing fills (closedPnl == "0"), spot fills
/// (coin starts with '@'), or unparseable numeric fields.
///
/// `source` identifies the caller ("import_hl" or "live_poll").
pub fn build_trade_close_event(
    fill: &UserFillByTime,
    user_id: Uuid,
    open_time_ms: Option<u64>,
    source: &str,
) -> Option<TradeCloseEvent> {
    // Filter non-closing fills
    if fill.closed_pnl == "0" || fill.closed_pnl == "0.0" {
        return None;
    }
    // Skip spot fills
    if fill.coin.starts_with('@') {
        return None;
    }

    let exit_price = Decimal::from_str(&fill.px).ok()?;
    let quantity = Decimal::from_str(&fill.sz).ok()?;
    let closed_pnl = Decimal::from_str(&fill.closed_pnl).ok()?;
    let fee = Decimal::from_str(&fill.fee).ok()?;

    if quantity == Decimal::ZERO {
        return None;
    }

    // Determine side from dir field
    let side = if fill.dir.contains("Long") {
        "LONG"
    } else if fill.dir.contains("Short") {
        "SHORT"
    } else {
        // Fallback: B = buy side, closing a short; A = sell side, closing a long
        match fill.side.as_str() {
            "B" => "SHORT",
            "A" => "LONG",
            _ => return None,
        }
    };

    // Derive entry price from closedPnl
    // Long: pnl = (exit - entry) * qty → entry = exit - (pnl / qty)
    // Short: pnl = (entry - exit) * qty → entry = exit + (pnl / qty)
    let entry_price = match side {
        "LONG" => exit_price - (closed_pnl / quantity),
        "SHORT" => exit_price + (closed_pnl / quantity),
        _ => exit_price,
    };

    let closed_at = timestamp_to_datetime(fill.time);
    let opened_at = open_time_ms
        .map(timestamp_to_datetime)
        .unwrap_or(closed_at);
    let symbol = format!("{}_USDT", fill.coin);

    Some(TradeCloseEvent {
        user_id,
        exchange: "hyperliquid".to_string(),
        symbol,
        side: side.to_string(),
        entry_price,
        exit_price,
        quantity,
        leverage: 1,
        fees: fee,
        stop_price: None,
        target_price: None,
        risk_amount: None,
        opened_at,
        closed_at,
        trade_group_id: None,
        source: Some(source.to_string()),
        exchange_fill_id: Some(fill.tid as i64),
        reasoning: None,
        confidence: None,
        setup_tag: None,
        kelly_inputs: None,
        needs_reconciliation: false,
    })
}

/// Convert a millisecond Unix timestamp to a `DateTime<Utc>`.
pub fn timestamp_to_datetime(ms: u64) -> DateTime<Utc> {
    let secs = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    Utc.timestamp_opt(secs, nanos)
        .single()
        .unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_closing_fill(
        coin: &str,
        dir: &str,
        side: &str,
        px: &str,
        sz: &str,
        closed_pnl: &str,
        fee: &str,
        oid: u64,
        tid: u64,
        time: u64,
    ) -> UserFillByTime {
        UserFillByTime {
            closed_pnl: closed_pnl.to_string(),
            coin: coin.to_string(),
            crossed: true,
            dir: dir.to_string(),
            hash: "0xabc".to_string(),
            oid,
            px: px.to_string(),
            side: side.to_string(),
            start_position: "0".to_string(),
            sz: sz.to_string(),
            time,
            fee: fee.to_string(),
            fee_token: "USDC".to_string(),
            tid,
            cloid: None,
        }
    }

    #[test]
    fn closing_long_fill_produces_correct_event() {
        let fill = make_closing_fill(
            "BTC", "Close Long", "A", "65000", "0.1", "500", "1.5",
            123, 456, 1710000000000,
        );
        let user_id = Uuid::new_v4();
        let event = build_trade_close_event(&fill, user_id, None, "test").unwrap();

        assert_eq!(event.exchange, "hyperliquid");
        assert_eq!(event.symbol, "BTC_USDT");
        assert_eq!(event.side, "LONG");
        assert_eq!(event.exit_price, dec!(65000));
        assert_eq!(event.quantity, dec!(0.1));
        // entry = exit - (pnl / qty) = 65000 - (500 / 0.1) = 65000 - 5000 = 60000
        assert_eq!(event.entry_price, dec!(60000));
        assert_eq!(event.fees, dec!(1.5));
        assert_eq!(event.exchange_fill_id, Some(456));
        assert_eq!(event.source, Some("test".to_string()));
    }

    #[test]
    fn closing_short_fill_produces_correct_event() {
        let fill = make_closing_fill(
            "ETH", "Close Short", "B", "3000", "1.0", "200", "0.5",
            789, 101, 1710000000000,
        );
        let user_id = Uuid::new_v4();
        let event = build_trade_close_event(&fill, user_id, None, "test").unwrap();

        assert_eq!(event.side, "SHORT");
        assert_eq!(event.exit_price, dec!(3000));
        // entry = exit + (pnl / qty) = 3000 + (200 / 1.0) = 3200
        assert_eq!(event.entry_price, dec!(3200));
    }

    #[test]
    fn non_closing_fill_returns_none() {
        let fill = make_closing_fill(
            "BTC", "Open Long", "B", "65000", "0.1", "0", "1.5",
            123, 456, 1710000000000,
        );
        assert!(build_trade_close_event(&fill, Uuid::new_v4(), None, "test").is_none());
    }

    #[test]
    fn zero_pnl_string_variant_returns_none() {
        let fill = make_closing_fill(
            "BTC", "Close Long", "A", "65000", "0.1", "0.0", "1.5",
            123, 456, 1710000000000,
        );
        assert!(build_trade_close_event(&fill, Uuid::new_v4(), None, "test").is_none());
    }

    #[test]
    fn spot_fill_returns_none() {
        let fill = make_closing_fill(
            "@BTC", "Close Long", "A", "65000", "0.1", "500", "1.5",
            123, 456, 1710000000000,
        );
        assert!(build_trade_close_event(&fill, Uuid::new_v4(), None, "test").is_none());
    }

    #[test]
    fn zero_quantity_returns_none() {
        let fill = make_closing_fill(
            "BTC", "Close Long", "A", "65000", "0", "500", "1.5",
            123, 456, 1710000000000,
        );
        assert!(build_trade_close_event(&fill, Uuid::new_v4(), None, "test").is_none());
    }

    #[test]
    fn side_fallback_from_side_field() {
        // dir doesn't contain "Long" or "Short" — use side field fallback
        let fill = make_closing_fill(
            "BTC", "Liquidation", "B", "65000", "0.1", "500", "1.5",
            123, 456, 1710000000000,
        );
        let event = build_trade_close_event(&fill, Uuid::new_v4(), None, "test").unwrap();
        assert_eq!(event.side, "SHORT"); // B = buying to close = was short
    }

    #[test]
    fn open_time_used_for_duration() {
        let fill = make_closing_fill(
            "BTC", "Close Long", "A", "65000", "0.1", "500", "1.5",
            123, 456, 1710000060000, // closed at T+60s
        );
        let event = build_trade_close_event(
            &fill, Uuid::new_v4(), Some(1710000000000), "test", // opened at T+0
        ).unwrap();

        let duration = (event.closed_at - event.opened_at).num_seconds();
        assert_eq!(duration, 60);
    }

    #[test]
    fn missing_open_time_uses_close_time() {
        let fill = make_closing_fill(
            "BTC", "Close Long", "A", "65000", "0.1", "500", "1.5",
            123, 456, 1710000000000,
        );
        let event = build_trade_close_event(&fill, Uuid::new_v4(), None, "test").unwrap();
        assert_eq!(event.opened_at, event.closed_at);
    }

    #[test]
    fn timestamp_to_datetime_basic() {
        let dt = timestamp_to_datetime(1710000000000);
        assert_eq!(dt.timestamp(), 1710000000);
    }

    #[test]
    fn timestamp_to_datetime_with_millis() {
        let dt = timestamp_to_datetime(1710000000500);
        assert_eq!(dt.timestamp(), 1710000000);
        assert_eq!(dt.timestamp_subsec_millis(), 500);
    }
}
