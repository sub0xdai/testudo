use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsResponse {
    pub stream: String,
    pub data: serde_json::Value, // any kind of JSON-like data - initialise using serde_json::json! macro
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsMessage {
    pub method: String,
    pub params: Vec<String>,
    pub id: u32,
}

impl WsMessage {
    pub fn parse_subscription(&self) -> Option<SubscriptionTarget> {
        if self.params.is_empty() {
            return None;
        }

        let subscription_id = &self.params[0];

        // Agent channels: agent.<type>.<user_id> (3 parts)
        if subscription_id.starts_with("agent.") {
            let parts: Vec<&str> = subscription_id.splitn(3, '.').collect();
            if parts.len() == 3 {
                let prefix = format!("{}.{}", parts[0], parts[1]);
                if let Some(ch) = AgentChannel::parse(&prefix) {
                    return Some(SubscriptionTarget::Agent(ch, parts[2].to_string()));
                }
            }
            return None;
        }

        // Standard channels: <type>.<topic> (2 parts)
        let parts: Vec<&str> = subscription_id.split('.').collect();
        if parts.len() != 2 {
            return None;
        }

        let subscription_type = SubscriptionType::parse(parts[0])?;
        Some(SubscriptionTarget::Standard(subscription_type, parts[1].to_string()))
    }
}

#[derive(Debug, Clone)]
pub enum SubscriptionType {
    #[allow(non_camel_case_types)]
    depth,
    #[allow(non_camel_case_types)]
    trade,
    #[allow(non_camel_case_types)]
    ticker,
    #[allow(non_camel_case_types)]
    balance,
    #[allow(non_camel_case_types)]
    order,
}

impl SubscriptionType {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "depth" => Some(SubscriptionType::depth),
            "trade" => Some(SubscriptionType::trade),
            "ticker" => Some(SubscriptionType::ticker),
            "balance" => Some(SubscriptionType::balance),
            "order" => Some(SubscriptionType::order),
            _ => None,
        }
    }
}

/// Unified subscription target — either a standard channel or an agent channel.
#[derive(Debug, Clone)]
pub enum SubscriptionTarget {
    Standard(SubscriptionType, String),  // type + topic (e.g. depth.BTC_USDT)
    Agent(AgentChannel, String),          // agent channel + user_id
}

impl SubscriptionTarget {
    /// Produce the channel name string used for LISTEN/UNLISTEN and WsResponse.stream.
    pub fn subscription_id(&self) -> String {
        match self {
            SubscriptionTarget::Standard(st, topic) => format!("{:?}.{}", st, topic),
            SubscriptionTarget::Agent(ch, user_id) => ch.channel_name(user_id),
        }
    }
}

/// Agent-specific WebSocket channels.
#[derive(Debug, Clone)]
pub enum AgentChannel {
    Alert,
    Execution,
    Order,
    Balance,
}

impl AgentChannel {
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Alert => "agent.alert",
            Self::Execution => "agent.execution",
            Self::Order => "agent.order",
            Self::Balance => "agent.balance",
        }
    }

    pub fn channel_name(&self, user_id: impl AsRef<str>) -> String {
        format!("{}.{}", self.prefix(), user_id.as_ref())
    }

    pub fn parse(prefix: &str) -> Option<Self> {
        match prefix {
            "agent.alert" => Some(Self::Alert),
            "agent.execution" => Some(Self::Execution),
            "agent.order" => Some(Self::Order),
            "agent.balance" => Some(Self::Balance),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SupportedAssetPairs {
    #[allow(non_camel_case_types)]
    BTC_USDT,
    #[allow(non_camel_case_types)]
    ETH_USDT,
    #[allow(non_camel_case_types)]
    SOL_USDT,
    #[allow(non_camel_case_types)]
    SOL_USDC,
}

impl SupportedAssetPairs {
    pub fn parse(asset_pair_str: &str) -> Result<SupportedAssetPairs, &'static str> {
        match asset_pair_str {
            "BTC_USDT" => Ok(SupportedAssetPairs::BTC_USDT),
            "ETH_USDT" => Ok(SupportedAssetPairs::ETH_USDT),
            "SOL_USDT" => Ok(SupportedAssetPairs::SOL_USDT),
            "SOL_USDC" => Ok(SupportedAssetPairs::SOL_USDC),
            _ => Err("Unsupported asset pair"),
        }
    }
}

// ---------------------------------------------------------------------------
// Re-export agent types from common_utils (single source of truth).
pub use common_utils::agent::{AgentAlert, AlertSeverity, AlertType, ExecutionReport};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    #[test]
    fn parse_agent_alert_subscription() {
        let msg = WsMessage {
            method: "SUBSCRIBE".to_string(),
            params: vec!["agent.alert.user123".to_string()],
            id: 1,
        };
        let target = msg.parse_subscription().expect("should parse agent.alert subscription");
        match target {
            SubscriptionTarget::Agent(ch, user_id) => {
                assert_eq!(ch.channel_name("user123"), "agent.alert.user123");
                assert_eq!(user_id, "user123");
            }
            _ => panic!("expected Agent target"),
        }
    }

    #[test]
    fn parse_agent_execution_subscription() {
        let msg = WsMessage {
            method: "SUBSCRIBE".to_string(),
            params: vec!["agent.execution.550e8400-e29b-41d4-a716-446655440000".to_string()],
            id: 2,
        };
        let target = msg.parse_subscription().expect("should parse agent.execution");
        let id = target.subscription_id();
        assert!(id.starts_with("agent.execution."));
    }

    #[test]
    fn standard_subscriptions_still_parse() {
        let msg = WsMessage {
            method: "SUBSCRIBE".to_string(),
            params: vec!["depth.BTC_USDT".to_string()],
            id: 3,
        };
        let target = msg.parse_subscription().expect("standard subscription should still work");
        assert_eq!(target.subscription_id(), "depth.BTC_USDT");
    }

    #[test]
    fn unknown_agent_prefix_returns_none() {
        let msg = WsMessage {
            method: "SUBSCRIBE".to_string(),
            params: vec!["agent.unknown.user123".to_string()],
            id: 4,
        };
        assert!(msg.parse_subscription().is_none());
    }

    #[test]
    fn agent_alert_serializes_correctly() {
        let alert = AgentAlert {
            alert_type: AlertType::DrawdownWarning,
            severity: AlertSeverity::Notable,
            message: "Drawdown at 4.2%, approaching 5% limit".to_string(),
            current_value: Some(Decimal::new(42, 1)), // 4.2
            limit_value: Some(Decimal::new(5, 0)),     // 5
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&alert).unwrap();
        assert!(json.contains("drawdown_warning"));
        assert!(json.contains("notable"));
    }

    #[test]
    fn execution_report_serializes_correctly() {
        let report = ExecutionReport {
            trade_group_id: Uuid::new_v4(),
            order_id: "binance-order-123".to_string(),
            status: "filled".to_string(),
            fill_price: Some(Decimal::new(50000, 0)),
            exchange: "binance".to_string(),
            latency_ms: 342,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("trade_group_id"));
        assert!(json.contains("binance-order-123"));
        assert!(json.contains("filled"));
        assert!(json.contains("latency_ms\":342"));
    }
}
