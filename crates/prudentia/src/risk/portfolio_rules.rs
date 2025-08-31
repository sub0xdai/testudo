//! Task 4a: Portfolio Risk Rules - MaxPortfolioRiskRule implementation
//!
//! This module implements portfolio-level risk rules that consider the aggregate
//! risk across all positions, following Roman discipline in capital allocation.

use crate::risk::assessment_rules::{RiskRule, AssessmentError};
use crate::types::{TradeProposal, RiskAssessment, ProtocolLimits, ViolationSeverity, ProtocolViolation};
use disciplina::PositionSizingCalculator;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::SystemTime;

/// Represents an open position for portfolio risk calculations
#[derive(Debug, Clone)]
pub struct OpenPosition {
    /// Unique identifier for the position
    pub id: String,
    /// Symbol being traded
    pub symbol: String,
    /// Current risk amount in dollars
    pub risk_amount: Decimal,
    /// Risk as percentage of account equity when opened
    pub risk_percentage: Decimal,
    /// Timestamp when position was opened
    pub opened_at: SystemTime,
    /// Current unrealized P&L
    pub unrealized_pnl: Decimal,
}

/// Task 4a: MaxPortfolioRiskRule implementation
/// 
/// This rule ensures that the total portfolio risk exposure across all open
/// positions plus the proposed new trade does not exceed the maximum allowed
/// portfolio risk percentage as defined by the Testudo Protocol.
#[derive(Debug, Clone)]
pub struct MaxPortfolioRiskRule {
    /// Protocol limits for validation
    limits: ProtocolLimits,
    /// Van Tharp position sizing calculator
    position_calculator: Arc<PositionSizingCalculator>,
    /// Current open positions for portfolio risk calculation
    open_positions: HashMap<String, OpenPosition>,
    /// Cache of current total portfolio risk percentage
    cached_portfolio_risk: Decimal,
    /// Last time portfolio risk was calculated
    last_calculation: SystemTime,
}

impl MaxPortfolioRiskRule {
    /// Create a new MaxPortfolioRiskRule with default protocol limits
    pub fn new() -> Self {
        Self::with_limits(ProtocolLimits::default())
    }
    
    /// Create a new MaxPortfolioRiskRule with custom protocol limits
    pub fn with_limits(limits: ProtocolLimits) -> Self {
        Self {
            limits,
            position_calculator: Arc::new(PositionSizingCalculator::new()),
            open_positions: HashMap::new(),
            cached_portfolio_risk: Decimal::ZERO,
            last_calculation: SystemTime::now(),
        }
    }
    
    /// Create a conservative MaxPortfolioRiskRule for new traders
    pub fn conservative() -> Self {
        Self::with_limits(ProtocolLimits::conservative_limits())
    }
    
    /// Create an aggressive MaxPortfolioRiskRule for experienced traders
    pub fn aggressive() -> Self {
        Self::with_limits(ProtocolLimits::aggressive_limits())
    }
    
    /// Add an open position to the portfolio tracking
    pub fn add_open_position(&mut self, position: OpenPosition) {
        self.open_positions.insert(position.id.clone(), position);
        self.invalidate_cache();
    }
    
    /// Remove an open position (when closed)
    pub fn remove_open_position(&mut self, position_id: &str) -> Option<OpenPosition> {
        let removed = self.open_positions.remove(position_id);
        if removed.is_some() {
            self.invalidate_cache();
        }
        removed
    }
    
    /// Update an existing position's risk or P&L
    pub fn update_position(&mut self, position_id: &str, unrealized_pnl: Decimal) {
        if let Some(position) = self.open_positions.get_mut(position_id) {
            position.unrealized_pnl = unrealized_pnl;
            self.invalidate_cache();
        }
    }
    
    /// Get current total portfolio risk percentage
    pub fn current_portfolio_risk(&mut self) -> Decimal {
        // Use cached value if recent (within 1 second)
        if self.last_calculation.elapsed().unwrap_or_default().as_secs() < 1 {
            return self.cached_portfolio_risk;
        }
        
        // Recalculate portfolio risk
        self.cached_portfolio_risk = self.calculate_portfolio_risk();
        self.last_calculation = SystemTime::now();
        self.cached_portfolio_risk
    }
    
    /// Calculate total portfolio risk across all open positions
    fn calculate_portfolio_risk(&self) -> Decimal {
        self.open_positions
            .values()
            .map(|position| position.risk_percentage)
            .sum()
    }
    
    /// Get number of open positions
    pub fn position_count(&self) -> usize {
        self.open_positions.len()
    }
    
    /// Get total risk amount in dollars across all positions
    pub fn total_risk_amount(&self) -> Decimal {
        self.open_positions
            .values()
            .map(|position| position.risk_amount)
            .sum()
    }
    
    /// Invalidate cached portfolio risk calculation
    fn invalidate_cache(&mut self) {
        self.last_calculation = SystemTime::UNIX_EPOCH; // Force recalculation
    }
}

