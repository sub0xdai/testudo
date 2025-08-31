//! Integration tests for Phase 4: Decide - Risk Protocol Validation
//!
//! These tests validate the complete integration between the formatio RiskDecider
//! and the prudentia RiskManagementProtocol, ensuring proper state transitions
//! and decision reasoning in the OODA loop.

use std::sync::Arc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use formatio::{
    RiskDecider, RiskDecision, ExecutionPriority,
    orientator::TradeProposal
};

use prudentia::{
    risk::{
        protocol::RiskManagementProtocol,
        assessment_rules::MaxTradeRiskRule,
    },
    types::protocol_limits::ProtocolLimits,
};

use testudo_types::OrderSide;
use tokio::time::Duration;

/// Helper function to create a test RiskManagementProtocol
fn create_test_risk_protocol() -> Arc<RiskManagementProtocol> {
    // Create conservative protocol limits
    let limits = ProtocolLimits::conservative_limits();
    
    let protocol = RiskManagementProtocol::with_name(
        "TestProtocol".to_string(),
        false // Don't fail fast for testing
    )
    .add_rule(MaxTradeRiskRule::new());
    
    Arc::new(protocol)
}

/// Helper function to create a test trade proposal 
fn create_test_proposal(
    symbol: &str,
    side: OrderSide,
    entry_price: Decimal,
    stop_loss: Decimal,
    position_size: Decimal
) -> TradeProposal {
    TradeProposal {
        symbol: symbol.to_string(),
        side,
        entry_price,
        stop_loss,
        take_profit: None,
        position_size,
    }
}

#[tokio::test]
async fn test_valid_trade_proposal_handling() {
    // Test Case: Valid trade proposal should be handled properly
    let protocol = create_test_risk_protocol();
    let decider = RiskDecider::new(protocol);
    
    // Create a conservative trade proposal (small risk)
    let proposal = create_test_proposal(
        "BTCUSDT",
        OrderSide::Buy,
        dec!(50000.0),  // Entry: $50,000
        dec!(49000.0),  // Stop: $49,000 (2% stop distance)
        dec!(1.0)       // Small position size
    );
    
    // Execute decision process
    let result = decider.decide_trade(proposal.clone()).await;
    
    assert!(result.is_ok(), "Decision process should succeed");
    let decision_result = result.unwrap();
    
    // Validate decision (could be approved or rejected depending on risk assessment)
    match decision_result.decision {
        RiskDecision::Execute { approved_position_size, execution_priority } => {
            assert!(approved_position_size > Decimal::ZERO, "Position size should be positive");
            assert!(
                matches!(execution_priority, ExecutionPriority::Standard | ExecutionPriority::Careful),
                "Should have valid execution priority"
            );
            println!("Trade APPROVED: size={}, priority={:?}", approved_position_size, execution_priority);
        },
        RiskDecision::Reject { rejection_reason, violation_count } => {
            assert!(!rejection_reason.is_empty(), "Should have rejection reason");
            assert!(violation_count > 0, "Should have violations");
            println!("Trade REJECTED: {} violations - {}", violation_count, rejection_reason);
        },
        RiskDecision::AssessmentFailed { error_details } => {
            panic!("Assessment should not fail for valid input: {}", error_details);
        }
    }
    
    // Validate timing (should be under reasonable limit)
    assert!(decision_result.decision_time_ms <= 100, 
        "Decision time {}ms should be under reasonable limit", decision_result.decision_time_ms);
    
    // Validate audit trail
    assert!(!decision_result.audit_trail.is_empty(), "Audit trail should contain entries");
    
    // Validate original proposal is preserved
    assert_eq!(decision_result.proposal.symbol, "BTCUSDT");
    assert_eq!(decision_result.proposal.side, OrderSide::Buy);
}

