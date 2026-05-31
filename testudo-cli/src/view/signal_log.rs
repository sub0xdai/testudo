// @anchor infra:cli:view:signal
// @tags ui

//! Recent signals log pane.

use crate::model::state::SignalEntry;
use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, signals: &[SignalEntry]) {
    let block = Block::default()
        .title(" Signal Log ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title_style(Style::default().fg(theme.accent));

    if signals.is_empty() {
        let p = Paragraph::new("No signals yet")
            .block(block)
            .style(Style::default().fg(theme.muted));
        frame.render_widget(p, area);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    let max_entries = area.height.saturating_sub(2) as usize;

    for entry in signals.iter().rev().take(max_entries) {
        let status_icon = match entry.status.as_str() {
            "filled" => Span::styled("✓", Style::default().fg(theme.signal_filled)),
            "rejected" => Span::styled("✗", Style::default().fg(theme.signal_rejected)),
            _ => Span::styled("⟳", Style::default().fg(theme.signal_pending)),
        };

        let mut spans = vec![
            Span::styled(
                format!("{} ", entry.timestamp),
                Style::default().fg(theme.muted),
            ),
            status_icon,
            Span::styled(
                format!(" {} {} {}", entry.symbol, entry.side, entry.status),
                Style::default().fg(theme.fg),
            ),
        ];

        if let Some(ref pnl) = entry.pnl {
            let pnl_val: f64 = pnl.parse().unwrap_or(0.0);
            let pnl_color = if pnl_val >= 0.0 {
                theme.pnl_positive
            } else {
                theme.pnl_negative
            };
            spans.push(Span::styled(
                format!(" ({})", pnl),
                Style::default().fg(pnl_color),
            ));
        }

        lines.push(Line::from(spans));

        // Show reasoning on next line if it fits
        if !entry.reasoning.is_empty() && lines.len() < max_entries {
            let reason = if entry.reasoning.len() > 60 {
                format!("  └ {}", &entry.reasoning[..57])
            } else {
                format!("  └ {}", entry.reasoning)
            };
            lines.push(Line::from(Span::styled(
                reason,
                Style::default().fg(theme.dim_fg),
            )));
        }
    }

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}
