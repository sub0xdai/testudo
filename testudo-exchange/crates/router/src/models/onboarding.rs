//! Onboarding status types for GET /api/v1/onboarding/status.
//!
//! Provides a single-call readiness endpoint for AI agents (Hermes, pi, OpenClaw)
//! to discover what the user needs before trading can begin.

// @anchor exchange:router:onboarding-types
// @tags api

use serde::Serialize;
use uuid::Uuid;

use common_utils::risk::RiskConfig;

/// Response from GET /api/v1/onboarding/status.
#[derive(Debug, Serialize)]
pub struct OnboardingStatus {
    /// True when the user has everything needed to start trading.
    pub is_ready: bool,

    /// Prescriptive next action for the agent to guide the user through.
    pub next_step: OnboardingStep,

    /// Human-readable descriptions of what's missing.
    /// Empty when is_ready is true.
    pub missing: Vec<String>,

    /// Available exchanges with credential requirements.
    /// Present when next_step is "connect_exchange".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_exchanges: Option<Vec<ExchangeOption>>,

    /// Pending agent wallet that needs EIP-712 approval.
    /// Present when next_step is "approve_agent_wallet".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_agent_wallet: Option<PendingAgentWallet>,

    /// Whether the user has any trade history at all.
    pub has_trades: bool,

    /// Current risk configuration (so agent can surface settings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_config: Option<RiskConfigSummary>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStep {
    /// Agent should guide user through SIWE/SIWS authentication.
    Authenticate,

    /// No exchange account connected. Agent should present exchange options.
    ConnectExchange,

    /// Agent wallet initialized but not approved. Agent should guide through EIP-712 signing.
    ApproveAgentWallet,

    /// Risk config is at defaults. Agent should offer to customize.
    ConfigureRisk,

    /// Everything is ready. Agent can start trading.
    ReadyToTrade,
}

#[derive(Debug, Serialize)]
pub struct ExchangeOption {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub exchange_type: String,
    pub required_credentials: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PendingAgentWallet {
    pub account_id: Uuid,
    pub agent_address: String,
    pub wallet_address: String,
    /// True if an existing agent wallet needs re-authorization.
    pub requires_reauthorization: bool,
}

#[derive(Debug, Serialize)]
pub struct RiskConfigSummary {
    pub account_risk_percent: String,
    pub max_leverage: i32,
    pub daily_drawdown_limit: Option<String>,
    pub stop_loss_required: bool,
}

impl From<RiskConfig> for RiskConfigSummary {
    fn from(config: RiskConfig) -> Self {
        Self {
            account_risk_percent: config.account_risk_percent.to_string(),
            max_leverage: config.max_leverage as i32,
            daily_drawdown_limit: config
                .daily_max_drawdown_percent
                .map(|d| d.to_string()),
            stop_loss_required: config.require_stop_loss,
        }
    }
}
