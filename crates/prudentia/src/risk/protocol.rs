//! Risk Management Protocol and Testudo Protocol implementation
//!
//! This module provides two main components:
//! 1. RiskManagementProtocol - Coordinates multiple risk rules for comprehensive assessment
//! 2. TestudoProtocol - Enforces immutable protocol limits and maintains trading state
//!
//! The RiskManagementProtocol serves as a collection-based system that runs multiple
//! RiskRules against TradeProposals, while the TestudoProtocol maintains state and
//! enforces the core Testudo Protocol limits.

use crate::risk::assessment_rules::{RiskRule, AssessmentError};
use crate::types::{ProtocolLimits, ProtocolViolation, TradeProposal, RiskAssessment, ApprovalStatus, ViolationSeverity};
use crate::types::protocol_limits::ProtocolLimitViolation;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, Duration};
use thiserror::Error;
use tracing::{debug, info, warn, error, instrument};

//=============================================================================
// RISK MANAGEMENT PROTOCOL - Task 3 Implementation
//=============================================================================

/// Errors that can occur during risk management protocol assessment
#[derive(Debug, Error, Clone)]
pub enum ProtocolError {
    #[error("Risk assessment failed for rule '{rule_name}': {reason}")]
    RuleAssessmentFailure { rule_name: String, reason: String },
    
    #[error("No risk rules configured - protocol cannot assess trade")]
    NoRulesConfigured,
    
    #[error("Protocol configuration error: {reason}")]
    ConfigurationError { reason: String },
    
    #[error("Multiple critical violations detected")]
    MultipleCriticalViolations,
}

/// The central risk management protocol that coordinates multiple risk rules
/// 
/// Following the Roman principle of layered defense, this protocol applies
/// multiple independent risk rules to create a comprehensive defense system
/// against capital destruction. This is the main coordination point for Task 3.
#[derive(Debug, Clone)]
pub struct RiskManagementProtocol {
    /// Collection of risk rules to apply to trade proposals
    risk_rules: Vec<Arc<dyn RiskRule>>,
    
    /// Name identifier for logging and debugging
    protocol_name: String,
    
    /// Whether to stop on first critical violation or collect all violations
    fail_fast: bool,
}

/// The result of assessing a trade proposal through the complete protocol
#[derive(Debug, Clone)]
pub struct ProtocolAssessmentResult {
    /// The consolidated risk assessment
    pub assessment: RiskAssessment,
    
    /// Individual results from each risk rule (for debugging/analysis)
    pub rule_results: Vec<RuleAssessmentResult>,
    
    /// Overall protocol decision
    pub protocol_decision: ProtocolDecision,
    
    /// Detailed reasoning for the protocol decision
    pub decision_reasoning: String,
}

/// Individual risk rule assessment result
#[derive(Debug, Clone)]
pub struct RuleAssessmentResult {
    pub rule_name: String,
    pub assessment: Result<RiskAssessment, AssessmentError>,
    pub execution_time_ms: u64,
}

/// Protocol-level decision enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolDecision {
    /// Trade approved by all risk rules
    Approved,
    
    /// Trade approved with warnings from some rules
    ApprovedWithWarnings,
    
    /// Trade rejected due to critical violations
    Rejected,
    
    /// Trade assessment failed due to system errors
    AssessmentFailed,
}

impl RiskManagementProtocol {
    /// Create a new risk management protocol with default settings
    pub fn new() -> Self {
        Self {
            risk_rules: Vec::new(),
            protocol_name: "RiskManagementProtocol".to_string(),
            fail_fast: false,
        }
    }
    
    /// Create a protocol with custom name and settings
    pub fn with_name(name: String, fail_fast: bool) -> Self {
        Self {
            risk_rules: Vec::new(),
            protocol_name: name,
            fail_fast,
        }
    }
    
