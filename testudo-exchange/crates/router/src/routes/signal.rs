//! Agent Signal Endpoint
//!
//! POST /api/v1/signals accepts a trade signal from an external agent,
//! runs it through the DecisionLoop risk engine, and places a shadow order
//! on approval. This is the programmatic counterpart to the browser
//! extension's POST /api/v1/trades.

use actix_web::{web, HttpResponse};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::decision_loop::{DecisionInputBuilder, DecisionLoop, DecisionOrderSide, DecisionOrderType};
use crate::middleware::auth::AuthenticatedUser;
use crate::decision_loop::DecisionResult;
use crate::models::agent_signal::{ExecutionMode, SignalInput, SignalResult, SignalSide};
use crate::services::exchange_api::{
    ApiOrderType, ExchangeApi, OrderSide as ExchangeOrderSide, PlaceOrderRequest, ShadowExchangeApi,
};
use common_utils::risk::{RiskConfig, SizingMethod};

/// POST /api/v1/signals
///
/// Accepts a SignalInput payload, validates it through the DecisionLoop
/// risk engine, and places a shadow order on approval. Returns a
/// SignalResult with trade_group_id on success or rejection details on
/// failure.
pub async fn create_signal(
    _user: AuthenticatedUser,
    body: web::Json<SignalInput>,
    _app_state: web::Data<crate::types::app::AppState>,
) -> HttpResponse {
    let input = body.into_inner();

    // Validate: symbol must be non-empty
    if input.symbol.trim().is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "symbol is required"
        }));
    }

    // Convert SignalSide → DecisionOrderSide
    let decision_side = match input.side {
        SignalSide::Long => DecisionOrderSide::Long,
        SignalSide::Short => DecisionOrderSide::Short,
    };

    // CP-1: shadow mode only — reject live for now
    if matches!(input.execution_mode, ExecutionMode::Live) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "live execution mode not yet implemented"
        }));
    }

    let leverage = input.leverage.unwrap_or(1);

    // Build DecisionInput
    let decision_input = match DecisionInputBuilder::new()
        .user_id(_user.user_id)
        .symbol(&input.symbol)
        .side(decision_side)
        .order_type(DecisionOrderType::Limit)
        .entry_price(input.entry_price)
        .stop_loss(input.stop_loss.unwrap_or(Decimal::ZERO))
        .leverage(leverage)
        .build()
    {
        Ok(di) => di,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("invalid signal: {}", e)
            }));
        }
    };

    // Get account state for risk validation.
    // CP-1: derive from shadow engine balances.
    let account_state = {
        let balances = _app_state.engine_handle.get_balances(_user.user_id).await;
        let usdt_balance = balances
            .iter()
            .find(|b| b.asset == "USDT")
            .map(|b| b.available + b.reserved)
            .unwrap_or(Decimal::from(10000));

        let positions = _app_state.engine_handle.get_positions(_user.user_id).await;

        common_utils::risk::AccountState {
            balance: usdt_balance,
            open_position_count: positions.len() as u32,
            daily_pnl: Decimal::ZERO,
            starting_balance: Decimal::from(10000),
        }
    };

    // Run DecisionLoop
    let decision_loop = DecisionLoop::new(RiskConfig::default());
    let decision_result = decision_loop.execute(&decision_input, &account_state, None);

    if !decision_result.approved {
        let rejection = decision_result.rejection.as_ref();
        let reason = rejection.map(|r| r.to_string()).unwrap_or_default();
        let code = rejection
            .map(|r| format!("{:?}", r))
            .unwrap_or_else(|| "unknown".to_string());

        return HttpResponse::build(actix_web::http::StatusCode::UNPROCESSABLE_ENTITY)
            .json(SignalResult::rejected(
                reason,
                code,
                ExecutionMode::Shadow,
            ));
    }

    let position_size = decision_result.position_size.unwrap_or(Decimal::ZERO);
    let sizing_method = decision_result.sizing_method.unwrap_or(SizingMethod::FixedFractional);
    let warnings: Vec<String> = decision_result
        .warnings
        .iter()
        .map(|w| w.to_string())
        .collect();

    // CP-1: Place shadow order
    let shadow_api = ShadowExchangeApi::new(_app_state.engine_handle.clone());
    let order_result = shadow_api
        .place_order(PlaceOrderRequest {
            user_id: _user.user_id,
            symbol: input.symbol.clone(),
            side: match input.side {
                SignalSide::Long => ExchangeOrderSide::Buy,
                SignalSide::Short => ExchangeOrderSide::Sell,
            },
            order_type: ApiOrderType::Limit,
            quantity: position_size,
            price: Some(input.entry_price),
            stop_price: None,
            leverage: leverage.max(1),
            exchange_account_id: input.exchange_account_id,
            reduce_only: false,
            client_order_id: None,
            stop_loss_trigger: None,
            take_profit_trigger: None,
        })
        .await;

    match order_result {
        Ok(result) => {
            let trade_group_id = Uuid::new_v4();
            HttpResponse::Ok().json(SignalResult::success(
                trade_group_id,
                result.id,
                position_size,
                sizing_method,
                ExecutionMode::Shadow,
                warnings,
            ))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to place shadow order: {}", e)
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::agent_signal::{
        ExecutionMode as SignalExecMode, SignalInput, SignalSide as SignalSideEnum,
    };
    use engine::{EngineActor, EngineHandle, ShadowEngine};
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    /// Create a test EngineHandle with an initialized user.
    async fn test_handle(user_id: Uuid) -> EngineHandle {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        let _ = handle.init_user(user_id).await;
        handle
    }

    /// Build a minimal valid shadow signal payload.
    fn valid_signal(_user_id: Uuid) -> SignalInput {
        SignalInput {
            symbol: "BTC_USDT".to_string(),
            side: SignalSideEnum::Long,
            entry_price: dec!(50000),
            stop_loss: Some(dec!(49000)),
            take_profit: vec![],
            exchange_account_id: None,
            execution_mode: SignalExecMode::Shadow,
            reasoning: None,
            source: None,
            confidence: None,
            idempotency_key: None,
            leverage: Some(1),
            management: None,
        }
    }

    /// Test helper: run the core decision pipeline and return the result.
    async fn run_decision(
        user_id: Uuid,
        signal: &SignalInput,
    ) -> (DecisionResult, common_utils::risk::AccountState) {
        let decision_side = match signal.side {
            SignalSideEnum::Long => DecisionOrderSide::Long,
            SignalSideEnum::Short => DecisionOrderSide::Short,
        };

        let leverage = signal.leverage.unwrap_or(1);

        let decision_input = DecisionInputBuilder::new()
            .user_id(user_id)
            .symbol(&signal.symbol)
            .side(decision_side)
            .order_type(DecisionOrderType::Limit)
            .entry_price(signal.entry_price)
            .stop_loss(signal.stop_loss.unwrap_or(Decimal::ZERO))
            .leverage(leverage)
            .build()
            .expect("valid DecisionInput");

        let account_state = common_utils::risk::AccountState {
            balance: dec!(10000),
            open_position_count: 0,
            daily_pnl: Decimal::ZERO,
            starting_balance: dec!(10000),
        };

        let decision_loop = DecisionLoop::new(RiskConfig::default());
        let result = decision_loop.execute(&decision_input, &account_state, None);

        (result, account_state)
    }

    // --- Test: valid shadow signal is approved ---

    #[tokio::test]
    async fn signal_valid_shadow_signal_is_approved() {
        let user_id = Uuid::new_v4();
        let signal = valid_signal(user_id);
        let (result, _account) = run_decision(user_id, &signal).await;

        assert!(result.approved, "valid signal should be approved");
        assert!(result.position_size.is_some(), "should have a position size");
        assert!(result.sizing_method.is_some(), "should have a sizing method");
        assert!(
            result.rejection.is_none(),
            "should not be rejected: {:?}",
            result.rejection
        );
    }

    // --- Test: rejection when stop loss is missing ---

    #[tokio::test]
    async fn signal_rejected_when_stop_loss_missing() {
        let user_id = Uuid::new_v4();
        let config = RiskConfig::new().with_require_stop_loss(true);
        // Build DecisionInput without stop_loss
        let decision_input = DecisionInputBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(DecisionOrderSide::Long)
            .order_type(DecisionOrderType::Limit)
            .entry_price(dec!(50000))
            // NO stop_loss call → None
            .leverage(1)
            .build()
            .expect("valid DecisionInput without stop");

        let account_state = common_utils::risk::AccountState {
            balance: dec!(10000),
            open_position_count: 0,
            daily_pnl: Decimal::ZERO,
            starting_balance: dec!(10000),
        };

        let decision_loop = DecisionLoop::new(config);
        let result = decision_loop.execute(&decision_input, &account_state, None);

        assert!(!result.approved, "should be rejected without stop loss");
        assert!(
            matches!(
                result.rejection,
                Some(common_utils::risk::RiskRejection::StopLossRequired)
            ),
            "rejection should be StopLossRequired, got: {:?}",
            result.rejection
        );
    }

    // --- Test: rejection when max positions reached ---

    #[tokio::test]
    async fn signal_rejected_when_max_positions_reached() {
        let user_id = Uuid::new_v4();
        let config = RiskConfig::new()
            .with_max_open_positions(2)
            .with_require_stop_loss(false);

        let decision_input = DecisionInputBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(DecisionOrderSide::Long)
            .order_type(DecisionOrderType::Limit)
            .entry_price(dec!(50000))
            .stop_loss(dec!(49000))
            .leverage(1)
            .build()
            .expect("valid DecisionInput");

        let account_state = common_utils::risk::AccountState {
            balance: dec!(10000),
            open_position_count: 2, // at max
            daily_pnl: Decimal::ZERO,
            starting_balance: dec!(10000),
        };

        let decision_loop = DecisionLoop::new(config);
        let result = decision_loop.execute(&decision_input, &account_state, None);

        assert!(!result.approved, "should be rejected at max positions");
        assert!(
            matches!(
                result.rejection,
                Some(common_utils::risk::RiskRejection::MaxPositionsReached { .. })
            ),
            "rejection should be MaxPositionsReached, got: {:?}",
            result.rejection
        );
    }

    // --- Test: execution mode conversion ---

    #[test]
    fn signal_side_to_decision_side() {
        let long = match SignalSideEnum::Long {
            SignalSideEnum::Long => DecisionOrderSide::Long,
            SignalSideEnum::Short => DecisionOrderSide::Short,
        };
        assert_eq!(long, DecisionOrderSide::Long);

        let short = match SignalSideEnum::Short {
            SignalSideEnum::Long => DecisionOrderSide::Long,
            SignalSideEnum::Short => DecisionOrderSide::Short,
        };
        assert_eq!(short, DecisionOrderSide::Short);
    }

    // --- Test: SignalResult serialization ---

    #[test]
    fn signal_result_success_serializes_correctly() {
        use common_utils::risk::SizingMethod;
        let trade_group_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let result = SignalResult::success(
            trade_group_id,
            "shadow-order-1".to_string(),
            dec!(0.5),
            SizingMethod::FixedFractional,
            ExecutionMode::Shadow,
            vec![],
        );

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("550e8400-e29b-41d4-a716-446655440000"));
        assert!(json.contains("shadow-order-1"));
        assert!(json.contains("fixed_fractional"));
    }

    #[test]
    fn signal_result_rejected_serializes_correctly() {
        let result = SignalResult::rejected(
            "Stop loss required".to_string(),
            "StopLossRequired".to_string(),
            ExecutionMode::Shadow,
        );

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("Stop loss required"));
        assert!(json.contains("StopLossRequired"));
        assert!(!json.contains("trade_group_id"), "trade_group_id should be absent on rejection");
    }
}
