// @anchor exchange:router:hl_network
// @tags api

use hyperliquid_sdk_rs::Network;

/// Hyperliquid network selection - single source of truth.
///
/// Every endpoint the router talks to is derived here, so no call site matches
/// on a network variant to pick a URL string. This replaces the SDK's own
/// `Network` enum across the router. The one conversion back to the SDK type,
/// [`HlNetwork::to_sdk`], is deleted with the vendored crate in HL-12 CP-6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HlNetwork {
    Mainnet,
    Testnet,
}

impl HlNetwork {
    /// REST info endpoint: market data and clearinghouse queries.
    pub const fn info_url(self) -> &'static str {
        match self {
            Self::Mainnet => "https://api.hyperliquid.xyz/info",
            Self::Testnet => "https://api.hyperliquid-testnet.xyz/info",
        }
    }

    /// REST exchange endpoint: signed actions and approvals.
    pub const fn exchange_url(self) -> &'static str {
        match self {
            Self::Mainnet => "https://api.hyperliquid.xyz/exchange",
            Self::Testnet => "https://api.hyperliquid-testnet.xyz/exchange",
        }
    }

    /// WebSocket endpoint: fill and order-update streams.
    pub const fn ws_url(self) -> &'static str {
        match self {
            Self::Mainnet => "wss://api.hyperliquid.xyz/ws",
            Self::Testnet => "wss://api.hyperliquid-testnet.xyz/ws",
        }
    }

    /// Hyperliquid chain string carried in EIP-712 messages.
    pub const fn chain_string(self) -> &'static str {
        match self {
            Self::Mainnet => "Mainnet",
            Self::Testnet => "Testnet",
        }
    }

    /// Arbitrum chain ID the signing wallet must be connected to.
    ///
    /// Mainnet is Arbitrum One, testnet is Arbitrum Sepolia. MetaMask signs
    /// approvals against the connected chain, not the L1 signing domain.
    pub const fn arbitrum_chain_id(self) -> u64 {
        match self {
            Self::Mainnet => 42161,
            Self::Testnet => 421614,
        }
    }

    /// Convert to the vendored SDK's network enum.
    ///
    /// Every SDK call that is not yet migrated still needs this type. Deleted
    /// in HL-12 CP-6 together with the vendored crate.
    pub const fn to_sdk(self) -> Network {
        match self {
            Self::Mainnet => Network::Mainnet,
            Self::Testnet => Network::Testnet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mainnet_endpoints_are_the_production_hosts() {
        assert_eq!(
            HlNetwork::Mainnet.info_url(),
            "https://api.hyperliquid.xyz/info"
        );
        assert_eq!(
            HlNetwork::Mainnet.exchange_url(),
            "https://api.hyperliquid.xyz/exchange"
        );
        assert_eq!(HlNetwork::Mainnet.ws_url(), "wss://api.hyperliquid.xyz/ws");
        assert_eq!(HlNetwork::Mainnet.chain_string(), "Mainnet");
        assert_eq!(HlNetwork::Mainnet.arbitrum_chain_id(), 42161);
    }

    #[test]
    fn testnet_endpoints_are_the_sandbox_hosts() {
        assert_eq!(
            HlNetwork::Testnet.info_url(),
            "https://api.hyperliquid-testnet.xyz/info"
        );
        assert_eq!(
            HlNetwork::Testnet.exchange_url(),
            "https://api.hyperliquid-testnet.xyz/exchange"
        );
        assert_eq!(
            HlNetwork::Testnet.ws_url(),
            "wss://api.hyperliquid-testnet.xyz/ws"
        );
        assert_eq!(HlNetwork::Testnet.chain_string(), "Testnet");
        assert_eq!(HlNetwork::Testnet.arbitrum_chain_id(), 421614);
    }

    #[test]
    fn sdk_conversion_maps_each_variant() {
        assert_eq!(HlNetwork::Mainnet.to_sdk(), Network::Mainnet);
        assert_eq!(HlNetwork::Testnet.to_sdk(), Network::Testnet);
    }
}
