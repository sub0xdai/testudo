//! Position orientation module

use crate::ooda::{OodaLoop, OodaState};
use crate::types::{MarketObservation, TradeDirection, TradeProposal};
use disciplina::calculator::PositionSizingCalculator;
use disciplina::types::{AccountEquity, PricePoint, RiskPercentage};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use testudo_types::OrderSide;

/// Orientator component that analyzes market observations and creates trade proposals.
#[derive(Default)]
pub struct PositionOrientator {
    calculator: PositionSizingCalculator,
}

/// Result of the orientation process containing a trade proposal.
#[derive(Debug, Clone)]
pub struct TradeOrientation {
    pub proposal: TradeProposal,
    pub orientation_duration_ms: u64,
    pub confidence: f64,
}

/// Errors that can occur during the orientation process.
#[derive(Debug, thiserror::Error)]
pub enum OrientationError {
    #[error("Position sizing calculation failed: {0}")]
    PositionSizingFailed(String),
    #[error("Invalid market observation: {0}")]
    InvalidObservation(String),
    #[error("State transition failed: {0}")]
    StateTransitionFailed(String),
}

impl PositionOrientator {
    pub fn new() -> Self {
        Self {
            calculator: PositionSizingCalculator::new(),
        }
    }

    pub async fn orient(
        &self,
        observation: &MarketObservation,
        ooda_loop: &OodaLoop,
        account_equity: Decimal,
        risk_percentage: Decimal,
        stop_loss_distance_percent: Decimal,
    ) -> Result<TradeOrientation, OrientationError> {
        let start_time = std::time::Instant::now();
        self.validate_observation(observation)?;

        let (entry_price, stop_loss, take_profit) =
            self.analyze_market_conditions(observation, stop_loss_distance_percent)?;

        let position_size = self.calculate_position_size(
            account_equity,
            risk_percentage,
            entry_price,
            stop_loss,
        )?;

        let proposal = TradeProposal {
            symbol: observation.symbol.clone(),
            side: TradeDirection::Long.into(), // FIXME: Hardcoded to Long
            entry_price,
            stop_loss,
            take_profit: Some(take_profit),
            position_size,
        };

        ooda_loop
            .transition_to(OodaState::Deciding)
            .await
            .map_err(|e| OrientationError::StateTransitionFailed(e.to_string()))?;

        let orientation_duration_ms = start_time.elapsed().as_millis() as u64;
        let confidence = self.calculate_confidence(observation, entry_price, stop_loss);

        Ok(TradeOrientation {
            proposal,
            orientation_duration_ms,
            confidence,
        })
    }

    fn validate_observation(&self, observation: &MarketObservation) -> Result<(), OrientationError> {
        if observation.symbol.is_empty() {
            return Err(OrientationError::InvalidObservation("Symbol cannot be empty".to_string()));
        }
        if observation.price <= 0.0 {
            return Err(OrientationError::InvalidObservation("Price must be positive".to_string()));
        }
        if observation.timestamp.elapsed() > std::time::Duration::from_secs(5) {
            return Err(OrientationError::InvalidObservation(format!(
                "Market data is stale: {} seconds old",
                observation.timestamp.elapsed().as_secs()
            )));
        }
        Ok(())
    }

    fn analyze_market_conditions(
        &self,
        observation: &MarketObservation,
        stop_loss_distance_percent: Decimal,
    ) -> Result<(Decimal, Decimal, Decimal), OrientationError> {
        let entry_price = Decimal::from_f64_retain(observation.price)
            .ok_or_else(|| OrientationError::InvalidObservation("Cannot convert price to decimal".to_string()))?;
        let stop_distance = entry_price * stop_loss_distance_percent;
        let stop_loss = entry_price - stop_distance;
        let take_profit = entry_price + (stop_distance * dec!(2.0)); // 2:1 R:R
        Ok((entry_price, stop_loss, take_profit))
    }

    fn calculate_position_size(
        &self,
        account_equity: Decimal,
        risk_percentage: Decimal,
        entry_price: Decimal,
        stop_loss: Decimal,
    ) -> Result<Decimal, OrientationError> {
        let account_equity_typed = AccountEquity::new(account_equity)
            .map_err(|e| OrientationError::PositionSizingFailed(format!("Invalid account equity: {}", e)))?;
        let risk_percentage_typed = RiskPercentage::new(risk_percentage)
            .map_err(|e| OrientationError::PositionSizingFailed(format!("Invalid risk percentage: {}", e)))?;
        let entry_price_typed = PricePoint::new(entry_price)
            .map_err(|e| OrientationError::PositionSizingFailed(format!("Invalid entry price: {}", e)))?;
        let stop_loss_typed = PricePoint::new(stop_loss)
            .map_err(|e| OrientationError::PositionSizingFailed(format!("Invalid stop loss: {}", e)))?;

        self.calculator
            .calculate_position_size(
                account_equity_typed,
                risk_percentage_typed,
                entry_price_typed,
                stop_loss_typed,
            )
            .map(|ps| ps.value())
            .map_err(|e| OrientationError::PositionSizingFailed(format!("Position sizing failed: {}", e)))
    }

    fn calculate_confidence(
        &self,
        observation: &MarketObservation,
        entry_price: Decimal,
        stop_loss: Decimal,
    ) -> f64 {
        let mut confidence: f64 = 1.0;
        if observation.timestamp.elapsed().as_secs_f64() > 1.0 {
            confidence *= 0.9;
        }
        if observation.volume < 1000.0 {
            confidence *= 0.8;
        }
        if (entry_price - stop_loss) / entry_price < dec!(0.005) {
            confidence *= 0.7;
        }
        confidence.max(0.0).min(1.0)
    }
}

impl From<TradeDirection> for OrderSide {
    fn from(direction: TradeDirection) -> Self {
        match direction {
            TradeDirection::Long => OrderSide::Buy,
            TradeDirection::Short => OrderSide::Sell,
        }
    }
}