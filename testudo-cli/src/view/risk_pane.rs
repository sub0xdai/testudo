// @anchor infra:cli:view:risk
// @tags ui

//! Risk pane — drawdown gauge, position counter, signal counter.

use crate::model::state::RiskSnapshot;
use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, risk: &RiskSnapshot) {
    let block = Block::default()
        .title(" Risk ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title_style(Style::default().fg(theme.accent));

    let drawdown_bar = progress_bar(risk.drawdown_pct, risk.drawdown_limit_pct, 20);
    let dd_color = if risk.drawdown_pct > risk.drawdown_limit_pct * 0.8 {
        theme.danger
    } else if risk.drawdown_pct > risk.drawdown_limit_pct * 0.5 {
        theme.warning
    } else {
        theme.success
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Drawdown: ", Style::default().fg(theme.fg)),
            Span::styled(drawdown_bar, Style::default().fg(dd_color)),
            Span::styled(
                format!(
                    " {:.1}% / {:.1}%",
                    risk.drawdown_pct, risk.drawdown_limit_pct
                ),
                Style::default().fg(theme.dim_fg),
            ),
        ]),
        Line::from(vec![Span::styled(
            format!(
                "Active: {}/{} positions",
                risk.active_positions, risk.max_positions
            ),
            Style::default().fg(theme.fg),
        )]),
        Line::from(vec![Span::styled(
            format!(
                "Session: {}/{} signals",
                risk.session_signals, risk.max_signals_per_hour
            ),
            Style::default().fg(theme.fg),
        )]),
        Line::from(vec![Span::styled(
            format!("Exposure: ${}", risk.total_exposure),
            Style::default().fg(theme.fg),
        )]),
    ];

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn progress_bar(value: f64, max: f64, width: usize) -> String {
    let ratio = (value / max).clamp(0.0, 1.0);
    let filled = (ratio * width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