impl RiskRule for MaxPortfolioRiskRule {
    fn assess(&self, proposal: &TradeProposal) -> Result<RiskAssessment, AssessmentError> {
        // Step 1: Calculate position size for the proposed trade
        let position_size = self.position_calculator
            .calculate_position_size(
                proposal.account_equity,
                proposal.risk_percentage,
                proposal.entry_price,
                proposal.stop_loss,
            )
            .map_err(|e| AssessmentError::PositionSizingFailure { 
                reason: e.to_string() 
            })?;
        
        // Step 2: Calculate risk metrics for proposed trade
        let risk_distance = proposal.risk_distance();
        let risk_amount = position_size.value() * risk_distance;
        let trade_risk_percentage = proposal.risk_percentage.value();
        let portfolio_impact = risk_amount / proposal.account_equity.value();
        
        // Step 3: Calculate current portfolio risk
        let current_portfolio_risk = self.calculate_portfolio_risk();
        let projected_portfolio_risk = current_portfolio_risk + trade_risk_percentage;
        
        // Step 4: Create initial assessment
        let mut assessment = RiskAssessment::new(
            proposal.id,
            position_size,
            risk_amount,
            trade_risk_percentage,
            proposal.risk_reward_ratio(),
            portfolio_impact,
        );
        
        // Step 5: Check if projected portfolio risk exceeds limits
        if projected_portfolio_risk > self.limits.max_total_portfolio_risk {
            let violation = ProtocolViolation::new(
                self.rule_name().to_string(),
                ViolationSeverity::Critical,
                format!(
                    "Portfolio risk would reach {:.1}% (current {:.1}% + trade {:.1}%) exceeding maximum {:.1}%",
                    projected_portfolio_risk * dec!(100),
                    current_portfolio_risk * dec!(100),
                    trade_risk_percentage * dec!(100),
                    self.limits.max_total_portfolio_risk * dec!(100)
                ),
                projected_portfolio_risk,
                self.limits.max_total_portfolio_risk,
                format!(
                    "Reduce position size or close existing positions. Available risk budget: {:.1}%",
                    (self.limits.max_total_portfolio_risk - current_portfolio_risk) * dec!(100)
                ),
            );
            assessment.add_violation(violation);
        }
        
        // Step 6: Add portfolio context to reasoning
        let reasoning = if assessment.is_approved() {
            format!(
                "Portfolio risk approved: Adding {:.1}% trade risk to current {:.1}% portfolio risk = {:.1}% total (within {:.1}% limit). {} positions currently open.",
                trade_risk_percentage * dec!(100),
                current_portfolio_risk * dec!(100),
                projected_portfolio_risk * dec!(100),
                self.limits.max_total_portfolio_risk * dec!(100),
                self.open_positions.len()
            )
        } else {
            format!(
                "Portfolio risk violation: Adding {:.1}% trade risk to current {:.1}% portfolio risk would exceed {:.1}% limit. Available budget: {:.1}%",
                trade_risk_percentage * dec!(100),
                current_portfolio_risk * dec!(100),
                self.limits.max_total_portfolio_risk * dec!(100),
                (self.limits.max_total_portfolio_risk - current_portfolio_risk) * dec!(100)
            )
        };
        
        Ok(assessment.with_reasoning(reasoning))
    }
    
    fn rule_name(&self) -> &str {
        "MaxPortfolioRisk"
    }
    
    fn description(&self) -> &str {
        "Validates that total portfolio risk across all positions does not exceed maximum protocol limit"
    }
}

impl Default for MaxPortfolioRiskRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Task 4b: DailyLossLimitRule implementation
/// 
/// This rule tracks daily profit and loss and prevents trades that would cause
/// the daily loss to exceed configured limits. Automatically resets at market open.
#[derive(Debug, Clone)]
pub struct DailyLossLimitRule {
    /// Protocol limits for validation
    limits: ProtocolLimits,
    /// Van Tharp position sizing calculator
    position_calculator: Arc<PositionSizingCalculator>,
    /// Current daily profit/loss (negative = loss)
    daily_pnl: Decimal,
    /// Maximum daily loss allowed (positive value, e.g., 1000 = $1000 max loss)
    max_daily_loss: Decimal,
    /// Date when daily P&L was last reset (for automatic reset)
    last_reset_date: SystemTime,
    /// Number of trades taken today (for context)
    daily_trade_count: u32,
    /// Trading session timezone offset in hours (default: UTC)
    timezone_offset_hours: i8,
}

impl DailyLossLimitRule {
    /// Create a new DailyLossLimitRule with default limits
    pub fn new() -> Self {
        Self::with_daily_limit(Decimal::from(1000)) // $1000 default daily loss limit
    }
    
    /// Create a new DailyLossLimitRule with custom daily loss limit
    pub fn with_daily_limit(max_daily_loss: Decimal) -> Self {
        Self {
            limits: ProtocolLimits::default(),
            position_calculator: Arc::new(PositionSizingCalculator::new()),
            daily_pnl: Decimal::ZERO,
            max_daily_loss,
            last_reset_date: SystemTime::now(),
            daily_trade_count: 0,
            timezone_offset_hours: 0, // UTC default
        }
    }
    
    /// Create a conservative DailyLossLimitRule for new traders
    pub fn conservative() -> Self {
        Self::with_daily_limit(Decimal::from(500)) // $500 conservative limit
    }
    
    /// Create an aggressive DailyLossLimitRule for experienced traders
    pub fn aggressive() -> Self {
        Self::with_daily_limit(Decimal::from(2000)) // $2000 aggressive limit
    }
    
    /// Set timezone offset for market hours (e.g., -5 for EST, +0 for UTC)
    pub fn with_timezone_offset(mut self, hours: i8) -> Self {
        self.timezone_offset_hours = hours;
        self
    }
    
    /// Record a completed trade's P&L
    pub fn record_trade_pnl(&mut self, pnl: Decimal) {
        self.check_daily_reset();
        self.daily_pnl += pnl;
        self.daily_trade_count += 1;
    }
    
    /// Get current daily P&L
    pub fn current_daily_pnl(&mut self) -> Decimal {
        self.check_daily_reset();
        self.daily_pnl
    }
    
    /// Get current daily loss (negative P&L, returned as positive value)
    pub fn current_daily_loss(&mut self) -> Decimal {
        let pnl = self.current_daily_pnl();
        if pnl < Decimal::ZERO {
            pnl.abs() // Return loss as positive value
        } else {
            Decimal::ZERO
        }
    }
    
    /// Get available loss budget remaining today
    pub fn available_loss_budget(&mut self) -> Decimal {
        let current_loss = self.current_daily_loss();
        if current_loss >= self.max_daily_loss {
            Decimal::ZERO
        } else {
            self.max_daily_loss - current_loss
        }
    }
    
    /// Get number of trades taken today
    pub fn daily_trade_count(&mut self) -> u32 {
        self.check_daily_reset();
        self.daily_trade_count
    }
    
    /// Check if we need to reset daily counters (new trading day)
    fn check_daily_reset(&mut self) {
        let now = SystemTime::now();
        
        // Calculate market open time for today in the configured timezone
        // For simplicity, assume market opens at 9:30 AM in the configured timezone
        let seconds_since_epoch = now.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let today_market_open_seconds = self.calculate_market_open_seconds(seconds_since_epoch);
        let last_reset_seconds = self.last_reset_date.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Reset if it's been more than 24 hours or if we've crossed market open
        if seconds_since_epoch >= today_market_open_seconds && 
           last_reset_seconds < today_market_open_seconds {
            self.daily_pnl = Decimal::ZERO;
            self.daily_trade_count = 0;
            self.last_reset_date = now;
        }
    }
    
