// @anchor test:cli:api
// @tags api

use testudo_cli::api::client::ApiClient;
use testudo_cli::api::types::ApiError;
use testudo_cli::config::ApiConfig;

fn make_api_config() -> ApiConfig {
    ApiConfig {
        base_url: "http://localhost:8080/api/v1".into(),
        agent_key: "testudo_sk_test123".into(),
        ws_url: "ws://localhost:8081".into(),
    }
}

#[test]
fn api_client_new_stores_config() {
    let cfg = make_api_config();
    let client = ApiClient::new(&cfg);

    assert_eq!(client.base_url(), "http://localhost:8080/api/v1");
    assert_eq!(client.agent_key(), "testudo_sk_test123");
}

#[test]
fn api_client_strips_trailing_slash() {
    let cfg = ApiConfig {
        base_url: "http://localhost:8080/api/v1/".into(),
        agent_key: "key".into(),
        ws_url: "ws://localhost:8081".into(),
    };
    let client = ApiClient::new(&cfg);
    assert_eq!(client.base_url(), "http://localhost:8080/api/v1");
}

#[test]
fn api_error_display_unauthorized() {
    let err = ApiError::Unauthorized;
    let msg = format!("{}", err);
    assert!(msg.contains("Unauthorized") || msg.contains("unauthorized"));
}

#[test]
fn api_error_display_network() {
    let err = ApiError::Network("connection refused".into());
    let msg = format!("{}", err);
    assert!(msg.contains("connection refused"));
}

#[test]
fn signal_input_deserializes_from_json() {
    let json = r#"{
        "symbol": "ETH_USDT",
        "side": "LONG",
        "entry_price": "3200.50",
        "stop_loss": "3150.00",
        "take_profit": [{"price": "3300.00", "quantity": "0.5"}],
        "execution_mode": "SHADOW",
        "reasoning": "mean reversion setup",
        "confidence": "0.75",
        "idempotency_key": "550e8400-e29b-41d4-a716-446655440000"
    }"#;

    let input: testudo_cli::api::types::SignalInput =
        serde_json::from_str(json).expect("should deserialize SignalInput");

    assert_eq!(input.symbol, "ETH_USDT");
    assert_eq!(input.entry_price.to_string(), "3200.50");
    assert!(matches!(
        input.side,
        testudo_cli::api::types::SignalSide::Long
    ));
}

#[test]
fn signal_result_deserializes_success() {
    let json = r#"{
        "success": true,
        "trade_group_id": "550e8400-e29b-41d4-a716-446655440000",
        "entry_order_id": "ord_abc123",
        "position_size": "1.5",
        "execution_mode": "SHADOW",
        "agent_key_id": "660e8400-e29b-41d4-a716-446655440001"
    }"#;

    let result: testudo_cli::api::types::SignalResult =
        serde_json::from_str(json).expect("should deserialize SignalResult");

    assert!(result.success);
    assert_eq!(result.position_size.unwrap().to_string(), "1.5");
}

#[test]
fn agent_summary_deserializes_from_json() {
    let json = r#"{
        "timeframe": {"label": "Last 30 days", "from": "2026-05-01", "to": "2026-05-31"},
        "overall": {
            "trade_count": 42,
            "win_rate": "0.62",
            "avg_r_multiple": "1.8",
            "total_pnl": "1250.50",
            "max_drawdown": "340.00",
            "profit_factor": "2.1"
        },
        "by_setup": [],
        "top_trades": [],
        "equity": []
    }"#;

    let summary: testudo_cli::api::types::AgentSummary =
        serde_json::from_str(json).expect("should deserialize AgentSummary");

    assert_eq!(summary.overall.trade_count, 42);
    assert_eq!(summary.overall.total_pnl.to_string(), "1250.50");
}

#[test]
fn kline_data_deserializes_from_json() {
    let json = r#"{
        "timestamp": 1717200000,
        "open": "3200.00",
        "high": "3250.00",
        "low": "3180.00",
        "close": "3220.00",
        "volume": "150.5",
        "quote_volume": "485000.00"
    }"#;

    let kline: testudo_cli::api::types::KlineData =
        serde_json::from_str(json).expect("should deserialize KlineData");

    assert_eq!(kline.timestamp, 1717200000);
    assert_eq!(kline.close.to_string(), "3220.00");
}

#[test]
fn onboarding_status_deserializes_from_json() {
    let json = r#"{
        "is_ready": false,
        "next_step": "connect_exchange",
        "missing": ["No exchange account connected"],
        "has_trades": false
    }"#;

    let status: testudo_cli::api::types::OnboardingStatus =
        serde_json::from_str(json).expect("should deserialize OnboardingStatus");

    assert!(!status.is_ready);
    assert_eq!(status.next_step, "connect_exchange");
    assert_eq!(status.missing.len(), 1);
}

#[test]
fn risk_config_deserializes_from_json() {
    let json = r#"{
        "account_risk_percent": "2.0",
        "max_leverage": 5,
        "require_stop_loss": true,
        "max_open_positions": 3
    }"#;

    let cfg: testudo_cli::api::types::RiskConfigData =
        serde_json::from_str(json).expect("should deserialize RiskConfigData");

    assert_eq!(cfg.account_risk_percent.to_string(), "2.0");
    assert_eq!(cfg.max_leverage, 5);
    assert!(cfg.require_stop_loss);
}
