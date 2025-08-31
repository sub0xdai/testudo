//! Risk decision module - Phase 4 of OODA Loop (Decide)
//!
//! The Decider implements the critical "Decide" phase of the OODA loop where 
//! TradeProposals from the Orientator are validated against the Testudo Protocol
//! through the prudentia::RiskManagementProtocol.
//!
//! Following Roman military principles of disciplined decision-making, the RiskDecider
//! applies systematic risk assessment before transitioning to either Acting or Failed states.

use std::sync::Arc;
use rust_decimal::Decimal;
use tokio::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, warn};

use prudentia::risk::protocol::{RiskManagementProtocol, ProtocolDecision, ProtocolAssessmentResult};
use prudentia::risk::assessment::TradeProposal;

use crate::orientator::TradeProposal as FormatioTradeProposal;
use crate::types::DecisionError;

/// The result of the risk decision process
#[derive(Debug, Clone)]
pub struct DecisionResult {
    /// The original trade proposal
    pub proposal: FormatioTradeProposal,
    
    /// Protocol assessment result from prudentia
    pub assessment: ProtocolAssessmentResult,
    
    /// Final decision with reasoning
    pub decision: RiskDecision,
    
    /// Time taken for decision process (target: <25ms)
    pub decision_time_ms: u64,
    
    /// Audit trail for decision reasoning
    pub audit_trail: Vec<String>,
}

/// Risk decision enumeration with clear state transition guidance
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskDecision {
    /// Execute the trade - transition OODA loop to Acting state
    Execute {
        approved_position_size: Decimal,
        execution_priority: ExecutionPriority,
    },
    
    /// Reject the trade - transition OODA loop to Failed state  
    Reject {
        rejection_reason: String,
        violation_count: u32,
    },
    
    /// Assessment failed - transition OODA loop to Failed state
    AssessmentFailed {
        error_details: String,
    },
}

/// Execution priority for approved trades
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionPriority {
    /// Standard execution (approved without warnings)
    Standard,
    
    /// Careful execution (approved with warnings - monitor closely)
    Careful,
}

/// The RiskDecider component - implements Phase 4 (Decide) of OODA loop
/// 
/// Roman principle: "Deliberare utilia mora, periculosa celeritas" 
/// (Useful deliberation takes time, dangerous speed)
pub struct RiskDecider {
    /// The risk management protocol for trade assessment
    protocol: Arc<RiskManagementProtocol>,
    
    /// Component name for logging and metrics
    name: String,
    
    /// Maximum decision time before timeout (default: 25ms)
    max_decision_time: Duration,
}

impl RiskDecider {
    /// Create a new RiskDecider with the specified protocol
    pub fn new(protocol: Arc<RiskManagementProtocol>) -> Self {
        Self {
            protocol,
            name: "RiskDecider".to_string(),
            max_decision_time: Duration::from_millis(25), // Testudo performance target
        }
    }
    
    /// Create a RiskDecider with custom configuration
    pub fn with_config(
        protocol: Arc<RiskManagementProtocol>,
        name: String,
        max_decision_time: Duration,
    ) -> Self {
        Self {
            protocol,
            name,
            max_decision_time,
        }
    }
    