    /// Calculate market open time in seconds since epoch for the current day
    fn calculate_market_open_seconds(&self, current_seconds: u64) -> u64 {
        // Simplified: assume market opens at 9:30 AM in configured timezone
        let seconds_per_day = 86400;
        let market_open_hour = 9;
        let market_open_minute = 30;
        let timezone_offset_seconds = (self.timezone_offset_hours as i64) * 3600;
        
        // Get current day start (midnight UTC) and adjust for timezone and market open
        let days_since_epoch = (current_seconds as i64 + timezone_offset_seconds) / (seconds_per_day as i64);
        let market_open_seconds = (days_since_epoch * seconds_per_day as i64) + 
                                 (market_open_hour * 3600) + 
                                 (market_open_minute * 60) - 
                                 timezone_offset_seconds;
        
        market_open_seconds as u64
    }
    
    /// Manually reset daily counters (for testing or manual intervention)
    pub fn reset_daily_counters(&mut self) {
        self.daily_pnl = Decimal::ZERO;
        self.daily_trade_count = 0;
        self.last_reset_date = SystemTime::now();
    }
}

impl RiskRule for DailyLossLimitRule {
    fn assess(&self, proposal: &TradeProposal) -> Result<RiskAssessment, AssessmentError> {
        // Create mutable copy to check daily reset
        let mut mutable_self = self.clone();
        mutable_self.check_daily_reset();
        
        // Step 1: Calculate position size for the proposed trade
        let position_size = mutable_self.position_calculator
            .calculate_position_size(
                proposal.account_equity,
                proposal.risk_percentage,
                proposal.entry_price,
                proposal.stop_loss,
            )
            .map_err(|e| AssessmentError::PositionSizingFailure { 
                reason: e.to_string() 
            })?;
        
        // Step 2: Calculate potential loss from this trade
        let risk_distance = proposal.risk_distance();
        let potential_trade_loss = position_size.value() * risk_distance;
        let trade_risk_percentage = proposal.risk_percentage.value();
        let portfolio_impact = potential_trade_loss / proposal.account_equity.value();
        
        // Step 3: Calculate projected daily loss if this trade hits stop loss
        let current_daily_loss = mutable_self.current_daily_loss();
        let projected_daily_loss = current_daily_loss + potential_trade_loss;
        
        // Step 4: Create initial assessment
        let mut assessment = RiskAssessment::new(
            proposal.id,
            position_size,
            potential_trade_loss,
            trade_risk_percentage,
            proposal.risk_reward_ratio(),
            portfolio_impact,
        );
        
        // Step 5: Check if projected daily loss exceeds limits
        let available_budget = mutable_self.available_loss_budget();
        if projected_daily_loss > mutable_self.max_daily_loss {
            
            let violation = ProtocolViolation::new(
                self.rule_name().to_string(),
                ViolationSeverity::Critical,
                format!(
                    "Daily loss would reach ${:.2} (current ${:.2} + potential ${:.2}) exceeding limit ${:.2}",
                    projected_daily_loss,
                    current_daily_loss,
                    potential_trade_loss,
                    mutable_self.max_daily_loss
                ),
                projected_daily_loss,
                mutable_self.max_daily_loss,
                if available_budget > Decimal::ZERO {
                    format!("Reduce position size to risk no more than ${:.2}", available_budget)
                } else {
                    "Stop trading for today - daily loss limit reached".to_string()
                },
            );
            assessment.add_violation(violation);
        }
        
        // Step 6: Add daily context to reasoning
        let reasoning = if assessment.is_approved() {
            format!(
                "Daily loss approved: Current daily P&L ${:.2}, potential trade loss ${:.2}, projected daily loss ${:.2} within ${:.2} limit. {} trades taken today.",
                mutable_self.daily_pnl,
                potential_trade_loss,
                projected_daily_loss,
                mutable_self.max_daily_loss,
                mutable_self.daily_trade_count
            )
        } else {
            format!(
                "Daily loss violation: Current daily loss ${:.2}, potential trade loss ${:.2} would exceed ${:.2} limit. Available budget: ${:.2}",
                current_daily_loss,
                potential_trade_loss,
                mutable_self.max_daily_loss,
                available_budget
            )
        };
        
        Ok(assessment.with_reasoning(reasoning))
    }
    
    fn rule_name(&self) -> &str {
        "DailyLossLimit"
    }
    
    fn description(&self) -> &str {
        "Validates that daily loss does not exceed configured daily loss limits"
    }
}

