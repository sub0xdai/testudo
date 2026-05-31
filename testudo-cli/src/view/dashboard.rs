// @anchor infra:cli:view:dashboard
// @tags ui

//! Full dashboard layout compositor.

use crate::model::state::{AppState, Screen};
use crate::view::status_bar;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Text,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the full dashboard (6 panes + status bar) or a placeholder screen.
pub fn render(frame: &mut Frame, state: &AppState) {
    let theme = &state.theme;

    match state.screen {
        Screen::Dashboard => render_dashboard(frame, state),
        Screen::Help => render_help(frame, theme),
        other => render_placeholder(frame, theme, other),
    }

    // Status bar always visible
    let status_area = status_bar_area(frame.area());
    status_bar::render(frame, state, status_area);
}

fn render_dashboard(frame: &mut Frame, state: &AppState) {
    let theme = &state.theme;
    let main_area = main_area(frame.area());

    // 3-row × 2-col grid
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(main_area);

    let top_cols = horizontal_split(rows[0]);
    let mid_cols = horizontal_split(rows[1]);
    let bot_cols = horizontal_split(rows[2]);

    render_pane(frame, top_cols[0], theme, "Positions", "No open positions");
    render_pane(frame, top_cols[1], theme, "Agent Reasoning", "Agent not running");
    render_pane(frame, mid_cols[0], theme, "P&L Chart", "No data");
    render_pane(frame, mid_cols[1], theme, "Signal Log", "No signals yet");
    render_pane(frame, bot_cols[0], theme, "Journal Summary", "No journal entries");
    render_pane(frame, bot_cols[1], theme, "Risk", "No risk data");
}

fn render_placeholder(frame: &mut Frame, theme: &crate::theme::Theme, screen: Screen) {
    let label = match screen {
        Screen::Journal => "Journal",
        Screen::Strategies => "Strategies",
        Screen::Logs => "Logs",
        _ => "Unknown",
    };
    let main = main_area(frame.area());
    let text = format!("{} — Not yet implemented", label);
    let p = Paragraph::new(Text::from(text.as_str()))
        .block(
            Block::default()
                .title(format!(" {} ", label))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .title_style(Style::default().fg(theme.accent)),
        )
        .style(Style::default().fg(theme.muted));
    frame.render_widget(p, main);
}

fn render_help(frame: &mut Frame, theme: &crate::theme::Theme) {
    let main = main_area(frame.area());
    let help_text = [
        "  F1          Dashboard",
        "  F2          Journal",
        "  F3          Strategies",
        "  F4          Logs",
        "  ?           This help",
        "  q / Esc     Quit",
    ];
    let p = Paragraph::new(Text::from(help_text.join("\n")))
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .title_style(Style::default().fg(theme.accent)),
        )
        .style(Style::default().fg(theme.fg));
    frame.render_widget(p, main);
}

fn render_pane(
    frame: &mut Frame,
    area: Rect,
    theme: &crate::theme::Theme,
    title: &str,
    placeholder: &str,
) {
    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title_style(Style::default().fg(theme.accent));

    let text = Text::from(placeholder);
    let p = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(theme.muted));

    frame.render_widget(p, area);
}

fn horizontal_split(area: Rect) -> [Rect; 2] {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);
    [cols[0], cols[1]]
}

/// Main area with room at bottom for status bar.
fn main_area(full: Rect) -> Rect {
    Rect {
        height: full.height.saturating_sub(1),
        ..full
    }
}

/// Bottom row for status bar.
fn status_bar_area(full: Rect) -> Rect {
    Rect {
        y: full.y + full.height.saturating_sub(1),
        height: 1,
        ..full
    }
}
