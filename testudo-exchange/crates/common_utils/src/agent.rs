//! AGENT-02: Agent alert and execution report types.
//!
//! Shared between ws-stream (WebSocket subscriber) and router (alert emitter).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAlert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_value: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_value: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AlertType {
    RiskBreach,
    DrawdownWarning,
    DrawdownLimit,
    MarginCall,
    AgentWalletExpiring,
    AgentWalletExpired,
    MaxPositionsReached,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Info,
    Notable,
    Concerning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub trade_group_id: Uuid,
    pub order_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_price: Option<Decimal>,
    pub exchange: String,
    pub latency_ms: u64,
    pub timestamp: DateTime<Utc>,
}