    /// Add a risk rule to the protocol
    /// 
    /// Rules are executed in the order they are added. For optimal performance,
    /// add cheaper rules first (like individual trade limits) before expensive
    /// rules (like portfolio-wide calculations).
    pub fn add_rule<R: RiskRule + 'static>(mut self, rule: R) -> Self {
        self.risk_rules.push(Arc::new(rule));
        self
    }
    
    /// Add a risk rule by Arc reference (for sharing rules across protocols)
    pub fn add_rule_ref(mut self, rule: Arc<dyn RiskRule>) -> Self {
        self.risk_rules.push(rule);
        self
    }
    
    /// Get the number of configured risk rules
    pub fn rule_count(&self) -> usize {
        self.risk_rules.len()
    }
    
    /// Get the protocol name
    pub fn name(&self) -> &str {
        &self.protocol_name
    }
    
    /// Assess a trade proposal against all configured risk rules
    /// 
    /// This is the main entry point for trade validation. It runs all risk rules
    /// and consolidates their assessments into a single protocol decision.
    /// This implements the core requirement for Task 3.
    #[instrument(skip(self, proposal), fields(proposal_id = %proposal.id, symbol = %proposal.symbol))]
    pub fn assess_trade(&self, proposal: &TradeProposal) -> Result<ProtocolAssessmentResult, ProtocolError> {
        if self.risk_rules.is_empty() {
            error!("No risk rules configured for protocol '{}'", self.protocol_name);
            return Err(ProtocolError::NoRulesConfigured);
        }
        
        debug!(
            "Starting risk assessment for trade {} ({})", 
            proposal.id, 
            proposal.symbol
        );
        
        let start_time = std::time::Instant::now();
        let mut rule_results = Vec::new();
        let mut consolidated_violations = Vec::new();
        let mut critical_violations = 0;
        let mut warnings = 0;
        let mut assessment_failures = 0;
        
        // Primary assessment from first successful rule (for position sizing baseline)
        let mut primary_assessment: Option<RiskAssessment> = None;
        
        // Execute each risk rule
        for rule in &self.risk_rules {
            let rule_start = std::time::Instant::now();
            let rule_name = rule.rule_name().to_string();
            
            debug!("Executing risk rule: {}", rule_name);
            
            let assessment_result = rule.assess(proposal);
            let execution_time = rule_start.elapsed().as_millis() as u64;
            
            match &assessment_result {
                Ok(assessment) => {
                    // Use the first successful assessment as the primary one for position sizing
                    if primary_assessment.is_none() {
                        primary_assessment = Some(assessment.clone());
                    }
                    
                    // Collect violations from this rule
                    for violation in &assessment.violations {
                        match violation.severity {
                            ViolationSeverity::Critical => critical_violations += 1,
                            ViolationSeverity::Warning => warnings += 1,
                            ViolationSeverity::High => warnings += 1,
                            ViolationSeverity::Blocking => critical_violations += 1,
                        }
                        consolidated_violations.push(violation.clone());
                    }
                    
                    debug!(
                        "Rule '{}' completed in {}ms - violations: {}", 
                        rule_name, 
                        execution_time,
                        assessment.violations.len()
                    );
                }
                Err(error) => {
                    assessment_failures += 1;
                    warn!(
                        "Risk rule '{}' failed: {} (execution time: {}ms)",
                        rule_name,
                        error,
                        execution_time
                    );
                    
                    // If fail_fast is enabled and this is a critical failure, abort
                    if self.fail_fast {
                        return Err(ProtocolError::RuleAssessmentFailure {
                            rule_name: rule_name.clone(),
                            reason: error.to_string(),
                        });
                    }
                }
            }
            
            // Store the individual rule result
            rule_results.push(RuleAssessmentResult {
                rule_name,
                assessment: assessment_result,
                execution_time_ms: execution_time,
            });
        }
        
        // Determine protocol decision
        let protocol_decision = self.determine_protocol_decision(
            critical_violations, 
            warnings, 
            assessment_failures
        );
        
        // Create consolidated assessment
        let consolidated_assessment = self.create_consolidated_assessment(
            proposal,
            primary_assessment,
            consolidated_violations,
            &protocol_decision,
        )?;
        
        // Generate decision reasoning
        let decision_reasoning = self.generate_decision_reasoning(
            critical_violations,
            warnings,
            assessment_failures,
        );
        
        let total_time = start_time.elapsed().as_millis();
        
        info!(
            "Protocol assessment completed in {}ms - Decision: {:?} (Critical: {}, Warnings: {}, Failures: {})",
            total_time,
            protocol_decision,
            critical_violations,
            warnings,
            assessment_failures
        );
        
        Ok(ProtocolAssessmentResult {
            assessment: consolidated_assessment,
            rule_results,
            protocol_decision,
            decision_reasoning,
        })
    }
    
    /// Determine the overall protocol decision based on rule results
    fn determine_protocol_decision(
        &self,
        critical_violations: u32,
        warnings: u32,
        assessment_failures: u32,
    ) -> ProtocolDecision {
        // Any assessment failures require manual review
        if assessment_failures > 0 {
            return ProtocolDecision::AssessmentFailed;
        }
        
        // Any critical violations mean rejection
        if critical_violations > 0 {
            return ProtocolDecision::Rejected;
        }
        
        // Warnings are allowed but noted
        if warnings > 0 {
            return ProtocolDecision::ApprovedWithWarnings;
        }
        
        // No violations means approval
        ProtocolDecision::Approved
    }
    
    /// Create a consolidated risk assessment from all rule results
    fn create_consolidated_assessment(
        &self,
        _proposal: &TradeProposal,
        primary_assessment: Option<RiskAssessment>,
        violations: Vec<ProtocolViolation>,
        protocol_decision: &ProtocolDecision,
    ) -> Result<RiskAssessment, ProtocolError> {
        match primary_assessment {
            Some(mut assessment) => {
                // Update the assessment with consolidated violations
                assessment.violations = violations;
                
                // Update approval status based on protocol decision
                assessment.approval_status = match protocol_decision {
                    ProtocolDecision::Approved => ApprovalStatus::Approved,
                    ProtocolDecision::ApprovedWithWarnings => ApprovalStatus::Approved,
                    ProtocolDecision::Rejected => ApprovalStatus::Rejected,
                    ProtocolDecision::AssessmentFailed => ApprovalStatus::Rejected,
                };
                
                Ok(assessment)
            }
            None => {
                // No primary assessment available - this shouldn't happen if rules are configured
                Err(ProtocolError::ConfigurationError {
                    reason: "No successful risk rule assessments to consolidate".to_string(),
                })
            }
        }
    }
    
    /// Generate detailed reasoning for the protocol decision
    fn generate_decision_reasoning(
        &self,
        critical_violations: u32,
        warnings: u32,
        assessment_failures: u32,
    ) -> String {
        match (critical_violations, warnings, assessment_failures) {
            (0, 0, 0) => {
                "Trade proposal approved: All risk rules passed without violations. Trade meets Testudo Protocol requirements.".to_string()
            }
            (0, w, 0) if w > 0 => {
                format!(
                    "Trade proposal approved with {} warning(s): No critical violations detected. Warnings noted for monitoring.",
                    w
                )
            }
            (c, _, 0) if c > 0 => {
                format!(
                    "Trade proposal rejected: {} critical violation(s) detected. Trade violates Testudo Protocol limits.",
                    c
                )
            }
            (_, _, f) if f > 0 => {
                format!(
                    "Trade assessment failed: {} risk rule(s) failed to execute. Manual review required.",
                    f
                )
            }
            _ => {
                "Trade assessment completed with mixed results. Review individual rule results for details.".to_string()
            }
        }
    }
}