impl Default for DailyLossLimitRule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TradeSide, ApprovalStatus};
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};
    use rust_decimal_macros::dec;
    use std::time::SystemTime;

    fn create_test_proposal(risk_pct: Decimal) -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(100)).unwrap(),
            PricePoint::new(dec!(95)).unwrap(), // $5 risk distance
            Some(PricePoint::new(dec!(110)).unwrap()), // 2:1 reward/risk ratio (10/5 = 2:1)
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(risk_pct).unwrap(),
        ).unwrap()
    }

    #[test]
    fn test_max_portfolio_risk_rule_creation() {
        let rule = MaxPortfolioRiskRule::new();
        assert_eq!(rule.rule_name(), "MaxPortfolioRisk");
        assert!(!rule.description().is_empty());
        assert_eq!(rule.position_count(), 0);
        assert_eq!(rule.total_risk_amount(), dec!(0));
    }

    #[test]
    fn test_portfolio_risk_variants() {
        let standard_rule = MaxPortfolioRiskRule::new();
        let conservative_rule = MaxPortfolioRiskRule::conservative();
        let aggressive_rule = MaxPortfolioRiskRule::aggressive();
        
        // Verify different limits are applied
        assert!(conservative_rule.limits.max_total_portfolio_risk < standard_rule.limits.max_total_portfolio_risk);
        assert!(standard_rule.limits.max_total_portfolio_risk < aggressive_rule.limits.max_total_portfolio_risk);
    }

    #[test]
    fn test_empty_portfolio_trade_approval() {
        let rule = MaxPortfolioRiskRule::new();
        let proposal = create_test_proposal(dec!(0.05)); // 5% trade risk
        
        let result = rule.assess(&proposal);
        if let Err(ref e) = result {
            println!("Assessment error: {:?}", e);
        }
        assert!(result.is_ok());
        
        let assessment = result.unwrap();
        assert_eq!(assessment.proposal_id, proposal.id);
        assert!(assessment.is_approved());
        assert_eq!(assessment.approval_status, ApprovalStatus::Approved);
        assert!(assessment.violations.is_empty());
        
        // Verify reasoning mentions portfolio context
        let reasoning = assessment.reasoning.unwrap();
        assert!(reasoning.contains("0.0% portfolio risk"));
        assert!(reasoning.contains("0 positions"));
    }

    #[test]
    fn test_portfolio_position_management() {
        let mut rule = MaxPortfolioRiskRule::new();
        
        // Add some open positions
        let position1 = OpenPosition {
            id: "pos1".to_string(),
            symbol: "BTCUSDT".to_string(),
            risk_amount: dec!(200),
            risk_percentage: dec!(0.02), // 2%
            opened_at: SystemTime::now(),
            unrealized_pnl: dec!(50),
        };
        
        let position2 = OpenPosition {
            id: "pos2".to_string(),
            symbol: "ETHUSDT".to_string(),
            risk_amount: dec!(150),
            risk_percentage: dec!(0.015), // 1.5%
            opened_at: SystemTime::now(),
            unrealized_pnl: dec!(-25),
        };
        
        rule.add_open_position(position1);
        rule.add_open_position(position2);
        
        assert_eq!(rule.position_count(), 2);
        assert_eq!(rule.total_risk_amount(), dec!(350)); // 200 + 150
        assert_eq!(rule.current_portfolio_risk(), dec!(0.035)); // 2% + 1.5%
        
        // Remove a position
        let removed = rule.remove_open_position("pos1");
        assert!(removed.is_some());
        assert_eq!(rule.position_count(), 1);
        assert_eq!(rule.total_risk_amount(), dec!(150));
        assert_eq!(rule.current_portfolio_risk(), dec!(0.015));
    }

    #[test]
    fn test_portfolio_risk_limit_enforcement() {
        let mut rule = MaxPortfolioRiskRule::new(); // 10% max portfolio risk
        
        // Add positions totaling 7% risk
        let existing_position = OpenPosition {
            id: "existing".to_string(),
            symbol: "BTCUSDT".to_string(),
            risk_amount: dec!(700),
            risk_percentage: dec!(0.07), // 7%
            opened_at: SystemTime::now(),
            unrealized_pnl: dec!(0),
        };
        rule.add_open_position(existing_position);
        
        // Try to add 5% more risk (would total 12%, exceeding 10% limit)
        let proposal = create_test_proposal(dec!(0.05)); // 5% risk
        
        let result = rule.assess(&proposal);
        assert!(result.is_ok());
        
        let assessment = result.unwrap();
        assert!(!assessment.is_approved());
        assert!(!assessment.violations.is_empty());
        
        // Verify the violation details
        let violation = &assessment.violations[0];
        assert_eq!(violation.rule_name, "MaxPortfolioRisk");
        assert_eq!(violation.severity, ViolationSeverity::Critical);
        assert!(violation.description.contains("12.0%"));
        assert!(violation.description.contains("maximum 10.0%"));
        assert_eq!(violation.current_value, dec!(0.12)); // 12%
        assert_eq!(violation.limit_value, dec!(0.10)); // 10% limit
        
        // Verify rejection reasoning
        let reasoning = assessment.reasoning.unwrap();
        assert!(reasoning.contains("violation"));
        assert!(reasoning.contains("Available budget: 3.0%")); // 10% - 7% = 3%
    }

    #[test]
    fn test_conservative_portfolio_stricter_limits() {
        let mut conservative_rule = MaxPortfolioRiskRule::conservative(); // 5% max portfolio
        let mut standard_rule = MaxPortfolioRiskRule::new(); // 10% max portfolio
        
        // Add existing positions
        let existing_position = OpenPosition {
            id: "test".to_string(),
            symbol: "BTCUSDT".to_string(),
            risk_amount: dec!(300),
            risk_percentage: dec!(0.03), // 3%
            opened_at: SystemTime::now(),
            unrealized_pnl: dec!(0),
        };
        
        conservative_rule.add_open_position(existing_position.clone());
        standard_rule.add_open_position(existing_position);
        
        // Try to add 3% more risk (total would be 6%)
        let proposal = create_test_proposal(dec!(0.03)); // 3% risk
        
        // Conservative rule should reject (6% > 5% limit)
        let conservative_assessment = conservative_rule.assess(&proposal).unwrap();
        assert!(!conservative_assessment.is_approved());
        
        // Standard rule should approve (6% < 10% limit)
        let standard_assessment = standard_rule.assess(&proposal).unwrap();
        assert!(standard_assessment.is_approved());
    }

    #[test]
    fn test_portfolio_position_updates() {
        let mut rule = MaxPortfolioRiskRule::new();
        
        let position = OpenPosition {
            id: "test".to_string(),
            symbol: "BTCUSDT".to_string(),
            risk_amount: dec!(200),
            risk_percentage: dec!(0.02),
            opened_at: SystemTime::now(),
            unrealized_pnl: dec!(0),
        };
        
        rule.add_open_position(position);
        
        // Update P&L
        rule.update_position("test", dec!(100));
        
        // Verify P&L was updated
        let updated_position = &rule.open_positions["test"];
        assert_eq!(updated_position.unrealized_pnl, dec!(100));
    }

    #[test]
    fn test_portfolio_risk_caching() {
        let mut rule = MaxPortfolioRiskRule::new();
        
        let position = OpenPosition {
            id: "test".to_string(),
            symbol: "BTCUSDT".to_string(),
            risk_amount: dec!(200),
            risk_percentage: dec!(0.02),
            opened_at: SystemTime::now(),
            unrealized_pnl: dec!(0),
        };
        rule.add_open_position(position);
        
        // First call should calculate and cache
        let risk1 = rule.current_portfolio_risk();
        let time1 = rule.last_calculation;
        
        // Second call within 1 second should use cache
        let risk2 = rule.current_portfolio_risk();
        let time2 = rule.last_calculation;
        
        assert_eq!(risk1, risk2);
        assert_eq!(time1, time2); // Time shouldn't change if cached
    }

    // Task 4b: DailyLossLimitRule tests
    #[test]
    fn test_daily_loss_limit_rule_creation() {
        let rule = DailyLossLimitRule::new();
        assert_eq!(rule.rule_name(), "DailyLossLimit");
        assert!(!rule.description().is_empty());
        assert_eq!(rule.max_daily_loss, dec!(1000)); // Default $1000 limit
    }

    #[test]
    fn test_daily_loss_limit_variants() {
        let standard_rule = DailyLossLimitRule::new();
        let conservative_rule = DailyLossLimitRule::conservative();
        let aggressive_rule = DailyLossLimitRule::aggressive();
        
        // Verify different limits
        assert_eq!(standard_rule.max_daily_loss, dec!(1000));
        assert_eq!(conservative_rule.max_daily_loss, dec!(500));
        assert_eq!(aggressive_rule.max_daily_loss, dec!(2000));
    }

    #[test]
    fn test_empty_daily_loss_trade_approval() {
        let rule = DailyLossLimitRule::new(); // $1000 daily limit
        let proposal = create_test_proposal(dec!(0.02)); // 2% risk = $200 potential loss
        
        let result = rule.assess(&proposal);
        assert!(result.is_ok());
        
        let assessment = result.unwrap();
        assert!(assessment.is_approved());
        assert!(assessment.violations.is_empty());
        
        // Verify reasoning mentions daily context
        let reasoning = assessment.reasoning.unwrap();
        assert!(reasoning.contains("Daily loss approved"));
        assert!(reasoning.contains("$0.00")); // No current daily loss
        assert!(reasoning.contains("0 trades taken today"));
    }

    #[test]
    fn test_daily_loss_pnl_tracking() {
        let mut rule = DailyLossLimitRule::new();
        
        // Record some trades
        rule.record_trade_pnl(dec!(50)); // Win $50
        rule.record_trade_pnl(dec!(-100)); // Lose $100
        rule.record_trade_pnl(dec!(25)); // Win $25
        
        // Net P&L should be -$25
        assert_eq!(rule.current_daily_pnl(), dec!(-25));
        assert_eq!(rule.current_daily_loss(), dec!(25)); // Loss as positive value
        assert_eq!(rule.daily_trade_count(), 3);
        assert_eq!(rule.available_loss_budget(), dec!(975)); // $1000 - $25 = $975
    }

    #[test]
    fn test_daily_loss_limit_enforcement() {
        let mut rule = DailyLossLimitRule::new(); // $1000 limit
        
        // Record losses bringing us close to limit
        rule.record_trade_pnl(dec!(-850)); // $850 loss
        
        // Try to place trade that would risk $400 (would exceed $1000 limit)
        let proposal = create_test_proposal(dec!(0.04)); // 4% risk = $400 potential loss
        
        let result = rule.assess(&proposal);
        assert!(result.is_ok());
        
        let assessment = result.unwrap();
        assert!(!assessment.is_approved());
        assert!(!assessment.violations.is_empty());
        
        // Verify violation details
        let violation = &assessment.violations[0];
        assert_eq!(violation.rule_name, "DailyLossLimit");
        assert_eq!(violation.severity, ViolationSeverity::Critical);
        assert!(violation.description.contains("$1250.00")); // $850 + $400 projected
        assert!(violation.description.contains("exceeding limit $1000.00"));
        
        // Available budget should be $150 ($1000 limit - $850 current loss)
        assert!(violation.suggested_action.contains("$150.00")); // Available budget
    }

    #[test]
    fn test_daily_loss_budget_exhausted() {
        let mut rule = DailyLossLimitRule::conservative(); // $500 limit
        
        // Record losses that exhaust the budget
        rule.record_trade_pnl(dec!(-500)); // Exactly at limit
        
        assert_eq!(rule.current_daily_loss(), dec!(500));
        assert_eq!(rule.available_loss_budget(), dec!(0));
        
        // Try to place any trade
        let proposal = create_test_proposal(dec!(0.01)); // Even small 1% risk
        
        let result = rule.assess(&proposal);
        assert!(result.is_ok());
        
        let assessment = result.unwrap();
        assert!(!assessment.is_approved());
        
        let violation = &assessment.violations[0];
        assert!(violation.suggested_action.contains("Stop trading for today"));
    }

    #[test]
    fn test_daily_loss_with_profits() {
        let mut rule = DailyLossLimitRule::new(); // $1000 limit
        
        // Record mixed results with net profit
        rule.record_trade_pnl(dec!(-200)); // Loss
        rule.record_trade_pnl(dec!(300)); // Win
        rule.record_trade_pnl(dec!(-50)); // Small loss
        
        // Net P&L is +$50 (profit)
        assert_eq!(rule.current_daily_pnl(), dec!(50));
        assert_eq!(rule.current_daily_loss(), dec!(0)); // No net loss
        assert_eq!(rule.available_loss_budget(), dec!(1000)); // Full budget available
        
        // Should approve even large trades
        let proposal = create_test_proposal(dec!(0.05)); // 5% risk
        let assessment = rule.assess(&proposal).unwrap();
        assert!(assessment.is_approved());
    }

    #[test]
    fn test_daily_reset_functionality() {
        let mut rule = DailyLossLimitRule::new();
        
        // Record some activity
        rule.record_trade_pnl(dec!(-500));
        assert_eq!(rule.daily_trade_count(), 1);
        assert_eq!(rule.current_daily_loss(), dec!(500));
        
        // Manually reset (simulating new day)
        rule.reset_daily_counters();
        
        // Should be reset to zero
        assert_eq!(rule.current_daily_pnl(), dec!(0));
        assert_eq!(rule.daily_trade_count(), 0);
        assert_eq!(rule.available_loss_budget(), dec!(1000));
    }

    #[test]
    fn test_timezone_configuration() {
        let rule_utc = DailyLossLimitRule::new();
        let rule_est = DailyLossLimitRule::new().with_timezone_offset(-5);
        let rule_pst = DailyLossLimitRule::new().with_timezone_offset(-8);
        
        // Verify timezone offsets are set correctly
        assert_eq!(rule_utc.timezone_offset_hours, 0);
        assert_eq!(rule_est.timezone_offset_hours, -5);
        assert_eq!(rule_pst.timezone_offset_hours, -8);
    }

    #[test]
    fn test_daily_loss_reasoning_messages() {
        let mut rule = DailyLossLimitRule::new();
        rule.record_trade_pnl(dec!(-200)); // $200 current loss
        
        let proposal = create_test_proposal(dec!(0.01)); // Small trade
        let assessment = rule.assess(&proposal).unwrap();
        
        let reasoning = assessment.reasoning.unwrap();
        assert!(reasoning.contains("Current daily P&L $-200.00"));
        assert!(reasoning.contains("1 trades taken today"));
    }
}

