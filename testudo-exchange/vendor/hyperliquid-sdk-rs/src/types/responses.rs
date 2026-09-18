use serde::Deserialize;

// ==================== Order Status Types ====================

#[derive(Debug, Clone, Deserialize)]
pub struct RestingOrder {
    pub oid: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilledOrder {
    pub total_sz: String,
    pub avg_px: String,
    pub oid: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExchangeDataStatus {
    Success,
    WaitingForFill,
    WaitingForTrigger,
    Error(String),
    Resting(RestingOrder),
    Filled(FilledOrder),
}

// ==================== Exchange Response Types ====================

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeDataStatuses {
    pub statuses: Vec<ExchangeDataStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeResponse {
    #[serde(rename = "type")]
    pub response_type: String,
    pub data: Option<ExchangeDataStatuses>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "status", content = "response")]
pub enum ExchangeResponseStatus {
    Ok(ExchangeResponse),
    Err(String),
}

// ==================== Convenience Methods ====================

impl ExchangeResponseStatus {
    /// Check if the response was successful
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    /// Get the error message if this was an error response
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Err(msg) => Some(msg),
            _ => None,
        }
    }

    /// Get the inner response if successful
    pub fn into_result(self) -> Result<ExchangeResponse, String> {
        match self {
            Self::Ok(response) => Ok(response),
            Self::Err(msg) => Err(msg),
        }
    }
}

impl ExchangeDataStatus {
    /// Check if this status represents a successful order
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success | Self::Resting(_) | Self::Filled(_))
    }

    /// Get order ID if available
    pub fn order_id(&self) -> Option<u64> {
        match self {
            Self::Resting(order) => Some(order.oid),
            Self::Filled(order) => Some(order.oid),
            _ => None,
        }
    }
}
