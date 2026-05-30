//! Agent Signal Types
//!
//! Wire-level types for the POST /api/v1/signals endpoint.
//! SignalInput is deserialized from the agent's JSON payload;
//! SignalResult is the structured response returned after
//! the risk engine processes the signal.

// @anchor exchange:router:agent_signal
// @tags api

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common_utils::risk::SizingMethod;

/// Signal submitted by an external agent.
#[derive(Debug, Deserialize)]
pub struct SignalInput {
    pub symbol: String,
    pub side: SignalSide,
    pub entry_price: Decimal,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Vec<TakeProfitTarget>,
    pub exchange_account_id: Option<Uuid>,
    pub execution_mode: ExecutionMode,
    pub reasoning: Option<String>,
    /// Agent identifier, e.g. "agent:hermes_v1.2"
    pub source: Option<String>,
    /// 0.0–1.0, stored for Kelly calibration
    pub confidence: Option<Decimal>,
    pub idempotency_key: Option<Uuid>,
    pub leverage: Option<u8>,
    pub management: Option<SignalManagement>,
}

#[derive(Debug, Deserialize)]
pub struct TakeProfitTarget {
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SignalSide {
    Long,
    Short,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecutionMode {
    Shadow,
    Live,
}

#[derive(Debug, Deserialize)]
pub struct SignalManagement {
    pub break_even_enabled: Option<bool>,
    pub break_even_at: Option<Decimal>,
    pub trailing_stop: Option<TrailingStopConfig>,
    pub partial_tp: Option<PartialTpConfig>,
}

#[derive(Debug, Deserialize)]
pub struct TrailingStopConfig {
    pub enabled: bool,
    pub distance_percent: u32,
}

#[derive(Debug, Deserialize)]
pub struct PartialTpConfig {
    pub enabled: bool,
    pub close_percent: u32,
}

/// Structured result returned to the agent after signal processing.
#[derive(Debug, Serialize)]
pub struct SignalResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_group_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_size: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizing_method: Option<SizingMethod>,
    pub execution_mode: ExecutionMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_key_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection: Option<SignalRejection>,
}

#[derive(Debug, Serialize)]
pub struct SignalRejection {
    pub reason: String,
    pub code: String,
}

impl SignalResult {
    pub fn success(
        trade_group_id: Uuid,
        entry_order_id: String,
        position_size: Decimal,
        sizing_method: SizingMethod,
        execution_mode: ExecutionMode,
        warnings: Vec<String>,
        agent_key_id: Option<Uuid>,
    ) -> Self {
        Self {
            success: true,
            trade_group_id: Some(trade_group_id),
            entry_order_id: Some(entry_order_id),
            position_size: Some(position_size),
            sizing_method: Some(sizing_method),
            execution_mode,
            warnings,
            agent_key_id,
            rejection: None,
        }
    }

    pub fn rejected(reason: String, code: String, execution_mode: ExecutionMode) -> Self {
        Self {
            success: false,
            trade_group_id: None,
            entry_order_id: None,
            position_size: None,
            sizing_method: None,
            execution_mode,
            warnings: Vec::new(),
            agent_key_id: None,
            rejection: Some(SignalRejection { reason, code }),
        }
    }
}