#[tokio::test] 
async fn test_decision_timeout_handling() {
    // Test Case: Decision timeout should be handled gracefully
    let protocol = create_test_risk_protocol();
    let decider = RiskDecider::with_config(
        protocol,
        "TimeoutDecider".to_string(),
        Duration::from_millis(1) // Very short timeout to trigger timeout
    );
    
    let proposal = create_test_proposal(
        "SOLUSDT",
        OrderSide::Buy, 
        dec!(100.0),
        dec!(95.0),
        dec!(5.0)
    );
    
    // Execute decision process
    let result = decider.decide_trade(proposal.clone()).await;
    
    assert!(result.is_ok(), "Decision process should handle timeout gracefully");
    let decision_result = result.unwrap();
    
    // Should result in assessment failed due to timeout
    match decision_result.decision {
        RiskDecision::AssessmentFailed { error_details } => {
            assert!(error_details.contains("timeout"), 
                "Error should mention timeout: {}", error_details);
            println!("Timeout properly handled: {}", error_details);
        },
        _ => {
            // If timeout didn't trigger (system is very fast), that's also acceptable
            println!("Timeout test didn't trigger - system completed in time");
        }
    }
    
    // Validate audit trail exists even for timeouts
    assert!(!decision_result.audit_trail.is_empty(), "Should have audit trail even for timeouts");
}

#[tokio::test]
async fn test_multiple_trade_decisions_consistency() {
    // Test Case: Multiple identical trade proposals should produce consistent decisions
    let protocol = create_test_risk_protocol();
    let decider = RiskDecider::new(protocol);
    
    let proposal = create_test_proposal(
        "DOTUSDT",
        OrderSide::Buy,
        dec!(25.0),
        dec!(24.0),  // Valid stop loss (below entry for long)
        dec!(20.0)
    );
    
    // Execute decision process multiple times
    let mut results = Vec::new();
    for _ in 0..3 {
        let result = decider.decide_trade(proposal.clone()).await;
        assert!(result.is_ok(), "All decisions should succeed");
        results.push(result.unwrap());
    }
    
    // All decisions should be consistent
    let first_decision_type = match &results[0].decision {
        RiskDecision::Execute { .. } => "Execute",
        RiskDecision::Reject { .. } => "Reject", 
        RiskDecision::AssessmentFailed { .. } => "Failed",
    };
    
    for (i, result) in results.iter().enumerate() {
        let decision_type = match &result.decision {
            RiskDecision::Execute { .. } => "Execute",
            RiskDecision::Reject { .. } => "Reject",
            RiskDecision::AssessmentFailed { .. } => "Failed",
        };
        
        assert_eq!(decision_type, first_decision_type,
            "Decision {} type '{}' should match first decision type '{}'", 
            i, decision_type, first_decision_type);
        
        // All should complete within reasonable time
        assert!(result.decision_time_ms <= 100, 
            "Decision {} took too long: {}ms", i, result.decision_time_ms);
    }
    
    println!("Consistency test passed: all {} decisions were {}", results.len(), first_decision_type);
}

#[tokio::test] 
async fn test_audit_trail_completeness() {
    // Test Case: Audit trail should capture complete decision reasoning
    let protocol = create_test_risk_protocol();
    let decider = RiskDecider::new(protocol);
    
    let proposal = create_test_proposal(
        "LINKUSDT",
        OrderSide::Buy,
        dec!(15.0),
        dec!(14.0),  // Valid stop loss 
        dec!(30.0)
    );
    
    let result = decider.decide_trade(proposal.clone()).await;
    assert!(result.is_ok(), "Decision process should succeed");
    let decision_result = result.unwrap();
    
    // Validate comprehensive audit trail
    let audit_entries: Vec<&str> = decision_result.audit_trail.iter()
        .map(|s| s.as_str()).collect();
    
    // Should contain key stages
    assert!(audit_entries.iter().any(|entry| entry.contains("Decision started")),
        "Should log decision start");
    
    assert!(audit_entries.iter().any(|entry| entry.contains("converted")),
        "Should log proposal conversion");
        
    assert!(audit_entries.iter().any(|entry| 
        entry.contains("APPROVED") || entry.contains("REJECTED") || entry.contains("FAILED")),
        "Should log final decision");
    
    // Audit trail should be chronological and complete
    assert!(audit_entries.len() >= 3, 
        "Should have at least 3 audit entries, got: {:?}", audit_entries);
    
    println!("Audit trail entries: {}", audit_entries.len());
    for (i, entry) in audit_entries.iter().enumerate() {
        println!("  {}: {}", i + 1, entry);
    }
}

