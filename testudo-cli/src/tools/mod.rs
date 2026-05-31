// @anchor infra:cli:tools:mod
// @tags api

//! Tool registry — all available LLM tool definitions.

pub mod check_onboarding;
pub mod check_risk;
pub mod fetch_klines;
pub mod list_positions;
pub mod read_journal;
pub mod submit_signal;
pub mod types;
pub mod write_journal;

use types::ToolDef;

/// Return all 7 tool definitions available to the LLM agent.
pub fn all_tools() -> Vec<ToolDef> {
    vec![
        fetch_klines::tool_def(),
        submit_signal::tool_def(),
        read_journal::tool_def(),
        write_journal::tool_def(),
        list_positions::tool_def(),
        check_risk::tool_def(),
        check_onboarding::tool_def(),
    ]
}
