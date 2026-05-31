// @anchor test:cli:config
// @tags infra

use testudo_cli::config::Config;

#[test]
fn default_config_has_correct_values() {
    let cfg = Config::default();

    // UI
    assert_eq!(cfg.ui.theme, "vanilla-amoled");

    // API
    assert_eq!(cfg.api.base_url, "http://localhost:8080/api/v1");
    assert_eq!(cfg.api.agent_key, "");

    // Agent
    assert_eq!(cfg.agent.loop_interval_secs, 60);
    assert!(cfg.agent.shadow_only);

    // LLM
    assert_eq!(cfg.llm.provider, "anthropic");
    assert_eq!(cfg.llm.api_key, "");
    assert_eq!(cfg.llm.model, "claude-sonnet-4-20250514");
}

#[test]
fn config_roundtrip_serialize_deserialize() {
    let cfg = Config::default();

    // Serialize to TOML string
    let toml_str = toml::to_string_pretty(&cfg).expect("serialize should succeed");

    // Deserialize back
    let cfg2: Config = toml::from_str(&toml_str).expect("deserialize should succeed");

    // Verify all fields match
    assert_eq!(cfg2.ui.theme, cfg.ui.theme);
    assert_eq!(cfg2.api.base_url, cfg.api.base_url);
    assert_eq!(cfg2.api.agent_key, cfg.api.agent_key);
    assert_eq!(cfg2.agent.loop_interval_secs, cfg.agent.loop_interval_secs);
    assert_eq!(cfg2.agent.shadow_only, cfg.agent.shadow_only);
    assert_eq!(cfg2.llm.provider, cfg.llm.provider);
    assert_eq!(cfg2.llm.api_key, cfg.llm.api_key);
    assert_eq!(cfg2.llm.model, cfg.llm.model);
}