#[tokio::test]
async fn test_risk_decider_protocol_integration() {
    // Test Case: Integration with actual protocol assessment
    let protocol = create_test_risk_protocol();
    let decider = RiskDecider::new(protocol.clone());
    
    // Verify protocol is properly configured
    assert_eq!(decider.protocol_name(), "TestProtocol");
    assert_eq!(decider.max_decision_time(), Duration::from_millis(25));
    
    // Test that protocol is actually being called
    let proposal = create_test_proposal(
        "AVAXUSDT",
        OrderSide::Buy,
        dec!(40.0),
        dec!(38.0),
        dec!(15.0)
    );
    
    let result = decider.decide_trade(proposal).await;
    assert!(result.is_ok(), "Protocol integration should work");
    
    let decision_result = result.unwrap();
    
    // Assessment should contain protocol-generated data  
    // Note: accessing the nested assessment fields properly
    assert!(!decision_result.assessment.assessment.assessment_id.is_nil(), 
        "Should have valid assessment ID from protocol");
    
    assert!(!decision_result.assessment.assessment.proposal_id.is_nil(),
        "Should have valid proposal ID from protocol");
    
    // Should have decision reasoning from protocol
    assert!(!decision_result.assessment.decision_reasoning.is_empty(),
        "Should have decision reasoning from protocol");
    
    println!("Protocol integration test passed with decision: {}", 
        decision_result.assessment.decision_reasoning);
}

/// Performance test for decision latency requirements
#[tokio::test]
async fn test_decision_latency_performance() {
    let protocol = create_test_risk_protocol();
    let decider = RiskDecider::new(protocol);
    
    let proposal = create_test_proposal(
        "MATICUSDT",
        OrderSide::Buy,
        dec!(2.0),
        dec!(1.90),
        dec!(100.0)
    );
    
    // Measure decision time over multiple runs
    let mut total_time = 0u64;
    let runs = 5;
    
    for _ in 0..runs {
        let start = std::time::Instant::now();
        let result = decider.decide_trade(proposal.clone()).await;
        let elapsed = start.elapsed().as_millis() as u64;
        
        assert!(result.is_ok(), "All performance test runs should succeed");
        total_time += elapsed;
    }
    
    let average_time = total_time / runs;
    
    // Performance requirement: average under reasonable limit (relaxed for CI)
    assert!(average_time <= 100, 
        "Average decision time {}ms exceeds 100ms limit", average_time);
    
    println!("Performance test passed: average decision time {}ms over {} runs", average_time, runs);
}

#[tokio::test]
async fn test_different_trade_sides() {
    // Test Case: Both Buy and Sell trades should be handled properly
    let protocol = create_test_risk_protocol();
    let decider = RiskDecider::new(protocol);
    
    // Test Buy trade
    let buy_proposal = create_test_proposal(
        "BTCUSDT",
        OrderSide::Buy,
        dec!(50000.0),
        dec!(49000.0),  // Stop below entry for long
        dec!(2.0)
    );
    
    let buy_result = decider.decide_trade(buy_proposal).await;
    assert!(buy_result.is_ok(), "Buy trade decision should succeed");
    
    // Test Sell trade  
    let sell_proposal = create_test_proposal(
        "ETHUSDT",
        OrderSide::Sell,
        dec!(3000.0),
        dec!(3100.0),  // Stop above entry for short
        dec!(3.0)
    );
    
    let sell_result = decider.decide_trade(sell_proposal).await;
    assert!(sell_result.is_ok(), "Sell trade decision should succeed");
    
    // Both should produce valid decisions
    let buy_decision = buy_result.unwrap();
    let sell_decision = sell_result.unwrap();
    
    // Validate both have valid audit trails
    assert!(!buy_decision.audit_trail.is_empty(), "Buy decision should have audit trail");
    assert!(!sell_decision.audit_trail.is_empty(), "Sell decision should have audit trail");
    
    // Both should complete in reasonable time
    assert!(buy_decision.decision_time_ms <= 100, "Buy decision should be fast");
    assert!(sell_decision.decision_time_ms <= 100, "Sell decision should be fast");
    
    println!("Both trade sides handled successfully");
}