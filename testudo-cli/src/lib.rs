// @anchor infra:cli:main
// @tags infra, api

//! Testudo trading agent harness.
//!
//! TUI + CLI for autonomous trading: onboard, run strategies, monitor
//! positions, journal results. Consumes the Testudo REST + WebSocket API.

mod app;
mod auth;
mod cmd;
mod config;
mod msg;
mod update;

mod model {
    pub mod agent;
    pub mod journal;
    pub mod market;
    pub mod positions;
    pub mod session;
    pub mod state;
}

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

mod tools {
    pub mod check_onboarding;
    pub mod check_risk;
    pub mod fetch_klines;
    pub mod list_positions;
    pub mod read_journal;
    pub mod submit_signal;
    pub mod types;
    pub mod write_journal;
}

mod llm {
    pub mod anthropic;
    pub mod client;
    pub mod gemini;
    pub mod ollama;
    pub mod openai;
    pub mod stream;
}

mod api {
    pub mod agent_keys;
    pub mod client;
    pub mod exchanges;
    pub mod journal;
    pub mod klines;
    pub mod onboarding;
    pub mod risk;
    pub mod signals;
}

mod ws {
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
