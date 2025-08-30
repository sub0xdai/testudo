//! Risk assessment types and results
//!
//! This module defines the structures that represent the outcome of
//! risk validation performed on trade proposals.

use disciplina::PositionSize;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

/// Result of risk assessment performed on a trade proposal
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskAssessment {
    /// Unique identifier for this assessment
    pub assessment_id: Uuid,
    
    /// ID of the trade proposal being assessed
    pub proposal_id: Uuid,
    
    /// Calculated position size using Van Tharp methodology
    pub position_size: PositionSize,
    
    /// Dollar amount at risk if stop loss is hit
    pub risk_amount: Decimal,
    
    /// Percentage of account equity at risk
    pub risk_percentage: Decimal,
    
    /// Risk/reward ratio (reward distance / risk distance)
    pub reward_risk_ratio: Option<Decimal>,
    
    /// Impact on total portfolio risk if this trade is executed
    pub portfolio_impact: Decimal,
    
    /// List of protocol violations found during assessment
    pub violations: Vec<ProtocolViolation>,
    
    /// Final approval status for the trade
    pub approval_status: ApprovalStatus,
    
    /// Timestamp when this assessment was performed
    pub timestamp: SystemTime,
    
    /// Additional context or reasoning for the assessment
    pub reasoning: Option<String>,
}

/// Protocol violation found during risk assessment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtocolViolation {
    /// Type of rule that was violated
    pub rule_name: String,
    
    /// Severity level of the violation
    pub severity: ViolationSeverity,
    
    /// Detailed description of the violation
    pub description: String,
    
    /// Current value that violated the rule
    pub current_value: Decimal,
    
    /// Maximum allowed value for this rule
    pub limit_value: Decimal,
    
    /// Suggested corrective action
    pub suggested_action: String,
}

/// Severity levels for protocol violations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViolationSeverity {
    /// Warning - trade can proceed with caution
    Warning,
    /// High - trade should be modified before execution
    High,
    /// Critical - trade must be rejected or significantly modified
    Critical,
    /// Blocking - trade cannot be executed under any circumstances
    Blocking,
}

/// Final approval status for a trade proposal
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// Trade approved for execution without modifications
    Approved,
    /// Trade approved with warnings - user should review
    ApprovedWithWarnings,
    /// Trade requires position size reduction to comply with protocol
    RequiresReduction,
    /// Trade rejected due to protocol violations
    Rejected,
    /// Trade blocked due to circuit breaker or system safety measures
    Blocked,
}

impl RiskAssessment {
    /// Create a new risk assessment
    pub fn new(
        proposal_id: Uuid,
        position_size: PositionSize,
        risk_amount: Decimal,
        risk_percentage: Decimal,
        reward_risk_ratio: Option<Decimal>,
        portfolio_impact: Decimal,
    ) -> Self {
        Self {
            assessment_id: Uuid::new_v4(),
            proposal_id,
            position_size,
            risk_amount,
            risk_percentage,
            reward_risk_ratio,
            portfolio_impact,
            violations: Vec::new(),
            approval_status: ApprovalStatus::Approved, // Default to approved, will be updated based on violations
            timestamp: SystemTime::now(),
            reasoning: None,
        }
    }
    
    /// Add a protocol violation to this assessment
    pub fn add_violation(&mut self, violation: ProtocolViolation) {
        // Update approval status based on violation severity
        match violation.severity {
            ViolationSeverity::Blocking => {
                self.approval_status = ApprovalStatus::Blocked;
            }
            ViolationSeverity::Critical => {
                if self.approval_status != ApprovalStatus::Blocked {
                    self.approval_status = ApprovalStatus::Rejected;
                }
            }
            ViolationSeverity::High => {
                if matches!(self.approval_status, ApprovalStatus::Approved | ApprovalStatus::ApprovedWithWarnings) {
                    self.approval_status = ApprovalStatus::RequiresReduction;
                }
            }
            ViolationSeverity::Warning => {
                if self.approval_status == ApprovalStatus::Approved {
                    self.approval_status = ApprovalStatus::ApprovedWithWarnings;
                }
            }
        }
        
        self.violations.push(violation);
    }
    
    /// Check if the trade is approved for execution (with or without warnings)
    pub fn is_approved(&self) -> bool {
        matches!(self.approval_status, ApprovalStatus::Approved | ApprovalStatus::ApprovedWithWarnings)
    }
    
    /// Check if the trade requires modification before execution
    pub fn requires_modification(&self) -> bool {
        matches!(self.approval_status, ApprovalStatus::RequiresReduction)
    }
    
    /// Check if the trade is blocked or rejected
    pub fn is_blocked(&self) -> bool {
        matches!(self.approval_status, ApprovalStatus::Rejected | ApprovalStatus::Blocked)
    }
    
    /// Get the highest severity violation
    pub fn highest_violation_severity(&self) -> Option<ViolationSeverity> {
        self.violations.iter().map(|v| v.severity).max()
    }
    
