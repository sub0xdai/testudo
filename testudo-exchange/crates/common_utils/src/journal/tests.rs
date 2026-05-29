// @anchor exchange:common_utils:tests
// @tags infra

use chrono::{Duration, TimeZone, Utc};
use rust_decimal_macros::dec;
use uuid::Uuid;

use super::{hash_source_fills, reconstruct_trades, FillSide, RawFill, TradeSide};

fn make_fill(
    exec_id: &str,
    symbol: &str,
    side: FillSide,
    price: rust_decimal::Decimal,
    qty: rust_decimal::Decimal,
    offset_secs: i64,
) -> RawFill {
    let base = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    RawFill {
        user_id: Uuid::nil(),
        exchange: "bybit".into(),
        exec_id: exec_id.into(),
        symbol: symbol.into(),
        side,
        price,
        qty,
        fee: dec!(0.1),
        fee_asset: "USDT".into(),
        exec_time: base + Duration::seconds(offset_secs),
        order_id: None,
        raw_json: serde_json::Value::Null,
    }
}

// T1 — Long round trip
#[test]
fn test_long_round_trip() {
    let fills = vec![
        make_fill("A", "BTC_USDT", FillSide::Buy, dec!(50000), dec!(0.1), 1),
        make_fill("B", "BTC_USDT", FillSide::Sell, dec!(51000), dec!(0.1), 2),
    ];
    let trades = reconstruct_trades(&fills);
    assert_eq!(trades.len(), 1);
    let t = &trades[0];
    assert_eq!(t.side, TradeSide::Long);
    assert_eq!(t.entry_price, dec!(50000));
    assert_eq!(t.exit_price, dec!(51000));
    assert_eq!(t.quantity, dec!(0.1));
    assert_eq!(t.realized_pnl, dec!(100)); // (51000-50000)*0.1
}

// T2 — Short round trip
#[test]
fn test_short_round_trip() {
    let fills = vec![
        make_fill("A", "BTC_USDT", FillSide::Sell, dec!(50000), dec!(0.1), 1),
        make_fill("B", "BTC_USDT", FillSide::Buy, dec!(49000), dec!(0.1), 2),
    ];
    let trades = reconstruct_trades(&fills);
    assert_eq!(trades.len(), 1);
    let t = &trades[0];
    assert_eq!(t.side, TradeSide::Short);
    assert_eq!(t.entry_price, dec!(50000));
    assert_eq!(t.exit_price, dec!(49000));
    assert_eq!(t.realized_pnl, dec!(100)); // (50000-49000)*0.1
}

// T3 — FR-13 Bybit manual close: no SL/TP IDs, no clientOrderId, just entry+exit fills
#[test]
fn test_fr13_bybit_manual_close() {
    let fills = vec![
        make_fill("exec_entry", "BTC_USDT", FillSide::Buy, dec!(50000), dec!(0.05), 1),
        make_fill("exec_close", "BTC_USDT", FillSide::Sell, dec!(51000), dec!(0.05), 2),
    ];
    let trades = reconstruct_trades(&fills);
    assert_eq!(trades.len(), 1, "manual close must produce exactly one trade");
    let t = &trades[0];
    assert_eq!(t.side, TradeSide::Long);
    assert_eq!(t.entry_price, dec!(50000));
    assert_eq!(t.exit_price, dec!(51000));
    assert_eq!(t.quantity, dec!(0.05));
    assert_eq!(t.realized_pnl, dec!(50)); // (51000-50000)*0.05
    assert_eq!(t.exchange, "bybit");
    assert_eq!(t.symbol, "BTC_USDT");
}

// T4 — Partial entry fills, single close
#[test]
fn test_partial_entry_single_close() {
    let fills = vec![
        make_fill("A", "BTC_USDT", FillSide::Buy, dec!(100), dec!(0.5), 1),
        make_fill("B", "BTC_USDT", FillSide::Buy, dec!(110), dec!(0.5), 2),
        make_fill("C", "BTC_USDT", FillSide::Sell, dec!(120), dec!(1.0), 3),
    ];
    let trades = reconstruct_trades(&fills);
    assert_eq!(trades.len(), 1);
    let t = &trades[0];
    // entry = (100*0.5 + 110*0.5) / 1.0 = 105
    assert_eq!(t.entry_price, dec!(105));
    assert_eq!(t.exit_price, dec!(120));
    assert_eq!(t.quantity, dec!(1.0));
    assert_eq!(t.realized_pnl, dec!(15)); // (120-105)*1.0
}

