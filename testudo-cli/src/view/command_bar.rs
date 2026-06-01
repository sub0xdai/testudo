// @anchor infra:cli:view:command_bar
// @tags ui

//! Command bar widget — renders the slash-command input line with autocomplete hint.

use crate::commands;
use crate::model::state::AppState;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Render the command bar below the main content area.
pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;

    let cursor_char = "█";

    // Find autocomplete ghost text
    let ghost = if state.command_input.len() > 1 {
        let matches = commands::autocomplete(&state.command_input);
        if matches.len() == 1 {
            let full = &matches[0];
            if full.len() > state.command_input.len() {
                Some(full[state.command_input.len()..].to_string())
            } else {
                None
            }
        } else if matches.len() > 1 {
            // Show count of matches
            Some(format!("  [{} matches]", matches.len()))
        } else {
            None
        }
    } else {
        None
    };

    let input_line = if let Some(ref g) = ghost {
        Line::from(vec![
            Span::styled(&state.command_input, Style::default().fg(theme.fg)),
            Span::styled(g, Style::default().fg(theme.dim_fg)),
            Span::styled(cursor_char, Style::default().fg(theme.accent)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!("{}{}", state.command_input, cursor_char),
                Style::default().fg(theme.fg),
            ),
        ])
    };

    let hint_text = if state.command_input.len() <= 1 {
        "Type a command (Tab to complete, Esc to cancel)"
    } else if commands::autocomplete(&state.command_input).is_empty() {
        "Unknown command"
    } else {
        ""
    };

    let text = vec![
        input_line,
        Line::from(vec![
            Span::styled(hint_text, Style::default().fg(theme.dim_fg)),
        ]),
    ];

    let para = Paragraph::new(text);
    frame.render_widget(para, area);
}