impl Default for RiskManagementProtocol {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert ProtocolLimitViolation to ProtocolViolation for consistency
fn convert_limit_violation(violation: ProtocolLimitViolation) -> ProtocolViolation {
    use crate::types::protocol_limits::ProtocolLimitViolation as PLV;
    use crate::types::ViolationSeverity;
    
    match violation {
        PLV::ExceedsMaxIndividualRisk { current, limit } => {
            ProtocolViolation::new(
                "MaxIndividualTradeRisk".to_string(),
                ViolationSeverity::Critical,
                format!("Individual trade risk {}% exceeds maximum limit {}%", current * Decimal::from(100), limit * Decimal::from(100)),
                current,
                limit,
                format!("Reduce position risk to maximum {}% of account equity", limit * Decimal::from(100)),
            )
        },
        PLV::BelowMinIndividualRisk { current, limit } => {
            ProtocolViolation::new(
                "MinIndividualTradeRisk".to_string(),
                ViolationSeverity::Warning,
                format!("Individual trade risk {}% below minimum recommended {}%", current * Decimal::from(100), limit * Decimal::from(100)),
                current,
                limit,
                format!("Consider increasing position size to at least {}%", limit * Decimal::from(100)),
            )
        },
        PLV::ExceedsMaxPortfolioRisk { current, limit } => {
            ProtocolViolation::new(
                "MaxPortfolioRisk".to_string(),
                ViolationSeverity::Critical,
                format!("Portfolio risk {}% exceeds maximum limit {}%", current * Decimal::from(100), limit * Decimal::from(100)),
                current,
                limit,
                format!("Reduce total portfolio exposure to maximum {}%", limit * Decimal::from(100)),
            )
        },
        PLV::ExceedsMaxConsecutiveLosses { current, limit } => {
            ProtocolViolation::new(
                "MaxConsecutiveLosses".to_string(),
                ViolationSeverity::Critical,
                format!("Consecutive losses {} exceeds maximum limit {}", current, limit),
                Decimal::from(current),
                Decimal::from(limit),
                "Wait for winning trade to reset consecutive loss counter".to_string(),
            )
        },
        PLV::BelowMinRewardRiskRatio { current, limit } => {
            ProtocolViolation::new(
                "MinRewardRiskRatio".to_string(),
                ViolationSeverity::High,
                format!("Reward-to-risk ratio {:.1}:1 below minimum requirement {:.1}:1", current, limit),
                current,
                limit,
                format!("Adjust take profit target to achieve minimum {:.1}:1 reward-to-risk ratio", limit),
            )
        },
        PLV::ExceedsMaxOpenPositions { current, limit } => {
            ProtocolViolation::new(
                "MaxOpenPositions".to_string(),
                ViolationSeverity::High,
                format!("Open positions {} exceeds maximum recommended limit {}", current, limit),
                Decimal::from(current),
                Decimal::from(limit),
                "Close some positions before opening new ones".to_string(),
            )
        },
        PLV::ExceedsMaxDailyLoss { current, limit } => {
            ProtocolViolation::new(
                "MaxDailyLoss".to_string(),
                ViolationSeverity::Critical,
                format!("Daily loss {}% exceeds maximum limit {}%", current * Decimal::from(100), limit * Decimal::from(100)),
                current,
                limit,
                "Stop trading for today to prevent further losses".to_string(),
            )
        },
        PLV::ExceedsMaxDrawdown { current, limit } => {
            ProtocolViolation::new(
                "MaxDrawdown".to_string(),
                ViolationSeverity::Critical,
                format!("Current drawdown {}% exceeds maximum limit {}%", current * Decimal::from(100), limit * Decimal::from(100)),
                current,
                limit,
                "Reduce position sizes or stop trading until account recovers".to_string(),
            )
        },
    }
}

impl ProtocolAssessmentResult {
    /// Check if the trade was approved by the protocol
    pub fn is_approved(&self) -> bool {
        matches!(self.protocol_decision, ProtocolDecision::Approved | ProtocolDecision::ApprovedWithWarnings)
    }
    
    /// Check if the trade was rejected
    pub fn is_rejected(&self) -> bool {
        matches!(self.protocol_decision, ProtocolDecision::Rejected)
    }
    
    /// Check if there were assessment failures
    pub fn has_failures(&self) -> bool {
        matches!(self.protocol_decision, ProtocolDecision::AssessmentFailed)
    }
    
    /// Get all violations from the consolidated assessment
    pub fn violations(&self) -> &[ProtocolViolation] {
        &self.assessment.violations
    }
    
    /// Get critical violations only
    pub fn critical_violations(&self) -> Vec<&ProtocolViolation> {
        self.assessment.violations.iter()
            .filter(|v| v.severity == ViolationSeverity::Critical)
            .collect()
    }
    
    /// Get the number of rules that failed to execute
    pub fn failed_rule_count(&self) -> usize {
        self.rule_results.iter()
            .filter(|r| r.assessment.is_err())
            .count()
    }
    
    /// Get total assessment execution time
    pub fn total_execution_time_ms(&self) -> u64 {
        self.rule_results.iter()
            .map(|r| r.execution_time_ms)
            .sum()
    }
}

//=============================================================================
// TESTUDO PROTOCOL - Original implementation 
//=============================================================================

/// The Testudo Protocol enforcer
///
/// This struct maintains the state and enforces the immutable rules
/// that protect traders from catastrophic losses. It tracks portfolio
/// risk exposure, consecutive losses, and daily limits.
#[derive(Debug, Clone)]
pub struct TestudoProtocol {
    /// Immutable protocol limits
    limits: ProtocolLimits,
    /// Current portfolio risk exposure by asset/symbol
    portfolio_exposure: HashMap<String, Decimal>,
    /// Total portfolio risk percentage
    total_portfolio_risk: Decimal,
    /// Consecutive loss tracking
    consecutive_losses: u32,
    /// Last loss timestamp for consecutive loss tracking
    last_loss_time: Option<SystemTime>,
    /// Daily loss tracking
    daily_loss: Decimal,
    /// Last daily reset timestamp
    last_daily_reset: SystemTime,
    /// Number of open positions
    open_positions: u32,
    /// Circuit breaker state
    circuit_breaker_active: bool,
    /// Timestamp when circuit breaker was activated
    circuit_breaker_activated_at: Option<SystemTime>,
}

impl TestudoProtocol {
    /// Create a new Testudo Protocol enforcer with default limits
    pub fn new() -> Self {
        Self::with_limits(ProtocolLimits::default())
    }
    