// ===== CONSECUTIVE LOSS LIMIT RULE =====

/// Track consecutive losses and enforce circuit breaker protection
///
/// This rule implements the Testudo Protocol's circuit breaker system that automatically
/// halts trading after a specified number of consecutive losses. This protection prevents
/// emotional "revenge trading" and gives traders time to reassess their strategy.
///
/// # Circuit Breaker Logic
/// - Tracks each trade outcome (win/loss)
/// - Consecutive loss count increases only on losses
/// - Any winning trade resets the counter to zero
/// - When consecutive losses reach the limit, all new trades are blocked
/// - Manual reset required to resume trading after circuit breaker
///
/// # Roman Military Principle
/// Like Roman generals who would halt an attack after consecutive defeats to regroup,
/// this rule enforces strategic withdrawal to prevent total destruction of capital.
#[derive(Debug, Clone)]
pub struct ConsecutiveLossLimitRule {
    /// Protocol limits for validation
    limits: ProtocolLimits,
    
    /// Position size calculator for risk assessment
    position_calculator: Arc<PositionSizingCalculator>,
    
    /// Current count of consecutive losing trades
    consecutive_losses: u32,
    
    /// Total loss amount from consecutive losses
    consecutive_loss_amount: Decimal,
    
    /// Timestamp of the last losing trade
    last_loss_timestamp: SystemTime,
    
