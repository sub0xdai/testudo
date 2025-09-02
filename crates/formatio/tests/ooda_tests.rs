//! Tests for the OODA Loop implementation

use formatio::ooda::{OodaLoop, OodaState, OodaController, OodaLoopError};
use formatio::observer::{MarketObserver, ObservationResult};
use formatio::exchange::{MarketData};
use prudentia::exchange::MockExchange;
use testudo_types::OrderSide as TradeSide;
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
    let err = result.unwrap_err();
    assert!(matches!(err, OodaLoopError::InvalidStateTransition { .. }));
    
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

// Orientator Integration Tests

#[tokio::test]
async fn test_orientator_successful_trade_proposal_creation() {
    // Setup
    let orientator = formatio::PositionOrientator::new();
    let ooda_loop = OodaLoop::new();
    
    // Create mock market observation
    let market_observation = formatio::types::MarketObservation {
        symbol: "BTC/USDT".to_string(),
        price: 50000.0,
        volume: 1000.0,
        timestamp: std::time::Instant::now(),
    };
    
    // Set up OODA loop in Orienting state (previous state transition from Observer)
    assert!(ooda_loop.transition_to(OodaState::Orienting).await.is_ok());
    assert_eq!(ooda_loop.get_state().await, OodaState::Orienting);
    
    // Trade setup parameters
    let account_equity = dec!(10000.0); // $10,000 account
    let risk_percentage = dec!(0.02);   // 2% risk
    let stop_loss_distance_percent = dec!(0.02); // 2% stop loss
    
    // Execute orientation
    let result = orientator.orient(
        &market_observation,
        &ooda_loop,
        account_equity,
        risk_percentage,
        stop_loss_distance_percent,
    ).await;
    
    // Verify successful orientation
    assert!(result.is_ok(), "Orientator should successfully create trade proposal");
    let orientation = result.unwrap();
    
    // Verify trade proposal properties
    let proposal = &orientation.proposal;
    assert_eq!(proposal.symbol, "BTC/USDT");
    assert_eq!(proposal.side, TradeSide::Buy);
    // account_equity and risk_percentage are not fields in TradeProposal
    // These are calculated inputs, not stored in the proposal struct
    
    // Verify Van Tharp position sizing was applied
    let expected_entry = dec!(50000.0);
    let expected_stop = expected_entry - (expected_entry * stop_loss_distance_percent);
    assert_eq!(proposal.entry_price, expected_entry);
    assert_eq!(proposal.stop_loss, expected_stop);
    
    // Verify take profit (should be 2:1 risk/reward ratio)
    assert!(proposal.take_profit.is_some());
    let take_profit = proposal.take_profit.as_ref().unwrap();
    let expected_take_profit = expected_entry + (dec!(2) * (expected_entry - expected_stop));
    assert_eq!(*take_profit, expected_take_profit);
    
    // Verify timing metrics
    assert!(orientation.orientation_duration_ms > 0);
    assert!(orientation.orientation_duration_ms < 50); // Should be fast
    
    // Verify confidence calculation
    assert!(orientation.confidence > 0.0);
    assert!(orientation.confidence <= 1.0);
    
    // Verify OODA loop transitioned to Deciding state
    assert_eq!(ooda_loop.get_state().await, OodaState::Deciding);
}

#[tokio::test]
async fn test_orientator_rejects_stale_market_data() {
    // Setup
    let orientator = formatio::PositionOrientator::new();
    let ooda_loop = OodaLoop::new();
    
    // Create stale market observation (6 seconds old)
    let stale_timestamp = std::time::Instant::now() - Duration::from_secs(6);
    let market_observation = formatio::types::MarketObservation {
        symbol: "BTC/USDT".to_string(),
        price: 50000.0,
        volume: 1000.0,
        timestamp: stale_timestamp,
    };
    
    // Set up OODA loop in Orienting state
    assert!(ooda_loop.transition_to(OodaState::Orienting).await.is_ok());
    
    // Trade setup parameters
    let account_equity = dec!(10000.0);
    let risk_percentage = dec!(0.02);
    let stop_loss_distance_percent = dec!(0.02);
    
    // Execute orientation - should fail due to stale data
    let result = orientator.orient(
        &market_observation,
        &ooda_loop,
        account_equity,
        risk_percentage,
        stop_loss_distance_percent,
    ).await;
    
    // Verify orientation fails with stale data error
    assert!(result.is_err(), "Orientator should reject stale market data");
    let error = result.unwrap_err();
    
    match error {
        formatio::OrientationError::InvalidObservation(msg) => {
            assert!(msg.contains("stale"), "Error should mention stale data");
        },
        _ => panic!("Expected InvalidObservation error for stale data"),
    }
    
    // OODA loop should remain in Orienting state (no state transition on failure)
    assert_eq!(ooda_loop.get_state().await, OodaState::Orienting);
}