    /// Create a new Testudo Protocol enforcer with custom limits
    pub fn with_limits(limits: ProtocolLimits) -> Self {
        Self {
            limits,
            portfolio_exposure: HashMap::new(),
            total_portfolio_risk: Decimal::ZERO,
            consecutive_losses: 0,
            last_loss_time: None,
            daily_loss: Decimal::ZERO,
            last_daily_reset: SystemTime::now(),
            open_positions: 0,
            circuit_breaker_active: false,
            circuit_breaker_activated_at: None,
        }
    }
    
    /// Conservative protocol for new traders
    pub fn conservative() -> Self {
        Self::with_limits(ProtocolLimits::conservative_limits())
    }
    
    /// Aggressive protocol for experienced traders
    pub fn aggressive() -> Self {
        Self::with_limits(ProtocolLimits::aggressive_limits())
    }
    
    /// Validate a trade proposal against all protocol limits
    pub fn validate_trade(&mut self, proposal: &TradeProposal) -> Result<(), Vec<ProtocolViolation>> {
        let mut violations = Vec::new();
        
        // Reset daily tracking if needed
        self.reset_daily_tracking_if_needed();
        
        // Check if circuit breaker should be reset
        self.check_circuit_breaker_reset();
        
        // 1. Check circuit breaker status
        if self.circuit_breaker_active {
            violations.push(ProtocolViolation::new(
                "ExceedsMaxConsecutiveLosses".to_string(),
                ViolationSeverity::Critical,
                format!("Consecutive losses {} exceeds limit {}", self.consecutive_losses, self.limits.max_consecutive_losses),
                Decimal::from(self.consecutive_losses),
                Decimal::from(self.limits.max_consecutive_losses),
                "Wait for winning trade to reset consecutive loss counter".to_string(),
            ));
        }
        
        // 2. Validate individual trade risk
        if let Err(violation) = self.limits.validate_individual_trade_risk(proposal.risk_percentage.value()) {
            violations.push(convert_limit_violation(violation));
        }
        
        // 3. Calculate potential new portfolio risk
        let trade_risk = proposal.risk_percentage.value();
        let potential_portfolio_risk = self.total_portfolio_risk + trade_risk;
        
        if let Err(violation) = self.limits.validate_portfolio_risk(potential_portfolio_risk) {
            violations.push(convert_limit_violation(violation));
        }
        
        // 4. Check consecutive losses
        if let Err(violation) = self.limits.validate_consecutive_losses(self.consecutive_losses) {
            violations.push(convert_limit_violation(violation));
        }
        
        // 5. Check open positions limit
        if self.open_positions >= self.limits.max_open_positions {
            violations.push(ProtocolViolation::new(
                "ExceedsMaxOpenPositions".to_string(),
                ViolationSeverity::High,
                format!("Open positions {} exceeds recommended limit {}", self.open_positions, self.limits.max_open_positions),
                Decimal::from(self.open_positions),
                Decimal::from(self.limits.max_open_positions),
                "Consider closing some positions before opening new ones".to_string(),
            ));
        }
        
        // 6. Check reward/risk ratio if take profit is set
        if let Some(ratio) = proposal.risk_reward_ratio() {
            if let Err(violation) = self.limits.validate_reward_risk_ratio(ratio) {
                violations.push(convert_limit_violation(violation));
            }
        }
        
        // 7. Check daily loss limit
        let potential_daily_loss = self.daily_loss + trade_risk * proposal.account_equity.value();
        let daily_loss_percentage = potential_daily_loss / proposal.account_equity.value();
        
        if daily_loss_percentage > self.limits.max_daily_loss {
            violations.push(ProtocolViolation::new(
                "ExceedsMaxDailyLoss".to_string(),
                ViolationSeverity::Critical,
                format!("Daily loss {}% exceeds limit {}%", daily_loss_percentage * Decimal::from(100), self.limits.max_daily_loss * Decimal::from(100)),
                daily_loss_percentage,
                self.limits.max_daily_loss,
                "Stop trading for the day to prevent further losses".to_string(),
            ));
        }
        
        if violations.is_empty() {
            info!("Trade proposal {} passed Testudo Protocol validation", proposal.id);
            Ok(())
        } else {
            warn!(
                "Trade proposal {} failed Testudo Protocol validation with {} violations",
                proposal.id,
                violations.len()
            );
            Err(violations)
        }
    }
    
    /// Record a successful trade execution
    pub fn record_trade_execution(&mut self, proposal: &TradeProposal) {
        let trade_risk = proposal.risk_percentage.value();
        
        // Add to portfolio exposure
        let current_exposure = self.portfolio_exposure
            .get(&proposal.symbol)
            .copied()
            .unwrap_or(Decimal::ZERO);
        
        self.portfolio_exposure.insert(
            proposal.symbol.clone(),
            current_exposure + trade_risk,
        );
        
        // Update total portfolio risk
        self.total_portfolio_risk += trade_risk;
        
        // Increment open positions
        self.open_positions += 1;
        
        info!(
            "Recorded trade execution for {}: risk={:.2}%, total_portfolio_risk={:.2}%, open_positions={}",
            proposal.symbol,
            trade_risk * Decimal::from(100),
            self.total_portfolio_risk * Decimal::from(100),
            self.open_positions
        );
    }
    
