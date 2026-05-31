// @anchor test:cli:loader
// @tags api

use testudo_cli::strategies::loader::StrategyLoader;
use std::path::PathBuf;

fn proofs_dir() -> PathBuf {
    // Relative to testudo-cli/ crate root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("testudo-proofs")
        .join("Proofs")
}

#[test]
fn loader_finds_kelly_artifact() {
    let dir = proofs_dir();
    let loader = StrategyLoader::new(dir);
    let artifacts = loader.load_all().expect("should load artifacts");

    assert!(artifacts.contains_key("kelly"), "should contain kelly");
    let kelly = &artifacts["kelly"];
    assert_eq!(kelly.meta.name, "kelly");
    assert_eq!(kelly.theorem.name, "kelly_maximizes_growth");
}

#[test]
fn loader_parses_constraints_as_numbers() {
    let dir = proofs_dir();
    let loader = StrategyLoader::new(dir);
    let artifacts = loader.load_all().unwrap();
    let kelly = &artifacts["kelly"];

    let max_lev = kelly.constraints["max_leverage"].as_integer().unwrap() as f64;
    assert_eq!(max_lev, 5.0, "Kelly max_leverage should be 5");

    let risk_pct = kelly.constraints["max_account_risk_pct"].as_float().unwrap();
    assert_eq!(risk_pct, 2.0, "Kelly account risk should be 2%");
}

#[test]
fn loader_parses_all_seven_artifacts() {
    let dir = proofs_dir();
    let loader = StrategyLoader::new(dir);
    let artifacts = loader.load_all().unwrap();

    let expected = [
        "kelly", "gamblers-ruin", "wasserstein", "ou-reversion",
        "momentum", "funding-arb", "delta-neutral",
    ];
    for name in expected {
        assert!(
            artifacts.contains_key(name),
            "should contain artifact: {}", name
        );
    }
}

#[test]
fn constraint_merge_picks_most_conservative() {
    let mut cs = ConstraintSet::defaults();
    cs.apply_artifact("kelly", 5.0, 2.0, 20.0);   // max_leverage=5, risk=2%, drawdown=20%
    cs.apply_artifact("gamblers-ruin", 3.0, 1.5, 15.0); // more conservative on all

    assert_eq!(cs.max_leverage, 3.0, "should pick 3 (more conservative)");
    assert_eq!(cs.max_account_risk_pct, 1.5);
    assert_eq!(cs.max_drawdown_pct, 15.0);
}

#[test]
fn constraint_merge_skips_when_no_new_bounds() {
    let mut cs = ConstraintSet::defaults();
    cs.apply_artifact("kelly", 5.0, 2.0, 20.0);
    cs.apply_artifact("lenient", 10.0, 5.0, 30.0); // all looser

    assert_eq!(cs.max_leverage, 5.0, "should keep kelly's tighter bound");
    assert_eq!(cs.max_account_risk_pct, 2.0);
}

#[test]
fn constraint_user_can_only_tighten() {
    let mut cs = ConstraintSet::defaults();
    cs.apply_artifact("kelly", 5.0, 2.0, 20.0);

    // User sets max_leverage=2 (tighter than artifact's 5)
    cs.intersect_user(2.0, 1.0, 10.0);

    assert_eq!(cs.max_leverage, 2.0, "user tightened from 5 to 2");
    assert_eq!(cs.max_account_risk_pct, 1.0, "user tightened from 2 to 1");
    assert_eq!(cs.max_drawdown_pct, 10.0, "user tightened from 20 to 10");
}

#[test]
fn constraint_user_cannot_loosen() {
    let mut cs = ConstraintSet::defaults();
    cs.apply_artifact("kelly", 5.0, 2.0, 20.0);

    // User tries to set max_leverage=10 (looser than artifact's 5)
    cs.intersect_user(10.0, 5.0, 30.0);

    assert_eq!(cs.max_leverage, 5.0, "artifact bound of 5 should win over user's 10");
    assert_eq!(cs.max_account_risk_pct, 2.0);
    assert_eq!(cs.max_drawdown_pct, 20.0);
}

use testudo_cli::strategies::tools::ToolConstrainer;
use testudo_cli::strategies::constraints::ConstraintSet;
use testudo_cli::tools::types::ToolDef;