    /// Add reasoning for the assessment decision
    pub fn with_reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = Some(reasoning);
        self
    }
    
    /// Get violations by severity level
    pub fn violations_by_severity(&self, severity: ViolationSeverity) -> Vec<&ProtocolViolation> {
        self.violations.iter().filter(|v| v.severity == severity).collect()
    }
}

impl ProtocolViolation {
    /// Create a new protocol violation
    pub fn new(
        rule_name: String,
        severity: ViolationSeverity,
        description: String,
        current_value: Decimal,
        limit_value: Decimal,
        suggested_action: String,
    ) -> Self {
        Self {
            rule_name,
            severity,
            description,
            current_value,
            limit_value,
            suggested_action,
        }
    }
    
    /// Calculate how much the current value exceeds the limit
    pub fn excess_amount(&self) -> Decimal {
        if self.current_value > self.limit_value {
            self.current_value - self.limit_value
        } else {
            Decimal::ZERO
        }
    }
    
    /// Calculate the percentage by which the limit is exceeded
    pub fn excess_percentage(&self) -> Decimal {
        if self.limit_value.is_zero() {
            Decimal::ZERO
        } else {
            (self.excess_amount() / self.limit_value) * Decimal::from(100)
        }
    }
}

impl std::fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalStatus::Approved => write!(f, "APPROVED"),
            ApprovalStatus::ApprovedWithWarnings => write!(f, "APPROVED_WITH_WARNINGS"),
            ApprovalStatus::RequiresReduction => write!(f, "REQUIRES_REDUCTION"),
            ApprovalStatus::Rejected => write!(f, "REJECTED"),
            ApprovalStatus::Blocked => write!(f, "BLOCKED"),
        }
    }
}

impl std::fmt::Display for ViolationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViolationSeverity::Warning => write!(f, "WARNING"),
            ViolationSeverity::High => write!(f, "HIGH"),
            ViolationSeverity::Critical => write!(f, "CRITICAL"),
            ViolationSeverity::Blocking => write!(f, "BLOCKING"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    #[test]
    fn test_risk_assessment_creation() {
        let proposal_id = Uuid::new_v4();
        let position_size = PositionSize::new(dec!(100)).unwrap();
        
        let assessment = RiskAssessment::new(
            proposal_id,
            position_size,
            dec!(1000), // $1000 at risk
            dec!(0.02), // 2% risk
            Some(dec!(2)), // 2:1 reward/risk ratio
            dec!(0.05), // 5% portfolio impact
        );
        
        assert_eq!(assessment.proposal_id, proposal_id);
        assert_eq!(assessment.risk_amount, dec!(1000));
        assert_eq!(assessment.approval_status, ApprovalStatus::Approved);
        assert!(assessment.violations.is_empty());
    }
    
    #[test]
    fn test_adding_violations_updates_status() {
        let mut assessment = RiskAssessment::new(
            Uuid::new_v4(),
            PositionSize::new(dec!(100)).unwrap(),
            dec!(1000),
            dec!(0.02),
            None,
            dec!(0.05),
        );
        
        // Add a warning violation
        let warning_violation = ProtocolViolation::new(
            "TestRule".to_string(),
            ViolationSeverity::Warning,
            "Test warning".to_string(),
            dec!(100),
            dec!(90),
            "Reduce position".to_string(),
        );
        
        assessment.add_violation(warning_violation);
        assert_eq!(assessment.approval_status, ApprovalStatus::ApprovedWithWarnings);
        
        // Add a critical violation
        let critical_violation = ProtocolViolation::new(
            "CriticalRule".to_string(),
            ViolationSeverity::Critical,
            "Test critical".to_string(),
            dec!(200),
            dec!(100),
            "Reject trade".to_string(),
        );
        
        assessment.add_violation(critical_violation);
        assert_eq!(assessment.approval_status, ApprovalStatus::Rejected);
    }
    
    #[test]
    fn test_violation_excess_calculations() {
        let violation = ProtocolViolation::new(
            "TestRule".to_string(),
            ViolationSeverity::High,
            "Exceeds limit".to_string(),
            dec!(120), // Current value
            dec!(100), // Limit value
            "Reduce position".to_string(),
        );
        
        assert_eq!(violation.excess_amount(), dec!(20)); // 120 - 100
        assert_eq!(violation.excess_percentage(), dec!(20)); // (20/100) * 100
    }
    
    #[test]
    fn test_approval_status_checks() {
        let mut assessment = RiskAssessment::new(
            Uuid::new_v4(),
            PositionSize::new(dec!(100)).unwrap(),
            dec!(1000),
            dec!(0.02),
            None,
            dec!(0.05),
        );
        
        // Initially approved
        assert!(assessment.is_approved());
        assert!(!assessment.requires_modification());
        assert!(!assessment.is_blocked());
        
        // Add blocking violation
        let blocking_violation = ProtocolViolation::new(
            "BlockingRule".to_string(),
            ViolationSeverity::Blocking,
            "Circuit breaker active".to_string(),
            dec!(1),
            dec!(0),
            "Wait for circuit breaker reset".to_string(),
        );
        
        assessment.add_violation(blocking_violation);
        assert!(!assessment.is_approved());
        assert!(!assessment.requires_modification());
        assert!(assessment.is_blocked());
    }
}