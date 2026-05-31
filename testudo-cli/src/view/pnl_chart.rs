// @anchor infra:cli:view:pnl
// @tags ui

//! P&L sparkline pane — ASCII equity curve.

use crate::model::state::PnlPoint;
use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[allow(clippy::needless_range_loop)]
pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, points: &[PnlPoint]) {
    let block = Block::default()
        .title(" P&L Chart ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title_style(Style::default().fg(theme.accent));

    if points.len() < 2 {
        let msg = if points.is_empty() {
            "No data — start trading to build equity curve"
        } else {
            "Insufficient data — need 2+ data points"
        };
        let p = Paragraph::new(msg)
            .block(block)
            .style(Style::default().fg(theme.muted));
        frame.render_widget(p, area);
        return;
    }

    let height = area.height.saturating_sub(2) as usize;
    let width = area.width.saturating_sub(2) as usize;
    if height == 0 || width == 0 {
        return;
    }

    // Find min/max for scaling
    let values: Vec<f64> = points.iter().map(|p| p.cumulative_pnl.parse().unwrap_or(0.0)).collect();
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = (max - min).max(1.0);

    // Build sparkline rows
    let mut grid = vec![vec![' '; width]; height];
    let step = (values.len() as f64 / width as f64).max(1.0);

    for col in 0..width {
        let idx = ((col as f64) * step) as usize;
        if idx >= values.len() {
            break;
        }
        let normalized = ((values[idx] - min) / range * (height - 1) as f64) as usize;
        let row = height - 1 - normalized;
        grid[row][col] = '█';
    }

    let lines: Vec<Line> = grid
        .into_iter()
        .map(|row| {
            let s: String = row.into_iter().collect();
            let pnl_color = if min >= 0.0 {
                theme.pnl_positive
            } else if max <= 0.0 {
                theme.pnl_negative
            } else {
                theme.fg
            };
            Line::from(Span::styled(s, Style::default().fg(pnl_color)))
        })
        .collect();

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}