    /// Whether the circuit breaker is currently active
    circuit_breaker_active: bool,
    
    /// Reason for circuit breaker activation (for logging/debugging)
    halt_reason: Option<String>,
    
    /// Total number of trades tracked
    total_trades_tracked: u32,
}

impl ConsecutiveLossLimitRule {
    /// Create new consecutive loss limit rule with default limits
    pub fn new() -> Self {
        Self::with_limits(ProtocolLimits::default())
    }
    
    /// Create conservative consecutive loss rule (2 losses maximum)
    pub fn conservative() -> Self {
        Self::with_limits(ProtocolLimits::conservative_limits())
    }
    
    /// Create aggressive consecutive loss rule (5 losses maximum)
    pub fn aggressive() -> Self {
        Self::with_limits(ProtocolLimits::aggressive_limits())
    }
    
    /// Create consecutive loss rule with custom limits
    pub fn with_limits(limits: ProtocolLimits) -> Self {
        Self {
            limits,
            position_calculator: Arc::new(PositionSizingCalculator::new()),
            consecutive_losses: 0,
            consecutive_loss_amount: Decimal::ZERO,
            last_loss_timestamp: SystemTime::UNIX_EPOCH,
            circuit_breaker_active: false,
            halt_reason: None,
            total_trades_tracked: 0,
        }
    }
    
    /// Record the outcome of a trade
    ///
    /// # Arguments
    /// * `pnl` - Profit/loss amount (positive for profit, negative for loss)
    ///
    /// # Returns
    /// * True if circuit breaker was triggered by this trade
    pub fn record_trade_outcome(&mut self, pnl: Decimal) -> bool {
        self.total_trades_tracked += 1;
        let was_active = self.circuit_breaker_active;
        
        if pnl < Decimal::ZERO {
            // Loss recorded
            self.consecutive_losses += 1;
            self.consecutive_loss_amount += pnl.abs();
            self.last_loss_timestamp = SystemTime::now();
            
            // Check if we need to trigger circuit breaker
            if self.consecutive_losses >= self.limits.max_consecutive_losses {
                self.circuit_breaker_active = true;
                self.halt_reason = Some(format!(
                    "Circuit breaker triggered: {} consecutive losses totaling ${:.2}",
                    self.consecutive_losses,
                    self.consecutive_loss_amount
                ));
            }
        } else if pnl > Decimal::ZERO {
            // Win recorded - reset consecutive loss tracking
            self.reset_consecutive_losses();
        }
        // Note: Break-even trades (pnl == 0) don't affect consecutive loss count
        
        // Return true if circuit breaker was just triggered
        !was_active && self.circuit_breaker_active
    }
    
    /// Manually reset consecutive loss tracking and circuit breaker
    ///
    /// This should be called after trader review and strategy adjustment.
    /// Used for resuming trading after circuit breaker activation.
    pub fn reset_consecutive_losses(&mut self) {
        self.consecutive_losses = 0;
        self.consecutive_loss_amount = Decimal::ZERO;
        self.circuit_breaker_active = false;
        self.halt_reason = None;
    }
    
    /// Get current consecutive loss count
    pub fn consecutive_losses(&self) -> u32 {
        self.consecutive_losses
    }
    
    /// Get total loss amount from consecutive losses
    pub fn consecutive_loss_amount(&self) -> Decimal {
        self.consecutive_loss_amount
    }
    
    /// Check if circuit breaker is currently active
    pub fn is_circuit_breaker_active(&self) -> bool {
        self.circuit_breaker_active
    }
    
    /// Get the reason for circuit breaker activation
    pub fn halt_reason(&self) -> Option<&str> {
        self.halt_reason.as_deref()
    }
    
    /// Get timestamp of last loss
    pub fn last_loss_timestamp(&self) -> SystemTime {
        self.last_loss_timestamp
    }
    
    /// Get total number of trades tracked
    pub fn total_trades_tracked(&self) -> u32 {
        self.total_trades_tracked
    }
    
    /// Calculate time since last loss
    pub fn time_since_last_loss(&self) -> Option<std::time::Duration> {
        if self.last_loss_timestamp == SystemTime::UNIX_EPOCH {
            None
        } else {
            SystemTime::now().duration_since(self.last_loss_timestamp).ok()
        }
    }
}

impl Default for ConsecutiveLossLimitRule {
    fn default() -> Self {
        Self::new()
    }
}