    /// Record a trade outcome (win or loss)
    pub fn record_trade_outcome(&mut self, symbol: &str, trade_risk: Decimal, was_loss: bool, loss_amount: Option<Decimal>) {
        // Remove from portfolio exposure
        if let Some(current_exposure) = self.portfolio_exposure.get_mut(symbol) {
            *current_exposure = (*current_exposure - trade_risk).max(Decimal::ZERO);
            if current_exposure.is_zero() {
                self.portfolio_exposure.remove(symbol);
            }
        }
        
        // Update total portfolio risk
        self.total_portfolio_risk = (self.total_portfolio_risk - trade_risk).max(Decimal::ZERO);
        
        // Decrement open positions
        self.open_positions = self.open_positions.saturating_sub(1);
        
        // Handle consecutive loss tracking
        if was_loss {
            self.consecutive_losses += 1;
            self.last_loss_time = Some(SystemTime::now());
            
            // Add to daily loss if amount is provided
            if let Some(loss) = loss_amount {
                self.daily_loss += loss;
            }
            
            // Activate circuit breaker if limit reached
            if self.consecutive_losses >= self.limits.max_consecutive_losses {
                self.activate_circuit_breaker();
            }
            
            warn!(
                "Recorded loss for {}: consecutive_losses={}, daily_loss=${:.2}",
                symbol, self.consecutive_losses, self.daily_loss
            );
        } else {
            // Reset consecutive losses on win
            self.consecutive_losses = 0;
            self.last_loss_time = None;
            
            info!(
                "Recorded win for {}: consecutive losses reset, daily_loss=${:.2}",
                symbol, self.daily_loss
            );
        }
        
        info!(
            "Trade outcome recorded for {}: total_portfolio_risk={:.2}%, open_positions={}",
            symbol,
            self.total_portfolio_risk * Decimal::from(100),
            self.open_positions
        );
    }
    
    /// Activate the circuit breaker
    fn activate_circuit_breaker(&mut self) {
        if !self.circuit_breaker_active {
            self.circuit_breaker_active = true;
            self.circuit_breaker_activated_at = Some(SystemTime::now());
            
            warn!(
                "🚨 CIRCUIT BREAKER ACTIVATED: {} consecutive losses detected. Trading halted for safety.",
                self.consecutive_losses
            );
        }
    }
    
    /// Check if circuit breaker should be reset (after timeout or manual intervention)
    fn check_circuit_breaker_reset(&mut self) {
        if self.circuit_breaker_active {
            if let Some(activated_at) = self.circuit_breaker_activated_at {
                let elapsed = SystemTime::now().duration_since(activated_at).unwrap_or_default();
                
                // Reset after 1 hour (configurable in real system)
                if elapsed > Duration::from_secs(3600) {
                    self.reset_circuit_breaker();
                }
            }
        }
    }
    
    /// Manually reset the circuit breaker (admin function)
    pub fn reset_circuit_breaker(&mut self) {
        if self.circuit_breaker_active {
            self.circuit_breaker_active = false;
            self.circuit_breaker_activated_at = None;
            self.consecutive_losses = 0;
            self.last_loss_time = None;
            
            info!("✅ Circuit breaker reset. Trading can resume.");
        }
    }
    
    /// Reset daily tracking if we've crossed into a new day
    fn reset_daily_tracking_if_needed(&mut self) {
        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_daily_reset).unwrap_or_default();
        
        // Reset after 24 hours (simplified - real system would use market hours)
        if elapsed > Duration::from_secs(24 * 3600) {
            self.daily_loss = Decimal::ZERO;
            self.last_daily_reset = now;
            info!("🌅 Daily risk tracking reset");
        }
    }
    
    /// Get current protocol status
    pub fn get_status(&self) -> ProtocolStatus {
        ProtocolStatus {
            total_portfolio_risk: self.total_portfolio_risk,
            consecutive_losses: self.consecutive_losses,
            daily_loss: self.daily_loss,
            open_positions: self.open_positions,
            circuit_breaker_active: self.circuit_breaker_active,
            risk_utilization: self.total_portfolio_risk / self.limits.max_total_portfolio_risk,
            days_since_last_reset: SystemTime::now()
                .duration_since(self.last_daily_reset)
                .unwrap_or_default()
                .as_secs() / 86400,
            portfolio_exposure: self.portfolio_exposure.clone(),
        }
    }
    
    /// Get protocol limits
    pub fn limits(&self) -> &ProtocolLimits {
        &self.limits
    }
    
    /// Check if trading is currently allowed
    pub fn is_trading_allowed(&mut self) -> bool {
        self.reset_daily_tracking_if_needed();
        self.check_circuit_breaker_reset();
        !self.circuit_breaker_active
    }
    
    /// Calculate remaining risk budget
    pub fn remaining_risk_budget(&self) -> Decimal {
        (self.limits.max_total_portfolio_risk - self.total_portfolio_risk).max(Decimal::ZERO)
    }
    
    /// Calculate remaining daily loss budget
    pub fn remaining_daily_budget(&self, account_equity: Decimal) -> Decimal {
        if account_equity.is_zero() {
            return Decimal::ZERO;
        }
        
        let daily_limit = account_equity * self.limits.max_daily_loss;
        (daily_limit - self.daily_loss).max(Decimal::ZERO)
    }
}

impl Default for TestudoProtocol {
    fn default() -> Self {
        Self::new()
    }
}

