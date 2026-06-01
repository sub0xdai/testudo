// @anchor infra:cli:view:dashboard
// @tags ui

//! Full dashboard layout compositor.

use crate::model::state::{AppState, Screen};
use crate::view::{command_bar, status_bar};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Text,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the full dashboard (6 panes + optional command bar + status bar).
pub fn render(frame: &mut Frame, state: &AppState) {
    let theme = &state.theme;

    // Reserve space for status bar (1 row) + optional command bar (2 rows)
    let bottom_reserved = if state.command_mode { 3 } else { 1 };
    let content_area = Rect {
        height: frame.area().height.saturating_sub(bottom_reserved),
        ..frame.area()
    };

    match state.screen {
        Screen::Dashboard => render_dashboard(frame, state, content_area),
        Screen::Help => render_help(frame, theme, content_area),
        Screen::Settings => crate::view::settings::render(frame, theme, content_area),
        other => render_placeholder(frame, theme, other, content_area),
    }

    // Command bar (above status bar, only when active)
    if state.command_mode {
        let cmd_area = Rect {
            y: content_area.y + content_area.height,
            height: 2,
            ..frame.area()
        };
        command_bar::render(frame, state, cmd_area);
    }

    // Status bar always visible at the absolute bottom
    let status_y = frame.area().y + frame.area().height.saturating_sub(1);
    let status_area = Rect {
        y: status_y,
        height: 1,
        ..frame.area()
    };
    status_bar::render(frame, state, status_area);
}

fn render_dashboard(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;

    // 3-row × 2-col grid
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    let top_cols = horizontal_split(rows[0]);
    let mid_cols = horizontal_split(rows[1]);
    let bot_cols = horizontal_split(rows[2]);

    // Top-left: Positions (live data)
    crate::view::positions_pane::render(
        frame, top_cols[0], theme, &state.positions,
    );
    // Top-right: Agent Reasoning
    crate::view::agent_pane::render(frame, top_cols[1], theme, "");
    // Mid-left: P&L Chart (live sparkline)
    crate::view::pnl_chart::render(
        frame, mid_cols[0], theme, &state.equity_curve,
    );
    // Mid-right: Signal Log (live data)
    crate::view::signal_log::render(
        frame, mid_cols[1], theme, &state.signal_log,
    );
    // Bottom-left: Journal Summary
    if let Some(ref summary) = state.journal_summary {
        crate::view::journal_pane::render(
            frame, bot_cols[0], theme, summary,
        );
    } else {
        render_pane(frame, bot_cols[0], theme, "Journal Summary", "No journal entries");
    }
    // Bottom-right: Risk
    if let Some(ref risk) = state.risk_snapshot {
        crate::view::risk_pane::render(
            frame, bot_cols[1], theme, risk,
        );
    } else {
        render_pane(frame, bot_cols[1], theme, "Risk", "No risk data");
    }
}

fn render_placeholder(frame: &mut Frame, theme: &crate::theme::Theme, screen: Screen, area: Rect) {
    let label = match screen {
        Screen::Journal => "Journal",
        Screen::Strategies => "Strategies",
        Screen::Logs => "Logs",
        Screen::Settings => "Settings",
        _ => "Unknown",
    };
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
    frame.render_widget(p, area);
}

fn render_help(frame: &mut Frame, theme: &crate::theme::Theme, area: Rect) {
    let help_text = [
        "  F1          Dashboard",
        "  F2          Journal",
        "  F3          Strategies",
        "  F4          Logs",
        "  / or :      Command palette",
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
    frame.render_widget(p, area);
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
