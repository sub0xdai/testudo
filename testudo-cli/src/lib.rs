// @anchor infra:cli:main
// @tags infra, api

//! Testudo trading agent harness.
//!
//! TUI + CLI for autonomous trading: onboard, run strategies, monitor
//! positions, journal results. Consumes the Testudo REST + WebSocket API.

pub mod cmd;
pub use cmd::{run_agent, run_journal, run_listen, AgentAction, Command, StrategyAction};

pub mod config;
pub mod theme;

pub mod model {
    pub mod agent;
    pub mod journal;
    pub mod market;
    pub mod positions;
    pub mod session;
    pub mod state;
}

pub use model::agent::{AgentMode, AgentPhase, AgentState};

pub mod msg;
pub mod update;

pub mod app;
mod auth;

mod view {
    pub mod agent_pane;
    pub mod dashboard;
    pub mod journal_pane;
    pub mod pnl_chart;
    pub mod positions_pane;
    pub mod risk_pane;
    pub mod signal_log;
    pub mod status_bar;
}

pub mod tools;
pub use tools::all_tools;
pub use tools::types::ToolDef;

pub mod llm {
    pub mod anthropic;
    pub mod client;
    pub mod gemini;
    pub mod ollama;
    pub mod openai;
    pub mod stream;
    pub mod types;
}

pub mod api {
    pub mod agent_keys;
    pub mod client;
    pub mod exchanges;
    pub mod journal;
    pub mod klines;
    pub mod onboarding;
    pub mod risk;
    pub mod signals;
    pub mod types;
}

pub mod ws {
    pub mod client;
    pub mod stream;
}

mod strategies {
    pub mod registry;
    pub mod template;
}

mod risk {
    pub mod precheck;
}

mod daemon;
