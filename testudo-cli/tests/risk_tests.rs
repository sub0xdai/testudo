// @anchor test:cli:risk
// @tags api

use testudo_cli::risk::precheck::RiskPrecheck;
use testudo_cli::strategies::template::StrategyConstraints;

fn make_constraints(max_leverage: Option<u8>, allowed_symbols: Option<Vec<String>>) -> StrategyConstraints {
    StrategyConstraints {
        max_leverage,
        max_position_notional: None,
        allowed_symbols,
        shadow_only: None,
    }
}

#[test]
fn precheck_passes_within_limits() {
    let constraints = make_constraints(Some(5), None);
    let precheck = RiskPrecheck::new(&constraints, 0, 10);
    let result = precheck.validate_leverage(3);
    assert!(result.passed, "leverage 3 with max 5 should pass");
}

#[test]
fn precheck_rejects_over_leveraged() {
    let constraints = make_constraints(Some(3), None);
    let precheck = RiskPrecheck::new(&constraints, 0, 10);
    let result = precheck.validate_leverage(10);
    assert!(!result.passed, "leverage 10 with max 3 should be rejected");
    assert!(result.reason.unwrap().contains("3"), "reason should mention max leverage");
}

#[test]
fn precheck_passes_when_no_leverage_limit() {
    let constraints = make_constraints(None, None);
    let precheck = RiskPrecheck::new(&constraints, 0, 10);
    let result = precheck.validate_leverage(50);
    assert!(result.passed, "no leverage limit should allow any leverage");
}

#[test]
fn precheck_rejects_when_max_positions_exceeded() {
    let constraints = make_constraints(None, None);
    let precheck = RiskPrecheck::new(&constraints, 5, 5); // 5 current, 5 max
    let result = precheck.validate_positions();
    assert!(!result.passed, "5/5 positions should be rejected");
}

#[test]
fn precheck_allows_when_positions_under_limit() {
    let constraints = make_constraints(None, None);
    let precheck = RiskPrecheck::new(&constraints, 3, 5);
    let result = precheck.validate_positions();
    assert!(result.passed, "3/5 positions should pass");
}

#[test]
fn precheck_rejects_disallowed_symbol() {
    let constraints = make_constraints(
        None,
        Some(vec!["ETH_USDT".into(), "BTC_USDT".into()]),
    );
    let precheck = RiskPrecheck::new(&constraints, 0, 10);
    let result = precheck.validate_symbol("SOL_USDT");
    assert!(!result.passed, "SOL should be rejected");
    assert!(result.reason.unwrap().contains("not in"));
}

#[test]
fn precheck_allows_allowed_symbol() {
    let constraints = make_constraints(
        None,
        Some(vec!["ETH_USDT".into(), "BTC_USDT".into()]),
    );
    let precheck = RiskPrecheck::new(&constraints, 0, 10);
    let result = precheck.validate_symbol("ETH_USDT");
    assert!(result.passed, "ETH should be allowed");
}

#[test]
fn precheck_allow_all_symbols_when_no_list() {
    let constraints = make_constraints(None, None);
    let precheck = RiskPrecheck::new(&constraints, 0, 10);
    let result = precheck.validate_symbol("SOL_USDT");
    assert!(result.passed, "any symbol should be allowed when no list");
}