// T5 — Scaled in / scaled out
#[test]
fn test_scaled_in_scaled_out() {
    let fills = vec![
        make_fill("A", "BTC_USDT", FillSide::Buy, dec!(100), dec!(0.3), 1),
        make_fill("B", "BTC_USDT", FillSide::Buy, dec!(200), dec!(0.7), 2),
        make_fill("C", "BTC_USDT", FillSide::Sell, dec!(300), dec!(0.4), 3),
        make_fill("D", "BTC_USDT", FillSide::Sell, dec!(400), dec!(0.6), 4),
    ];
    let trades = reconstruct_trades(&fills);
    assert_eq!(trades.len(), 1);
    let t = &trades[0];
    // entry = (100*0.3 + 200*0.7) / 1.0 = (30+140)/1.0 = 170
    assert_eq!(t.entry_price, dec!(170));
    // exit = (300*0.4 + 400*0.6) / 1.0 = (120+240)/1.0 = 360
    assert_eq!(t.exit_price, dec!(360));
    assert_eq!(t.quantity, dec!(1.0));
    assert_eq!(t.realized_pnl, dec!(190)); // (360-170)*1.0
}

// T6 — Side flip: Sell 2 closes the long AND opens a short; then Buy 1 closes the short
#[test]
fn test_side_flip_produces_two_trades() {
    let fills = vec![
        make_fill("A", "BTC_USDT", FillSide::Buy, dec!(100), dec!(1), 1),
        make_fill("B", "BTC_USDT", FillSide::Sell, dec!(110), dec!(2), 2), // flip fill
        make_fill("C", "BTC_USDT", FillSide::Buy, dec!(120), dec!(1), 3),
    ];
    let trades = reconstruct_trades(&fills);
    assert_eq!(trades.len(), 2, "side flip must produce two round-trip trades");
    // Trade 1: Long 100→110
    assert_eq!(trades[0].side, TradeSide::Long);
    assert_eq!(trades[0].entry_price, dec!(100));
    assert_eq!(trades[0].exit_price, dec!(110));
    assert_eq!(trades[0].quantity, dec!(1));
    // Trade 2: Short 110→120
    assert_eq!(trades[1].side, TradeSide::Short);
    assert_eq!(trades[1].entry_price, dec!(110));
    assert_eq!(trades[1].exit_price, dec!(120));
    assert_eq!(trades[1].quantity, dec!(1));
    // Hashes must differ
    assert_ne!(trades[0].source_fills_hash, trades[1].source_fills_hash);
}

// T7 — Multi-symbol interleaving: one trade per symbol
#[test]
fn test_multi_symbol_interleaving() {
    let fills = vec![
        make_fill("A", "BTC_USDT", FillSide::Buy, dec!(50000), dec!(0.1), 1),
        make_fill("C", "ETH_USDT", FillSide::Sell, dec!(2000), dec!(1.0), 2),
        make_fill("B", "BTC_USDT", FillSide::Sell, dec!(51000), dec!(0.1), 3),
        make_fill("D", "ETH_USDT", FillSide::Buy, dec!(1900), dec!(1.0), 4),
    ];
    let trades = reconstruct_trades(&fills);
    assert_eq!(trades.len(), 2);
    let btc = trades.iter().find(|t| t.symbol == "BTC_USDT").unwrap();
    let eth = trades.iter().find(|t| t.symbol == "ETH_USDT").unwrap();
    assert_eq!(btc.side, TradeSide::Long);
    assert_eq!(btc.realized_pnl, dec!(100));
    assert_eq!(eth.side, TradeSide::Short);
    assert_eq!(eth.realized_pnl, dec!(100));
}

