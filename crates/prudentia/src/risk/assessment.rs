//! Trade risk assessment utilities
//!
//! This module provides utility functions and types for performing
//! detailed risk analysis on individual trades.

use crate::types::{TradeProposal, TradeSide};
use disciplina::PositionSize;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Detailed risk assessment for an individual trade
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeRiskAssessment {
    /// Calculated position size
    pub position_size: PositionSize,
    
    /// Dollar amount at risk if stop loss is hit
    pub risk_amount: Decimal,
    
    /// Percentage of account equity at risk
    pub risk_percentage: Decimal,
    
    /// Distance between entry and stop loss (in price units)
    pub risk_distance: Decimal,
    
    /// Distance between entry and take profit (if set)
    pub reward_distance: Option<Decimal>,
    
    /// Reward-to-risk ratio (if take profit is set)
    pub reward_risk_ratio: Option<Decimal>,
    
    /// Maximum potential loss if stop is hit
    pub max_loss: Decimal,
    
    /// Maximum potential profit if take profit is hit
    pub max_profit: Option<Decimal>,
    
    /// Break-even price (entry price for simplicity)
    pub break_even_price: Decimal,
    
    /// Percentage move needed to hit stop loss
    pub stop_loss_percentage: Decimal,
    
    /// Percentage move needed to hit take profit (if set)
    pub take_profit_percentage: Option<Decimal>,
    
    /// Position value at entry (position size × entry price)
    pub position_value: Decimal,
    
    /// Leverage ratio (position value / risk amount)
    pub effective_leverage: Decimal,
}

impl TradeRiskAssessment {
    /// Create a new trade risk assessment
    pub fn new(proposal: &TradeProposal, position_size: PositionSize) -> Self {
        let risk_distance = proposal.risk_distance();
        let reward_distance = proposal.reward_distance();
        let reward_risk_ratio = proposal.risk_reward_ratio();
        
        let risk_amount = position_size.value() * risk_distance;
        let position_value = position_size.value() * proposal.entry_price.value();
        
        let max_loss = risk_amount;
        let max_profit = reward_distance.map(|rd| position_size.value() * rd);
        
        let stop_loss_percentage = Self::calculate_percentage_move(
            proposal.entry_price.value(),
            proposal.stop_loss.value(),
            proposal.side,
        );
        
        let take_profit_percentage = proposal.take_profit.map(|tp| {
            Self::calculate_percentage_move(
                proposal.entry_price.value(),
                tp.value(),
                proposal.side,
            )
        });
        
        let effective_leverage = if risk_amount.is_zero() {
            Decimal::ZERO
        } else {
            position_value / risk_amount
        };
        
        Self {
            position_size,
            risk_amount,
            risk_percentage: proposal.risk_percentage.value(),
            risk_distance,
            reward_distance,
            reward_risk_ratio,
            max_loss,
            max_profit,
            break_even_price: proposal.entry_price.value(),
            stop_loss_percentage,
            take_profit_percentage,
            position_value,
            effective_leverage,
        }
    }
    
    /// Calculate percentage move from entry to target price
    fn calculate_percentage_move(entry: Decimal, target: Decimal, side: TradeSide) -> Decimal {
        let abs_move = (target - entry).abs();
        let percentage = abs_move / entry;
        
        match side {
            TradeSide::Long => {
                if target < entry {
                    -percentage // Negative for stop loss
                } else {
                    percentage // Positive for take profit
                }
            }
            TradeSide::Short => {
                if target > entry {
                    -percentage // Negative for stop loss
                } else {
                    percentage // Positive for take profit
                }
            }
        }
    }
    
    /// Calculate the expected value of the trade (requires win probability)
    pub fn expected_value(&self, win_probability: Decimal) -> Option<Decimal> {
        if let Some(max_profit) = self.max_profit {
            let loss_probability = Decimal::ONE - win_probability;
            let expected_profit = win_probability * max_profit;
            let expected_loss = loss_probability * self.max_loss;
            Some(expected_profit - expected_loss)
        } else {
            None
        }
    }
    
