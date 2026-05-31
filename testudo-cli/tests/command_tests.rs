// @anchor test:cli:commands
// @tags api

use testudo_cli::cmd::{run_journal, run_listen};
use testudo_cli::config::{ApiConfig, Config};

fn make_config_no_key() -> Config {
    Config {
        api: ApiConfig {
            base_url: "http://localhost:8080/api/v1".into(),
            agent_key: "".into(),
            ws_url: "ws://localhost:8081".into(),
        },
        ..Config::default()
    }
}

#[test]
fn run_journal_rejects_empty_agent_key() {
    let config = make_config_no_key();
    let result = run_journal(&config);
    assert!(result.is_err(), "journal should fail with empty agent key");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("agent key") || err.contains("Agent key"),
        "error should mention agent key, got: {}",
        err
    );
}

#[test]
fn listen_rejects_empty_agent_key() {
    let config = make_config_no_key();
    let result = run_listen(&config);
    assert!(result.is_err(), "listen should fail with empty agent key");
}
