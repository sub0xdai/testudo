// @anchor test:cli:panes
// @tags ui

use testudo_cli::model::state::{SignalEntry, Position};

#[test]
fn position_has_required_fields() {
    let pos = Position {
        symbol: "ETH_USDT".into(),
        side: "LONG".into(),
        entry_price: "3200.00".into(),
        current_price: "3250.00".into(),
        unrealized_pnl: "50.00".into(),
        quantity: "1.5".into(),
    };
    assert_eq!(pos.symbol, "ETH_USDT");
    assert_eq!(pos.side, "LONG");
    assert!(pos.unrealized_pnl.parse::<f64>().unwrap() > 0.0);
}

#[test]
fn signal_entry_has_required_fields() {
    let entry = SignalEntry {
        timestamp: "12:34:56".into(),
        symbol: "ETH_USDT".into(),
        side: "LONG".into(),
        status: "filled".into(),
        pnl: Some("+125.50".into()),
        reasoning: "Breakout confirmed".into(),
    };
    assert_eq!(entry.status, "filled");
    assert_eq!(entry.symbol, "ETH_USDT");
}

#[test]
fn signal_entry_rejected_status() {
    let entry = SignalEntry {
        timestamp: "12:35:00".into(),
        symbol: "BTC_USDT".into(),
        side: "SHORT".into(),
        status: "rejected".into(),
        pnl: None,
        reasoning: "Over-leveraged".into(),
    };
    assert_eq!(entry.status, "rejected");
    assert!(entry.pnl.is_none());
}

#[test]
fn position_list_can_be_empty() {
    let positions: Vec<Position> = vec![];
    assert!(positions.is_empty());
}

#[test]
fn signal_log_can_be_empty() {
    let signals: Vec<SignalEntry> = vec![];
    assert!(signals.is_empty());
}