    /// Calculate the Kelly Criterion optimal position size percentage
    /// Formula: f* = (bp - q) / b
    /// where b = reward/risk ratio, p = win probability, q = loss probability
    pub fn kelly_criterion_size(&self, win_probability: Decimal) -> Option<Decimal> {
        if let Some(ratio) = self.reward_risk_ratio {
            if ratio > Decimal::ZERO {
                let loss_probability = Decimal::ONE - win_probability;
                let kelly_fraction = (ratio * win_probability - loss_probability) / ratio;
                // Cap at reasonable maximum (25% for safety)
                Some(kelly_fraction.min(dec!(0.25)).max(Decimal::ZERO))
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Check if this is a high-conviction trade (good risk/reward ratio)
    pub fn is_high_conviction(&self) -> bool {
        self.reward_risk_ratio.map_or(false, |ratio| ratio >= dec!(3.0))
    }
    
    /// Check if this trade has asymmetric risk (limited downside, unlimited upside)
    pub fn has_asymmetric_risk(&self) -> bool {
        // For our purposes, any trade with take profit set has "limited" upside
        // True asymmetric risk would require options or other derivatives
        self.reward_distance.is_none() && self.risk_distance > Decimal::ZERO
    }
    
    /// Calculate position size as percentage of total account value
    pub fn position_size_percentage(&self, account_value: Decimal) -> Decimal {
        if account_value.is_zero() {
            Decimal::ZERO
        } else {
            self.position_value / account_value
        }
    }
    
    /// Get a risk rating from 1 (low risk) to 10 (high risk)
    pub fn risk_rating(&self) -> u8 {
        let mut score = 0;
        
        // Risk percentage scoring (40% of total score)
        if self.risk_percentage > dec!(0.05) {
            score += 4; // > 5% is high risk
        } else if self.risk_percentage > dec!(0.03) {
            score += 3; // 3-5% is medium-high risk
        } else if self.risk_percentage > dec!(0.02) {
            score += 2; // 2-3% is medium risk
        } else if self.risk_percentage > dec!(0.01) {
            score += 1; // 1-2% is low-medium risk
        }
        // <= 1% gets 0 points (very low risk)
        
        // Reward/risk ratio scoring (30% of total score)
        if let Some(ratio) = self.reward_risk_ratio {
            if ratio < dec!(1.5) {
                score += 3; // Poor ratio
            } else if ratio < dec!(2.0) {
                score += 2; // Below ideal
            } else if ratio < dec!(3.0) {
                score += 1; // Good ratio
            }
            // >= 3.0 gets 0 points (excellent ratio)
        } else {
            score += 2; // No target = unknown reward potential
        }
        
        // Stop loss percentage scoring (30% of total score)
        let stop_pct = self.stop_loss_percentage.abs();
        if stop_pct > dec!(0.10) {
            score += 3; // > 10% move to stop is high risk
        } else if stop_pct > dec!(0.05) {
            score += 2; // 5-10% is medium risk
        } else if stop_pct > dec!(0.02) {
            score += 1; // 2-5% is low-medium risk
        }
        // <= 2% gets 0 points (tight stop, low risk)
        
        // Convert score to 1-10 rating
        match score {
            0..=1 => 1,  // Very low risk
            2..=3 => 2,  // Low risk
            4..=5 => 3,  // Low-medium risk
            6..=7 => 4,  // Medium risk
            8..=9 => 5,  // Medium risk
            10..=11 => 6, // Medium-high risk
            12..=13 => 7, // High risk
            14..=15 => 8, // High risk
            16..=17 => 9, // Very high risk
            _ => 10,      // Extreme risk
        }
    }
}

/// Risk analysis utilities
pub struct RiskAnalyzer;

impl RiskAnalyzer {
    /// Analyze multiple trade proposals and rank them by risk-adjusted return potential
    pub fn rank_trades_by_risk_adjusted_return(
        assessments: &[TradeRiskAssessment],
        win_probability: Decimal,
    ) -> Vec<(usize, Decimal)> {
        let mut ranked: Vec<(usize, Decimal)> = assessments
            .iter()
            .enumerate()
            .filter_map(|(idx, assessment)| {
                assessment.expected_value(win_probability).map(|ev| {
                    // Risk-adjusted return = expected value / risk amount
                    let risk_adjusted = if assessment.risk_amount.is_zero() {
                        Decimal::ZERO
                    } else {
                        ev / assessment.risk_amount
                    };
                    (idx, risk_adjusted)
                })
            })
            .collect();
        
        // Sort by risk-adjusted return (highest first)
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        ranked
    }
    
    /// Calculate portfolio-level risk if all given trades were executed
    pub fn calculate_portfolio_risk(
        assessments: &[TradeRiskAssessment],
        account_value: Decimal,
        correlation_factor: Decimal, // 0.0 = uncorrelated, 1.0 = fully correlated
    ) -> Decimal {
        if assessments.is_empty() || account_value.is_zero() {
            return Decimal::ZERO;
        }
        
        let total_risk: Decimal = assessments.iter().map(|a| a.risk_amount).sum::<Decimal>();
        
        // Adjust for correlation (simplified model)
        let adjusted_risk = if correlation_factor.is_zero() {
            // For uncorrelated positions, use square root rule
            let variance: Decimal = assessments
                .iter()
                .map(|a| a.risk_amount * a.risk_amount)
                .sum::<Decimal>();
            // Simple approximation: use the total risk for now (avoiding sqrt dependency)
            total_risk
        } else {
            // Linear interpolation between uncorrelated and fully correlated
            let uncorrelated_risk = {
                let variance: Decimal = assessments
                    .iter()
                    .map(|a| a.risk_amount * a.risk_amount)
                    .sum::<Decimal>();
                // Simple approximation: use the total risk for now (avoiding sqrt dependency)
            total_risk
            };
            
            uncorrelated_risk * (Decimal::ONE - correlation_factor) + total_risk * correlation_factor
        };
        
        adjusted_risk / account_value
    }
    
    /// Generate risk report summary
    pub fn generate_risk_report(assessments: &[TradeRiskAssessment]) -> RiskReport {
        if assessments.is_empty() {
            return RiskReport::default();
        }
        
        let total_assessments = assessments.len();
        let total_risk_amount: Decimal = assessments.iter().map(|a| a.risk_amount).sum::<Decimal>();
        let avg_risk_percentage: Decimal = assessments.iter().map(|a| a.risk_percentage).sum::<Decimal>() / Decimal::from(total_assessments);
        
        let avg_reward_risk_ratio = {
            let ratios: Vec<Decimal> = assessments.iter().filter_map(|a| a.reward_risk_ratio).collect();
            if ratios.is_empty() {
                None
            } else {
                Some(ratios.iter().sum::<Decimal>() / Decimal::from(ratios.len()))
            }
        };
        
        let risk_ratings: Vec<u8> = assessments.iter().map(|a| a.risk_rating()).collect();
        let avg_risk_rating = risk_ratings.iter().map(|&r| Decimal::from(r)).sum::<Decimal>() 
            / Decimal::from(total_assessments);
        
        let high_risk_count = risk_ratings.iter().filter(|&&r| r >= 7).count();
        let low_risk_count = risk_ratings.iter().filter(|&&r| r <= 3).count();
        
        RiskReport {
            total_assessments,
            total_risk_amount,
            avg_risk_percentage,
            avg_reward_risk_ratio,
            avg_risk_rating,
            high_risk_count,
            low_risk_count,
        }
    }
}

/// Summary report of risk analysis
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RiskReport {
    pub total_assessments: usize,
    pub total_risk_amount: Decimal,
    pub avg_risk_percentage: Decimal,
    pub avg_reward_risk_ratio: Option<Decimal>,
    pub avg_risk_rating: Decimal,
    pub high_risk_count: usize,
    pub low_risk_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TradeSide;
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};
    use rust_decimal_macros::dec;
    
    fn create_test_proposal() -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(), // 2000 risk distance (4%)
            Some(PricePoint::new(dec!(54000)).unwrap()), // 4000 reward distance (8%, 2:1 ratio)
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(), // 2% risk
        ).unwrap()
    }
    
