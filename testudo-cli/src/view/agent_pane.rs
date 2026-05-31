// @anchor infra:cli:view:agent
// @tags ui

//! LLM reasoning stream pane.

use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, reasoning: &str) {
    let block = Block::default()
        .title(" Agent Reasoning ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title_style(Style::default().fg(theme.accent));

    if reasoning.is_empty() {
        let p = Paragraph::new("Agent not running — start with:\n  testudo agent start")
            .block(block)
            .style(Style::default().fg(theme.muted));
        frame.render_widget(p, area);
        return;
    }

    // Show last N lines that fit
    let max_lines = area.height.saturating_sub(2) as usize;
    let text_lines: Vec<&str> = reasoning.lines().collect();
    let visible = if text_lines.len() > max_lines {
        &text_lines[text_lines.len() - max_lines..]
    } else {
        &text_lines[..]
    };

    let lines: Vec<Line> = visible
        .iter()
        .map(|l| Line::from(Span::styled(*l, Style::default().fg(theme.fg))))
        .collect();

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}
