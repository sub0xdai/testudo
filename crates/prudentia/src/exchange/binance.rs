//! Binance adapter (placeholder)

use crate::Result;

pub struct BinanceAdapter;

impl BinanceAdapter {
    pub fn new(_config: crate::ExchangeConfig) -> Result<Self> {
        Ok(Self)
    }
}