    #[test]
    fn test_trade_risk_assessment_creation() {
        let proposal = create_test_proposal();
        let position_size = PositionSize::new(dec!(100)).unwrap(); // 100 units
        
        let assessment = TradeRiskAssessment::new(&proposal, position_size);
        
        assert_eq!(assessment.position_size.value(), dec!(100));
        assert_eq!(assessment.risk_distance, dec!(2000)); // 50000 - 48000
        assert_eq!(assessment.reward_distance.unwrap(), dec!(4000)); // 54000 - 50000
        assert_eq!(assessment.reward_risk_ratio.unwrap(), dec!(2)); // 4000 / 2000
        assert_eq!(assessment.risk_amount, dec!(200000)); // 100 * 2000
        assert_eq!(assessment.max_loss, dec!(200000));
        assert_eq!(assessment.max_profit.unwrap(), dec!(400000)); // 100 * 4000
        assert_eq!(assessment.position_value, dec!(5000000)); // 100 * 50000
    }
    
    #[test]
    fn test_percentage_move_calculations() {
        let proposal = create_test_proposal();
        let position_size = PositionSize::new(dec!(100)).unwrap();
        
        let assessment = TradeRiskAssessment::new(&proposal, position_size);
        
        // Stop loss is 4% below entry for long position
        assert_eq!(assessment.stop_loss_percentage, dec!(-0.04)); // Negative for loss
        
        // Take profit is 8% above entry for long position  
        assert_eq!(assessment.take_profit_percentage.unwrap(), dec!(0.08)); // Positive for profit
    }
    
