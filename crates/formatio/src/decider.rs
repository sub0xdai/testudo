//! Risk decider for OODA loop - Phase 3 (Decide)

use crate::types::DecisionError;
use prudentia::risk::RiskManagementProtocol;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// The final decision made by the risk management protocol.
#[derive(Debug, Clone)]
pub enum RiskDecision {
    Execute {
        approved_position_size: Decimal,
        execution_priority: ExecutionPriority,
    },
    Reject {
        rejection_reason: String,
        violation_count: u32,
    },
    AssessmentFailed {
        error_details: String,
    },
}

/// The result of the decision-making process.
#[derive(Debug, Clone)]
pub struct DecisionResult {
    pub decision: RiskDecision,
    pub decision_latency_ms: u64,
    pub audit_trail: Vec<String>,
}

/// Priority level for trade execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionPriority {
    Standard,
    Careful,
}

/// The RiskDecider component for the OODA loop's Decide phase.
pub struct RiskDecider {
    protocol: Arc<RiskManagementProtocol>,
    decision_timeout: Duration,
}

impl RiskDecider {
    pub fn new(protocol: Arc<RiskManagementProtocol>) -> Self {
        Self {
            protocol,
            decision_timeout: Duration::from_millis(25),
        }
    }

    pub async fn decide_trade(
        &self,
        proposal: prudentia::types::TradeProposal,
    ) -> Result<DecisionResult, DecisionError> {
        let start_time = std::time::Instant::now();

        // Corrected method call from .assess to .assess_trade (now synchronous)
        match timeout(self.decision_timeout, async { self.protocol.assess_trade(&proposal) }).await {
            Ok(Ok(assessment)) => {
                let decision = if matches!(assessment.protocol_decision, 
                    prudentia::risk::ProtocolDecision::Approved | 
                    prudentia::risk::ProtocolDecision::ApprovedWithWarnings) {
                    // The proposal from prudentia doesn't have a separate `approved_position_size`
                    // it either approves the size in the proposal or rejects.
                    RiskDecision::Execute {
                        approved_position_size: assessment.assessment.position_size.value(),
                        execution_priority: ExecutionPriority::Standard,
                    }
                } else {
                    RiskDecision::Reject {
                        rejection_reason: assessment.decision_reasoning.clone(),
                        violation_count: assessment.assessment.violations.len() as u32,
                    }
                };
                Ok(DecisionResult {
                    decision,
                    decision_latency_ms: start_time.elapsed().as_millis() as u64,
                    audit_trail: vec![
                        format!("Rules executed: {}", assessment.rule_results.len()),
                        format!("Decision: {:?}", assessment.protocol_decision),
                        format!("Reasoning: {}", assessment.decision_reasoning),
                    ],
                })
            }
            Ok(Err(e)) => Ok(DecisionResult {
                decision: RiskDecision::AssessmentFailed {
                    error_details: e.to_string(),
                },
                decision_latency_ms: start_time.elapsed().as_millis() as u64,
                audit_trail: vec![format!("Risk assessment failed: {}", e)],
            }),
            Err(_) => Err(DecisionError::AssessmentTimeout(format!(
                "Timeout after {}ms",
                self.decision_timeout.as_millis()
            ))),
        }
    }
}