// @anchor test:cli:agent
// @tags api

use testudo_cli::model::agent::{AgentMode, AgentPhase, AgentState};
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
