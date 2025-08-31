//! Tests for the OODA Loop implementation

use formatio::ooda::{OodaLoop, OodaState, OodaController};
use formatio::types::TradeIntent;
use std::sync::Arc;

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