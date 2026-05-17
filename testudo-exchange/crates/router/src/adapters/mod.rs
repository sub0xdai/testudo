//! Exchange Adapters Module
//!
//! Provides concrete implementations of the `ExchangeAdapter` trait for:
//! - `ShadowEngineAdapter`: Paper trading via the Shadow Engine
//!
//! Live trading is now handled via the CEX sidecar service (012-ccxt-multi-exchange),
//! accessed through `CexExchangeApi` in the services layer.

pub mod shadow_adapter;

pub use shadow_adapter::ShadowEngineAdapter;
