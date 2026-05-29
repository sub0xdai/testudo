//! Decision Loop
//!
//! Orchestrates the order execution flow:
//! 1. Validate input parameters
//! 2. Create shadow order for simulation
//! 3. Run risk checks
//! 4. Approve or reject the order
//! 5. Execute on exchange (if live mode)
//!
//! This module bridges the router layer with the risk management system
//! and shadow engine for paper trading.

// @anchor exchange:router:decision_loop
// @tags api

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common_utils::adapters::{BinanceOrderResult, ExecutionMode};
use common_utils::risk::{
    AccountState, MarketData, OrderRequest, OrderSide as RiskOrderSide, RiskConfig, RiskRejection,
    RiskService, RiskWarning, SizingMethod,
};

/// Input for the decision loop
#[derive(Debug, Clone)]
pub struct DecisionInput {
    /// User ID placing the order
    pub user_id: Uuid,
    /// Trading symbol (e.g., "BTC_USDC")
    pub symbol: String,
    /// Order side
    pub side: DecisionOrderSide,
    /// Order type
    pub order_type: DecisionOrderType,
    /// Requested quantity (optional, will be calculated if not provided)
    pub quantity: Option<Decimal>,
    /// Entry price (required for limit orders, current price for market)
    pub entry_price: Decimal,
    /// Stop loss price (required if config.require_stop_loss)
    pub stop_loss_price: Option<Decimal>,
    /// Take profit price
    pub take_profit_price: Option<Decimal>,
    /// Requested leverage (default 1)
    pub leverage: u8,
    /// Execution mode (Shadow for paper trading, Live for real execution)
    pub execution_mode: ExecutionMode,
}

/// Order side for decision input
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DecisionOrderSide {
    Buy,
    Sell,
    Long,
    Short,
}

impl From<DecisionOrderSide> for RiskOrderSide {
    fn from(side: DecisionOrderSide) -> Self {
        match side {
            DecisionOrderSide::Buy | DecisionOrderSide::Long => RiskOrderSide::Long,
            DecisionOrderSide::Sell | DecisionOrderSide::Short => RiskOrderSide::Short,
        }
    }
}

/// Order type for decision input
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DecisionOrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,
    StopLimit,
}

/// Result of the decision loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResult {
    /// Whether the order is approved
    pub approved: bool,
    /// Final position size (if approved)
    pub position_size: Option<Decimal>,
    /// Which sizing method was used
    pub sizing_method: Option<SizingMethod>,
    /// Rejection reason (if not approved)
    pub rejection: Option<RiskRejection>,
    /// Warnings (even if approved)
    pub warnings: Vec<RiskWarning>,
    /// Shadow order ID (for paper trading simulation)
    pub shadow_order_id: Option<Uuid>,
    /// Execution mode used
    pub execution_mode: ExecutionMode,
    /// Binance order result (if live mode and executed)
    pub binance_order: Option<BinanceOrderResult>,
    /// Execution error (if live mode and failed)
    pub execution_error: Option<String>,
}

impl DecisionResult {
    /// Create an approved result (defaults to Shadow mode)
    pub fn approved(size: Decimal, method: SizingMethod) -> Self {
        Self {
            approved: true,
            position_size: Some(size),
            sizing_method: Some(method),
            rejection: None,
            warnings: Vec::new(),
            shadow_order_id: Some(Uuid::new_v4()),
            execution_mode: ExecutionMode::Shadow,
            binance_order: None,
            execution_error: None,
        }
    }

    /// Create a rejected result
    pub fn rejected(reason: RiskRejection) -> Self {
        Self {
            approved: false,
            position_size: None,
            sizing_method: None,
            rejection: Some(reason),
            warnings: Vec::new(),
            shadow_order_id: None,
            execution_mode: ExecutionMode::Shadow,
            binance_order: None,
            execution_error: None,
        }
    }

    /// Add warnings to the result
    pub fn with_warnings(mut self, warnings: Vec<RiskWarning>) -> Self {
        self.warnings = warnings;
        self
    }

    /// Set execution mode
    pub fn with_execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    /// Set Binance order result
    pub fn with_binance_order(mut self, order: BinanceOrderResult) -> Self {
        self.binance_order = Some(order);
        self
    }