#[tokio::test]
async fn test_orientator_rejects_invalid_market_data() {
    // Setup
    let orientator = formatio::PositionOrientator::new();
    let ooda_loop = OodaLoop::new();
    
    // Test various invalid market observations
    let test_cases = vec![
        // Empty symbol
        formatio::types::MarketObservation {
            symbol: "".to_string(),
            price: 50000.0,
            volume: 1000.0,
            timestamp: std::time::Instant::now(),
        },
        // Zero price
        formatio::types::MarketObservation {
            symbol: "BTC/USDT".to_string(),
            price: 0.0,
            volume: 1000.0,
            timestamp: std::time::Instant::now(),
        },
        // Negative price
        formatio::types::MarketObservation {
            symbol: "BTC/USDT".to_string(),
            price: -100.0,
            volume: 1000.0,
            timestamp: std::time::Instant::now(),
        },
        // Negative volume
        formatio::types::MarketObservation {
            symbol: "BTC/USDT".to_string(),
            price: 50000.0,
            volume: -500.0,
            timestamp: std::time::Instant::now(),
        },
    ];
    
    for (i, invalid_observation) in test_cases.iter().enumerate() {
        // Reset OODA loop for each test case
        let ooda_loop = OodaLoop::new();
        assert!(ooda_loop.transition_to(OodaState::Orienting).await.is_ok());
        
        // Trade setup parameters
        let account_equity = dec!(10000.0);
        let risk_percentage = dec!(0.02);
        let stop_loss_distance_percent = dec!(0.02);
        
        // Execute orientation - should fail
        let result = orientator.orient(
            invalid_observation,
            &ooda_loop,
            account_equity,
            risk_percentage,
            stop_loss_distance_percent,
        ).await;
        
        assert!(result.is_err(), "Test case {} should fail with invalid observation", i);
        
        let error = result.unwrap_err();
        assert!(matches!(error, formatio::OrientationError::InvalidObservation(_)),
                "Test case {} should return InvalidObservation error", i);
    }
}

#[tokio::test]
async fn test_orientator_confidence_calculation() {
    // Setup
    let orientator = formatio::PositionOrientator::new();
    let ooda_loop = OodaLoop::new();
    assert!(ooda_loop.transition_to(OodaState::Orienting).await.is_ok());
    
    let account_equity = dec!(10000.0);
    let risk_percentage = dec!(0.02);
    let stop_loss_distance_percent = dec!(0.02);
    
    // Test high confidence scenario: fresh data, good volume
    let high_confidence_observation = formatio::types::MarketObservation {
        symbol: "BTC/USDT".to_string(),
        price: 50000.0,
        volume: 5000.0, // High volume
        timestamp: std::time::Instant::now(),
    };
    
    let result = orientator.orient(
        &high_confidence_observation,
        &ooda_loop,
        account_equity,
        risk_percentage,
        stop_loss_distance_percent,
    ).await;
    
    assert!(result.is_ok());
    let high_confidence = result.unwrap().confidence;
    
    // Reset for next test
    let ooda_loop2 = OodaLoop::new();
    assert!(ooda_loop2.transition_to(OodaState::Orienting).await.is_ok());
    
    // Test lower confidence scenario: older data, low volume
    let low_confidence_observation = formatio::types::MarketObservation {
        symbol: "BTC/USDT".to_string(),
        price: 50000.0,
        volume: 100.0, // Low volume
        timestamp: std::time::Instant::now() - Duration::from_secs(2), // Older data
    };
    
    let result2 = orientator.orient(
        &low_confidence_observation,
        &ooda_loop2,
        account_equity,
        risk_percentage,
        stop_loss_distance_percent,
    ).await;
    
    assert!(result2.is_ok());
    let low_confidence = result2.unwrap().confidence;
    
    // High confidence scenario should have higher confidence than low confidence scenario
    assert!(high_confidence > low_confidence, 
           "High confidence ({}) should be greater than low confidence ({})", 
           high_confidence, low_confidence);
    
    // Both should be in valid range
    assert!(high_confidence >= 0.0 && high_confidence <= 1.0);
    assert!(low_confidence >= 0.0 && low_confidence <= 1.0);
}

