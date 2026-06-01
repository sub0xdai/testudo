// @anchor test:cli:tui
// @tags ui

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use testudo_cli::model::state::{AppState, Screen, StatusBar};
use testudo_cli::msg::Message;
use testudo_cli::theme::Theme;
use testudo_cli::update::update;

fn make_state() -> AppState {
    AppState {
        screen: Screen::Dashboard,
        status: StatusBar {
            version: "v0.1.0".into(),
            mode: "SHADOW".into(),
            last_ticker: "ETH: $—".into(),
            uptime_secs: 0,
        },
        theme: Theme::vanilla_amoled(),
        error: None,
        positions: Vec::new(),
        signal_log: Vec::new(),
        equity_curve: Vec::new(),
        risk_snapshot: None,
        journal_summary: None,
        command_mode: false,
        command_input: String::new(),
        command_history: Vec::new(),
        command_history_idx: None,
        command_error: None,
    }
}

fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

#[test]
fn update_quit_on_q() {
    let mut state = make_state();
    let msg = Message::KeyPress(key_event(KeyCode::Char('q')));
    let should_continue = update(&mut state, msg);
    assert!(!should_continue, "pressing q should quit");
}

#[test]
fn update_quit_on_esc() {
    let mut state = make_state();
    let msg = Message::KeyPress(key_event(KeyCode::Esc));
    let should_continue = update(&mut state, msg);
    assert!(!should_continue, "pressing Esc should quit");
}

#[test]
fn update_f1_switches_to_dashboard() {
    let mut state = make_state();
    state.screen = Screen::Help; // start from non-dashboard
    let msg = Message::KeyPress(key_event(KeyCode::F(1)));
    let _ = update(&mut state, msg);
    assert!(matches!(state.screen, Screen::Dashboard));
}

#[test]
fn update_f2_switches_to_journal() {
    let mut state = make_state();
    let msg = Message::KeyPress(key_event(KeyCode::F(2)));
    let _ = update(&mut state, msg);
    assert!(matches!(state.screen, Screen::Journal));
}

#[test]
fn update_f3_switches_to_strategies() {
    let mut state = make_state();
    let msg = Message::KeyPress(key_event(KeyCode::F(3)));
    let _ = update(&mut state, msg);
    assert!(matches!(state.screen, Screen::Strategies));
}

#[test]
fn update_f4_switches_to_logs() {
    let mut state = make_state();
    let msg = Message::KeyPress(key_event(KeyCode::F(4)));
    let _ = update(&mut state, msg);
    assert!(matches!(state.screen, Screen::Logs));
}

#[test]
fn update_question_mark_switches_to_help() {
    let mut state = make_state();
    let msg = Message::KeyPress(key_event(KeyCode::Char('?')));
    let _ = update(&mut state, msg);
    assert!(matches!(state.screen, Screen::Help));
}

#[test]
fn update_tick_increments_uptime() {
    let mut state = make_state();
    assert_eq!(state.status.uptime_secs, 0);
    let msg = Message::Tick;
    let _ = update(&mut state, msg);
    assert_eq!(state.status.uptime_secs, 1);
    // Second tick
    let _ = update(&mut state, Message::Tick);
    assert_eq!(state.status.uptime_secs, 2);
}

#[test]
fn update_switch_screen_directly() {
    let mut state = make_state();
    let msg = Message::SwitchScreen(Screen::Logs);
    let _ = update(&mut state, msg);
    assert!(matches!(state.screen, Screen::Logs));
}

#[test]
fn update_error_sets_error_field() {
    let mut state = make_state();
    let msg = Message::Error("test error".into());
    let _ = update(&mut state, msg);
    assert_eq!(state.error, Some("test error".into()));
}

#[test]
fn update_clear_error_clears_field() {
    let mut state = make_state();
    state.error = Some("existing error".into());
    let msg = Message::ClearError;
    let _ = update(&mut state, msg);
    assert_eq!(state.error, None);
}

#[test]
fn update_other_keys_continue() {
    let mut state = make_state();
    let msg = Message::KeyPress(key_event(KeyCode::Char('x')));
    let should_continue = update(&mut state, msg);
    assert!(should_continue, "non-quit keys should continue");
}

#[test]
fn update_resize_stores_dimensions() {
    // Resize doesn't store dimensions in the current spec, but verifies
    // the message type is handled without crashing
    let mut state = make_state();
    let msg = Message::Resize(120, 40);
    let should_continue = update(&mut state, msg);
    assert!(should_continue);
}

// ── Command mode tests (CLI-08) ────────────────────────────────

#[test]
fn command_mode_enter_with_slash() {
    let mut state = make_state();
    assert!(!state.command_mode);
    assert!(state.command_input.is_empty());

    let msg = Message::KeyPress(key_event(KeyCode::Char('/')));
    let _ = update(&mut state, msg);

    assert!(state.command_mode, "should enter command mode on '/''");
    assert_eq!(state.command_input, "/");
}

#[test]
fn command_mode_enter_with_colon() {
    let mut state = make_state();
    let msg = Message::KeyPress(key_event(KeyCode::Char(':')));
    let _ = update(&mut state, msg);

    assert!(state.command_mode, "should enter command mode on ':'");
    assert_eq!(state.command_input, ":");
}

#[test]
fn command_mode_typing_appends() {
    let mut state = make_state();
    // Enter command mode
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('/'))));
    // Type characters
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('s'))));
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('e'))));
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('t'))));

    assert_eq!(state.command_input, "/set");
}

#[test]
fn command_mode_backspace() {
    let mut state = make_state();
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('/'))));
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('x'))));
    assert_eq!(state.command_input, "/x");

    update(&mut state, Message::KeyPress(key_event(KeyCode::Backspace)));
    assert_eq!(state.command_input, "/");

    // Backspace at leader char does nothing
    update(&mut state, Message::KeyPress(key_event(KeyCode::Backspace)));
    assert_eq!(state.command_input, "/");
}

#[test]
fn command_mode_esc_cancels() {
    let mut state = make_state();
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('/'))));
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('s'))));
    assert!(state.command_mode);

    update(&mut state, Message::KeyPress(key_event(KeyCode::Esc)));
    assert!(!state.command_mode, "Esc should exit command mode");
    assert!(state.command_input.is_empty(), "input should clear on cancel");
}

#[test]
fn command_mode_f_keys_ignored() {
    let mut state = make_state();
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('/'))));
    let initial_screen = state.screen;

    // F1 should NOT switch screen when in command mode
    update(&mut state, Message::KeyPress(key_event(KeyCode::F(1))));
    assert_eq!(state.screen, initial_screen, "F-keys ignored in command mode");
    assert!(state.command_mode, "should stay in command mode");
}

#[test]
fn command_mode_q_not_quit() {
    let mut state = make_state();
    update(&mut state, Message::KeyPress(key_event(KeyCode::Char('/'))));

    // 'q' in command mode should append, not quit
    let should_continue = update(&mut state, Message::KeyPress(key_event(KeyCode::Char('q'))));
    assert!(should_continue, "q in command mode should not quit");
    assert_eq!(state.command_input, "/q");
}
