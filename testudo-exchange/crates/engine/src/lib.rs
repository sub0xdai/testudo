//! Engine Library
//!
//! Provides the core trading engine functionality including:
//! - Shadow Engine for paper trading
//! - Order matching engine
//! - Position management

#![allow(clippy::type_complexity)]
#![allow(clippy::module_inception)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unwrap_or_default)]
// Deny unwrap() and eprintln! in production code — tests are exempt via cfg_attr
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::print_stderr))]

pub mod engine;
pub mod shadow;
pub mod types;

// Re-export commonly used types
pub use engine::engine::Engine;
pub use engine::error::CoreEngineError;
pub use shadow::{
    BreakEvenConfig, EngineActor, EngineError, EngineHandle, ExchangeCancel, FillEvent, OrderGroup,
    OrderGroupManager, OrderGroupStatus, OrderRole, PriceUpdateResult, ShadowBalance,
    ShadowBalanceManager, ShadowEngine, ShadowOrder, ShadowOrderManager, ShadowOrderSide,
    ShadowOrderStatus, ShadowOrderType, ShadowPosition, ShadowPositionManager, TakeProfitTarget,
    TradeEvent, TradeEventType,
};
