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
    pub equity_curve: Vec<PnlPoint>,
    pub risk_snapshot: Option<RiskSnapshot>,
    pub journal_summary: Option<JournalSummary>,
    /// True when the command bar is active (pressed / or :)
    pub command_mode: bool,
    /// Current text in the command input buffer
    pub command_input: String,
    /// History of executed commands (last 20)
    pub command_history: Vec<String>,
    /// Current position in history when navigating (None = at tip)
    pub command_history_idx: Option<usize>,
    /// Flash error message shown briefly in the command bar
    pub command_error: Option<String>,
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

/// Daily P&L point for the equity curve sparkline.
#[derive(Debug, Clone)]
pub struct PnlPoint {
    pub date: String,
    pub cumulative_pnl: String,
    pub equity: Option<String>,
}

/// Risk metrics displayed in the risk pane.
#[derive(Debug, Clone)]
pub struct RiskSnapshot {
    pub drawdown_pct: f64,
    pub drawdown_limit_pct: f64,
    pub active_positions: usize,
    pub max_positions: usize,
    pub session_signals: u32,
    pub max_signals_per_hour: u32,
    pub total_exposure: String,
}

/// Journal summary stats for the journal pane.
#[derive(Debug, Clone)]
pub struct JournalSummary {
    pub trade_count: i64,
    pub win_rate: String,
    pub profit_factor: String,
    pub avg_r_multiple: String,
    pub total_pnl: String,
    pub best_setup: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    Dashboard,
    Journal,
    Strategies,
    Logs,
    Help,
    Settings,
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
