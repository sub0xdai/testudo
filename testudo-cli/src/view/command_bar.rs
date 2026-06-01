// @anchor infra:cli:view:command_bar
// @tags ui

//! Command bar widget — renders the slash-command input line.

use crate::model::state::AppState;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Render the command bar below the main content area.
/// Only renders when `state.command_mode` is true.
pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;

    let cursor_char = "█";
    let input_with_cursor = format!("{}{}", state.command_input, cursor_char);

    let hint = if state.command_input.len() <= 1 {
        "Type a command (Tab to complete, Esc to cancel)"
    } else {
        ""
    };

    let text = vec![
        Line::from(vec![
            Span::styled(input_with_cursor, Style::default().fg(theme.fg)),
        ]),
        Line::from(vec![
            Span::styled(hint, Style::default().fg(theme.dim_fg)),
        ]),
    ];

    let para = Paragraph::new(text);
    frame.render_widget(para, area);
}