    /// Set execution error
    pub fn with_execution_error(mut self, error: String) -> Self {
        self.execution_error = Some(error);
        self
    }
}

/// The Decision Loop executor
pub struct DecisionLoop {
    risk_service: RiskService,
}

impl DecisionLoop {
    /// Create a new decision loop with the given risk configuration
    pub fn new(config: RiskConfig) -> Self {
        Self {
            risk_service: RiskService::new(config),
        }
    }

    /// Create a decision loop with default risk configuration
    pub fn with_defaults() -> Self {
        Self::new(RiskConfig::default())
    }

    /// Execute the decision loop for an order
    ///
    /// # Arguments
    /// * `input` - The order input to process
    /// * `account` - Current account state
    /// * `market_data` - Optional market data for volatility-based sizing
    ///
    /// # Returns
    /// DecisionResult with approval/rejection and calculated size
    pub fn execute(
        &self,
        input: &DecisionInput,
        account: &AccountState,
        market_data: Option<&MarketData>,
    ) -> DecisionResult {
        // Step 1: Convert input to OrderRequest for risk service
        let order_request = OrderRequest {
            symbol: input.symbol.clone(),
            side: input.side.into(),
            user_size: input.quantity,
            entry_price: input.entry_price,
            stop_loss_price: input.stop_loss_price,
            take_profit_price: input.take_profit_price,
            leverage: input.leverage.max(1),
        };

        // Step 2: Run risk validation
        let risk_result = self.risk_service.validate(
            &order_request,
            account,
            market_data,
            None, // Trading stats not available yet
        );

        // Step 3: Convert risk result to decision result
        if risk_result.approved {
            let size = risk_result.calculated_size.unwrap_or(dec!(0));
            let method = risk_result
                .sizing_method_used
                .unwrap_or(SizingMethod::FixedFractional);

            DecisionResult::approved(size, method).with_warnings(risk_result.warnings)
        } else {
            let rejection = risk_result
                .rejection_reason
                .unwrap_or(RiskRejection::StopLossRequired);

            DecisionResult::rejected(rejection).with_warnings(risk_result.warnings)
        }
    }

    /// Quick validation check without full sizing calculation
    pub fn quick_validate(&self, input: &DecisionInput, account: &AccountState) -> bool {
        let result = self.execute(input, account, None);
        result.approved
    }
}

/// Builder for creating DecisionInput from order parameters
pub struct DecisionInputBuilder {
    user_id: Option<Uuid>,
    symbol: Option<String>,
    side: Option<DecisionOrderSide>,
    order_type: DecisionOrderType,
    quantity: Option<Decimal>,
    entry_price: Option<Decimal>,
    stop_loss_price: Option<Decimal>,
    take_profit_price: Option<Decimal>,
    leverage: u8,
    execution_mode: ExecutionMode,
}

impl DecisionInputBuilder {
    pub fn new() -> Self {
        Self {
            user_id: None,
            symbol: None,
            side: None,
            order_type: DecisionOrderType::Market,
            quantity: None,
            entry_price: None,
            stop_loss_price: None,
            take_profit_price: None,
            leverage: 1,
            execution_mode: ExecutionMode::Shadow,
        }
    }

    pub fn user_id(mut self, id: Uuid) -> Self {
        self.user_id = Some(id);
        self
    }

    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    pub fn side(mut self, side: DecisionOrderSide) -> Self {
        self.side = Some(side);
        self
    }

    pub fn order_type(mut self, order_type: DecisionOrderType) -> Self {
        self.order_type = order_type;
        self
    }

    pub fn quantity(mut self, qty: Decimal) -> Self {
        self.quantity = Some(qty);
        self
    }

    pub fn entry_price(mut self, price: Decimal) -> Self {
        self.entry_price = Some(price);
        self
    }

    pub fn stop_loss(mut self, price: Decimal) -> Self {
        self.stop_loss_price = Some(price);
        self
    }

    pub fn take_profit(mut self, price: Decimal) -> Self {
        self.take_profit_price = Some(price);
        self
    }

    pub fn leverage(mut self, lev: u8) -> Self {
        self.leverage = lev;
        self
    }

    pub fn execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    pub fn live_mode(mut self) -> Self {
        self.execution_mode = ExecutionMode::Live;
        self
    }