    /// Execute the risk decision process for a trade proposal
    /// 
    /// This is the main entry point for Phase 4 of the OODA loop. It takes a
    /// TradeProposal from the Orientator and applies the Testudo Protocol
    /// to determine if the trade should be executed or rejected.
    #[instrument(skip(self, proposal), fields(
        symbol = %proposal.symbol,
        entry_price = %proposal.entry_price,
        position_size = %proposal.position_size
    ))]
    pub async fn decide_trade(&self, proposal: FormatioTradeProposal) -> Result<DecisionResult, DecisionError> {
        let start_time = Instant::now();
        let mut audit_trail = Vec::new();
        
        debug!("Starting risk decision for trade proposal: {} {}", proposal.symbol, proposal.side);
        audit_trail.push(format!("Decision started at {:?} for {} {}", start_time, proposal.symbol, proposal.side));
        
        // Convert FormationTradeProposal to PrudentiaTradeProposal for protocol assessment
        let prudentia_proposal = self.convert_proposal(&proposal)?;
        audit_trail.push("Trade proposal converted for protocol assessment".to_string());
        
        // Apply timeout to prevent decision delays
        let assessment_result = tokio::time::timeout(
            self.max_decision_time,
            self.assess_with_protocol(&prudentia_proposal)
        ).await;
        
        let protocol_result = match assessment_result {
            Ok(result) => result?,
            Err(_timeout) => {
                let error_msg = format!("Decision timeout exceeded {}ms limit", self.max_decision_time.as_millis());
                error!("{}", error_msg);
                audit_trail.push(error_msg.clone());
                
                return Ok(DecisionResult {
                    proposal,
                    assessment: self.create_failed_assessment(&error_msg)?,
                    decision: RiskDecision::AssessmentFailed { error_details: error_msg },
                    decision_time_ms: start_time.elapsed().as_millis() as u64,
                    audit_trail,
                });
            }
        };
        
        // Create the final risk decision based on protocol result
        let risk_decision = self.create_risk_decision(&protocol_result, &mut audit_trail);
        
        let decision_time_ms = start_time.elapsed().as_millis() as u64;
        
        // Log final decision
        match &risk_decision {
            RiskDecision::Execute { approved_position_size, execution_priority } => {
                info!("Trade APPROVED: {} {} size={} priority={:?} ({}ms)", 
                    proposal.symbol, proposal.side, approved_position_size, execution_priority, decision_time_ms);
                audit_trail.push(format!("APPROVED: Position size {} with {:?} priority", approved_position_size, execution_priority));
            },
            RiskDecision::Reject { rejection_reason, violation_count } => {
                warn!("Trade REJECTED: {} {} - {} ({} violations, {}ms)", 
                    proposal.symbol, proposal.side, rejection_reason, violation_count, decision_time_ms);
                audit_trail.push(format!("REJECTED: {} violations - {}", violation_count, rejection_reason));
            },
            RiskDecision::AssessmentFailed { error_details } => {
                error!("Assessment FAILED: {} {} - {} ({}ms)", 
                    proposal.symbol, proposal.side, error_details, decision_time_ms);
                audit_trail.push(format!("ASSESSMENT FAILED: {}", error_details));
            },
        }
        
        Ok(DecisionResult {
            proposal,
            assessment: protocol_result,
            decision: risk_decision,
            decision_time_ms,
            audit_trail,
        })
    }
    
    /// Convert FormationTradeProposal to PrudentiaTradeProposal
    fn convert_proposal(&self, proposal: &FormatioTradeProposal) -> Result<TradeProposal, DecisionError> {
        // Generate a unique ID for the proposal
        let proposal_id = format!("{}-{}-{}", 
            proposal.symbol, 
            proposal.entry_price, 
            chrono::Utc::now().timestamp_millis()
        );
        
        Ok(TradeProposal {
            id: proposal_id,
            symbol: proposal.symbol.clone(),
            side: proposal.side.clone(),
            entry_price: proposal.entry_price,
            stop_loss: proposal.stop_loss,
            take_profit: proposal.take_profit,
            position_size: proposal.position_size,
        })
    }
    
    /// Apply the risk management protocol to assess the trade
    async fn assess_with_protocol(&self, proposal: &TradeProposal) -> Result<ProtocolAssessmentResult, DecisionError> {
        debug!("Applying risk management protocol: {}", self.protocol.name());
        
        let assessment_result = self.protocol.assess_trade(proposal)
            .map_err(|e| DecisionError::ProtocolError(e.to_string()))?;
        
        debug!("Protocol assessment completed - Decision: {:?}", assessment_result.protocol_decision);
        
        Ok(assessment_result)
    }
    
    /// Create a risk decision from protocol assessment result
    fn create_risk_decision(&self, result: &ProtocolAssessmentResult, audit_trail: &mut Vec<String>) -> RiskDecision {
        match result.protocol_decision {
            ProtocolDecision::Approved => {
                audit_trail.push("Protocol decision: APPROVED - no violations detected".to_string());
                RiskDecision::Execute {
                    approved_position_size: result.assessment.recommended_position_size,
                    execution_priority: ExecutionPriority::Standard,
                }
            },
            
            ProtocolDecision::ApprovedWithWarnings => {
                let warning_count = result.assessment.violations.len();
                audit_trail.push(format!("Protocol decision: APPROVED WITH WARNINGS - {} warnings noted", warning_count));
                RiskDecision::Execute {
                    approved_position_size: result.assessment.recommended_position_size,
                    execution_priority: ExecutionPriority::Careful,
                }
            },
            
            ProtocolDecision::Rejected => {
                let violation_count = result.assessment.violations.len() as u32;
                audit_trail.push(format!("Protocol decision: REJECTED - {} violations detected", violation_count));
                RiskDecision::Reject {
                    rejection_reason: result.decision_reasoning.clone(),
                    violation_count,
                }
            },
            
            ProtocolDecision::AssessmentFailed => {
                audit_trail.push("Protocol decision: ASSESSMENT FAILED - system errors occurred".to_string());
                RiskDecision::AssessmentFailed {
                    error_details: result.decision_reasoning.clone(),
                }
            },
        }
    }
    
    /// Create a failed assessment result for timeout/error cases
    fn create_failed_assessment(&self, error_msg: &str) -> Result<ProtocolAssessmentResult, DecisionError> {
        // This is a simplified assessment result for error cases
        // In practice, we would use proper prudentia types
        Err(DecisionError::AssessmentTimeout(error_msg.to_string()))
    }
    
    /// Get the configured protocol name
    pub fn protocol_name(&self) -> &str {
        self.protocol.name()
    }
    
    /// Get the configured maximum decision time
    pub fn max_decision_time(&self) -> Duration {
        self.max_decision_time
    }
}