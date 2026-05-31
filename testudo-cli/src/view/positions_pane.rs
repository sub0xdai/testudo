// @anchor infra:cli:view:positions
// @tags ui

//! Open positions table pane.

use crate::model::state::Position;
use crate::theme::Theme;
use ratatui::{
    layout::{Constraint, Rect},
    style::Style,
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, positions: &[Position]) {
    let block = Block::default()
        .title(" Positions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title_style(Style::default().fg(theme.accent));

    if positions.is_empty() {
        let p = ratatui::widgets::Paragraph::new("No open positions")
            .block(block)
            .style(Style::default().fg(theme.muted));
        frame.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["Symbol", "Side", "Entry", "Current", "P&L", "Qty"])
        .style(Style::default().fg(theme.positions_header));

    let rows: Vec<Row> = positions
        .iter()
        .map(|p| {
            let pnl_val: f64 = p.unrealized_pnl.parse().unwrap_or(0.0);
            let pnl_color = if pnl_val >= 0.0 {
                theme.pnl_positive
            } else {
                theme.pnl_negative
            };

            Row::new(vec![
                Cell::from(p.symbol.as_str()),
                Cell::from(p.side.as_str()),
                Cell::from(format!("${}", p.entry_price)),
                Cell::from(format!("${}", p.current_price)),
                Cell::from(format!("${}", p.unrealized_pnl))
                    .style(Style::default().fg(pnl_color)),
                Cell::from(p.quantity.as_str()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(12),
        Constraint::Length(6),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(8),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1);

    frame.render_widget(table, area);
}
