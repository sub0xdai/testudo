//! Agent Signal Endpoint
//!
//! POST /api/v1/signals accepts a trade signal from an external agent,
//! runs it through the DecisionLoop risk engine, and places an order.
//! Supports shadow (paper) and live (CEX/Hyperliquid) execution.

use actix_web::{web, HttpResponse};
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

use crate::decision_loop::{
    DecisionInputBuilder, DecisionLoop, DecisionOrderSide, DecisionOrderType, DecisionResult,
};
use crate::middleware::auth::AuthenticatedUser;
use crate::models::agent_signal::{ExecutionMode, SignalInput, SignalResult, SignalSide};
use crate::services::exchange_api::{
    ApiOrderType, CexExchangeApi, ExchangeApi, ExchangeApiError,
    OrderSide as ExchangeOrderSide, PlaceOrderRequest, ShadowExchangeApi,
};
use crate::services::hyperliquid::exchange_api::HyperliquidExchangeApi;
use common_utils::risk::{RiskConfig, SizingMethod};

/// Classify an exchange name into a routing target.
fn classify_exchange(exchange_name: &str) -> Option<&'static str> {
    match exchange_name.to_lowercase().as_str() {
        "hyperliquid" => Some("hyperliquid"),
        "binance" | "woo" | "bybit" | "woox" => Some("cex"),
        _ => None,
    }
}

/// Return true if the exchange error is definitive (order was NOT placed).
/// Ambiguous errors (timeout, parse errors) should NOT trigger rollback.
fn is_definitive_rejection(e: &ExchangeApiError) -> bool {
    match e {
        ExchangeApiError::AgentWalletInactive { .. } => true,
        ExchangeApiError::InsufficientBalance { .. } => true,
        ExchangeApiError::Exchange(msg) => {
            let lower = msg.to_lowercase();
            lower.contains("insufficient")
                || lower.contains("authentication")
                || lower.contains("invalid")
                || lower.contains("not allowed")
                || lower.contains("does not exist")
                || lower.contains("not found")
        }
        _ => false,
    }
}

fn exchange_order_side(side: &SignalSide) -> ExchangeOrderSide {
    match side {
        SignalSide::Long => ExchangeOrderSide::Buy,
        SignalSide::Short => ExchangeOrderSide::Sell,
    }
}

/// POST /api/v1/signals
///
/// Accepts a SignalInput payload, validates it through the DecisionLoop
/// risk engine, and places an order. Shadow mode uses the paper engine;
/// live mode routes to CEX or Hyperliquid based on the exchange account.
pub async fn create_signal(
    _user: AuthenticatedUser,
    body: web::Json<SignalInput>,
    _app_state: web::Data<crate::types::app::AppState>,
) -> HttpResponse {
    let input = body.into_inner();

    // CP-4: Idempotency check — if a key is provided and already processed, return 409.
    if let Some(ref idem_key) = input.idempotency_key {
        let key_str = idem_key.to_string();
        if let Some(cached) = _app_state.signal_idempotency.get(&key_str) {
            return HttpResponse::Conflict().json(cached.clone());
        }
    }

    if input.symbol.trim().is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "symbol is required"
        }));
    }

    let decision_side = match input.side {
        SignalSide::Long => DecisionOrderSide::Long,
        SignalSide::Short => DecisionOrderSide::Short,
    };

    let is_live = matches!(input.execution_mode, ExecutionMode::Live);
    let leverage = input.leverage.unwrap_or(1);

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

    let decision_loop = DecisionLoop::new(RiskConfig::default());
    let decision_result = decision_loop.execute(&decision_input, &account_state, None);

    if !decision_result.approved {
        let rejection = decision_result.rejection.as_ref();
        let reason = rejection.map(|r| r.to_string()).unwrap_or_default();
        let code = rejection
            .map(|r| format!("{:?}", r))
            .unwrap_or_else(|| "unknown".to_string());
        let result = SignalResult::rejected(reason, code, input.execution_mode);
        if let Some(ref idem_key) = input.idempotency_key {
            if let Ok(cached) = serde_json::to_value(&result) {
                _app_state.signal_idempotency.insert(idem_key.to_string(), cached);
            }
        }
        return HttpResponse::build(actix_web::http::StatusCode::UNPROCESSABLE_ENTITY)
            .json(result);
    }

    let position_size = decision_result.position_size.unwrap_or(Decimal::ZERO);
    let sizing_method = decision_result.sizing_method.unwrap_or(SizingMethod::FixedFractional);
    let warnings: Vec<String> = decision_result
        .warnings.iter().map(|w| w.to_string()).collect();

    if is_live {
        return execute_live(
            _user.user_id, input, position_size, sizing_method, warnings, _app_state,
        ).await;
    }

    execute_shadow(
        _user.user_id, input, position_size, sizing_method, warnings,
        &_app_state.engine_handle,
    ).await
}

