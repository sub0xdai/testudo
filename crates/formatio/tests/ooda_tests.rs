//! Tests for the OODA Loop implementation

use formatio::ooda::{OodaLoop, OodaState, OodaController};
use formatio::observer::{MarketObserver, ObservationResult};
use formatio::exchange::{MockExchange, MarketData};
use rust_decimal_macros::dec;
use std::sync::Arc;
use std::time::{SystemTime, Duration};

#[tokio::test]
async fn test_ooda_loop_initialization() {
    let ooda = OodaLoop::new();
    let state = ooda.get_state().await;
    assert_eq!(state, OodaState::Idle, "OODA loop should start in Idle state");
}

#[tokio::test]
async fn test_valid_state_transitions() {
    let ooda = OodaLoop::new();
    
    // Idle -> Observing
    assert!(ooda.transition_to(OodaState::Observing).await.is_ok());
    assert_eq!(ooda.get_state().await, OodaState::Observing);
    
    // Observing -> Orienting
    assert!(ooda.transition_to(OodaState::Orienting).await.is_ok());
    assert_eq!(ooda.get_state().await, OodaState::Orienting);
    
    // Orienting -> Deciding
    assert!(ooda.transition_to(OodaState::Deciding).await.is_ok());
    assert_eq!(ooda.get_state().await, OodaState::Deciding);
    
    // Deciding -> Acting
    assert!(ooda.transition_to(OodaState::Acting).await.is_ok());
    assert_eq!(ooda.get_state().await, OodaState::Acting);
    
    // Acting -> Completed
    assert!(ooda.transition_to(OodaState::Completed).await.is_ok());
    assert_eq!(ooda.get_state().await, OodaState::Completed);
    
    // Completed -> Idle (reset)
    assert!(ooda.transition_to(OodaState::Idle).await.is_ok());
    assert_eq!(ooda.get_state().await, OodaState::Idle);
}

