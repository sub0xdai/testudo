//! Trade Manager Service
//!
//! Automated position management engine (EXT-09).
//! Monitors positions against price feed and executes management rules:
//! break-even, trailing stop, and partial take-profit.

// @anchor exchange:router:mod
// @tags api

pub mod evaluator;
pub mod repository;
pub mod service;
pub mod types;

pub use service::{ManagementEvent, TradeManagerService};
pub use types::*;
