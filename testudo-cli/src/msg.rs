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
}
