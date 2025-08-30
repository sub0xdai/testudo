//! Real-time risk monitoring and portfolio tracking
//!
//! This module provides continuous monitoring of portfolio risk exposure,
//! consecutive loss tracking, and real-time risk metrics calculation.

pub mod portfolio_tracker;
pub mod loss_tracker;
pub mod metrics;

pub use portfolio_tracker::{PortfolioTracker, PortfolioRiskMetrics};
pub use loss_tracker::{ConsecutiveLossTracker, CircuitBreakerState, CircuitBreakerAction};
pub use metrics::{RealTimeRiskMetrics, RiskMetricsCalculator};