    pub fn shadow_mode(mut self) -> Self {
        self.execution_mode = ExecutionMode::Shadow;
        self
    }

    pub fn build(self) -> Result<DecisionInput, DecisionInputError> {
        let user_id = self.user_id.ok_or(DecisionInputError::MissingUserId)?;
        let symbol = self.symbol.ok_or(DecisionInputError::MissingSymbol)?;
        let side = self.side.ok_or(DecisionInputError::MissingSide)?;
        let entry_price = self
            .entry_price
            .ok_or(DecisionInputError::MissingEntryPrice)?;

        Ok(DecisionInput {
            user_id,
            symbol,
            side,
            order_type: self.order_type,
            quantity: self.quantity,
            entry_price,
            stop_loss_price: self.stop_loss_price,
            take_profit_price: self.take_profit_price,
            leverage: self.leverage.max(1),
            execution_mode: self.execution_mode,
        })
    }
}

impl Default for DecisionInputBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur when building decision input
#[derive(Debug, thiserror::Error)]
pub enum DecisionInputError {
    #[error("Missing user ID")]
    MissingUserId,

    #[error("Missing symbol")]
    MissingSymbol,

    #[error("Missing order side")]
    MissingSide,

    #[error("Missing entry price")]
    MissingEntryPrice,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_account() -> AccountState {
        AccountState {
            balance: dec!(10000),
            open_position_count: 0,
            daily_pnl: dec!(0),
            starting_balance: dec!(10000),
        }
    }

    fn default_input() -> DecisionInput {
        DecisionInputBuilder::new()
            .user_id(Uuid::new_v4())
            .symbol("BTC_USDC")
            .side(DecisionOrderSide::Long)
            .entry_price(dec!(50000))
            .stop_loss(dec!(49000))
            .take_profit(dec!(52000))
            .build()
            .unwrap()
    }

    #[test]
    fn test_decision_loop_approves_valid_order() {
        let loop_ = DecisionLoop::with_defaults();
        let input = default_input();
        let account = default_account();

        let result = loop_.execute(&input, &account, None);

        assert!(result.approved);
        assert!(result.position_size.is_some());
        assert!(result.sizing_method.is_some());
        assert!(result.shadow_order_id.is_some());
    }

    #[test]
    fn test_decision_loop_rejects_no_stop_loss() {
        let config = RiskConfig::new().with_require_stop_loss(true);
        let loop_ = DecisionLoop::new(config);

        let input = DecisionInputBuilder::new()
            .user_id(Uuid::new_v4())
            .symbol("BTC_USDC")
            .side(DecisionOrderSide::Long)
            .entry_price(dec!(50000))
            // No stop loss
            .build()
            .unwrap();

        let result = loop_.execute(&input, &default_account(), None);

        assert!(!result.approved);
        assert!(matches!(
            result.rejection,
            Some(RiskRejection::StopLossRequired)
        ));
    }

    #[test]
    fn test_decision_loop_rejects_max_positions() {
        let config = RiskConfig::new()
            .with_max_open_positions(3)
            .with_require_stop_loss(false);
        let loop_ = DecisionLoop::new(config);

        let mut account = default_account();
        account.open_position_count = 3;

        let input = default_input();
        let result = loop_.execute(&input, &account, None);

        assert!(!result.approved);
        assert!(matches!(
            result.rejection,
            Some(RiskRejection::MaxPositionsReached {
                current: 3,
                maximum: 3
            })
        ));
    }

    #[test]
    fn test_decision_loop_returns_warnings() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_daily_max_drawdown(dec!(5));
        let loop_ = DecisionLoop::new(config);

        let mut account = default_account();
        account.daily_pnl = dec!(-450); // 4.5% drawdown - approaching limit

        let result = loop_.execute(&default_input(), &account, None);

        assert!(result.approved);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_quick_validate() {
        let loop_ = DecisionLoop::with_defaults();

        assert!(loop_.quick_validate(&default_input(), &default_account()));

        // Invalid order - max positions reached
        let config = RiskConfig::new()
            .with_max_open_positions(0)
            .with_require_stop_loss(false);
        let loop_strict = DecisionLoop::new(config);

        assert!(!loop_strict.quick_validate(&default_input(), &default_account()));
    }

