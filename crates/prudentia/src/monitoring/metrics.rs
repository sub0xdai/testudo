//! Real-time risk metrics calculation and monitoring

use rust_decimal::Decimal;
use std::collections::HashMap;
use std::time::SystemTime;

/// Real-time risk metrics for monitoring
#[derive(Debug, Clone)]
pub struct RealTimeRiskMetrics {
    /// Individual trade risk percentage
    pub individual_trade_risk: Decimal,
    /// Total portfolio risk percentage
    pub total_portfolio_risk: Decimal,
    /// Available risk budget remaining
    pub available_risk_budget: Decimal,
    /// Percentage of maximum risk being utilized
    pub risk_utilization_percentage: Decimal,
    /// Current consecutive loss count
    pub consecutive_loss_count: u32,
    /// Daily profit/loss amount
    pub daily_pnl: Decimal,
    /// Risk of the largest single position
    pub largest_position_risk: Decimal,
    /// Risk correlation factor between positions
    pub correlation_risk_factor: Decimal,
    /// Timestamp when metrics were calculated
    pub calculated_at: SystemTime,
}

impl RealTimeRiskMetrics {
    /// Create new real-time risk metrics
    pub fn new() -> Self {
        Self {
            individual_trade_risk: Decimal::ZERO,
            total_portfolio_risk: Decimal::ZERO,
            available_risk_budget: Decimal::ZERO,
            risk_utilization_percentage: Decimal::ZERO,
            consecutive_loss_count: 0,
            daily_pnl: Decimal::ZERO,
            largest_position_risk: Decimal::ZERO,
            correlation_risk_factor: Decimal::ZERO,
            calculated_at: SystemTime::now(),
        }
    }
}

impl Default for RealTimeRiskMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculates real-time risk metrics
#[derive(Debug, Clone)]
pub struct RiskMetricsCalculator {
    max_portfolio_risk: Decimal,
    max_individual_risk: Decimal,
    account_equity: Decimal,
}

impl RiskMetricsCalculator {
    /// Create new risk metrics calculator
    pub fn new(max_portfolio_risk: Decimal, max_individual_risk: Decimal, account_equity: Decimal) -> Self {
        Self {
            max_portfolio_risk,
            max_individual_risk,
            account_equity,
        }
    }
    
    /// Calculate current real-time metrics
    pub fn calculate_metrics(
        &self,
        current_portfolio_risk: Decimal,
        proposed_trade_risk: Decimal,
        consecutive_losses: u32,
        daily_pnl: Decimal,
        position_risks: &[Decimal],
    ) -> RealTimeRiskMetrics {
        // Calculate available risk budget
        let available_risk_budget = self.max_portfolio_risk - current_portfolio_risk;
        
        // Calculate risk utilization percentage
        let risk_utilization_percentage = if self.max_portfolio_risk > Decimal::ZERO {
            (current_portfolio_risk / self.max_portfolio_risk) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        // Find largest position risk
        let largest_position_risk = position_risks.iter()
            .max()
            .cloned()
            .unwrap_or(Decimal::ZERO);
        
        // Calculate simple correlation risk factor (more sophisticated in production)
        let correlation_risk_factor = if position_risks.len() > 1 {
            // Simple heuristic: if we have many positions, correlation risk increases
            let position_count = Decimal::from(position_risks.len());
            let base_correlation = Decimal::from_str("0.3").unwrap(); // 30% base correlation
            base_correlation * (position_count / Decimal::from(10)).min(Decimal::ONE)
        } else {
            Decimal::ZERO
        };
        
        RealTimeRiskMetrics {
            individual_trade_risk: proposed_trade_risk,
            total_portfolio_risk: current_portfolio_risk,
            available_risk_budget,
            risk_utilization_percentage,
            consecutive_loss_count: consecutive_losses,
            daily_pnl,
            largest_position_risk,
            correlation_risk_factor,
            calculated_at: SystemTime::now(),
        }
    }
    
    /// Update account equity for calculations
    pub fn update_account_equity(&mut self, new_equity: Decimal) {
        self.account_equity = new_equity;
    }
}

impl Default for RiskMetricsCalculator {
    fn default() -> Self {
        Self::new(
            Decimal::from_str("0.10").unwrap(), // 10% max portfolio risk
            Decimal::from_str("0.06").unwrap(), // 6% max individual risk
            Decimal::from(10000), // Default $10,000 account
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    #[test]
    fn test_metrics_calculation() {
        let calculator = RiskMetricsCalculator::new(
            dec!(0.10), // 10% max portfolio
            dec!(0.06), // 6% max individual
            dec!(10000) // $10,000 account
        );
        
        let position_risks = vec![dec!(0.03), dec!(0.02), dec!(0.015)]; // 3%, 2%, 1.5%
        let current_portfolio_risk = dec!(0.065); // 6.5% total
        let proposed_trade_risk = dec!(0.02); // 2%
        
        let metrics = calculator.calculate_metrics(
            current_portfolio_risk,
            proposed_trade_risk,
            1, // 1 consecutive loss
            dec!(-250), // $250 loss today
            &position_risks
        );
        
        assert_eq!(metrics.total_portfolio_risk, dec!(0.065));
        assert_eq!(metrics.available_risk_budget, dec!(0.035)); // 10% - 6.5% = 3.5%
        assert_eq!(metrics.risk_utilization_percentage, dec!(65)); // 65%
        assert_eq!(metrics.largest_position_risk, dec!(0.03)); // 3%
        assert_eq!(metrics.consecutive_loss_count, 1);
        assert_eq!(metrics.daily_pnl, dec!(-250));
    }
    
    #[test]
    fn test_correlation_risk_factor() {
        let calculator = RiskMetricsCalculator::default();
        
        // Single position - no correlation risk
        let single_position = vec![dec!(0.05)];
        let metrics = calculator.calculate_metrics(
            dec!(0.05), dec!(0.02), 0, dec!(0), &single_position
        );
        assert_eq!(metrics.correlation_risk_factor, dec!(0));
        
        // Multiple positions - some correlation risk
        let multiple_positions = vec![dec!(0.02), dec!(0.02), dec!(0.01)];
        let metrics = calculator.calculate_metrics(
            dec!(0.05), dec!(0.02), 0, dec!(0), &multiple_positions
        );
        assert!(metrics.correlation_risk_factor > dec!(0));
    }
    
    #[test]
    fn test_risk_utilization_percentage() {
        let calculator = RiskMetricsCalculator::new(dec!(0.10), dec!(0.06), dec!(10000));
        
        // 50% utilization
        let metrics = calculator.calculate_metrics(
            dec!(0.05), dec!(0.02), 0, dec!(0), &vec![dec!(0.05)]
        );
        assert_eq!(metrics.risk_utilization_percentage, dec!(50));
        
        // 80% utilization
        let metrics = calculator.calculate_metrics(
            dec!(0.08), dec!(0.02), 0, dec!(0), &vec![dec!(0.08)]
        );
        assert_eq!(metrics.risk_utilization_percentage, dec!(80));
    }
}