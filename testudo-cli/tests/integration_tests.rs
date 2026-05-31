// @anchor test:cli:integration
// @tags api, infra

use testudo_cli::config::Config;
use testudo_cli::daemon::{self, DaemonState};

#[test]
fn integration_daemon_lifecycle_pid_file() {
    let pid_path = daemon::pid_path();
    // Clean up any existing PID
    let _ = std::fs::remove_file(&pid_path);

    // Write PID
    daemon::write_pid_file().unwrap();
    assert!(pid_path.exists(), "PID file should exist after write");

    // Read back and verify it's a valid number
    let pid_str = std::fs::read_to_string(&pid_path).unwrap();
    let _pid: u32 = pid_str.trim().parse().unwrap();

    // Cleanup
    daemon::remove_pid_file();
    assert!(!pid_path.exists(), "PID file should be removed");
}

#[test]
fn integration_daemon_state_status_json() {
    let state = DaemonState {
        phase: "Acting".into(),
        signal_count: 7,
        uptime_secs: 300,
        last_error: None,
    };

    let json = serde_json::to_string(&state).unwrap();
    let parsed: DaemonState = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.phase, "Acting");
    assert_eq!(parsed.signal_count, 7);
    assert_eq!(parsed.uptime_secs, 300);
}

#[test]
fn integration_config_full_roundtrip() {
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg).unwrap();

    // Verify all sections present
    assert!(toml_str.contains("[ui]"));
    assert!(toml_str.contains("[api]"));
    assert!(toml_str.contains("[agent]"));
    assert!(toml_str.contains("[llm]"));

    // Roundtrip
    let cfg2: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(cfg2.api.base_url, cfg.api.base_url);
    assert_eq!(cfg2.agent.shadow_only, cfg.agent.shadow_only);
}

#[test]
fn integration_agent_loop_requires_valid_config() {
    let mut cfg = Config::default();
    cfg.llm.api_key = String::new(); // no key

    let result = testudo_cli::cmd::run_agent(&cfg, None);
    assert!(result.is_err(), "agent should fail without LLM key");
}

#[test]
fn integration_strategy_list_always_has_builtins() {
    let tmp = tempfile::tempdir().unwrap();
    let result = testudo_cli::cmd::run_strategy_list(tmp.path());
    assert!(result.is_ok(), "strategy list should always succeed");
}

#[test]
fn integration_shadow_only_enforcement() {
    use testudo_cli::model::agent::AgentMode;

    let shadow = AgentMode::Shadow;
    assert!(shadow.is_shadow_only());

    let live = AgentMode::Live;
    assert!(!live.is_shadow_only());
}
