//! Hyperliquid Native Integration
//!
//! Direct Rust SDK integration for Hyperliquid exchange, bypassing the
//! Node.js sidecar entirely. Implements the `ExchangeApi` trait for
//! seamless integration with the existing `TradeManagerService`.

pub mod agent_approval;
pub mod auth;
pub mod exchange_api;
pub mod routing;
pub mod universe;
pub mod ws_fills;

#[cfg(test)]
mod tests;

pub use auth::{AuthMode, HyperliquidAuth};
pub use exchange_api::HyperliquidExchangeApi;
pub use routing::RoutingExchangeApi;
pub use universe::AssetUniverse;
pub use ws_fills::HyperliquidFillSubscriber;