impl RiskRule for ConsecutiveLossLimitRule {
    fn assess(&self, proposal: &TradeProposal) -> Result<RiskAssessment, AssessmentError> {
        // Step 1: Calculate position size and potential loss (standard risk assessment)
        let position_size = self.position_calculator.calculate_position_size(
            proposal.account_equity,
            proposal.risk_percentage,
            proposal.entry_price,
            proposal.stop_loss,
        ).map_err(|e| AssessmentError::PositionSizingFailure { reason: e.to_string() })?;
        
        let potential_trade_loss = position_size.value() * (proposal.entry_price.value() - proposal.stop_loss.value()).abs();
        let trade_risk_percentage = potential_trade_loss / proposal.account_equity.value();
        let portfolio_impact = trade_risk_percentage; // For individual assessment
        
        // Step 2: Create initial assessment
        let mut assessment = RiskAssessment::new(
            proposal.id,
            position_size,
            potential_trade_loss,
            trade_risk_percentage,
            proposal.risk_reward_ratio(),
            portfolio_impact,
        );
        
        // Step 3: Check circuit breaker status
        if self.circuit_breaker_active {
            // Circuit breaker is active - block all trades
            let violation = ProtocolViolation::new(
                self.rule_name().to_string(),
                ViolationSeverity::Critical,
                format!(
                    "Circuit breaker active: {} consecutive losses (limit: {}). Trading halted for risk management.",
                    self.consecutive_losses,
                    self.limits.max_consecutive_losses
                ),
                Decimal::from(self.consecutive_losses),
                Decimal::from(self.limits.max_consecutive_losses),
                "Review trading strategy and manually reset circuit breaker to resume trading".to_string(),
            );
            
            assessment.add_violation(violation);
        } else if self.consecutive_losses > 0 {
            // Not at limit yet, but warn about approaching danger
            let remaining_losses = self.limits.max_consecutive_losses - self.consecutive_losses;
            
            if remaining_losses <= 1 {
                // One loss away from circuit breaker
                let violation = ProtocolViolation::new(
                    self.rule_name().to_string(),
                    ViolationSeverity::Warning,
                    format!(
                        "Warning: {} consecutive losses recorded. Circuit breaker will trigger after {} more loss(es).",
                        self.consecutive_losses,
                        remaining_losses
                    ),
                    Decimal::from(self.consecutive_losses),
                    Decimal::from(self.limits.max_consecutive_losses),
                    format!("Consider reducing position size or reviewing strategy. Total consecutive losses: ${:.2}", self.consecutive_loss_amount),
                );
                
                assessment.add_violation(violation);
            }
        }
        
        // Step 4: Set reasoning
        let time_since_last_loss = self.time_since_last_loss()
            .map(|d| format!("{:.1} minutes ago", d.as_secs_f64() / 60.0))
            .unwrap_or_else(|| "N/A".to_string());
        
        assessment = assessment.with_reasoning(format!(
            "Consecutive Loss Analysis: {} consecutive losses out of {} limit. Last loss: {}. Total consecutive loss amount: ${:.2}. Circuit breaker: {}. Total trades tracked: {}.",
            self.consecutive_losses,
            self.limits.max_consecutive_losses,
            time_since_last_loss,
            self.consecutive_loss_amount,
            if self.circuit_breaker_active { "ACTIVE" } else { "inactive" },
            self.total_trades_tracked
        ));
        
        Ok(assessment)
    }
    
    fn rule_name(&self) -> &str {
        "ConsecutiveLossLimit"
    }
    
    fn description(&self) -> &str {
        "Enforces circuit breaker protection by blocking trades after consecutive losses exceed protocol limits, preventing emotional revenge trading and protecting capital from psychological pitfalls."
    }
}

// ===== CONSECUTIVE LOSS LIMIT TESTS =====

