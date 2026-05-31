// @anchor test:cli:parsing
// @tags infra

use clap::Parser;
use testudo_cli::Command;

#[test]
fn parse_dashboard_command() {
    let cmd = Command::try_parse_from(["testudo", "dashboard"]);
    assert!(cmd.is_ok(), "dashboard should parse");
    assert!(matches!(cmd.unwrap(), Command::Dashboard));
}

#[test]
fn parse_agent_start_command() {
    let cmd = Command::try_parse_from(["testudo", "agent", "start"]);
    assert!(cmd.is_ok(), "agent start should parse");
    assert!(matches!(cmd.unwrap(), Command::Agent(_)));
}

#[test]
fn parse_init_command() {
    let cmd = Command::try_parse_from(["testudo", "init"]);
    assert!(cmd.is_ok(), "init should parse");
    assert!(matches!(cmd.unwrap(), Command::Init));
}

#[test]
fn parse_listen_command() {
    let cmd = Command::try_parse_from(["testudo", "listen"]);
    assert!(cmd.is_ok(), "listen should parse");
    assert!(matches!(cmd.unwrap(), Command::Listen));
}

#[test]
fn parse_journal_command() {
    let cmd = Command::try_parse_from(["testudo", "journal"]);
    assert!(cmd.is_ok(), "journal should parse");
    assert!(matches!(cmd.unwrap(), Command::Journal));
}

#[test]
fn parse_strategy_list_command() {
    let cmd = Command::try_parse_from(["testudo", "strategy", "list"]);
    assert!(cmd.is_ok(), "strategy list should parse");
    assert!(matches!(cmd.unwrap(), Command::Strategy(_)));
}

#[test]
fn parse_attach_command() {
    let cmd = Command::try_parse_from(["testudo", "attach"]);
    assert!(cmd.is_ok(), "attach should parse");
    assert!(matches!(cmd.unwrap(), Command::Attach));
}

#[test]
fn unknown_subcommand_fails() {
    let cmd = Command::try_parse_from(["testudo", "nonexistent"]);
    assert!(cmd.is_err(), "unknown subcommand should fail");
}

#[test]
fn all_subcommands_have_descriptive_help() {
    let cmd = Command::try_parse_from(["testudo", "--help"]);
    assert!(cmd.is_err(), "--help triggers help output and exits");
}