    #[test]
    fn test_expected_value_calculation() {
        let proposal = create_test_proposal();
        let position_size = PositionSize::new(dec!(100)).unwrap();
        let assessment = TradeRiskAssessment::new(&proposal, position_size);
        
        // With 60% win probability
        let expected_value = assessment.expected_value(dec!(0.6)).unwrap();
        
        // Expected value = (0.6 * 400000) - (0.4 * 200000) = 240000 - 80000 = 160000
        assert_eq!(expected_value, dec!(160000));
    }
    
    #[test]
    fn test_kelly_criterion_calculation() {
        let proposal = create_test_proposal();
        let position_size = PositionSize::new(dec!(100)).unwrap();
        let assessment = TradeRiskAssessment::new(&proposal, position_size);
        
        // With 60% win probability and 2:1 ratio
        let kelly_size = assessment.kelly_criterion_size(dec!(0.6)).unwrap();
        
        // Kelly = (2 * 0.6 - 0.4) / 2 = (1.2 - 0.4) / 2 = 0.4
        // But capped at 0.25 for safety
        assert_eq!(kelly_size, dec!(0.25));
    }
    
    #[test]
    fn test_risk_rating() {
        let proposal = create_test_proposal();
        let position_size = PositionSize::new(dec!(100)).unwrap();
        let assessment = TradeRiskAssessment::new(&proposal, position_size);
        
        let rating = assessment.risk_rating();
        
        // With 2% risk, 2:1 ratio, and 4% stop, should be low-medium risk
        assert!(rating >= 1 && rating <= 5); // Should be in reasonable range
    }
    
    #[test]
    fn test_high_conviction_trade() {
        let high_conviction_proposal = TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(), // 2000 risk
            Some(PricePoint::new(dec!(56000)).unwrap()), // 6000 reward = 3:1 ratio
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap();
        
        let position_size = PositionSize::new(dec!(100)).unwrap();
        let assessment = TradeRiskAssessment::new(&high_conviction_proposal, position_size);
        
        assert!(assessment.is_high_conviction());
        assert_eq!(assessment.reward_risk_ratio.unwrap(), dec!(3));
    }
    
    #[test]
    fn test_risk_analyzer_portfolio_risk() {
        let proposal1 = create_test_proposal();
        let proposal2 = create_test_proposal();
        
        let position_size = PositionSize::new(dec!(100)).unwrap();
        let assessment1 = TradeRiskAssessment::new(&proposal1, position_size);
        let assessment2 = TradeRiskAssessment::new(&proposal2, position_size);
        
        let assessments = vec![assessment1, assessment2];
        let account_value = dec!(1000000); // $1M account
        
        // Test uncorrelated positions
        let uncorrelated_risk = RiskAnalyzer::calculate_portfolio_risk(&assessments, account_value, dec!(0.0));
        
        // Test fully correlated positions
        let correlated_risk = RiskAnalyzer::calculate_portfolio_risk(&assessments, account_value, dec!(1.0));
        
        // Correlated risk should be higher than uncorrelated
        assert!(correlated_risk > uncorrelated_risk);
        
        // Both should be reasonable percentages
        assert!(uncorrelated_risk > Decimal::ZERO && uncorrelated_risk < dec!(1.0));
        assert!(correlated_risk > Decimal::ZERO && correlated_risk < dec!(1.0));
    }
    
    #[test]
    fn test_risk_report_generation() {
        let proposal = create_test_proposal();
        let position_size = PositionSize::new(dec!(100)).unwrap();
        let assessment = TradeRiskAssessment::new(&proposal, position_size);
        
        let assessments = vec![assessment];
        let report = RiskAnalyzer::generate_risk_report(&assessments);
        
        assert_eq!(report.total_assessments, 1);
        assert!(report.total_risk_amount > Decimal::ZERO);
        assert_eq!(report.avg_risk_percentage, dec!(0.02));
        assert_eq!(report.avg_reward_risk_ratio.unwrap(), dec!(2));
    }
}