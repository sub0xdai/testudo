use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsResponse {
    pub stream: String,
    pub data: serde_json::Value, // any kind of JSON-like data
}
