// @anchor infra:cli:model:state
// @tags infra

//! Top-level application state (Elm Model).

use crate::theme::Theme;

/// The root model for the TEA loop — all UI state lives here.
#[derive(Debug, Clone)]
pub struct AppState {
    pub screen: Screen,
    pub status: StatusBar,
    pub theme: Theme,
    pub error: Option<String>,
    pub positions: Vec<Position>,
    pub signal_log: Vec<SignalEntry>,
}

/// A trading position displayed in the positions pane.
#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: String,
    pub side: String,
    pub entry_price: String,
    pub current_price: String,
    pub unrealized_pnl: String,
    pub quantity: String,
}

/// A signal entry displayed in the signal log pane.
#[derive(Debug, Clone)]
pub struct SignalEntry {
    pub timestamp: String,
    pub symbol: String,
    pub side: String,
    pub status: String,
    pub pnl: Option<String>,
    pub reasoning: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    Dashboard,
    Journal,
    Strategies,
    Logs,
    Help,
}

#[derive(Debug, Clone)]
pub struct StatusBar {
    pub version: String,
    pub mode: String,
    pub last_ticker: String,
    pub uptime_secs: u64,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self {
            version: "v0.1.0".into(),
            mode: "SHADOW".into(),
            last_ticker: "ETH: $—".into(),
            uptime_secs: 0,
        }
    }
}

impl StatusBar {
    pub fn new() -> Self {
        Self::default()
    }
}
