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
    pub fn parse_subscription(&self) -> Option<(SubscriptionType, String)> {
        if self.params.is_empty() {
            return None;
        }

        let subscription_id = &self.params[0];
        let parts: Vec<&str> = subscription_id.split('.').collect();

        if parts.len() != 2 {
            return None;
        }

        let subscription_type_str = parts[0];
        let topic_str = parts[1];

        let subscription_type = SubscriptionType::parse(subscription_type_str)?;
        // We allow dynamic topics (e.g. user IDs for private channels)
        // Asset pair validation can be added back conditionally if strictness is needed for public channels

        Some((subscription_type, topic_str.to_string()))
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
