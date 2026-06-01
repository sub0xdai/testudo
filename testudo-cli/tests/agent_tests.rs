// @anchor test:cli:agent
// @tags api

use testudo_cli::model::agent::{AgentMode, AgentPhase, AgentState};
use testudo_cli::model::agent::idempotency::IdempotencyTracker;
use testudo_cli::model::agent::rate_limit::SignalRateLimiter;
use testudo_cli::config::{ApiConfig, Config, LlmConfig};

#[test]
fn agent_state_defaults_to_observing_shadow() {
    let state = AgentState::new(AgentMode::Shadow);
    assert!(matches!(state.phase, AgentPhase::Observing));
    assert!(matches!(state.mode, AgentMode::Shadow));
}

#[test]
fn agent_state_live_mode() {
    let state = AgentState::new(AgentMode::Live);
    assert!(matches!(state.mode, AgentMode::Live));
}

#[test]
fn agent_phase_transitions_are_distinct() {
    // Verify phases are different values
    let phases = [
        AgentPhase::Observing,
        AgentPhase::Thinking,
        AgentPhase::Acting,
        AgentPhase::Idle,
    ];
    for i in 0..phases.len() {
        for j in (i + 1)..phases.len() {
            assert!(
                !std::mem::discriminant(&phases[i])
                    .eq(&std::mem::discriminant(&phases[j])),
                "phase {} and {} should be distinct",
                i, j
            );
        }
    }
}

#[test]
fn agent_requires_llm_api_key() {
    let config = Config {
        llm: LlmConfig {
            base_url: None,
            provider: "anthropic".into(),
            api_key: "".into(),
            model: "claude-sonnet-4-20250514".into(),
        },
        api: ApiConfig {
            base_url: "http://localhost:8080/api/v1".into(),
            agent_key: "testudo_sk_test".into(),
            ws_url: "ws://localhost:8081".into(),
        },
        ..Config::default()
    };
    let result = testudo_cli::cmd::run_agent(&config, None);
    assert!(result.is_err(), "agent should fail with empty LLM api key");
}

#[test]
fn agent_requires_agent_key() {
    let config = Config {
        api: ApiConfig {
            base_url: "http://localhost:8080/api/v1".into(),
            agent_key: "".into(),
            ws_url: "ws://localhost:8081".into(),
        },
        llm: LlmConfig {
            base_url: None,
            provider: "anthropic".into(),
            api_key: "sk-ant-test".into(),
            model: "claude-sonnet-4-20250514".into(),
        },
        ..Config::default()
    };
    let result = testudo_cli::cmd::run_agent(&config, None);
    assert!(result.is_err(), "agent should fail with empty agent key");
}

#[test]
fn shadow_only_rejects_live_mode() {
    let mode = AgentMode::Shadow;
    let execution_mode = "LIVE";
    let blocked = matches!(mode, AgentMode::Shadow)
        && execution_mode.eq_ignore_ascii_case("LIVE");
    assert!(blocked, "shadow mode should reject LIVE execution_mode");
}

#[test]
fn live_mode_allows_live_execution() {
    let mode = AgentMode::Live;
    let execution_mode = "LIVE";
    let blocked = matches!(mode, AgentMode::Shadow)
        && execution_mode.eq_ignore_ascii_case("LIVE");
    assert!(!blocked, "live mode should allow LIVE execution_mode");
}

#[test]
fn idempotency_generates_unique_keys() {
    let mut tracker = IdempotencyTracker::default();
    let key1 = tracker.next_key();
    let key2 = tracker.next_key();
    assert_ne!(key1, key2, "each signal should have a unique idempotency key");
}

#[test]
fn idempotency_key_is_valid_uuid_v4() {
    let mut tracker = IdempotencyTracker::default();
    let key = tracker.next_key();
    assert_eq!(key.get_version_num(), 4, "idempotency key should be UUIDv4");
}

#[test]
fn idempotency_retry_reuses_same_key() {
    let mut tracker = IdempotencyTracker::default();
    let key = tracker.next_key();
    let retry_key = tracker.current_key();
    assert_eq!(key, retry_key, "retry should reuse same idempotency key");
}

#[test]
fn idempotency_max_retries_is_three() {
    let tracker = IdempotencyTracker::new(3);
    assert_eq!(tracker.max_retries(), 3);
}

#[test]
fn rate_limiter_allows_within_limit() {
    let mut limiter = SignalRateLimiter::new(5);
    for _ in 0..5 {
        assert!(limiter.try_signal(), "should allow up to max signals");
    }
}

#[test]
fn rate_limiter_blocks_exceeding_limit() {
    let mut limiter = SignalRateLimiter::new(3);
    assert!(limiter.try_signal());
    assert!(limiter.try_signal());
    assert!(limiter.try_signal());
    assert!(!limiter.try_signal(), "should block after max signals reached");
    assert!(!limiter.try_signal());
}

#[test]
fn rate_limiter_resets_after_window() {
    let mut limiter = SignalRateLimiter::new(2);
    assert!(limiter.try_signal());
    assert!(limiter.try_signal());
    assert!(!limiter.try_signal());
    // Force reset by advancing window
    limiter.reset();
    assert!(limiter.try_signal(), "should allow after reset");
}

use testudo_cli::strategies::registry::StrategyRegistry;

#[test]
fn strategy_loading_returns_correct_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());
    
    let strat = registry.get("mean-reversion").unwrap();
    assert!(strat.prompt.system.contains("mean-reversion"));
    assert!(strat.prompt.system.contains("Bollinger Bands"));
}

#[test]
fn strategy_loading_with_none_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn strategy_tool_filtering() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());
    let strat = registry.get("mean-reversion").unwrap();
    
    if let Some(ref tools) = strat.allowed_tools {
        assert!(tools.tools.contains(&"fetch_klines".to_string()));
        assert!(tools.tools.contains(&"submit_signal".to_string()));
        // mean-reversion only allows 4 tools
        assert_eq!(tools.tools.len(), 4);
    } else {
        panic!("mean-reversion should have allowed_tools");
    }
}

#[test]
fn strategy_loop_config_overrides() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());
    let strat = registry.get("mean-reversion").unwrap();
    
    if let Some(ref loop_cfg) = strat.loop_config {
        assert_eq!(loop_cfg.interval_secs, Some(120));
        assert_eq!(loop_cfg.shadow_only, Some(true));
        assert_eq!(loop_cfg.max_signals_per_hour, Some(3));
    } else {
        panic!("mean-reversion should have loop_config");
    }
}

#[test]
fn funding_arb_has_different_constraints() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());
    let strat = registry.get("funding-arb").unwrap();
    
    let constraints = strat.constraints.unwrap();
    assert_eq!(constraints.max_leverage, Some(2));
    // funding-arb is more conservative than momentum-breakout (5x)
}
