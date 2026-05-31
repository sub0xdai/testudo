// @anchor infra:cli:view:journal
// @tags ui

//! Journal summary pane — key trading stats.

use crate::model::state::JournalSummary;
use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, summary: &JournalSummary) {
    let block = Block::default()
        .title(" Journal Summary ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title_style(Style::default().fg(theme.accent));

    let pnl_val: f64 = summary.total_pnl.parse().unwrap_or(0.0);
    let pnl_color = if pnl_val >= 0.0 {
        theme.pnl_positive
    } else {
        theme.pnl_negative
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Trades: ", Style::default().fg(theme.dim_fg)),
            Span::styled(
                summary.trade_count.to_string(),
                Style::default().fg(theme.fg),
            ),
            Span::styled(
                format!("  WR: {}%", summary.win_rate),
                Style::default().fg(theme.success),
            ),
        ]),
        Line::from(vec![
            Span::styled("PF: ", Style::default().fg(theme.dim_fg)),
            Span::styled(&summary.profit_factor, Style::default().fg(theme.fg)),
            Span::styled(
                format!("  Avg R: {}", summary.avg_r_multiple),
                Style::default().fg(theme.fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("P&L: ", Style::default().fg(theme.dim_fg)),
            Span::styled(&summary.total_pnl, Style::default().fg(pnl_color)),
        ]),
        Line::from(vec![
            Span::styled("Best: ", Style::default().fg(theme.dim_fg)),
            Span::styled(&summary.best_setup, Style::default().fg(theme.accent)),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}
