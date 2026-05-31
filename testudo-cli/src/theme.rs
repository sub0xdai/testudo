// @anchor infra:cli:theme
// @tags ui

//! Color palette for the entire TUI.
//! All panes and widgets reference this struct — no hardcoded colors in view code.

use ratatui::style::Color;

/// Color palette for the entire TUI.
/// All panes and widgets reference this struct — no hardcoded colors in view code.
#[derive(Debug, Clone)]
pub struct Theme {
    // ── Canvas ──
    pub bg: Color,
    pub fg: Color,
    pub dim_fg: Color,

    // ── Borders & separators ──
    pub border: Color,
    pub border_focused: Color,

    // ── Semantic colors ──
    pub accent: Color,
    pub success: Color,
    pub danger: Color,
    pub warning: Color,
    pub info: Color,
    pub muted: Color,

    // ── Pane-specific ──
    pub positions_header: Color,
    pub positions_long: Color,
    pub positions_short: Color,
    pub pnl_positive: Color,
    pub pnl_negative: Color,
    pub signal_filled: Color,
    pub signal_rejected: Color,
    pub signal_pending: Color,
    pub risk_gauge_fill: Color,
    pub risk_gauge_bg: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,

    // ── TUI chrome ──
    pub help_key: Color,
    pub help_desc: Color,
    pub input_cursor: Color,
}

impl Theme {
    /// Vanilla Amoled — true black background, desaturated pastel accents.
    /// Background: pure AMOLED black (#000000). Text: light gray.
    /// Accent: faint blue. Semantic: muted green/red/yellow.
    /// Matches the pi.dev `vanilla-amoled` theme for visual consistency.
    pub fn vanilla_amoled() -> Self {
        Self {
            bg: Color::Rgb(0, 0, 0),
            fg: Color::Rgb(187, 187, 187),
            dim_fg: Color::Rgb(102, 102, 102),

            border: Color::Rgb(74, 74, 74),
            border_focused: Color::Rgb(102, 102, 102),

            accent: Color::Rgb(138, 154, 184),
            success: Color::Rgb(129, 168, 134),
            danger: Color::Rgb(179, 128, 128),
            warning: Color::Rgb(179, 168, 112),
            info: Color::Rgb(122, 154, 154),
            muted: Color::Rgb(153, 153, 153),

            positions_header: Color::Rgb(138, 154, 184),
            positions_long: Color::Rgb(129, 168, 134),
            positions_short: Color::Rgb(179, 128, 128),
            pnl_positive: Color::Rgb(129, 168, 134),
            pnl_negative: Color::Rgb(179, 128, 128),
            signal_filled: Color::Rgb(129, 168, 134),
            signal_rejected: Color::Rgb(179, 128, 128),
            signal_pending: Color::Rgb(179, 168, 112),
            risk_gauge_fill: Color::Rgb(129, 168, 134),
            risk_gauge_bg: Color::Rgb(51, 51, 51),
            status_bar_bg: Color::Rgb(8, 8, 8),
            status_bar_fg: Color::Rgb(153, 153, 153),

            help_key: Color::Rgb(138, 154, 184),
            help_desc: Color::Rgb(187, 187, 187),
            input_cursor: Color::Rgb(138, 154, 184),
        }
    }

    /// Load theme from config name. Currently only "vanilla-amoled" exists.
    /// Future: "kanso-ink", "tokyo-night", "nord", "solarized-dark".
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "vanilla-amoled" => Self::vanilla_amoled(),
            other => {
                tracing::warn!(
                    "Unknown theme '{}', falling back to vanilla-amoled",
                    other
                );
                Self::vanilla_amoled()
            }
        }
    }
}
