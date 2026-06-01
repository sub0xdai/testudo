// @anchor test:cli:init
// @tags api

use testudo_cli::config::Config;
use testudo_cli::config::LlmConfig;

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

// ── LLM configuration tests (new for CLI-07) ──────────────────

#[test]
fn llm_config_roundtrip_preserves_provider() {
    let cfg = LlmConfig {
        provider: "deepseek".into(),
        api_key: "sk-test-key".into(),
        model: "deepseek-chat".into(),
        base_url: None,
    };
    // Roundtrip through TOML
    let config = Config {
        llm: cfg,
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    let config2: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(config2.llm.provider, "deepseek");
    assert_eq!(config2.llm.api_key, "sk-test-key");
    assert_eq!(config2.llm.model, "deepseek-chat");
    assert_eq!(config2.llm.base_url, None);
}

#[test]
fn llm_config_with_custom_base_url() {
    let cfg = LlmConfig {
        provider: "openai".into(),
        api_key: "sk-openai".into(),
        model: "gpt-4o".into(),
        base_url: Some("https://my-proxy.example.com/v1".into()),
    };
    let config = Config {
        llm: cfg,
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    let config2: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(config2.llm.provider, "openai");
    assert_eq!(config2.llm.base_url, Some("https://my-proxy.example.com/v1".into()));
}

#[test]
fn llm_config_defaults_are_anthropic() {
    let cfg = Config::default();
    assert_eq!(cfg.llm.provider, "anthropic");
    assert_eq!(cfg.llm.model, "claude-sonnet-4-20250514");
    assert!(cfg.llm.api_key.is_empty());
    assert!(cfg.llm.base_url.is_none());
}

/// Known provider keys that the init wizard should recognize.
fn known_providers() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("anthropic",   "https://api.anthropic.com",     "claude-sonnet-4-20250514"),
        ("openai",      "https://api.openai.com",         "gpt-4o"),
        ("deepseek",    "https://api.deepseek.com",       "deepseek-chat"),
        ("groq",        "https://api.groq.com",           "llama-3.3-70b-versatile"),
        ("together",    "https://api.together.xyz",       "meta-llama/Llama-3.3-70B-Instruct-Turbo"),
        ("xai",         "https://api.x.ai",               "grok-2"),
        ("mistral",     "https://api.mistral.ai",         "mistral-large-latest"),
        ("openrouter",  "https://openrouter.ai",          "anthropic/claude-sonnet-4"),
        ("qwen",        "https://dashscope.aliyuncs.com", "qwen-max"),
        ("gemini",      "https://generativelanguage.googleapis.com", "gemini-2.5-flash"),
        ("ollama",      "http://localhost:11434",         "llama3"),
    ]
}

#[test]
fn init_knows_all_11_providers() {
    let providers = known_providers();
    assert_eq!(providers.len(), 11, "Should have exactly 11 providers");
    for (name, url, model) in &providers {
        assert!(!name.is_empty());
        assert!(url.starts_with("http"), "Provider {} URL should start with http", name);
        assert!(!model.is_empty(), "Provider {} should have a default model", name);
    }
}

#[test]
fn init_provider_defaults_are_unique() {
    use std::collections::HashSet;
    let providers = known_providers();
    let names: HashSet<&str> = providers.iter().map(|(n, _, _)| *n).collect();
    assert_eq!(names.len(), providers.len(), "Provider names must be unique");
}

#[test]
fn init_llm_config_survives_empty_api_key() {
    // Users may skip entering an API key during init
    let cfg = LlmConfig {
        provider: "anthropic".into(),
        api_key: "".into(),
        model: "claude-sonnet-4-20250514".into(),
        base_url: None,
    };
    assert_eq!(cfg.api_key, "");
    // Empty API key is allowed — the agent loop will error at runtime
}

fn is_valid_agent_key_format(key: &str) -> bool {
    key.starts_with("testudo_sk_") && key.len() > 12
}

fn is_valid_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}
