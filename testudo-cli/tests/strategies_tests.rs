// @anchor test:cli:strategies
// @tags api

use testudo_cli::strategies::template::StrategyTemplate;
use testudo_cli::strategies::registry::StrategyRegistry;

#[test]
fn template_parses_valid_toml() {
    let toml_str = r#"
[meta]
name = "test-strat"
version = "0.1.0"
description = "A test strategy"

[prompt]
system = "You are a test trading agent."

[constraints]
max_leverage = 3
allowed_symbols = ["ETH_USDT", "BTC_USDT"]

[allowed_tools]
tools = ["fetch_klines", "submit_signal"]
"#;

    let tmpl: StrategyTemplate = toml::from_str(toml_str).expect("should parse valid TOML");
    assert_eq!(tmpl.meta.name, "test-strat");
    assert_eq!(tmpl.meta.version, "0.1.0");
    assert_eq!(tmpl.prompt.system, "You are a test trading agent.");
    assert_eq!(tmpl.constraints.as_ref().unwrap().max_leverage, Some(3));
    assert_eq!(
        tmpl.constraints.unwrap().allowed_symbols.unwrap(),
        vec!["ETH_USDT", "BTC_USDT"]
    );
}

#[test]
fn template_all_fields_optional_except_meta_and_prompt() {
    let toml_str = r#"
[meta]
name = "minimal"
version = "1.0"
description = "bare minimum"

[prompt]
system = "Minimal prompt."
"#;

    let tmpl: StrategyTemplate = toml::from_str(toml_str).expect("minimal TOML should parse");
    assert!(tmpl.loop_config.is_none());
    assert!(tmpl.constraints.is_none());
    assert!(tmpl.allowed_tools.is_none());
}

#[test]
fn registry_loads_builtins() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());
    let builtins = registry.list();

    let names: Vec<&str> = builtins.iter().map(|m| m.name.as_str()).collect();
    assert!(names.contains(&"mean-reversion"), "should contain mean-reversion");
    assert!(names.contains(&"momentum-breakout"), "should contain momentum-breakout");
    assert!(names.contains(&"funding-arb"), "should contain funding-arb");
}

#[test]
fn registry_get_returns_strategy() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());

    let strat = registry.get("mean-reversion");
    assert!(strat.is_some(), "mean-reversion should exist");
    let strat = strat.unwrap();
    assert!(!strat.prompt.system.is_empty());
    assert_eq!(strat.meta.name, "mean-reversion");
}

#[test]
fn registry_get_nonexistent_returns_none() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn registry_add_and_remove_user_strategy() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());

    let toml_content = r#"
[meta]
name = "user-strat"
version = "1.0"
description = "User-defined strategy"

[prompt]
system = "User prompt."
"#;

    // Add
    registry.add("user-strat", toml_content).expect("add should succeed");

    // Verify it's listed
    let list = registry.list();
    assert!(list.iter().any(|m| m.name == "user-strat"));

    // Remove
    registry.remove("user-strat").expect("remove should succeed");

    // Verify it's gone
    let list = registry.list();
    assert!(!list.iter().any(|m| m.name == "user-strat"));
}

#[test]
fn registry_cannot_remove_builtin() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = StrategyRegistry::new(tmp.path());
    let result = registry.remove("mean-reversion");
    assert!(result.is_err(), "should not be able to remove builtin");
}
