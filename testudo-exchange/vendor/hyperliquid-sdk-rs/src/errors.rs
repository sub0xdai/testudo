use thiserror::Error;

#[derive(Error, Debug)]
pub enum HyperliquidError {
    #[error("rate limited: {available} tokens available, {required} required")]
    RateLimited { available: u32, required: u32 },

    #[error("network error: {0}")]
    Network(String),

    #[error("hyper http error: {0}")]
    HyperHttp(#[from] hyper::http::Error),

    #[error("json parsing error: {0}")]
    Json(#[from] simd_json::Error),

    #[error("serde json error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("invalid response: {0}")]
    InvalidResponse(String),

    #[error("asset not found: {0}")]
    AssetNotFound(String),

    #[error("signer error: {0}")]
    Signer(#[from] crate::signers::signer::SignerError),

    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("HTTP error: status {status}, body: {body}")]
    Http { status: u16, body: String },

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Serialization error: {0}")]
    Serialize(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),
}
