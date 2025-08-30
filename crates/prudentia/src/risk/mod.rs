//! Core risk management and validation logic
//!
//! This module implements the Testudo Protocol enforcement through a comprehensive
//! risk validation system. Every trade must pass through multiple risk rules
//! before being approved for execution.

pub mod rules;
pub mod assessment;
pub mod protocol;
pub mod validator;
pub mod engine;

pub use rules::{RiskRule, RiskViolation};
pub use assessment::TradeRiskAssessment;
pub use protocol::TestudoProtocol;
pub use validator::{RiskValidator, RiskValidationResult};
pub use engine::RiskEngine;