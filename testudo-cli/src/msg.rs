// @anchor infra:cli:msg
// @tags infra

//! Message enum — all events in the TEA loop.

use crate::model::state::Screen;
use crossterm::event::KeyEvent;

/// Every event in the TEA loop is a Message.
#[derive(Debug, Clone)]
pub enum Message {
    KeyPress(KeyEvent),
    Resize(u16, u16),
    Tick,
    SwitchScreen(Screen),
    ShowHelp,
    Quit,
    Error(String),
    ClearError,
    /// User pressed / or : — enter command mode
    EnterCommandMode(char),
    /// Character typed while in command mode
    CommandInput(char),
    /// Backspace pressed in command mode
    CommandBackspace,
    /// Tab pressed for autocomplete (CP-3)
    CommandTab,
    /// Enter pressed — execute the command
    CommandExecute,
    /// Esc pressed — cancel command mode
    CommandCancel,
    /// Up arrow in command mode — previous history
    CommandHistoryUp,
    /// Down arrow in command mode — next history
    CommandHistoryDown,
    /// Flash error in command bar
    CommandError(String),
    /// Clear command error flash
    ClearCommandError,
}
