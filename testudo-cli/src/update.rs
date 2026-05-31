// @anchor infra:cli:update
// @tags infra

//! Pure update function: (&mut Model, Message) → bool (continue).

use crate::model::state::{AppState, Screen};
use crate::msg::Message;
use crossterm::event::KeyCode;

/// Apply a message to the model. Returns false if the app should quit.
pub fn update(state: &mut AppState, msg: Message) -> bool {
    match msg {
        Message::KeyPress(key) => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return false,
            KeyCode::F(1) => state.screen = Screen::Dashboard,
            KeyCode::F(2) => state.screen = Screen::Journal,
            KeyCode::F(3) => state.screen = Screen::Strategies,
            KeyCode::F(4) => state.screen = Screen::Logs,
            KeyCode::Char('?') => state.screen = Screen::Help,
            _ => {}
        },
        Message::Resize(_cols, _rows) => {
            // Terminal size stored for future use; no-op for now.
        }
        Message::Tick => {
            state.status.uptime_secs = state.status.uptime_secs.saturating_add(1);
        }
        Message::SwitchScreen(screen) => {
            state.screen = screen;
        }
        Message::ShowHelp => {
            state.screen = Screen::Help;
        }
        Message::Quit => return false,
        Message::Error(err) => {
            state.error = Some(err);
        }
        Message::ClearError => {
            state.error = None;
        }
    }
    true
}