#[tokio::test]
async fn test_invalid_state_transitions() {
    let ooda = OodaLoop::new();
    
    // Cannot go directly from Idle to Acting
    let result = ooda.transition_to(OodaState::Acting).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid state transition"));
    
    // Start proper sequence
    assert!(ooda.transition_to(OodaState::Observing).await.is_ok());
    
    // Cannot skip from Observing to Acting
    let result = ooda.transition_to(OodaState::Acting).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_failure_state_transitions() {
    let ooda = OodaLoop::new();
    
    // Can fail from Observing
    assert!(ooda.transition_to(OodaState::Observing).await.is_ok());
    assert!(ooda.transition_to(OodaState::Failed("Market data error".to_string())).await.is_ok());
    
    // Can reset from Failed to Idle
    assert!(ooda.transition_to(OodaState::Idle).await.is_ok());
    
    // Can fail from any active state
    assert!(ooda.transition_to(OodaState::Observing).await.is_ok());
    assert!(ooda.transition_to(OodaState::Orienting).await.is_ok());
    assert!(ooda.transition_to(OodaState::Failed("Calculation error".to_string())).await.is_ok());
}

#[tokio::test]
async fn test_decision_to_completion_without_action() {
    let ooda = OodaLoop::new();
    
    // Progress to Deciding
    assert!(ooda.transition_to(OodaState::Observing).await.is_ok());
    assert!(ooda.transition_to(OodaState::Orienting).await.is_ok());
    assert!(ooda.transition_to(OodaState::Deciding).await.is_ok());
    
    // Can complete directly from Deciding (no action needed)
    assert!(ooda.transition_to(OodaState::Completed).await.is_ok());
    assert_eq!(ooda.get_state().await, OodaState::Completed);
}

#[tokio::test]
async fn test_ooda_controller_registration() {
    let controller = OodaController::new();
    
    // Initially no loops
    assert_eq!(controller.active_loop_count().await, 0);
    
    // Register loops
    let loop1 = Arc::new(OodaLoop::new());
    let loop2 = Arc::new(OodaLoop::new());
    
    controller.register_loop(loop1).await;
    assert_eq!(controller.active_loop_count().await, 1);
    
    controller.register_loop(loop2).await;
    assert_eq!(controller.active_loop_count().await, 2);
}

#[tokio::test]
async fn test_concurrent_state_access() {
    let ooda = Arc::new(OodaLoop::new());
    
    // Test concurrent reads
    let ooda1 = ooda.clone();
    let ooda2 = ooda.clone();
    
    let handle1 = tokio::spawn(async move {
        ooda1.get_state().await
    });
    
    let handle2 = tokio::spawn(async move {
        ooda2.get_state().await
    });
    
    let state1 = handle1.await.unwrap();
    let state2 = handle2.await.unwrap();
    
    assert_eq!(state1, state2);
    assert_eq!(state1, OodaState::Idle);
}

// Observer Integration Tests

#[tokio::test]
async fn test_observer_successful_market_data_fetch() {
    // Setup
    let observer = MarketObserver::new();
    let mock_exchange = MockExchange::new();
    let ooda_loop = OodaLoop::new();
    
    // Ensure OODA loop starts in Idle state
    assert_eq!(ooda_loop.get_state().await, OodaState::Idle);
    
    // Transition to Observing state first
    assert!(ooda_loop.transition_to(OodaState::Observing).await.is_ok());
    assert_eq!(ooda_loop.get_state().await, OodaState::Observing);
    
    // Test observation with default BTC/USDT market data
    let result = observer.observe_symbol("BTC/USDT", &mock_exchange, &ooda_loop).await;
    
    // Verify successful observation
    assert!(result.is_ok(), "Observer should successfully fetch market data");
    let observation = result.unwrap();
    
    // Verify observation result
    assert_eq!(observation.symbol, "BTC/USDT");
    assert!(observation.is_success());
    assert_eq!(observation.price(), 50000.0); // Default BTC price in MockExchange
    assert_eq!(observation.volume(), 1000.0); // Default BTC volume in MockExchange
    assert!(observation.error_message().is_none());
    
    // Verify OODA loop transitioned to Orienting state
    assert_eq!(ooda_loop.get_state().await, OodaState::Orienting);
}

#[tokio::test]
async fn test_observer_handles_unsupported_symbol() {
    // Setup
    let observer = MarketObserver::new();
    let mock_exchange = MockExchange::new();
    let ooda_loop = OodaLoop::new();
    
    // Transition to Observing state
    assert!(ooda_loop.transition_to(OodaState::Observing).await.is_ok());
    
    // Test observation with unsupported symbol
    let result = observer.observe_symbol("UNSUPPORTED/USDT", &mock_exchange, &ooda_loop).await;
    
    // Should fail with observation error
    assert!(result.is_err(), "Observer should fail for unsupported symbol");
    
    // Verify OODA loop transitioned to Failed state
    let state = ooda_loop.get_state().await;
    assert!(matches!(state, OodaState::Failed(_)));
    
    if let OodaState::Failed(error_msg) = state {
        assert!(error_msg.contains("Exchange error"));
    }
}

#[tokio::test]
async fn test_observer_handles_unhealthy_exchange() {
    // Setup
    let observer = MarketObserver::new();
    let mock_exchange = MockExchange::new();
    let ooda_loop = OodaLoop::new();
    
    // Set exchange to unhealthy state
    mock_exchange.set_health(false).await;
    
    // Transition to Observing state
    assert!(ooda_loop.transition_to(OodaState::Observing).await.is_ok());
    
    // Test observation with unhealthy exchange
    let result = observer.observe_symbol("BTC/USDT", &mock_exchange, &ooda_loop).await;
    
    // Should fail with connection error
    assert!(result.is_err(), "Observer should fail when exchange is unhealthy");
    
    // Verify OODA loop transitioned to Failed state
    let state = ooda_loop.get_state().await;
    assert!(matches!(state, OodaState::Failed(_)));
}

#[tokio::test]
async fn test_observer_custom_data_age_threshold() {
    // Setup with custom data age threshold
    let observer = MarketObserver::with_max_data_age(Duration::from_secs(1));
    let mock_exchange = MockExchange::new();
    let ooda_loop = OodaLoop::new();
    
    // Verify custom threshold is set
    assert_eq!(observer.max_data_age(), Duration::from_secs(1));
    
    // Set up custom market data with old timestamp
    let old_market_data = MarketData {
        symbol: "BTC/USDT".to_string(),
        bid_price: dec!(49900.0),
        ask_price: dec!(50100.0),
        last_price: dec!(50000.0),
        volume_24h: dec!(1000.0),
        timestamp: SystemTime::now() - Duration::from_secs(10), // 10 seconds ago
    };
    
    mock_exchange.set_market_data("BTC/USDT".to_string(), old_market_data).await;
    
    // Transition to Observing state
    assert!(ooda_loop.transition_to(OodaState::Observing).await.is_ok());
    
    // Test observation - should fail due to stale data
    let result = observer.observe_symbol("BTC/USDT", &mock_exchange, &ooda_loop).await;
    
    // Should fail with stale data error
    assert!(result.is_err(), "Observer should reject stale market data");
    
    // Check that it's specifically a StaleMarketData error
    let error = result.unwrap_err();
    assert!(matches!(error, formatio::FormatioError::StaleMarketData { .. }));
}

#[tokio::test]
async fn test_observer_with_multiple_symbols() {
    // Setup
    let observer = MarketObserver::new();
    let mock_exchange = MockExchange::new();
    
    // Test observation of multiple symbols sequentially
    let symbols = vec!["BTC/USDT", "ETH/USDT"];
    let mut results = Vec::new();
    
    for symbol in &symbols {
        let ooda_loop = OodaLoop::new();
        assert!(ooda_loop.transition_to(OodaState::Observing).await.is_ok());
        
        let result = observer.observe_symbol(symbol, &mock_exchange, &ooda_loop).await;
        assert!(result.is_ok(), "Should successfully observe {}", symbol);
        
        let observation = result.unwrap();
        assert_eq!(observation.symbol, *symbol);
        assert!(observation.is_success());
        
        // Verify expected prices for default mock data
        match *symbol {
            "BTC/USDT" => assert_eq!(observation.price(), 50000.0),
            "ETH/USDT" => assert_eq!(observation.price(), 3000.0),
            _ => {}
        }
        
        // Verify OODA state transition
        assert_eq!(ooda_loop.get_state().await, OodaState::Orienting);
        
        results.push(observation);
    }
    
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_observer_default_constructor() {
    let observer = MarketObserver::default();
    
    // Should have default data age threshold
    assert_eq!(observer.max_data_age(), Duration::from_secs(5));
}

#[tokio::test]
async fn test_observation_result_helper_methods() {
    // Create a successful observation result
    let market_observation = formatio::types::MarketObservation {
        symbol: "BTC/USDT".to_string(),
        price: 50000.0,
        volume: 1000.0,
        timestamp: std::time::Instant::now(),
    };
    
    let success_result = ObservationResult {
        symbol: "BTC/USDT".to_string(),
        market_data: market_observation.clone(),
        success: true,
        error: None,
    };
    
    // Test helper methods
    assert!(success_result.is_success());
    assert_eq!(success_result.price(), 50000.0);
    assert_eq!(success_result.volume(), 1000.0);
    assert!(success_result.error_message().is_none());
    
    // Create a failed observation result
    let failed_result = ObservationResult {
        symbol: "INVALID/USDT".to_string(),
        market_data: market_observation,
        success: false,
        error: Some("Test error".to_string()),
    };
    
    assert!(!failed_result.is_success());
    assert_eq!(failed_result.error_message(), Some("Test error"));
}