#[test]
fn tool_constrainer_clamps_leverage() {
    let mut tool = ToolDef {
        name: "submit_signal".into(),
        description: "Submit a signal".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "leverage": {"type": "integer", "minimum": 1, "maximum": 20}
            },
            "required": ["symbol", "side"]
        }),
    };

    let mut cs = ConstraintSet::defaults();
    cs.apply_artifact("kelly", 5.0, 2.0, 15.0);
    cs.intersect_user(3.0, 1.0, 10.0);

    ToolConstrainer::constrain_signal_tool(&mut tool, &cs);

    let leverage_max = tool.parameters["properties"]["leverage"]["maximum"].as_u64().unwrap();
    assert_eq!(leverage_max, 3, "leverage max should be 3 after constraint");

    // Description should mention proof-backed constraints
    assert!(tool.description.contains("Max leverage"), "description should mention leverage");
}

#[test]
fn tool_constrainer_enforces_stop_loss() {
    let mut tool = ToolDef {
        name: "submit_signal".into(),
        description: "Submit a signal".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "stop_loss": {"type": "number"}
            },
            "required": ["symbol", "side"]
        }),
    };

    let mut cs = ConstraintSet::defaults();
    cs.stop_loss_required = true;

    ToolConstrainer::constrain_signal_tool(&mut tool, &cs);

    let required: Vec<String> = tool.parameters["required"]
        .as_array().unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(required.contains(&"stop_loss".to_string()), "stop_loss should be required");
}

#[test]
fn validator_rejects_missing_required_proof() {
    // Test that a strategy referencing a missing proof gets flagged
    // We're testing the validation logic, not the full registry integration
    use testudo_cli::strategies::template::StrategyTemplate;

    let tmpl = StrategyTemplate {
        meta: testudo_cli::strategies::template::StrategyMeta {
            name: "test-strat".into(),
            version: "1.0".into(),
            description: "test".into(),
        },
        loop_config: None,
        prompt: testudo_cli::strategies::template::StrategyPrompt {
            system: "test".into(),
        },
        parameters: None,
        constraints: None,
        allowed_tools: None,
        required_proofs: vec!["nonexistent-proof".into()],
    };

    let artifacts = std::collections::HashMap::new();
    let result = testudo_cli::strategies::validator::StrategyValidator::validate(
        &tmpl, &artifacts,
    );
    assert!(!result.valid, "should be invalid when required proof missing");
    assert!(result.errors.iter().any(|e| e.contains("nonexistent-proof")));
}

#[test]
fn validator_passes_when_all_proofs_present() {
    use testudo_cli::strategies::loader::StrategyArtifact;
    use testudo_cli::strategies::template::StrategyTemplate;

    let mut artifacts = std::collections::HashMap::new();
    artifacts.insert("kelly".into(), StrategyArtifact {
        meta: testudo_cli::strategies::loader::ArtifactMeta {
            name: "kelly".into(), version: "1.0".into(),
            description: "".into(), lean_file: "".into(),
        },
        theorem: testudo_cli::strategies::loader::TheoremRef {
            name: "test".into(), statement: "".into(), formula: "".into(),
            implications: vec![], lean_line: 1,
        },
        constraints: std::collections::HashMap::new(),
        prompt: testudo_cli::strategies::loader::PromptSection::default(),
    });

    let tmpl = StrategyTemplate {
        meta: testudo_cli::strategies::template::StrategyMeta {
            name: "test".into(), version: "1.0".into(), description: "".into(),
        },
        loop_config: None,
        prompt: testudo_cli::strategies::template::StrategyPrompt { system: "".into() },
        parameters: None, constraints: None, allowed_tools: None,
        required_proofs: vec!["kelly".into()],
    };

    let result = testudo_cli::strategies::validator::StrategyValidator::validate(
        &tmpl, &artifacts,
    );
    assert!(result.valid, "should be valid when all proofs present");
    assert!(result.errors.is_empty());
}

use testudo_cli::cmd::run_strategy_validate;

#[test]
fn validate_builtin_strategy_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let result = run_strategy_validate(tmp.path(), "mean-reversion");
    assert!(result.is_ok(), "validating a builtin should succeed");
}

#[test]
fn validate_nonexistent_strategy_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let result = run_strategy_validate(tmp.path(), "nonexistent");
    assert!(result.is_err(), "validating nonexistent should fail");
}
