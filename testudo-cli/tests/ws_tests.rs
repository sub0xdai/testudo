// @anchor test:cli:ws
// @tags api

use testudo_cli::ws::client::WsClient;
use testudo_cli::ws::stream::{AgentAlert, AlertSeverity, AlertType, ExecutionReport};

#[test]
fn ws_client_builds_from_config() {
    let client = WsClient::new("ws://localhost:8081", "testudo_sk_abc123");
    assert_eq!(client.ws_url(), "ws://localhost:8081");
}

#[test]
fn ws_client_strips_trailing_slash() {
    let client = WsClient::new("ws://localhost:8081/", "key");
    assert_eq!(client.ws_url(), "ws://localhost:8081");
}

#[test]
fn ws_event_alert_has_type() {
    // Verify AgentAlert can be constructed
    let alert = AgentAlert {
        alert_type: AlertType::RiskBreach,
        severity: AlertSeverity::Concerning,
        message: "Drawdown limit reached".into(),
        current_value: None,
        limit_value: None,
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(alert.message, "Drawdown limit reached");
}

#[test]
fn ws_event_execution_report_has_fields() {
    let report = ExecutionReport {
        trade_group_id: uuid::Uuid::new_v4(),
        order_id: "ord_001".into(),
        status: "FILLED".into(),
        fill_price: Some(rust_decimal::Decimal::new(3200, 0)),
        exchange: "hyperliquid".into(),
        latency_ms: 45,
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(report.status, "FILLED");
}
