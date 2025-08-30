//! Trade proposal types and validation logic
//!
//! This module defines the structure and validation for proposed trades
//! that will be assessed by the risk management system.

use disciplina::{AccountEquity, RiskPercentage, PricePoint};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

/// Represents a proposed trade awaiting risk validation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeProposal {
    /// Unique identifier for this trade proposal
    pub id: Uuid,
    
    /// Trading symbol (e.g., "BTCUSDT", "ETHUSDT")
    pub symbol: String,
    
    /// Trade direction (Long or Short)
    pub side: TradeSide,
    
    /// Intended entry price
    pub entry_price: PricePoint,
    
    /// Stop loss price (required for Van Tharp calculation)
    pub stop_loss: PricePoint,
    
    /// Take profit price (optional)
    pub take_profit: Option<PricePoint>,
    
    /// Current account equity for position sizing
    pub account_equity: AccountEquity,
    
    /// Desired risk percentage (must be within Testudo Protocol limits)
    pub risk_percentage: RiskPercentage,
    
    /// Timestamp when this proposal was created
    pub timestamp: SystemTime,
    
    /// Optional metadata for additional context
    pub metadata: Option<String>,
}

/// Trade direction enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TradeSide {
    /// Long position (buy to open)
    Long,
    /// Short position (sell to open)
    Short,
}

impl TradeProposal {
    /// Create a new trade proposal with validation
    pub fn new(
        symbol: String,
        side: TradeSide,
        entry_price: PricePoint,
        stop_loss: PricePoint,
        take_profit: Option<PricePoint>,
        account_equity: AccountEquity,
        risk_percentage: RiskPercentage,
    ) -> Result<Self, TradeProposalError> {
        // Basic validation
        if symbol.is_empty() {
            return Err(TradeProposalError::InvalidSymbol("Symbol cannot be empty".to_string()));
        }
        
        // Validate stop loss direction
        match side {
            TradeSide::Long => {
                if stop_loss.value() >= entry_price.value() {
                    return Err(TradeProposalError::InvalidStopLoss(
                        "For long trades, stop loss must be below entry price".to_string()
                    ));
                }
            }
            TradeSide::Short => {
                if stop_loss.value() <= entry_price.value() {
                    return Err(TradeProposalError::InvalidStopLoss(
                        "For short trades, stop loss must be above entry price".to_string()
                    ));
                }
            }
        }
        
        // Validate take profit direction if provided
        if let Some(tp) = take_profit {
            match side {
                TradeSide::Long => {
                    if tp.value() <= entry_price.value() {
                        return Err(TradeProposalError::InvalidTakeProfit(
                            "For long trades, take profit must be above entry price".to_string()
                        ));
                    }
                }
                TradeSide::Short => {
                    if tp.value() >= entry_price.value() {
                        return Err(TradeProposalError::InvalidTakeProfit(
                            "For short trades, take profit must be below entry price".to_string()
                        ));
                    }
                }
            }
        }
        
        Ok(TradeProposal {
            id: Uuid::new_v4(),
            symbol,
            side,
            entry_price,
            stop_loss,
            take_profit,
            account_equity,
            risk_percentage,
            timestamp: SystemTime::now(),
            metadata: None,
        })
    }
    
    /// Calculate the risk distance (difference between entry and stop loss)
    pub fn risk_distance(&self) -> Decimal {
        match self.side {
            TradeSide::Long => self.entry_price.value() - self.stop_loss.value(),
            TradeSide::Short => self.stop_loss.value() - self.entry_price.value(),
        }
    }
    
    /// Calculate the reward distance if take profit is set
    pub fn reward_distance(&self) -> Option<Decimal> {
        self.take_profit.map(|tp| match self.side {
            TradeSide::Long => tp.value() - self.entry_price.value(),
            TradeSide::Short => self.entry_price.value() - tp.value(),
        })
    }
    
    /// Calculate the risk/reward ratio if take profit is set
    pub fn risk_reward_ratio(&self) -> Option<Decimal> {
        self.reward_distance().map(|reward| {
            let risk = self.risk_distance();
            if risk.is_zero() {
                Decimal::ZERO
            } else {
                reward / risk
            }
        })
    }
    
    /// Add metadata to the trade proposal
    pub fn with_metadata(mut self, metadata: String) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Errors that can occur when creating or validating trade proposals
#[derive(Debug, thiserror::Error, Clone)]
pub enum TradeProposalError {
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),
    
    #[error("Invalid stop loss: {0}")]
    InvalidStopLoss(String),
    
    #[error("Invalid take profit: {0}")]
    InvalidTakeProfit(String),
    
    #[error("Risk percentage violates protocol limits: {0}")]
    ProtocolViolation(String),
}

impl std::fmt::Display for TradeSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeSide::Long => write!(f, "LONG"),
            TradeSide::Short => write!(f, "SHORT"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    fn create_sample_trade_long() -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(), // 4% risk distance
            Some(PricePoint::new(dec!(54000)).unwrap()), // 8% reward (2:1 ratio)
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(), // 2%
        ).unwrap()
    }
    
    #[test]
    fn test_trade_proposal_creation_long() {
        let proposal = create_sample_trade_long();
        
        assert_eq!(proposal.symbol, "BTCUSDT");
        assert_eq!(proposal.side, TradeSide::Long);
        assert_eq!(proposal.entry_price.value(), dec!(50000));
        assert_eq!(proposal.stop_loss.value(), dec!(48000));
        assert_eq!(proposal.take_profit.unwrap().value(), dec!(54000));
    }
    
    #[test]
    fn test_risk_distance_calculation() {
        let proposal = create_sample_trade_long();
        let risk_distance = proposal.risk_distance();
        
        assert_eq!(risk_distance, dec!(2000)); // 50000 - 48000
    }
    
    #[test]
    fn test_risk_reward_ratio() {
        let proposal = create_sample_trade_long();
        let ratio = proposal.risk_reward_ratio().unwrap();
        
        assert_eq!(ratio, dec!(2)); // 4000 reward / 2000 risk = 2:1
    }
    
    #[test]
    fn test_invalid_long_stop_loss() {
        let result = TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(52000)).unwrap(), // Stop loss above entry (invalid for long)
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        );
        
        assert!(result.is_err());
        match result {
            Err(TradeProposalError::InvalidStopLoss(_)) => (),
            _ => panic!("Expected InvalidStopLoss error"),
        }
    }
    
    #[test]
    fn test_short_trade_proposal() {
        let proposal = TradeProposal::new(
            "ETHUSDT".to_string(),
            TradeSide::Short,
            PricePoint::new(dec!(3000)).unwrap(),
            PricePoint::new(dec!(3150)).unwrap(), // Stop loss above entry (valid for short)
            Some(PricePoint::new(dec!(2700)).unwrap()), // Take profit below entry (valid for short)
            AccountEquity::new(dec!(5000)).unwrap(),
            RiskPercentage::new(dec!(0.03)).unwrap(), // 3%
        ).unwrap();
        
        assert_eq!(proposal.side, TradeSide::Short);
        assert_eq!(proposal.risk_distance(), dec!(150)); // 3150 - 3000
        assert_eq!(proposal.reward_distance().unwrap(), dec!(300)); // 3000 - 2700
        assert_eq!(proposal.risk_reward_ratio().unwrap(), dec!(2)); // 2:1 ratio
    }
}