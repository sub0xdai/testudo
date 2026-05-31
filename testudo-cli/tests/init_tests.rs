// @anchor test:cli:init
// @tags api

use testudo_cli::config::Config;

#[test]
fn init_config_has_valid_defaults() {
    let cfg = Config::default();
    assert_eq!(cfg.api.base_url, "https://testudo.vip/api/v1");
    assert_eq!(cfg.agent.loop_interval_secs, 60);
    assert!(cfg.agent.shadow_only);
}

#[test]
fn init_validates_agent_key_prefix() {
    // Valid keys start with testudo_sk_
    assert!(is_valid_agent_key_format("testudo_sk_abc123def456"));
    assert!(!is_valid_agent_key_format("wrong_prefix_key"));
    assert!(!is_valid_agent_key_format(""));
}

#[test]
fn init_validates_url_format() {
    assert!(is_valid_url("http://localhost:8080/api/v1"));
    assert!(is_valid_url("https://api.testudo.io/v1"));
    assert!(!is_valid_url("not-a-url"));
    assert!(!is_valid_url(""));
}

#[test]
fn init_config_roundtrip_preserves_values() {
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let cfg2: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(cfg2.api.base_url, cfg.api.base_url);
    assert_eq!(cfg2.api.agent_key, cfg.api.agent_key);
    assert_eq!(cfg2.agent.loop_interval_secs, cfg.agent.loop_interval_secs);
}

fn is_valid_agent_key_format(key: &str) -> bool {
    key.starts_with("testudo_sk_") && key.len() > 12
}

fn is_valid_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}
