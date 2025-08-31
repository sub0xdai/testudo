//! Position orientation module

use disciplina::calculator::PositionSizingCalculator;
use disciplina::types::{AccountEquity, RiskPercentage, PricePoint, PositionSize};
use testudo_types::OrderSide as TradeSide;
use crate::types::MarketObservation;
use crate::ooda::{OodaLoop, OodaState};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;
use std::time::SystemTime;

/// Orientator component that analyzes market observations and creates trade proposals
/// 
/// This implements the Orient phase of the OODA loop:
/// - Takes MarketObservation from the Observe phase
/// - Uses Van Tharp position sizing to calculate appropriate position size
/// - Creates TradeProposal for the Decide phase
/// - Transitions OODA loop state from Orienting to Deciding
pub struct PositionOrientator {
    /// Van Tharp position size calculator
    calculator: PositionSizingCalculator,
}
/// Simple trade proposal structure for orientation phase
#[derive(Debug, Clone)]
pub struct TradeProposal {
    /// Trading symbol
    pub symbol: String,
    /// Trade side (Buy/Sell)
    pub side: TradeSide,
    /// Entry price
    pub entry_price: Decimal,
    /// Stop loss price
    pub stop_loss: Decimal,
    /// Take profit price (optional)
    pub take_profit: Option<Decimal>,
    /// Calculated position size
    pub position_size: Decimal,
}

/// Result of the orientation process containing a trade proposal
#[derive(Debug, Clone)]
pub struct TradeOrientation {
    /// The generated trade proposal ready for risk assessment
    pub proposal: TradeProposal,
    /// Time taken to complete orientation (for performance monitoring)
    pub orientation_duration_ms: u64,
    /// Confidence level in the orientation (0.0 to 1.0)
    pub confidence: f64,
}

/// Errors that can occur during the orientation process
#[derive(Debug, thiserror::Error)]
pub enum OrientationError {
    #[error("Position sizing calculation failed: {0}")]
    PositionSizingFailed(String),
    
    #[error("Invalid market observation: {0}")]
    InvalidObservation(String),
    
    #[error("Trade proposal creation failed: {0}")]
    TradeProposalFailed(String),
    
    #[error("OODA loop state transition failed: {0}")]
    StateTransitionFailed(String),
}

impl PositionOrientator {
    /// Create a new PositionOrientator with default Van Tharp calculator
    pub fn new() -> Self {
        Self {
            calculator: PositionSizingCalculator::new(),
        }
    }
    
    /// Create a new PositionOrientator with custom calculator
    pub fn with_calculator(calculator: PositionSizingCalculator) -> Self {
        Self {
            calculator,
        }
    }
    
    /// Orient phase: Analyze market observation and create trade proposal
    /// 
    /// This is the core orientation logic that:
    /// 1. Validates the market observation
    /// 2. Determines trade parameters (entry, stop, side)
    /// 3. Uses Van Tharp calculator to determine position size
    /// 4. Creates a TradeProposal for risk assessment
    /// 5. Transitions OODA loop state to Deciding
    pub async fn orient(
        &self,
        observation: &MarketObservation,
        ooda_loop: &OodaLoop,
        // Trade setup parameters (in real implementation, these would come from strategy logic)
        account_equity: Decimal,
        risk_percentage: Decimal,
        stop_loss_distance_percent: Decimal, // e.g., 2% = 0.02
    ) -> Result<TradeOrientation, OrientationError> {
        let start_time = std::time::Instant::now();
        
        // Validate market observation
        self.validate_observation(observation)?;
        
        // Determine trade setup based on market conditions
        let trade_setup = self.analyze_market_conditions(observation, stop_loss_distance_percent)?;
        
        // Create trade proposal using Van Tharp position sizing
        let proposal = self.create_trade_proposal(
            observation,
            trade_setup.clone(),
            account_equity,
            risk_percentage,
        ).await?;
        
        // Transition OODA loop to Deciding state
        ooda_loop.transition_to(OodaState::Deciding).await
            .map_err(|e| OrientationError::StateTransitionFailed(e.to_string()))?;
        
        let orientation_duration = start_time.elapsed().as_millis() as u64;
        
        // Calculate confidence based on market conditions and data quality
        let confidence = self.calculate_confidence(observation, &trade_setup);
        
        Ok(TradeOrientation {
            proposal,
            orientation_duration_ms: orientation_duration,
            confidence,
        })
    }
    
