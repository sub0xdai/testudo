//! Core types for risk management and trade validation
//!
//! This module defines the fundamental data structures used throughout the
//! Prudentia risk management system, including trade proposals, risk assessments,
//! and protocol limits.

pub mod trade_proposal;
pub mod risk_assessment;
pub mod protocol_limits;

pub use trade_proposal::{TradeProposal, TradeSide};
pub use risk_assessment::{RiskAssessment, ApprovalStatus, ProtocolViolation, ViolationSeverity};
pub use protocol_limits::ProtocolLimits;