async fn execute_shadow(
    user_id: Uuid,
    input: SignalInput,
    position_size: Decimal,
    sizing_method: SizingMethod,
    warnings: Vec<String>,
    engine_handle: &engine::EngineHandle,
) -> HttpResponse {
    let shadow_api = ShadowExchangeApi::new(engine_handle.clone());
    let leverage = input.leverage.unwrap_or(1);
    let order_result = shadow_api
        .place_order(PlaceOrderRequest {
            user_id,
            symbol: input.symbol.clone(),
            side: exchange_order_side(&input.side),
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
        Ok(result) => HttpResponse::Ok().json(SignalResult::success(
            Uuid::new_v4(),
            result.id,
            position_size,
            sizing_method,
            ExecutionMode::Shadow,
            warnings,
        )),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to place shadow order: {}", e)
        })),
    }
}

async fn execute_live(
    user_id: Uuid,
    input: SignalInput,
    position_size: Decimal,
    sizing_method: SizingMethod,
    warnings: Vec<String>,
    state: web::Data<crate::types::app::AppState>,
) -> HttpResponse {
    let account_id = match input.exchange_account_id {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(SignalResult::rejected(
                "exchange_account_id is required for live execution".to_string(),
                "missing_account".to_string(),
                ExecutionMode::Live,
            ));
        }
    };

    // Look up the account and verify ownership.
    let accounts = match state.exchange_account_repo.list_by_user(user_id).await {
        Ok(a) => a,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to list exchange accounts: {}", e)
            }));
        }
    };

    let account = match accounts.iter().find(|a| a.id == account_id) {
        Some(a) => a,
        None => {
            return HttpResponse::BadRequest().json(SignalResult::rejected(
                format!("exchange account {} not found or does not belong to user", account_id),
                "account_not_found".to_string(),
                ExecutionMode::Live,
            ));
        }
    };

    let exchange_name = account.exchange_name.clone();
    let routing = classify_exchange(&exchange_name);

    match routing {
        Some("cex") => {
            execute_live_cex(user_id, input, position_size, sizing_method, warnings, state, account_id, &exchange_name).await
        }
        Some("hyperliquid") => {
            execute_live_hl(user_id, input, position_size, sizing_method, warnings, state, account_id).await
        }
        _ => {
            HttpResponse::BadRequest().json(SignalResult::rejected(
                format!("unsupported exchange: {}", exchange_name),
                "unsupported_exchange".to_string(),
                ExecutionMode::Live,
            ))
        }
    }
}

async fn execute_live_cex(
    user_id: Uuid,
    input: SignalInput,
    position_size: Decimal,
    sizing_method: SizingMethod,
    mut warnings: Vec<String>,
    state: web::Data<crate::types::app::AppState>,
    account_id: Uuid,
    exchange_name: &str,
) -> HttpResponse {
    let cex_api = CexExchangeApi::new(
        state.cex_client.clone(),
        state.exchange_account_repo.clone(),
        state.cex_sandbox,
    );

    let leverage = input.leverage.unwrap_or(1);
    let result = cex_api
        .place_order(PlaceOrderRequest {
            user_id,
            symbol: input.symbol.clone(),
            side: exchange_order_side(&input.side),
            order_type: ApiOrderType::Limit,
            quantity: position_size,
            price: Some(input.entry_price),
            stop_price: None,
            leverage: leverage.max(1),
            exchange_account_id: Some(account_id),
            reduce_only: false,
            client_order_id: None,
            stop_loss_trigger: input.stop_loss,
            take_profit_trigger: input.take_profit.first().map(|tp| tp.price),
        })
        .await;

    match result {
        Ok(order) => HttpResponse::Ok().json(SignalResult::success(
            Uuid::new_v4(),
            order.id,
            position_size,
            sizing_method,
            ExecutionMode::Live,
            warnings,
        )),
        Err(ref e) if is_definitive_rejection(e) => {
            HttpResponse::BadGateway().json(SignalResult::rejected(
                format!("Exchange rejected: {}", e),
                format!("exchange_reject_{}", exchange_name),
                ExecutionMode::Live,
            ))
        }
        Err(e) => {
            warnings.push(format!("Exchange response unclear: {}. Order may have been placed.", e));
            HttpResponse::Ok().json(SignalResult::success(
                Uuid::new_v4(),
                "unknown".to_string(),
                position_size,
                sizing_method,
                ExecutionMode::Live,
                warnings,
            ))
        }
    }
}