#[cfg(test)]
mod consecutive_loss_tests {
    use super::*;
    use std::time::SystemTime;
    use crate::types::TradeSide;
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};

    fn create_test_proposal(risk_pct: Decimal) -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(100)).unwrap(),
            PricePoint::new(dec!(95)).unwrap(), // $5 risk distance
            Some(PricePoint::new(dec!(110)).unwrap()), // 2:1 reward/risk ratio (10/5 = 2:1)
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(risk_pct).unwrap(),
        ).unwrap()
    }

    #[test]
    fn test_consecutive_loss_limit_rule_creation() {
        let rule = ConsecutiveLossLimitRule::new();
        
        assert_eq!(rule.consecutive_losses(), 0);
        assert_eq!(rule.consecutive_loss_amount(), Decimal::ZERO);
        assert!(!rule.is_circuit_breaker_active());
        assert!(rule.halt_reason().is_none());
        assert_eq!(rule.total_trades_tracked(), 0);
        assert!(rule.time_since_last_loss().is_none());
    }
    
    #[test]
    fn test_consecutive_loss_limit_variants() {
        let conservative_rule = ConsecutiveLossLimitRule::conservative();
        let standard_rule = ConsecutiveLossLimitRule::new();
        let aggressive_rule = ConsecutiveLossLimitRule::aggressive();
        
        // Conservative rule should have lower consecutive loss tolerance
        assert_eq!(conservative_rule.limits.max_consecutive_losses, 2);
        assert_eq!(standard_rule.limits.max_consecutive_losses, 3);
        assert_eq!(aggressive_rule.limits.max_consecutive_losses, 5);
    }
    
    #[test]
    fn test_recording_consecutive_losses() {
        let mut rule = ConsecutiveLossLimitRule::new();
        
        // Record first loss - should not trigger circuit breaker
        let triggered = rule.record_trade_outcome(dec!(-100));
        assert!(!triggered);
        assert_eq!(rule.consecutive_losses(), 1);
        assert_eq!(rule.consecutive_loss_amount(), dec!(100));
        assert!(!rule.is_circuit_breaker_active());
        assert_eq!(rule.total_trades_tracked(), 1);
        
        // Record second loss
        let triggered = rule.record_trade_outcome(dec!(-200));
        assert!(!triggered);
        assert_eq!(rule.consecutive_losses(), 2);
        assert_eq!(rule.consecutive_loss_amount(), dec!(300));
        
        // Record third loss - should trigger circuit breaker (default limit is 3)
        let triggered = rule.record_trade_outcome(dec!(-150));
        assert!(triggered);
        assert_eq!(rule.consecutive_losses(), 3);
        assert_eq!(rule.consecutive_loss_amount(), dec!(450));
        assert!(rule.is_circuit_breaker_active());
        assert!(rule.halt_reason().unwrap().contains("Circuit breaker triggered"));
        assert_eq!(rule.total_trades_tracked(), 3);
    }
    
    #[test]
    fn test_winning_trade_resets_consecutive_losses() {
        let mut rule = ConsecutiveLossLimitRule::new();
        
        // Record some losses
        rule.record_trade_outcome(dec!(-100));
        rule.record_trade_outcome(dec!(-200));
        assert_eq!(rule.consecutive_losses(), 2);
        assert_eq!(rule.consecutive_loss_amount(), dec!(300));
        
        // Record a winning trade - should reset the counter
        let triggered = rule.record_trade_outcome(dec!(150));
        assert!(!triggered);
        assert_eq!(rule.consecutive_losses(), 0);
        assert_eq!(rule.consecutive_loss_amount(), Decimal::ZERO);
        assert!(!rule.is_circuit_breaker_active());
    }
    
    #[test]
    fn test_breakeven_trades_dont_affect_count() {
        let mut rule = ConsecutiveLossLimitRule::new();
        
        // Record some losses
        rule.record_trade_outcome(dec!(-100));
        rule.record_trade_outcome(dec!(-200));
        assert_eq!(rule.consecutive_losses(), 2);
        
        // Record a breakeven trade - should not affect count
        rule.record_trade_outcome(Decimal::ZERO);
        assert_eq!(rule.consecutive_losses(), 2);
        assert_eq!(rule.consecutive_loss_amount(), dec!(300));
        assert_eq!(rule.total_trades_tracked(), 3);
    }
    
    #[test]
    fn test_circuit_breaker_blocks_new_trades() {
        let mut rule = ConsecutiveLossLimitRule::new();
        
        // Trigger circuit breaker
        rule.record_trade_outcome(dec!(-100));
        rule.record_trade_outcome(dec!(-200));
        rule.record_trade_outcome(dec!(-150)); // This triggers circuit breaker
        
        assert!(rule.is_circuit_breaker_active());
        
        // Try to assess a new trade - should be blocked
        let proposal = create_test_proposal(dec!(0.01)); // Small trade
        let assessment = rule.assess(&proposal).unwrap();
        
        assert!(!assessment.is_approved());
        assert!(!assessment.violations.is_empty());
        
        let violation = &assessment.violations[0];
        assert_eq!(violation.rule_name, "ConsecutiveLossLimit");
        assert_eq!(violation.severity, ViolationSeverity::Critical);
        assert!(violation.description.contains("Circuit breaker active"));
        assert!(violation.suggested_action.contains("manually reset circuit breaker"));
    }
    
    #[test]
    fn test_warning_when_approaching_limit() {
        let mut rule = ConsecutiveLossLimitRule::new(); // Default limit is 3
        
        // Record 2 losses - should generate warning (1 away from limit)
        rule.record_trade_outcome(dec!(-100));
        rule.record_trade_outcome(dec!(-200));
        
        let proposal = create_test_proposal(dec!(0.01));
        let assessment = rule.assess(&proposal).unwrap();
        
        assert!(assessment.is_approved()); // Trade allowed but with warning
        assert!(!assessment.violations.is_empty());
        
        let violation = &assessment.violations[0];
        assert_eq!(violation.rule_name, "ConsecutiveLossLimit");
        assert_eq!(violation.severity, ViolationSeverity::Warning);
        assert!(violation.description.contains("Warning"));
        assert!(violation.description.contains("2 consecutive losses"));
        assert!(violation.suggested_action.contains("Total consecutive losses: $300.00"));
    }
    
    #[test]
    fn test_manual_reset_functionality() {
        let mut rule = ConsecutiveLossLimitRule::new();
        
        // Trigger circuit breaker
        rule.record_trade_outcome(dec!(-100));
        rule.record_trade_outcome(dec!(-200));
        rule.record_trade_outcome(dec!(-150));
        
        assert!(rule.is_circuit_breaker_active());
        assert_eq!(rule.consecutive_losses(), 3);
        
        // Manually reset
        rule.reset_consecutive_losses();
        
        assert!(!rule.is_circuit_breaker_active());
        assert_eq!(rule.consecutive_losses(), 0);
        assert_eq!(rule.consecutive_loss_amount(), Decimal::ZERO);
        assert!(rule.halt_reason().is_none());
        
        // New trade should now be allowed
        let proposal = create_test_proposal(dec!(0.01));
        let assessment = rule.assess(&proposal).unwrap();
        assert!(assessment.is_approved());
        assert!(assessment.violations.is_empty());
    }
    
    #[test]
    fn test_conservative_rule_triggers_earlier() {
        let mut conservative_rule = ConsecutiveLossLimitRule::conservative(); // Limit: 2
        let mut standard_rule = ConsecutiveLossLimitRule::new(); // Limit: 3
        
        // Record 2 losses for both
        conservative_rule.record_trade_outcome(dec!(-100));
        conservative_rule.record_trade_outcome(dec!(-200));
        standard_rule.record_trade_outcome(dec!(-100));
        standard_rule.record_trade_outcome(dec!(-200));
        
        // Conservative should trigger circuit breaker at 2 losses
        assert!(conservative_rule.is_circuit_breaker_active());
        assert!(!standard_rule.is_circuit_breaker_active());
        
        // Standard rule should still allow trades with warning
        let proposal = create_test_proposal(dec!(0.01));
        let standard_assessment = standard_rule.assess(&proposal).unwrap();
        assert!(standard_assessment.is_approved()); // With warning
        
        // Conservative rule should block trades
        let conservative_assessment = conservative_rule.assess(&proposal).unwrap();
        assert!(!conservative_assessment.is_approved());
    }
    
    #[test]
    fn test_empty_consecutive_loss_trade_approval() {
        let rule = ConsecutiveLossLimitRule::new();
        let proposal = create_test_proposal(dec!(0.02));
        
        let assessment = rule.assess(&proposal).unwrap();
        assert!(assessment.is_approved());
        assert!(assessment.violations.is_empty());
        
        // Verify reasoning mentions clean state
        let reasoning = assessment.reasoning.unwrap();
        assert!(reasoning.contains("0 consecutive losses"));
        assert!(reasoning.contains("Circuit breaker: inactive"));
        assert!(reasoning.contains("Total trades tracked: 0"));
    }
    
    #[test]
    fn test_time_tracking_functionality() {
        let mut rule = ConsecutiveLossLimitRule::new();
        
        // Initially no time tracking
        assert!(rule.time_since_last_loss().is_none());
        
        // Record a loss
        let before = SystemTime::now();
        rule.record_trade_outcome(dec!(-100));
        let after = SystemTime::now();
        
        // Should have time tracking now
        let time_since = rule.time_since_last_loss().unwrap();
        let expected_min = before.duration_since(rule.last_loss_timestamp()).unwrap_or_default();
        let expected_max = after.duration_since(rule.last_loss_timestamp()).unwrap_or_default();
        
        assert!(time_since >= expected_min);
        assert!(time_since <= expected_max.checked_add(std::time::Duration::from_millis(1)).unwrap_or(expected_max));
    }
    
    #[test]
    fn test_consecutive_loss_reasoning_messages() {
        let mut rule = ConsecutiveLossLimitRule::new();
        rule.record_trade_outcome(dec!(-100));
        rule.record_trade_outcome(dec!(-200));
        
        let proposal = create_test_proposal(dec!(0.01));
        let assessment = rule.assess(&proposal).unwrap();
        
        let reasoning = assessment.reasoning.unwrap();
        assert!(reasoning.contains("2 consecutive losses"));
        assert!(reasoning.contains("out of 3 limit"));
        assert!(reasoning.contains("Total consecutive loss amount: $300.00"));
        assert!(reasoning.contains("Circuit breaker: inactive"));
        assert!(reasoning.contains("Total trades tracked: 2"));
    }
}