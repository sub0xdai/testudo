// ==================== Network Configuration ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Testnet,
}

impl Network {
    pub fn api_url(&self) -> &'static str {
        match self {
            Network::Mainnet => "https://api.hyperliquid.xyz",
            Network::Testnet => "https://api.hyperliquid-testnet.xyz",
        }
    }

    pub fn ws_url(&self) -> &'static str {
        match self {
            Network::Mainnet => "wss://api.hyperliquid.xyz/ws",
            Network::Testnet => "wss://api.hyperliquid-testnet.xyz/ws",
        }
    }
}

// ==================== Chain Configuration ====================

// Chain IDs
pub const CHAIN_ID_MAINNET: u64 = 42161; // Arbitrum One
pub const CHAIN_ID_TESTNET: u64 = 421614; // Arbitrum Sepolia

// Agent Sources
pub const AGENT_SOURCE_MAINNET: &str = "a";
pub const AGENT_SOURCE_TESTNET: &str = "b";

// Exchange Endpoints
pub const EXCHANGE_ENDPOINT_MAINNET: &str = "https://api.hyperliquid.xyz/exchange";
pub const EXCHANGE_ENDPOINT_TESTNET: &str =
    "https://api.hyperliquid-testnet.xyz/exchange";

// ==================== Rate Limit Weights ====================

// Info endpoints
pub const WEIGHT_ALL_MIDS: u32 = 2;
pub const WEIGHT_L2_BOOK: u32 = 1;
pub const WEIGHT_USER_STATE: u32 = 2;
pub const WEIGHT_USER_FILLS: u32 = 2;
pub const WEIGHT_USER_FUNDING: u32 = 2;
pub const WEIGHT_USER_FEES: u32 = 1;
pub const WEIGHT_OPEN_ORDERS: u32 = 1;
pub const WEIGHT_ORDER_STATUS: u32 = 1;
pub const WEIGHT_RECENT_TRADES: u32 = 1;
pub const WEIGHT_CANDLES: u32 = 2;
pub const WEIGHT_FUNDING_HISTORY: u32 = 2;
pub const WEIGHT_TOKEN_BALANCES: u32 = 1;
pub const WEIGHT_REFERRAL: u32 = 1;

// Exchange endpoints (these have higher weights)
pub const WEIGHT_PLACE_ORDER: u32 = 3;
pub const WEIGHT_CANCEL_ORDER: u32 = 2;
pub const WEIGHT_MODIFY_ORDER: u32 = 3;
pub const WEIGHT_BULK_ORDER: u32 = 10;
pub const WEIGHT_BULK_CANCEL: u32 = 8;

// ==================== Rate Limit Configuration ====================

pub const RATE_LIMIT_MAX_TOKENS: u32 = 1200;
pub const RATE_LIMIT_REFILL_RATE: u32 = 600; // per minute

// ==================== Time Constants ====================

pub const NONCE_WINDOW_MS: u64 = 60_000; // 60 seconds

// ==================== Order Constants ====================

pub const TIF_GTC: &str = "Gtc";
pub const TIF_IOC: &str = "Ioc";
pub const TIF_ALO: &str = "Alo";

pub const TPSL_TP: &str = "tp";
pub const TPSL_SL: &str = "sl";
