// @anchor infra:cli:view:status
// @tags ui

//! Bottom status bar: version, mode, ticker, uptime, key hints.

use crate::model::state::AppState;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Render the bottom status bar.
pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;

    let uptime_str = format_uptime(state.status.uptime_secs);

    let line = Line::from(vec![
        Span::styled(" testudo ", Style::default().fg(theme.accent)),
        Span::styled(
            state.status.version.as_str(),
            Style::default().fg(theme.status_bar_fg),
        ),
        Span::styled(" │ ", Style::default().fg(theme.muted)),
        Span::styled(
            state.status.mode.as_str(),
            Style::default().fg(theme.info),
        ),
        Span::styled(" │ ", Style::default().fg(theme.muted)),
        Span::styled(
            state.status.last_ticker.as_str(),
            Style::default().fg(theme.status_bar_fg),
        ),
        Span::styled(" │ ", Style::default().fg(theme.muted)),
        Span::styled(
            uptime_str.as_str(),
            Style::default().fg(theme.status_bar_fg),
        ),
        Span::styled(" │ ", Style::default().fg(theme.muted)),
        Span::styled("F1 ", Style::default().fg(theme.help_key)),
        Span::styled("Dash ", Style::default().fg(theme.help_desc)),
        Span::styled("F2 ", Style::default().fg(theme.help_key)),
        Span::styled("Jnl ", Style::default().fg(theme.help_desc)),
        Span::styled("F3 ", Style::default().fg(theme.help_key)),
        Span::styled("Strats ", Style::default().fg(theme.help_desc)),
        Span::styled("F4 ", Style::default().fg(theme.help_key)),
        Span::styled("Logs ", Style::default().fg(theme.help_desc)),
        Span::styled("q ", Style::default().fg(theme.help_key)),
        Span::styled("Quit", Style::default().fg(theme.help_desc)),
    ]);

    let p = Paragraph::new(line).style(
        Style::default()
            .fg(theme.status_bar_fg)
            .bg(theme.status_bar_bg),
    );

    // Fill entire status bar width with bg color
    frame.render_widget(
        ratatui::widgets::Block::default()
            .style(Style::default().bg(theme.status_bar_bg)),
        area,
    );
    frame.render_widget(p, area);
}

fn format_uptime(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    format!("{}h {}m", hours, minutes)
}
