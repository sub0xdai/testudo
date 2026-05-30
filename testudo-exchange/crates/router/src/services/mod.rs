//! Router Services
//!
//! Background services and service layer for the router.

// @anchor exchange:router:mod
// @tags api

pub mod agent_alert;
pub mod agent_journal;
pub mod agent_journal_formatter;
pub mod auth;
pub mod balance_snapshot;
pub mod calibration;
pub mod cex_client;
pub mod cex_history;
pub mod coach;
pub mod dignitas;
pub mod draw_to_trade;
pub mod exchange_api;
pub mod execution_service;
pub mod fill_detector;
pub mod hl_fill_journal;
pub mod import_worker;
pub mod journal_service;
pub mod journal_stats;
pub mod journal_syncer;
pub mod journal_timeseries;
pub mod onboarding;
pub mod hyperliquid;
pub mod price_feed;
pub mod reconciliation;
pub mod rehydration;
pub mod risk_snapshot;
pub mod sizing_preview;
pub mod sync_service;
pub mod trade_event_writer;
pub mod trade_manager;
pub mod ws_subscription_manager;

pub use cex_client::{
    CexClient, CexClientError, CexSidecarConfig, OrderUpdateEvent, SidecarCredentials,
};
pub use draw_to_trade::{DrawToTradeService, ProcessOrderError, ProcessedOrder};
pub use exchange_api::{CexExchangeApi, ExchangeApi, ShadowExchangeApi};
pub use hyperliquid::RoutingExchangeApi;
pub use execution_service::{ExecutionService, HealthStatus};
pub use price_feed::{PriceFeedService, PriceTick};
pub use sync_service::SyncService;
pub use trade_manager::*;
pub use ws_subscription_manager::{SubscriptionAction, WsSubscriptionManager};

/// Generate a numeric clientOrderId from group_id and role.
/// WOO X requires clientOrderId to be numeric (0 to i64::MAX).
/// Deterministic: same (group_id, role) always produces the same ID.
pub fn numeric_client_order_id(group_id: uuid::Uuid, role_digit: u8) -> String {
    let bytes = group_id.as_bytes();
    let high = u64::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]]);
    // Reserve last digit for role (1=entry, 2=sl, 3=tp)
    let base = high % 922_337_203_685_477_580;
    let id = base * 10 + role_digit as u64;
    id.to_string()
}

#[cfg(test)]
mod integration_tests;
