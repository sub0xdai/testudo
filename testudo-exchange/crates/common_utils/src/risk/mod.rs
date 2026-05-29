//! Risk Management Module
//!
//! Implements automated position sizing and risk controls for the
//! hybrid trading system. The core principle is "Conservative Wins":
//! always use the smallest position size among all calculated limits.
//!
//! # Components
//!
//! - **Risk Config**: User-defined risk parameters
//! - **Position Sizer**: Calculates position size based on risk rules
//! - **ATR Calculator**: Volatility-based stop distance calculation
//! - **Validator**: Validates orders against risk limits
//!
//! # Conservative Wins Policy (from PRD)
//!
//! Position size is the minimum of:
//! 1. Account % risk (e.g., 2% of account per trade)
//! 2. Fixed risk amount (e.g., max $100 loss per trade)
//! 3. Max position size (e.g., max 0.1 BTC per trade)
//!
//! # Example
//!
//! ```ignore
//! use risk::{RiskConfig, PositionSizer};
//!
//! let config = RiskConfig::new()
//!     .with_account_risk_percent(dec!(2))    // 2% per trade
//!     .with_max_risk_amount(dec!(100))       // Max $100 loss
//!     .with_max_position_size(dec!(0.1));    // Max 0.1 BTC
//!
//! let sizer = PositionSizer::new(config);
//! let size = sizer.calculate_position_size(
//!     account_balance,
//!     entry_price,
//!     stop_loss_price,
//! );
//! ```

// @anchor exchange:common_utils:mod
// @tags infra

pub mod config;
pub mod kelly;
pub mod pg_storage;
pub mod position_sizer;
pub mod service;
pub mod sizing;
pub mod types;
pub mod validator;

pub use config::{RiskConfig, RiskConfigError};
pub use pg_storage::{PgRiskConfigStorage, PgRiskStorageError};
pub use position_sizer::PositionSizer;
pub use service::{AccountState, OrderRequest, OrderSide, RiskService};
pub use sizing::{
    calculate_position_size, fixed_fractional, kelly_criterion, volatility_adjusted, MarketData,
    TradingStats,
};
pub use types::{RiskCheckResult, RiskRejection, RiskWarning, SizingMethod};
pub use validator::{RiskValidationResult, RiskValidator, RiskViolation};
