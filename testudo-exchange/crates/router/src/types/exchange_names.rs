// @anchor exchange:router:exchange_names
// @tags api

/// Supported exchange identifiers — single source of truth.
pub mod exchanges {
    pub const HYPERLIQUID: &str = "hyperliquid";
    pub const BINANCE: &str = "binance";
    pub const WOO: &str = "woo";
    pub const BYBIT: &str = "bybit";
    pub const OKX: &str = "okx";
    pub const BITGET: &str = "bitget";
    pub const GATE: &str = "gate";
    pub const PHEMEX: &str = "phemex";
    pub const BLOFIN: &str = "blofin";

    /// All supported exchanges (for validation and display).
    pub const SUPPORTED: &[&str] = &[
        HYPERLIQUID, BINANCE, WOO, BYBIT, OKX, BITGET, GATE, PHEMEX, BLOFIN,
    ];
}

/// Auth mode constants — single source of truth.
pub mod auth_modes {
    pub const API_KEY: &str = "api_key";
    pub const AGENT_WALLET: &str = "agent_wallet";
}
