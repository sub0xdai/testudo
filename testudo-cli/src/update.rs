// @anchor infra:cli:update
// @tags infra

//! Pure update function: (&mut Model, Message) → bool (continue).

use crate::commands::{self, TuiCommand};
use crate::model::state::{AppState, Screen};
use crate::msg::Message;
use crossterm::event::KeyCode;

/// Apply a message to the model. Returns false if the app should quit.
pub fn update(state: &mut AppState, msg: Message) -> bool {
    match msg {
        Message::KeyPress(key) => {
            // In command mode, most keys append to the input buffer
            if state.command_mode {
                return handle_command_key(state, key.code);
            }
            // Normal mode key handling
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    if state.screen == Screen::Settings {
                        state.screen = Screen::Dashboard;
                    } else {
                        return false;
                    }
                }
                KeyCode::Char('/') | KeyCode::Char(':') => {
                    state.command_mode = true;
                    state.command_input = key.code.to_string();
                }
                KeyCode::F(1) => state.screen = Screen::Dashboard,
                KeyCode::F(2) => state.screen = Screen::Journal,
                KeyCode::F(3) => state.screen = Screen::Strategies,
                KeyCode::F(4) => state.screen = Screen::Logs,
                KeyCode::Char('?') => state.screen = Screen::Help,
                _ => {}
            }
        }
        Message::EnterCommandMode(leader) => {
            state.command_mode = true;
            state.command_input = leader.to_string();
        }
        Message::CommandInput(ch) => {
            state.command_input.push(ch);
        }
        Message::CommandBackspace => {
            // Don't backspace past the leader char (/ or :)
            if state.command_input.len() > 1 {
                state.command_input.pop();
            }
        }
        Message::CommandCancel => {
            state.command_mode = false;
            state.command_input.clear();
        }
        Message::CommandExecute => {
            // CP-2: parse and execute
            state.command_mode = false;
            state.command_input.clear();
        }
        Message::CommandTab => {
            // CP-3: autocomplete
        }
        Message::CommandHistoryUp => {
            // CP-3: history navigation
        }
        Message::CommandHistoryDown => {
            // CP-3: history navigation
        }
        Message::CommandError(_) => {
            state.command_error = Some("".into());
        }
        Message::ClearCommandError => {
            state.command_error = None;
        }
        Message::Resize(_cols, _rows) => {}
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

/// Handle a key press while in command mode.
fn handle_command_key(state: &mut AppState, code: KeyCode) -> bool {
    match code {
        KeyCode::Esc => {
            state.command_mode = false;
            state.command_input.clear();
        }
        KeyCode::Enter => {
            return execute_command(state);
        }
        KeyCode::Backspace if state.command_input.len() > 1 => {
            state.command_input.pop();
        }
        KeyCode::Backspace => {
            // At leader char — nothing to backspace
        }
        KeyCode::Tab => {
            if state.autocomplete_matches.is_empty() {
                return true;
            }
            state.autocomplete_idx = (state.autocomplete_idx + 1) % state.autocomplete_matches.len();
            state.command_input = state.autocomplete_matches[state.autocomplete_idx].clone();
            // Store the matches for display in command bar
        }
        KeyCode::Up => {
            if state.command_history.is_empty() {
                return true;
            }
            let idx = match state.command_history_idx {
                None => state.command_history.len().saturating_sub(1),
                Some(0) => 0,
                Some(i) => i.saturating_sub(1),
            };
            state.command_history_idx = Some(idx);
            state.command_input = state.command_history[idx].clone();
        }
        KeyCode::Down => {
            match state.command_history_idx {
                None => return true,
                Some(i) if i + 1 >= state.command_history.len() => {
                    state.command_history_idx = None;
                    state.command_input.clear();
                    state.command_input.push('/');
                }
                Some(i) => {
                    state.command_history_idx = Some(i + 1);
                    state.command_input = state.command_history[i + 1].clone();
                }
            }
        }
        KeyCode::Char(c) => {
            state.command_input.push(c);
            // Recompute autocomplete matches on each keystroke
            state.autocomplete_matches = commands::autocomplete(&state.command_input);
            state.autocomplete_idx = 0;
        }
        // Ignore F-keys and other special keys in command mode
        _ => {}
    }
    true
}

/// Parse the command input and execute the corresponding action.
fn execute_command(state: &mut AppState) -> bool {
    let input = state.command_input.clone();
    state.command_history.push(input.clone());
    if state.command_history.len() > 20 {
        state.command_history.remove(0);
    }
    state.command_history_idx = None;
    state.command_mode = false;
    state.command_input.clear();
    state.autocomplete_idx = 0;
    state.autocomplete_matches.clear();

    match TuiCommand::from_input(&input) {
        Some(TuiCommand::Dashboard) => state.screen = Screen::Dashboard,
        Some(TuiCommand::Journal) => state.screen = Screen::Journal,
        Some(TuiCommand::Strategies) => state.screen = Screen::Strategies,
        Some(TuiCommand::Logs) => state.screen = Screen::Logs,
        Some(TuiCommand::Help) => state.screen = Screen::Help,
        Some(TuiCommand::Settings) => state.screen = Screen::Settings,
        Some(TuiCommand::Quit) => return false,
        None => {
            // Invalid command — flash error but it'll be implemented in CP-4
            state.command_error = Some(format!("Unknown command: {}", input));
        }
    }
    true
}