    #[test]
    fn test_decision_input_builder() {
        let user_id = Uuid::new_v4();

        let input = DecisionInputBuilder::new()
            .user_id(user_id)
            .symbol("ETH_USDC")
            .side(DecisionOrderSide::Short)
            .order_type(DecisionOrderType::Limit)
            .quantity(dec!(1.5))
            .entry_price(dec!(3000))
            .stop_loss(dec!(3100))
            .take_profit(dec!(2800))
            .leverage(5)
            .build()
            .unwrap();

        assert_eq!(input.user_id, user_id);
        assert_eq!(input.symbol, "ETH_USDC");
        assert_eq!(input.side, DecisionOrderSide::Short);
        assert_eq!(input.quantity, Some(dec!(1.5)));
        assert_eq!(input.leverage, 5);
    }

    #[test]
    fn test_decision_input_builder_errors() {
        // Missing user_id
        let result = DecisionInputBuilder::new()
            .symbol("BTC_USDC")
            .side(DecisionOrderSide::Long)
            .entry_price(dec!(50000))
            .build();
        assert!(matches!(result, Err(DecisionInputError::MissingUserId)));

        // Missing symbol
        let result = DecisionInputBuilder::new()
            .user_id(Uuid::new_v4())
            .side(DecisionOrderSide::Long)
            .entry_price(dec!(50000))
            .build();
        assert!(matches!(result, Err(DecisionInputError::MissingSymbol)));
    }

    #[test]
    fn test_order_side_conversion() {
        assert_eq!(
            RiskOrderSide::from(DecisionOrderSide::Buy),
            RiskOrderSide::Long
        );
        assert_eq!(
            RiskOrderSide::from(DecisionOrderSide::Long),
            RiskOrderSide::Long
        );
        assert_eq!(
            RiskOrderSide::from(DecisionOrderSide::Sell),
            RiskOrderSide::Short
        );
        assert_eq!(
            RiskOrderSide::from(DecisionOrderSide::Short),
            RiskOrderSide::Short
        );
    }

    // ==================== Execution Mode Tests ====================

    #[test]
    fn test_shadow_mode_default() {
        let input = default_input();
        assert_eq!(input.execution_mode, ExecutionMode::Shadow);
    }

    #[test]
    fn test_live_mode_builder() {
        let input = DecisionInputBuilder::new()
            .user_id(Uuid::new_v4())
            .symbol("BTC_USDC")
            .side(DecisionOrderSide::Long)
            .entry_price(dec!(50000))
            .stop_loss(dec!(49000))
            .live_mode()
            .build()
            .unwrap();

        assert_eq!(input.execution_mode, ExecutionMode::Live);
    }

    #[test]
    fn test_shadow_mode_builder() {
        let input = DecisionInputBuilder::new()
            .user_id(Uuid::new_v4())
            .symbol("BTC_USDC")
            .side(DecisionOrderSide::Long)
            .entry_price(dec!(50000))
            .stop_loss(dec!(49000))
            .shadow_mode()
            .build()
            .unwrap();

        assert_eq!(input.execution_mode, ExecutionMode::Shadow);
    }

    #[test]
    fn test_decision_result_execution_mode_shadow_by_default() {
        let loop_ = DecisionLoop::with_defaults();
        let input = default_input();
        let account = default_account();

        let result = loop_.execute(&input, &account, None);

        assert!(result.approved);
        assert_eq!(result.execution_mode, ExecutionMode::Shadow);
        // Shadow mode should not have binance order
        assert!(result.binance_order.is_none());
        assert!(result.execution_error.is_none());
    }

    #[test]
    fn test_decision_result_with_execution_mode() {
        let result = DecisionResult::approved(dec!(0.1), SizingMethod::FixedFractional)
            .with_execution_mode(ExecutionMode::Live);

        assert_eq!(result.execution_mode, ExecutionMode::Live);
    }

    #[test]
    fn test_execution_mode_explicit_setter() {
        let input = DecisionInputBuilder::new()
            .user_id(Uuid::new_v4())
            .symbol("BTC_USDC")
            .side(DecisionOrderSide::Long)
            .entry_price(dec!(50000))
            .stop_loss(dec!(49000))
            .execution_mode(ExecutionMode::Live)
            .build()
            .unwrap();

        assert_eq!(input.execution_mode, ExecutionMode::Live);
    }
}