// T8 — Out-of-order timestamps: sorted correctly, same output as ordered
#[test]
fn test_out_of_order_timestamps() {
    // Sell at t=2, Buy at t=1 — should be sorted to Buy first
    let fills = vec![
        make_fill("B", "BTC_USDT", FillSide::Sell, dec!(51000), dec!(0.1), 2),
        make_fill("A", "BTC_USDT", FillSide::Buy, dec!(50000), dec!(0.1), 1),
    ];
    let trades = reconstruct_trades(&fills);
    assert_eq!(trades.len(), 1);
    let t = &trades[0];
    assert_eq!(t.side, TradeSide::Long);
    assert_eq!(t.entry_price, dec!(50000));
    assert_eq!(t.exit_price, dec!(51000));
    assert_eq!(t.realized_pnl, dec!(100));
}

// T9 — Open position: no closing fill, must NOT emit a trade
#[test]
fn test_open_position_not_emitted() {
    let fills = vec![make_fill(
        "A",
        "BTC_USDT",
        FillSide::Buy,
        dec!(50000),
        dec!(1.0),
        1,
    )];
    let trades = reconstruct_trades(&fills);
    assert!(trades.is_empty(), "open positions must not be emitted");
}

// T10 — Duplicate exec_id idempotency: same hash on second run
#[test]
fn test_duplicate_exec_id_idempotency() {
    let fills = vec![
        make_fill("A", "BTC_USDT", FillSide::Buy, dec!(50000), dec!(0.1), 1),
        make_fill("B", "BTC_USDT", FillSide::Sell, dec!(51000), dec!(0.1), 2),
    ];
    let trades1 = reconstruct_trades(&fills);
    let trades2 = reconstruct_trades(&fills);
    assert_eq!(trades1.len(), 1);
    assert_eq!(trades2.len(), 1);
    assert_eq!(
        trades1[0].source_fills_hash, trades2[0].source_fills_hash,
        "same fills must produce identical hash on repeated runs"
    );
}

// T11 — Late-arriving fill: tick 2 adds new fills forming a second round trip
#[test]
fn test_late_arriving_fill() {
    let tick1_fills = vec![
        make_fill("A", "BTC_USDT", FillSide::Buy, dec!(50000), dec!(0.1), 1),
        make_fill("B", "BTC_USDT", FillSide::Sell, dec!(51000), dec!(0.1), 2),
    ];
    let tick2_fills = vec![
        make_fill("A", "BTC_USDT", FillSide::Buy, dec!(50000), dec!(0.1), 1),
        make_fill("B", "BTC_USDT", FillSide::Sell, dec!(51000), dec!(0.1), 2),
        make_fill("C", "BTC_USDT", FillSide::Buy, dec!(52000), dec!(0.1), 3),
        make_fill("D", "BTC_USDT", FillSide::Sell, dec!(53000), dec!(0.1), 4),
    ];

    let trades1 = reconstruct_trades(&tick1_fills);
    let trades2 = reconstruct_trades(&tick2_fills);

    assert_eq!(trades1.len(), 1);
    assert_eq!(trades2.len(), 2);

    let h1_tick1 = &trades1[0].source_fills_hash;
    let h1_tick2 = trades2.iter().find(|t| t.source_fills_hash == *h1_tick1);
    assert!(h1_tick2.is_some(), "first trade hash must be stable across ticks");

    let h2 = trades2.iter().find(|t| t.source_fills_hash != *h1_tick1).unwrap();
    assert_eq!(h2.entry_price, dec!(52000));
    assert_eq!(h2.exit_price, dec!(53000));
}

// hash_source_fills unit test: deterministic and order-independent
#[test]
fn test_hash_source_fills_determinism() {
    let ids1 = vec!["C".to_string(), "A".to_string(), "B".to_string()];
    let ids2 = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    assert_eq!(hash_source_fills(&ids1), hash_source_fills(&ids2));
    // Different set produces different hash
    let ids3 = vec!["A".to_string(), "B".to_string()];
    assert_ne!(hash_source_fills(&ids1), hash_source_fills(&ids3));
}