/// Current status of the Testudo Protocol
#[derive(Debug, Clone)]
pub struct ProtocolStatus {
    pub total_portfolio_risk: Decimal,
    pub consecutive_losses: u32,
    pub daily_loss: Decimal,
    pub open_positions: u32,
    pub circuit_breaker_active: bool,
    pub risk_utilization: Decimal, // Percentage of max risk used
    pub days_since_last_reset: u64,
    pub portfolio_exposure: HashMap<String, Decimal>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TradeSide;
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};
    use rust_decimal_macros::dec;
    
    fn create_test_proposal(risk_pct: Decimal) -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(),
            Some(PricePoint::new(dec!(54000)).unwrap()),
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(risk_pct).unwrap(),
        ).unwrap()
    }
    
    #[test]
    fn test_protocol_creation() {
        let protocol = TestudoProtocol::new();
        let status = protocol.get_status();
        
        assert_eq!(status.total_portfolio_risk, Decimal::ZERO);
        assert_eq!(status.consecutive_losses, 0);
        assert_eq!(status.open_positions, 0);
        assert!(!status.circuit_breaker_active);
    }
    
    #[test]
    fn test_valid_trade_validation() {
        let mut protocol = TestudoProtocol::new();
        let proposal = create_test_proposal(dec!(0.02)); // 2% risk
        
        let result = protocol.validate_trade(&proposal);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_excessive_individual_risk_rejection() {
        let mut protocol = TestudoProtocol::new();
        let proposal = create_test_proposal(dec!(0.08)); // 8% risk (exceeds 6% limit)
        
        let result = protocol.validate_trade(&proposal);
        assert!(result.is_err());
        
        let violations = result.unwrap_err();
        assert!(violations.iter().any(|v| v.rule_name.contains("MaxIndividualTradeRisk") || v.rule_name.contains("ExceedsMaxIndividualRisk")));
    }
    
    #[test]
    fn test_portfolio_risk_accumulation() {
        let mut protocol = TestudoProtocol::new();
        
        // Add several trades to approach portfolio limit
        let proposal1 = create_test_proposal(dec!(0.04)); // 4% risk
        let proposal2 = create_test_proposal(dec!(0.04)); // 4% risk
        let proposal3 = create_test_proposal(dec!(0.04)); // 4% risk (would exceed 10% total)
        
        // First two trades should pass
        assert!(protocol.validate_trade(&proposal1).is_ok());
        protocol.record_trade_execution(&proposal1);
        
        assert!(protocol.validate_trade(&proposal2).is_ok());
        protocol.record_trade_execution(&proposal2);
        
        // Third trade should fail due to portfolio limit
        let result = protocol.validate_trade(&proposal3);
        assert!(result.is_err());
        
        let violations = result.unwrap_err();
        assert!(violations.iter().any(|v| v.rule_name.contains("MaxPortfolioRisk") || v.rule_name.contains("ExceedsMaxPortfolioRisk")));
    }
    
    #[test]
    fn test_consecutive_loss_circuit_breaker() {
        let mut protocol = TestudoProtocol::new();
        let proposal = create_test_proposal(dec!(0.02));
        
        // Record consecutive losses
        protocol.record_trade_outcome("BTCUSDT", dec!(0.02), true, Some(dec!(200))); // Loss 1
        assert!(protocol.is_trading_allowed());
        
        protocol.record_trade_outcome("ETHUSDT", dec!(0.02), true, Some(dec!(200))); // Loss 2
        assert!(protocol.is_trading_allowed());
        
        protocol.record_trade_outcome("ADAUSDT", dec!(0.02), true, Some(dec!(200))); // Loss 3
        
        // Circuit breaker should now be active
        assert!(!protocol.is_trading_allowed());
        
        let result = protocol.validate_trade(&proposal);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_consecutive_loss_reset_on_win() {
        let mut protocol = TestudoProtocol::new();
        
        // Record two losses
        protocol.record_trade_outcome("BTCUSDT", dec!(0.02), true, Some(dec!(200)));
        protocol.record_trade_outcome("ETHUSDT", dec!(0.02), true, Some(dec!(200)));
        
        let status = protocol.get_status();
        assert_eq!(status.consecutive_losses, 2);
        
        // Record a win - should reset consecutive losses
        protocol.record_trade_outcome("ADAUSDT", dec!(0.02), false, None);
        
        let status = protocol.get_status();
        assert_eq!(status.consecutive_losses, 0);
        assert!(protocol.is_trading_allowed());
    }
    
    #[test]
    fn test_risk_budget_calculations() {
        let mut protocol = TestudoProtocol::new();
        let proposal = create_test_proposal(dec!(0.04)); // 4% risk
        
        // Initial budget should be 10% (full limit)
        assert_eq!(protocol.remaining_risk_budget(), dec!(0.10));
        
        // Execute trade
        protocol.record_trade_execution(&proposal);
        
        // Budget should now be 6% (10% - 4%)
        assert_eq!(protocol.remaining_risk_budget(), dec!(0.06));
        
        // Close the trade
        protocol.record_trade_outcome("BTCUSDT", dec!(0.04), false, None);
        
        // Budget should return to 10%
        assert_eq!(protocol.remaining_risk_budget(), dec!(0.10));
    }
    
    #[test]
    fn test_daily_loss_tracking() {
        let mut protocol = TestudoProtocol::new();
        let account_equity = dec!(10000);
        
        // Initial daily budget should be 5% of account (500)
        assert_eq!(protocol.remaining_daily_budget(account_equity), dec!(500));
        
        // Record some losses
        protocol.record_trade_outcome("BTCUSDT", dec!(0.02), true, Some(dec!(200)));
        assert_eq!(protocol.remaining_daily_budget(account_equity), dec!(300));
        
        protocol.record_trade_outcome("ETHUSDT", dec!(0.02), true, Some(dec!(150)));
        assert_eq!(protocol.remaining_daily_budget(account_equity), dec!(150));
    }
    
    #[test]
    fn test_portfolio_exposure_tracking() {
        let mut protocol = TestudoProtocol::new();
        
        let btc_proposal = create_test_proposal(dec!(0.03));
        let eth_proposal = TradeProposal::new(
            "ETHUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(3000)).unwrap(),
            PricePoint::new(dec!(2800)).unwrap(),
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap();
        
        // Execute trades
        protocol.record_trade_execution(&btc_proposal);
        protocol.record_trade_execution(&eth_proposal);
        
        let status = protocol.get_status();
        assert_eq!(status.portfolio_exposure.get("BTCUSDT"), Some(&dec!(0.03)));
        assert_eq!(status.portfolio_exposure.get("ETHUSDT"), Some(&dec!(0.02)));
        assert_eq!(status.total_portfolio_risk, dec!(0.05));
        
        // Close BTC trade
        protocol.record_trade_outcome("BTCUSDT", dec!(0.03), false, None);
        
        let status = protocol.get_status();
        assert_eq!(status.portfolio_exposure.get("BTCUSDT"), None);
        assert_eq!(status.portfolio_exposure.get("ETHUSDT"), Some(&dec!(0.02)));
        assert_eq!(status.total_portfolio_risk, dec!(0.02));
    }
    
    //=============================================================================
    // RISK MANAGEMENT PROTOCOL TESTS - Task 3
    //=============================================================================
    
    /// Helper function to create a test trade proposal for RiskManagementProtocol tests
    fn create_test_proposal_for_protocol() -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(),
            Some(PricePoint::new(dec!(54000)).unwrap()),
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap()
    }
    
    #[test]
    fn test_risk_management_protocol_creation() {
        let protocol = RiskManagementProtocol::new();
        assert_eq!(protocol.rule_count(), 0);
        assert_eq!(protocol.name(), "RiskManagementProtocol");
        
        let custom_protocol = RiskManagementProtocol::with_name("CustomProtocol".to_string(), true);
        assert_eq!(custom_protocol.name(), "CustomProtocol");
    }
    
    #[test]
    fn test_empty_protocol_fails_assessment() {
        let protocol = RiskManagementProtocol::new();
        let proposal = create_test_proposal_for_protocol();
        
        let result = protocol.assess_trade(&proposal);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ProtocolError::NoRulesConfigured => {}, // Expected
            other => panic!("Expected NoRulesConfigured, got: {:?}", other),
        }
    }
    
    #[test]
    fn test_single_rule_protocol_approval() {
        use crate::risk::assessment_rules::MaxTradeRiskRule;
        
        let protocol = RiskManagementProtocol::new()
            .add_rule(MaxTradeRiskRule::new());
        
        let proposal = create_test_proposal_for_protocol();
        let result = protocol.assess_trade(&proposal).unwrap();
        
        // Verify protocol decision
        assert!(result.is_approved());
        assert_eq!(result.protocol_decision, ProtocolDecision::Approved);
        assert!(!result.is_rejected());
        assert!(!result.has_failures());
        
        // Verify assessment details
        assert_eq!(result.rule_results.len(), 1);
        assert!(result.rule_results[0].assessment.is_ok());
        assert_eq!(result.rule_results[0].rule_name, "MaxTradeRisk");
        assert!(result.rule_results[0].execution_time_ms > 0);
        
        // Verify consolidated assessment
        assert!(result.assessment.is_approved());
        assert_eq!(result.assessment.approval_status, ApprovalStatus::Approved);
        assert!(result.violations().is_empty());
        assert!(result.critical_violations().is_empty());
        assert_eq!(result.failed_rule_count(), 0);
        
        // Verify decision reasoning
        assert!(!result.decision_reasoning.is_empty());
        assert!(result.decision_reasoning.contains("approved"));
        assert!(result.decision_reasoning.contains("Testudo Protocol"));
    }
    
    #[test]
    fn test_single_rule_protocol_rejection() {
        use crate::risk::assessment_rules::MaxTradeRiskRule;
        
        let protocol = RiskManagementProtocol::new()
            .add_rule(MaxTradeRiskRule::new());
        
        // Create a high-risk proposal that should be rejected
        let high_risk_proposal = TradeProposal::new(
            "ETHUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(3000)).unwrap(),
            PricePoint::new(dec!(2900)).unwrap(),
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.08)).unwrap(), // 8% risk - exceeds 6% limit
        ).unwrap();
        
        let result = protocol.assess_trade(&high_risk_proposal).unwrap();
        
        // Verify protocol decision
        assert!(result.is_rejected());
        assert_eq!(result.protocol_decision, ProtocolDecision::Rejected);
        assert!(!result.is_approved());
        
        // Verify violations
        assert!(!result.violations().is_empty());
        assert!(!result.critical_violations().is_empty());
        
        // Verify the specific violation
        let critical_violations = result.critical_violations();
        assert_eq!(critical_violations.len(), 1);
        assert_eq!(critical_violations[0].rule_name, "MaxTradeRisk");
        assert_eq!(critical_violations[0].severity, ViolationSeverity::Critical);
        
        // Verify decision reasoning
        assert!(result.decision_reasoning.contains("rejected"));
        assert!(result.decision_reasoning.contains("critical violation"));
    }
    
    #[test]
    fn test_multiple_rules_protocol_assessment() {
        use crate::risk::assessment_rules::MaxTradeRiskRule;
        
        let protocol = RiskManagementProtocol::new()
            .add_rule(MaxTradeRiskRule::new())
            .add_rule(MaxTradeRiskRule::conservative()); // More restrictive rule
        
        let proposal = create_test_proposal_for_protocol();
        let result = protocol.assess_trade(&proposal).unwrap();
        
        // Both rules should execute
        assert_eq!(result.rule_results.len(), 2);
        assert_eq!(protocol.rule_count(), 2);
        
        // First rule (standard) should pass
        assert!(result.rule_results[0].assessment.is_ok());
        
        // Second rule (conservative) should also pass for 2% risk
        assert!(result.rule_results[1].assessment.is_ok());
        
        // Overall result should be approved
        assert!(result.is_approved());
        assert_eq!(result.protocol_decision, ProtocolDecision::Approved);
        
        // Total execution time should be sum of both rules
        let total_time: u64 = result.rule_results.iter().map(|r| r.execution_time_ms).sum();
        assert_eq!(result.total_execution_time_ms(), total_time);
    }
    
    #[test]
    fn test_multiple_rules_with_mixed_results() {
        use crate::risk::assessment_rules::MaxTradeRiskRule;
        
        let protocol = RiskManagementProtocol::new()
            .add_rule(MaxTradeRiskRule::new())        // Standard: allows up to 6%
            .add_rule(MaxTradeRiskRule::conservative()); // Conservative: allows up to 2%
        
        // Create a 3% risk trade - standard rule passes, conservative rule fails
        let moderate_risk_proposal = TradeProposal::new(
            "ETHUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(3000)).unwrap(),
            PricePoint::new(dec!(2910)).unwrap(), // 3% risk distance
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.03)).unwrap(), // 3% risk
        ).unwrap();
        
        let result = protocol.assess_trade(&moderate_risk_proposal).unwrap();
        
        // First rule should pass
        assert!(result.rule_results[0].assessment.is_ok());
        let first_assessment = result.rule_results[0].assessment.as_ref().unwrap();
        assert!(first_assessment.is_approved());
        
        // Second rule should pass but create violations
        assert!(result.rule_results[1].assessment.is_ok());
        let second_assessment = result.rule_results[1].assessment.as_ref().unwrap();
        assert!(!second_assessment.is_approved()); // Conservative rule rejects 3%
        
        // Overall result should be rejected due to conservative rule violation
        assert!(result.is_rejected());
        assert!(!result.violations().is_empty());
    }
    
    #[test]
    fn test_protocol_assessment_result_methods() {
        use crate::risk::assessment_rules::MaxTradeRiskRule;
        
        let protocol = RiskManagementProtocol::new()
            .add_rule(MaxTradeRiskRule::new());
        
        let proposal = create_test_proposal_for_protocol();
        let result = protocol.assess_trade(&proposal).unwrap();
        
        // Test all convenience methods
        assert!(result.is_approved());
        assert!(!result.is_rejected());
        assert!(!result.has_failures());
        assert_eq!(result.failed_rule_count(), 0);
        assert!(result.total_execution_time_ms() > 0);
        assert!(result.violations().is_empty());
        assert!(result.critical_violations().is_empty());
        
        // Test assessment access
        assert_eq!(result.assessment.proposal_id, proposal.id);
        assert!(result.assessment.position_size.value() > Decimal::ZERO);
        assert_eq!(result.assessment.risk_percentage, dec!(0.02));
    }
    
    #[test]
    fn test_protocol_rule_ordering() {
        use crate::risk::assessment_rules::MaxTradeRiskRule;
        
        let protocol = RiskManagementProtocol::new()
            .add_rule(MaxTradeRiskRule::new())
            .add_rule(MaxTradeRiskRule::conservative())
            .add_rule(MaxTradeRiskRule::aggressive());
        
        let proposal = create_test_proposal_for_protocol();
        let result = protocol.assess_trade(&proposal).unwrap();
        
        // Verify rules executed in order
        assert_eq!(result.rule_results.len(), 3);
        
        // All should have executed and returned results
        for rule_result in &result.rule_results {
            assert!(rule_result.assessment.is_ok());
            assert!(rule_result.execution_time_ms > 0);
        }
        
        // The primary assessment should come from the first rule
        let first_assessment = result.rule_results[0].assessment.as_ref().unwrap();
        assert_eq!(result.assessment.position_size, first_assessment.position_size);
        assert_eq!(result.assessment.risk_percentage, first_assessment.risk_percentage);
    }
    
    #[test]
    fn test_protocol_fail_fast_behavior() {
        use crate::risk::assessment_rules::MaxTradeRiskRule;
        
        // Create a protocol with fail_fast enabled
        let protocol = RiskManagementProtocol::with_name("FailFastProtocol".to_string(), true)
            .add_rule(MaxTradeRiskRule::new());
        
        let proposal = create_test_proposal_for_protocol();
        
        // With valid proposal, should succeed even with fail_fast
        let result = protocol.assess_trade(&proposal);
        assert!(result.is_ok());
        
        // The protocol configuration should be correct
        assert_eq!(protocol.name(), "FailFastProtocol");
    }
    
    #[test]
    fn test_protocol_rule_sharing() {
        use crate::risk::assessment_rules::MaxTradeRiskRule;
        use std::sync::Arc;
        
        // Create a shared rule
        let shared_rule: Arc<dyn RiskRule> = Arc::new(MaxTradeRiskRule::new());
        
        // Use it in multiple protocols
        let protocol1 = RiskManagementProtocol::with_name("Protocol1".to_string(), false)
            .add_rule_ref(shared_rule.clone());
            
        let protocol2 = RiskManagementProtocol::with_name("Protocol2".to_string(), false)
            .add_rule_ref(shared_rule.clone());
        
        let proposal = create_test_proposal_for_protocol();
        
        // Both protocols should work with the shared rule
        let result1 = protocol1.assess_trade(&proposal).unwrap();
        let result2 = protocol2.assess_trade(&proposal).unwrap();
        
        assert!(result1.is_approved());
        assert!(result2.is_approved());
        assert_eq!(result1.rule_results.len(), 1);
        assert_eq!(result2.rule_results.len(), 1);
        
        // Both should have the same rule name
        assert_eq!(result1.rule_results[0].rule_name, "MaxTradeRisk");
        assert_eq!(result2.rule_results[0].rule_name, "MaxTradeRisk");
    }
    
    #[test]
    fn test_protocol_performance_tracking() {
        use crate::risk::assessment_rules::MaxTradeRiskRule;
        
        let protocol = RiskManagementProtocol::new()
            .add_rule(MaxTradeRiskRule::new())
            .add_rule(MaxTradeRiskRule::conservative())
            .add_rule(MaxTradeRiskRule::aggressive());
        
        let proposal = create_test_proposal_for_protocol();
        let result = protocol.assess_trade(&proposal).unwrap();
        
        // Verify performance tracking
        assert!(result.total_execution_time_ms() > 0);
        
        // Each rule should have recorded execution time
        for rule_result in &result.rule_results {
            assert!(rule_result.execution_time_ms > 0);
        }
        
        // Total time should be at least the sum of individual times
        let individual_sum: u64 = result.rule_results.iter().map(|r| r.execution_time_ms).sum();
        assert_eq!(result.total_execution_time_ms(), individual_sum);
    }
    
    #[test]
    fn test_protocol_default_implementation() {
        let protocol = RiskManagementProtocol::default();
        assert_eq!(protocol.rule_count(), 0);
        assert_eq!(protocol.name(), "RiskManagementProtocol");
    }
}