//! Binance adapter (placeholder)

use crate::Result;

/// Placeholder exchange configuration
#[derive(Debug, Clone)]
pub struct ExchangeConfig {
    pub api_key: String,
    pub secret_key: String,
}

pub struct BinanceAdapter;

impl BinanceAdapter {
    pub fn new(_config: ExchangeConfig) -> Result<Self> {
        Ok(Self)
    }
}