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

use testudo_cli::model::state::{PnlPoint, RiskSnapshot, JournalSummary};

#[test]
fn pnl_point_stores_equity_values() {
    let point = PnlPoint {
        date: "2026-05-01".into(),
        cumulative_pnl: "1250.50".into(),
        equity: Some("50000.00".into()),
    };
    assert_eq!(point.date, "2026-05-01");
    assert_eq!(point.cumulative_pnl, "1250.50");
}

#[test]
fn risk_snapshot_tracks_limits() {
    let risk = RiskSnapshot {
        drawdown_pct: 3.2,
        drawdown_limit_pct: 20.0,
        active_positions: 2,
        max_positions: 5,
        session_signals: 8,
        max_signals_per_hour: 30,
        total_exposure: "15000.00".into(),
    };
    assert_eq!(risk.active_positions, 2);
    assert!(risk.drawdown_pct < risk.drawdown_limit_pct);
}

#[test]
fn journal_summary_has_stats() {
    let summary = JournalSummary {
        trade_count: 42,
        win_rate: "0.62".into(),
        profit_factor: "2.1".into(),
        avg_r_multiple: "1.8".into(),
        total_pnl: "+1250.50".into(),
        best_setup: "mean-reversion".into(),
    };
    assert_eq!(summary.trade_count, 42);
    assert_eq!(summary.best_setup, "mean-reversion");
}

#[test]
fn pnl_points_can_be_empty() {
    let points: Vec<PnlPoint> = vec![];
    assert!(points.len() < 2); // sparkline needs 2+ points
}