    /// Validate that the market observation is suitable for orientation
    fn validate_observation(&self, observation: &MarketObservation) -> Result<(), OrientationError> {
        if observation.symbol.is_empty() {
            return Err(OrientationError::InvalidObservation(
                "Symbol cannot be empty".to_string()
            ));
        }
        
        if observation.price <= 0.0 {
            return Err(OrientationError::InvalidObservation(
                "Price must be positive".to_string()
            ));
        }
        
        if observation.volume < 0.0 {
            return Err(OrientationError::InvalidObservation(
                "Volume cannot be negative".to_string()
            ));
        }
        
        // Check data freshness (should be less than 5 seconds old)
        let data_age = observation.timestamp.elapsed();
        if data_age > std::time::Duration::from_secs(5) {
            return Err(OrientationError::InvalidObservation(
                format!("Market data is stale: {} seconds old", data_age.as_secs())
            ));
        }
        
        Ok(())
    }
    
    /// Analyze market conditions and determine trade setup
    fn analyze_market_conditions(
        &self,
        observation: &MarketObservation,
        stop_loss_distance_percent: Decimal,
    ) -> Result<TradeSetup, OrientationError> {
        // For this implementation, we'll create a simple long trade setup
        // In a real system, this would involve complex market analysis
        
        let entry_price = Decimal::from_f64_retain(observation.price)
            .ok_or_else(|| OrientationError::InvalidObservation(
                "Cannot convert price to decimal".to_string()
            ))?;
        
        let stop_distance = entry_price * stop_loss_distance_percent;
        let stop_loss = entry_price - stop_distance;
        
        // Take profit at 2:1 risk/reward ratio
        let profit_distance = stop_distance * dec!(2.0);
        let take_profit = entry_price + profit_distance;
        
        Ok(TradeSetup {
            symbol: observation.symbol.clone(),
            side: TradeSide::Buy,
            entry_price,
            stop_loss,
            take_profit: Some(take_profit),
        })
    }
    
    /// Create a trade proposal using the trade setup and Van Tharp position sizing
    async fn create_trade_proposal(
        &self,
        _observation: &MarketObservation,
        setup: TradeSetup,
        account_equity: Decimal,
        risk_percentage: Decimal,
    ) -> Result<TradeProposal, OrientationError> {
        
        // Convert to disciplina types for position sizing calculation
        let account_equity_typed = AccountEquity::new(account_equity)
            .map_err(|e| OrientationError::PositionSizingFailed(format!("Invalid account equity: {}", e)))?;
        
        let risk_percentage_typed = RiskPercentage::new(risk_percentage)
            .map_err(|e| OrientationError::PositionSizingFailed(format!("Invalid risk percentage: {}", e)))?;
        
        let entry_price_typed = PricePoint::new(setup.entry_price)
            .map_err(|e| OrientationError::PositionSizingFailed(format!("Invalid entry price: {}", e)))?;
        
        let stop_loss_typed = PricePoint::new(setup.stop_loss)
            .map_err(|e| OrientationError::PositionSizingFailed(format!("Invalid stop loss: {}", e)))?;
        
        // Calculate position size using Van Tharp methodology
        let position_size = self.calculator.calculate_position_size(
            account_equity_typed,
            risk_percentage_typed,
            entry_price_typed,
            stop_loss_typed,
        ).map_err(|e| OrientationError::PositionSizingFailed(format!("Position sizing failed: {}", e)))?;
        
        // Create trade proposal
        let proposal = TradeProposal {
            symbol: setup.symbol,
            side: setup.side,
            entry_price: setup.entry_price,
            stop_loss: setup.stop_loss,
            take_profit: setup.take_profit,
            position_size: position_size.value(),
        };
        
        Ok(proposal)
    }
    
    /// Calculate confidence level based on market conditions and data quality
    fn calculate_confidence(&self, observation: &MarketObservation, setup: &TradeSetup) -> f64 {
        let mut confidence:f32 = 1.0;
        
        // Reduce confidence for stale data
        let data_age_seconds = observation.timestamp.elapsed().as_secs_f64();
        if data_age_seconds > 1.0 {
            confidence *= 0.9; // 10% reduction for data older than 1 second
        }
        
        // Reduce confidence for low volume (indicates thin market)
        if observation.volume < 1000.0 {
            confidence *= 0.8; // 20% reduction for low volume
        }
        
        // Reduce confidence for very tight stop losses (high risk of noise)
        let stop_distance = setup.entry_price - setup.stop_loss;
        let stop_percent = stop_distance / setup.entry_price;
        if stop_percent < dec!(0.005) { // Less than 0.5%
            confidence *= 0.7; // 30% reduction for very tight stops
        }
        
        // Ensure confidence stays within bounds
        confidence.max(0.0).min(1.0).into()
    }
}

/// Internal struct to hold trade setup parameters
#[derive(Debug, Clone)]
struct TradeSetup {
    symbol: String,
    side: TradeSide,
    entry_price: Decimal,
    stop_loss: Decimal,
    take_profit: Option<Decimal>,
}

impl Default for PositionOrientator {
    fn default() -> Self {
        Self::new()
    }
}