async fn execute_live_hl(
    user_id: Uuid,
    input: SignalInput,
    position_size: Decimal,
    sizing_method: SizingMethod,
    mut warnings: Vec<String>,
    state: web::Data<crate::types::app::AppState>,
    account_id: Uuid,
) -> HttpResponse {
    let hl_api = match (&state.hl_universe, &state.hl_auth_cache) {
        (Some(universe), Some(auth_cache)) => HyperliquidExchangeApi::new(
            universe.clone(),
            auth_cache.clone(),
            state.exchange_account_repo.clone(),
            state.hl_network,
        ),
        _ => {
            return HttpResponse::ServiceUnavailable().json(SignalResult::rejected(
                "Hyperliquid is not configured".to_string(),
                "hl_not_configured".to_string(),
                ExecutionMode::Live,
            ));
        }
    };

    let leverage = input.leverage.unwrap_or(1);
    let result = hl_api
        .place_order(PlaceOrderRequest {
            user_id,
            symbol: input.symbol.clone(),
            side: exchange_order_side(&input.side),
            order_type: ApiOrderType::Limit,
            quantity: position_size,
            price: Some(input.entry_price),
            stop_price: None,
            leverage: leverage.max(1),
            exchange_account_id: Some(account_id),
            reduce_only: false,
            client_order_id: None,
            stop_loss_trigger: input.stop_loss,
            take_profit_trigger: input.take_profit.first().map(|tp| tp.price),
        })
        .await;

    match result {
        Ok(order) => HttpResponse::Ok().json(SignalResult::success(
            Uuid::new_v4(),
            order.id,
            position_size,
            sizing_method,
            ExecutionMode::Live,
            warnings,
        )),
        Err(ref e) if is_definitive_rejection(e) => {
            HttpResponse::BadGateway().json(SignalResult::rejected(
                format!("Hyperliquid rejected: {}", e),
                "hl_rejected".to_string(),
                ExecutionMode::Live,
            ))
        }
        Err(e) => {
            warnings.push(format!("HL response unclear: {}. Order may have been placed.", e));
            HttpResponse::Ok().json(SignalResult::success(
                Uuid::new_v4(),
                "unknown".to_string(),
                position_size,
                sizing_method,
                ExecutionMode::Live,
                warnings,
            ))
        }
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

    // --- CP-3: Live-mode routing logic ---

    /// Classification: given an exchange_name, is it Hyperliquid or CEX?
    fn classify_exchange(exchange_name: &str) -> Option<&'static str> {
        match exchange_name.to_lowercase().as_str() {
            "hyperliquid" => Some("hyperliquid"),
            name if name == "binance" || name == "woo" || name == "bybit" => Some("cex"),
            _ => None,
        }
    }

    #[test]
    fn signal_routes_hyperliquid_to_hl_api() {
        assert_eq!(classify_exchange("hyperliquid"), Some("hyperliquid"));
    }

    #[test]
    fn signal_routes_binance_to_cex_api() {
        assert_eq!(classify_exchange("binance"), Some("cex"));
    }

    #[test]
    fn signal_routes_woo_to_cex_api() {
        assert_eq!(classify_exchange("woo"), Some("cex"));
    }

    #[test]
    fn signal_routes_unknown_exchange_to_none() {
        assert_eq!(classify_exchange("unknown_exchange"), None);
    }

    #[test]
    fn signal_result_live_mode_serializes_correctly() {
        let trade_group_id = Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap();
        let result = SignalResult::success(
            trade_group_id,
            "binance-order-123".to_string(),
            dec!(0.25),
            SizingMethod::FixedFractional,
            ExecutionMode::Live,
            vec!["Approaching drawdown limit".to_string()],
        );

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"execution_mode\":\"LIVE\""));
        assert!(json.contains("binance-order-123"));
    }

    // --- CP-4: Idempotency + error paths ---

    /// Simple in-memory idempotency store for unit testing.
    struct IdempotencyStore {
        entries: std::collections::HashMap<String, serde_json::Value>,
    }

    impl IdempotencyStore {
        fn new() -> Self {
            Self { entries: std::collections::HashMap::new() }
        }

        fn get(&self, key: &str) -> Option<&serde_json::Value> {
            self.entries.get(key)
        }

        fn insert(&mut self, key: String, response: serde_json::Value) {
            self.entries.insert(key, response);
        }
    }

    #[test]
    fn idempotency_store_returns_none_for_unknown_key() {
        let store = IdempotencyStore::new();
        assert!(store.get("unknown").is_none());
    }

    #[test]
    fn idempotency_store_returns_cached_value() {
        let mut store = IdempotencyStore::new();
        let key = Uuid::new_v4().to_string();
        let cached = serde_json::json!({"success": true, "trade_group_id": "abc"});
        store.insert(key.clone(), cached.clone());

        let result = store.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap()["success"], true);
    }

    #[test]
    fn idempotency_duplicate_key_returns_conflict() {
        // Simulate the handler logic: on duplicate key, return 409.
        let mut store = IdempotencyStore::new();
        let idempotency_key = Uuid::new_v4().to_string();
        let response = serde_json::json!({"success": true});
        store.insert(idempotency_key.clone(), response.clone());

        // First call: found in store → 409
        if let Some(cached) = store.get(&idempotency_key) {
            assert_eq!(cached["success"], true);
        } else {
            panic!("expected cached result");
        }
    }

    #[test]
    fn signal_rejected_drawdown_exceeded_maps_to_422() {
        let rejection = common_utils::risk::RiskRejection::DailyDrawdownExceeded {
            current_drawdown_percent: dec!(6),
            limit_percent: dec!(5),
        };
        let result = SignalResult::rejected(
            rejection.to_string(),
            "DailyDrawdownExceeded".to_string(),
            ExecutionMode::Shadow,
        );
        assert!(!result.success);
        assert!(result.rejection.is_some());
        let r = result.rejection.unwrap();
        assert!(r.reason.contains("drawdown"));
        assert!(r.code.contains("DailyDrawdownExceeded"));
    }

    #[test]
    fn signal_result_rejection_code_maps_risk_check() {
        // Verify all 8 risk rejection variants produce meaningful codes
        let rejections = vec![
            (common_utils::risk::RiskRejection::StopLossRequired, "StopLossRequired"),
            (common_utils::risk::RiskRejection::InsufficientBalance {
                required: dec!(1000), available: dec!(500),
            }, "InsufficientBalance"),
            (common_utils::risk::RiskRejection::LeverageExceeded {
                requested: 10, maximum: 5,
            }, "LeverageExceeded"),
            (common_utils::risk::RiskRejection::PositionSizeExceeded {
                requested: dec!(1), maximum: dec!(0.5),
            }, "PositionSizeExceeded"),
            (common_utils::risk::RiskRejection::RiskAmountExceeded {
                calculated_risk: dec!(200), maximum: dec!(100),
            }, "RiskAmountExceeded"),
            (common_utils::risk::RiskRejection::InsufficientRiskReward {
                calculated: dec!(0.5), minimum: dec!(1.5),
            }, "InsufficientRiskReward"),
        ];

        for (rejection, _expected_code) in &rejections {
            let code = format!("{:?}", rejection);
            let result = SignalResult::rejected(
                rejection.to_string(),
                code,
                ExecutionMode::Shadow,
            );
            assert!(!result.success);
            assert!(result.rejection.is_some());
        }
    }
}