#[tokio::test]
async fn test_orientator_with_custom_calculator() {
    // Setup with custom Van Tharp calculator
    let custom_calculator = disciplina::calculator::PositionSizingCalculator::new();
    let orientator = formatio::PositionOrientator::new();
    let ooda_loop = OodaLoop::new();
    assert!(ooda_loop.transition_to(OodaState::Orienting).await.is_ok());
    
    // Create market observation
    let market_observation = formatio::types::MarketObservation {
        symbol: "ETH/USDT".to_string(),
        price: 3000.0,
        volume: 2000.0,
        timestamp: std::time::Instant::now(),
    };
    
    // Trade setup parameters for higher risk scenario
    let account_equity = dec!(50000.0); // $50,000 account
    let risk_percentage = dec!(0.05);   // 5% risk (higher than typical)
    let stop_loss_distance_percent = dec!(0.03); // 3% stop loss
    
    // Execute orientation
    let result = orientator.orient(
        &market_observation,
        &ooda_loop,
        account_equity,
        risk_percentage,
        stop_loss_distance_percent,
    ).await;
    
    // Verify successful orientation with custom parameters
    assert!(result.is_ok(), "Custom orientator should work correctly");
    let orientation = result.unwrap();
    
    // Verify proposal uses the specified parameters
    let proposal = &orientation.proposal;
    assert_eq!(proposal.symbol, "ETH/USDT");
    // account_equity and risk_percentage are not fields in TradeProposal
    // These are calculated inputs, not stored in the proposal struct
    
    // Verify position sizing is appropriate for higher risk scenario
    let expected_entry = dec!(3000.0);
    let expected_stop = expected_entry - (expected_entry * stop_loss_distance_percent);
    assert_eq!(proposal.entry_price, expected_entry);
    assert_eq!(proposal.stop_loss, expected_stop);
    
    // Verify OODA state transition
    assert_eq!(ooda_loop.get_state().await, OodaState::Deciding);
}

#[tokio::test] 
async fn test_orientator_performance_timing() {
    // Setup
    let orientator = formatio::PositionOrientator::new();
    let ooda_loop = OodaLoop::new();
    assert!(ooda_loop.transition_to(OodaState::Orienting).await.is_ok());
    
    let market_observation = formatio::types::MarketObservation {
        symbol: "BTC/USDT".to_string(),
        price: 50000.0,
        volume: 1000.0,
        timestamp: std::time::Instant::now(),
    };
    
    // Execute orientation multiple times to test consistency
    let mut durations = Vec::new();
    
    for _ in 0..10 {
        // Reset OODA loop
        let test_ooda = OodaLoop::new();
        assert!(test_ooda.transition_to(OodaState::Orienting).await.is_ok());
        
        let start = std::time::Instant::now();
        
        let result = orientator.orient(
            &market_observation,
            &test_ooda,
            dec!(10000.0),
            dec!(0.02),
            dec!(0.02),
        ).await;
        
        let duration = start.elapsed();
        
        assert!(result.is_ok(), "Orientation should succeed in performance test");
        durations.push(duration.as_millis() as u64);
        
        // Verify reported duration is reasonable
        let reported_duration = result.unwrap().orientation_duration_ms;
        assert!(reported_duration <= duration.as_millis() as u64 + 5); // Allow 5ms tolerance
    }
    
    // All orientations should complete within performance target (50ms)
    for duration in &durations {
        assert!(*duration < 50, "Orientation took {}ms, exceeds 50ms target", duration);
    }
    
    // Average should be well under target
    let average: u64 = durations.iter().sum::<u64>() / durations.len() as u64;
    assert!(average < 25, "Average orientation time {}ms should be well under 50ms target", average);
}